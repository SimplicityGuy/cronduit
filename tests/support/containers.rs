//! Test-only Docker fixtures. No process-global reaper or daemon-wide pruning.
//!
//! The guard is armed before the first await. A dedicated thread owns an independent
//! runtime, including the in-flight create RPC, so dropping a startup future or its
//! parent runtime cannot cancel creation before we learn the resource's ID. Drop
//! signals and joins that thread; cleanup never depends on the caller's executor.
#![allow(dead_code)] // Each integration-test binary uses a different fixture subset.

use std::{collections::HashMap, path::PathBuf, thread, time::Duration};

use anyhow::{Context, Result, anyhow, bail};
use bollard::{
    Docker,
    errors::Error,
    models::{ContainerCreateBody, HostConfig, PortBinding},
    query_parameters::{
        CreateContainerOptions, CreateImageOptions, InspectContainerOptions, InspectNetworkOptions,
        RemoveContainerOptions, StartContainerOptions,
    },
};
use futures_util::TryStreamExt;
use tokio::sync::oneshot;

const OWNER_LABEL: &str = "cronduit.test.owner";
const STARTUP_LIMIT: Duration = Duration::from_secs(60);

#[derive(Clone, Copy)]
pub enum Image {
    Postgres,
    Alpine,
}

#[derive(Clone)]
struct Info {
    id: String,
    url: Option<String>,
}

pub struct TestContainer {
    info: Option<Info>,
    shutdown: Option<oneshot::Sender<()>>,
    worker: Option<thread::JoinHandle<Result<()>>>,
}

impl TestContainer {
    pub async fn postgres() -> Result<Self> {
        Self::start(Image::Postgres).await
    }
    pub async fn alpine() -> Result<Self> {
        Self::start(Image::Alpine).await
    }

    pub async fn start(image: Image) -> Result<Self> {
        Self::start_at(image, docker_endpoint(), STARTUP_LIMIT).await
    }

    // Explicit endpoint/deadline also lets lifecycle tests exercise failure boundaries
    // without mutating process environment shared with other tests.
    pub async fn start_at(
        image: Image,
        endpoint: String,
        readiness_limit: Duration,
    ) -> Result<Self> {
        let token = ulid::Ulid::generate().to_string().to_lowercase();
        let name = format!("cronduit-test-{token}");
        let (shutdown, mut stopped) = oneshot::channel();
        let (ready, started) = oneshot::channel();
        let worker = thread::Builder::new().name(name.clone()).spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(async move {
                let docker = connect(&endpoint)?;
                let mut owned = OwnedContainer {
                    docker,
                    name,
                    token,
                    id: None,
                    create_attempted: false,
                };
                let result = async {
                    // Pulls own no container and can stop immediately. Only the create
                    // RPC is cancellation-shielded: its side effect may precede its ID.
                    tokio::select! {
                        biased;
                        _ = &mut stopped => bail!("fixture startup cancelled"),
                        result = owned.prepare_image(image) => result?,
                    }
                    owned.create(image).await?;
                    tokio::select! {
                        biased;
                        _ = &mut stopped => bail!("fixture startup cancelled"),
                        result = owned.start_and_wait(image, &endpoint, readiness_limit) => result,
                    }
                }
                .await;
                let success = result.is_ok();
                let delivered = ready.send(result.map_err(|e| format!("{e:#}"))).is_ok();
                if success && delivered {
                    let _ = stopped.await;
                }
                owned.cleanup().await
            })
        })?;
        let mut guard = Self {
            info: None,
            shutdown: Some(shutdown),
            worker: Some(worker),
        };
        guard.info = Some(
            started
                .await
                .context("fixture worker exited before readiness")?
                .map_err(|e| anyhow!(e))?,
        );
        Ok(guard)
    }

    pub fn id(&self) -> &str {
        &self.info.as_ref().expect("fixture ready").id
    }
    pub fn url(&self) -> &str {
        self.info
            .as_ref()
            .expect("fixture ready")
            .url
            .as_deref()
            .expect("Postgres fixture")
    }
}

impl Drop for TestContainer {
    fn drop(&mut self) {
        self.shutdown.take(); // Closing the channel also handles startup cancellation.
        if let Some(worker) = self.worker.take() {
            let result = worker
                .join()
                .map_err(|_| anyhow!("fixture worker panicked"))
                .and_then(|r| r);
            if let Err(error) = result {
                if thread::panicking() {
                    eprintln!("fixture cleanup failed during unwind: {error:#}");
                } else {
                    panic!("fixture cleanup failed: {error:#}");
                }
            }
        }
    }
}

struct OwnedContainer {
    docker: Docker,
    name: String,
    token: String,
    id: Option<String>,
    create_attempted: bool,
}

impl OwnedContainer {
    fn image(kind: Image) -> &'static str {
        match kind {
            Image::Postgres => "postgres:11-alpine",
            Image::Alpine => "alpine:latest",
        }
    }

    async fn prepare_image(&self, kind: Image) -> Result<()> {
        let image = Self::image(kind);
        match self.docker.inspect_image(image).await {
            Ok(_) => {}
            Err(Error::DockerResponseServerError {
                status_code: 404, ..
            }) => {
                self.docker
                    .create_image(
                        Some(CreateImageOptions {
                            from_image: Some(Self::image(kind).into()),
                            platform: std::env::var("DOCKER_DEFAULT_PLATFORM").unwrap_or_default(),
                            ..Default::default()
                        }),
                        None,
                        None,
                    )
                    .try_collect::<Vec<_>>()
                    .await
                    .context("pull fixture image")?;
            }
            Err(e) => return Err(e).context("inspect fixture image"),
        }
        Ok(())
    }

    async fn create(&mut self, kind: Image) -> Result<()> {
        let mut config = ContainerCreateBody {
            image: Some(Self::image(kind).into()),
            labels: Some(HashMap::from([(OWNER_LABEL.into(), self.token.clone())])),
            ..Default::default()
        };
        match kind {
            Image::Postgres => {
                config.env = Some(vec![
                    "POSTGRES_DB=postgres".into(),
                    "POSTGRES_USER=postgres".into(),
                    "POSTGRES_PASSWORD=postgres".into(),
                ]);
                config.cmd = Some(vec!["-c".into(), "fsync=off".into()]);
                config.exposed_ports = Some(vec!["5432/tcp".into()]);
                config.host_config = Some(HostConfig {
                    port_bindings: Some(HashMap::from([(
                        "5432/tcp".into(),
                        Some(vec![PortBinding {
                            host_ip: None,
                            host_port: Some(String::new()),
                        }]),
                    )])),
                    ..Default::default()
                });
            }
            Image::Alpine => {
                config.entrypoint = Some(vec!["sleep".into()]);
                config.cmd = Some(vec!["300".into()]);
            }
        }
        self.create_attempted = true;
        self.id = Some(
            self.docker
                .create_container(
                    Some(CreateContainerOptions {
                        name: Some(self.name.clone()),
                        platform: std::env::var("DOCKER_DEFAULT_PLATFORM").unwrap_or_default(),
                    }),
                    config,
                )
                .await
                .context("create fixture")?
                .id,
        );
        Ok(())
    }

    async fn start_and_wait(
        &self,
        kind: Image,
        endpoint: &str,
        readiness_limit: Duration,
    ) -> Result<Info> {
        let id = self.id.as_deref().expect("created ID");
        self.docker
            .start_container(id, None::<StartContainerOptions>)
            .await
            .context("start fixture")?;
        let url = if matches!(kind, Image::Postgres) {
            let inspected = self
                .docker
                .inspect_container(id, None::<InspectContainerOptions>)
                .await?;
            let bindings = inspected
                .network_settings
                .and_then(|n| n.ports)
                .and_then(|mut p| p.remove("5432/tcp"))
                .flatten()
                .context("Postgres published port missing")?;
            let port: u16 = bindings
                .iter()
                .find_map(|p| p.host_port.as_deref())
                .context("Postgres host port missing")?
                .parse()?;
            let host = published_host(&self.docker, endpoint).await?;
            let url = postgres_url(&host, port)?;
            tokio::time::timeout(readiness_limit, async {
                loop {
                    match sqlx::postgres::PgConnection::connect(&url).await {
                        Ok(mut connection) => {
                            // A real authenticated query avoids the temporary initialization
                            // server/log-message race in the official Postgres image.
                            use sqlx::{Connection, Executor};
                            connection.execute("SELECT 1").await?;
                            connection.close().await?;
                            return Ok::<_, anyhow::Error>(());
                        }
                        Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
                    }
                }
            })
            .await
            .context("Postgres readiness timed out")??;
            Some(url)
        } else {
            let inspected = self
                .docker
                .inspect_container(id, None::<InspectContainerOptions>)
                .await?;
            if inspected.state.and_then(|s| s.running) != Some(true) {
                bail!("Alpine fixture did not stay running");
            }
            None
        };
        Ok(Info { id: id.into(), url })
    }

    async fn cleanup(&self) -> Result<()> {
        if !self.create_attempted {
            return Ok(());
        }
        // The pre-known name covers a lost create response; the unpredictable owner
        // token prevents deleting a pre-existing or replaced container on any path.
        let target = self.id.as_deref().unwrap_or(&self.name);
        for attempt in 0..3 {
            let result = async {
                let inspect = match self
                    .docker
                    .inspect_container(target, None::<InspectContainerOptions>)
                    .await
                {
                    Ok(c) => c,
                    Err(Error::DockerResponseServerError {
                        status_code: 404, ..
                    }) => return Ok(()),
                    Err(e) => return Err(e.into()),
                };
                let labels = inspect.config.and_then(|c| c.labels).unwrap_or_default();
                if labels.get(OWNER_LABEL) != Some(&self.token) {
                    bail!("refusing to remove fixture with mismatched ownership: {target}");
                }
                let id = inspect.id.context("owned container has no ID")?;
                match self
                    .docker
                    .remove_container(
                        &id,
                        Some(RemoveContainerOptions {
                            force: true,
                            v: true,
                            ..Default::default()
                        }),
                    )
                    .await
                {
                    Ok(())
                    | Err(Error::DockerResponseServerError {
                        status_code: 404, ..
                    }) => Ok(()),
                    Err(e) => Err(e.into()),
                }
            }
            .await;
            match result {
                Ok(()) => return Ok(()),
                Err(e) if attempt == 2 => {
                    return Err(e).context(format!("remove owned fixture {target}"));
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
        unreachable!()
    }
}

fn connect(endpoint: &str) -> Result<Docker> {
    // Match the former fixtures: DOCKER_TLS_VERIFY=0 means plain HTTP.
    // Bollard's generic defaults instead test only whether the variable exists.
    if endpoint.starts_with("tcp://") || endpoint.starts_with("http://") {
        if std::env::var("DOCKER_TLS_VERIFY").as_deref() == Ok("1") {
            return Ok(Docker::connect_with_host(endpoint)?);
        }
        return Ok(Docker::connect_with_http(
            endpoint,
            120,
            bollard::API_DEFAULT_VERSION,
        )?);
    }
    Ok(Docker::connect_with_host(endpoint)?)
}

pub fn docker_endpoint() -> String {
    if let Ok(endpoint) = std::env::var("DOCKER_HOST") {
        return endpoint;
    }
    #[cfg(unix)]
    {
        let mut paths = vec![PathBuf::from("/var/run/docker.sock")];
        if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
            paths.push(PathBuf::from(dir).join(".docker/run/docker.sock"));
        }
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            paths.push(home.join(".docker/run/docker.sock"));
            paths.push(home.join(".docker/desktop/docker.sock"));
        }
        if let Some(path) = paths.into_iter().find(|p| p.exists()) {
            return format!("unix://{}", path.display());
        }
    }
    if cfg!(windows) {
        "npipe:////./pipe/docker_engine".into()
    } else {
        "unix:///var/run/docker.sock".into()
    }
}

async fn published_host(docker: &Docker, endpoint: &str) -> Result<String> {
    if let Ok(host) = std::env::var("TESTCONTAINERS_HOST_OVERRIDE") {
        return Ok(host);
    }
    let url = url::Url::parse(endpoint)?;
    match url.scheme() {
        "http" | "https" | "tcp" => Ok(url
            .host()
            .context("Docker endpoint has no host")?
            .to_string()),
        "unix" | "npipe" => {
            if std::path::Path::new("/.dockerenv").exists() {
                let network = docker
                    .inspect_network("bridge", None::<InspectNetworkOptions>)
                    .await
                    .ok();
                if let Some(gateway) = network
                    .and_then(|n| n.ipam)
                    .and_then(|i| i.config)
                    .unwrap_or_default()
                    .into_iter()
                    .find_map(|c| c.gateway)
                {
                    return Ok(gateway);
                }
            }
            Ok("localhost".into())
        }
        _ => bail!("unsupported Docker endpoint: {endpoint}"),
    }
}

pub fn postgres_url(host: &str, port: u16) -> Result<String> {
    // url::Host's Display brackets IPv6 literals exactly once.
    let host = match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V6(ip)) => url::Host::Ipv6(ip),
        _ => url::Host::parse(host)?,
    };
    Ok(format!(
        "postgres://postgres:postgres@{host}:{port}/postgres"
    ))
}

use sqlx::Connection;
