//! Regression coverage for ownership and cancellation of test-only Docker fixtures.
#[path = "support/containers.rs"]
mod containers;

use bollard::{Docker, errors::Error, query_parameters::InspectContainerOptions};
use containers::{Image, TestContainer};
use futures_util::FutureExt;
use serde_json::{Value, json};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::{Notify, oneshot};
use wiremock::{
    Mock, MockServer, Request, ResponseTemplate,
    matchers::{method, path_regex},
};

async fn assert_removed(docker: &Docker, id: &str) {
    assert!(
        matches!(
            docker
                .inspect_container(id, None::<InspectContainerOptions>)
                .await,
            Err(Error::DockerResponseServerError {
                status_code: 404,
                ..
            })
        ),
        "fixture {id} was not removed"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn live_cleanup_on_return_panic_timeout_abort_and_runtime_shutdown() {
    let docker = Docker::connect_with_host(&containers::docker_endpoint()).unwrap();
    // This independently owned resource must survive every other fixture's cleanup.
    let survivor = TestContainer::alpine().await.unwrap();

    let normal = TestContainer::alpine().await.unwrap();
    let id = normal.id().to_owned();
    drop(normal);
    assert_removed(&docker, &id).await;

    let panicking = TestContainer::alpine().await.unwrap();
    let id = panicking.id().to_owned();
    let panic = std::panic::AssertUnwindSafe(async move {
        let _guard = panicking;
        panic!("intentional fixture unwind");
    })
    .catch_unwind()
    .await;
    assert!(panic.is_err());
    assert_removed(&docker, &id).await;

    let timed = TestContainer::alpine().await.unwrap();
    let id = timed.id().to_owned();
    assert!(
        tokio::time::timeout(Duration::from_millis(20), async move {
            let _guard = timed;
            std::future::pending::<()>().await;
        })
        .await
        .is_err()
    );
    assert_removed(&docker, &id).await;

    let (ready, started) = oneshot::channel();
    let task = tokio::spawn(async move {
        let guard = TestContainer::alpine().await.unwrap();
        ready.send(guard.id().to_owned()).unwrap();
        let _guard = guard;
        std::future::pending::<()>().await;
    });
    let id = started.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_removed(&docker, &id).await;

    // Drop a parent runtime while a task still holds the guard. Cleanup must run
    // on the worker's independent executor, without spawning onto the dying one.
    let id = std::thread::spawn(|| {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let guard = runtime.block_on(TestContainer::alpine()).unwrap();
        let id = guard.id().to_owned();
        runtime.spawn(async move {
            let _guard = guard;
            std::future::pending::<()>().await;
        });
        runtime.block_on(tokio::task::yield_now());
        drop(runtime);
        id
    })
    .join()
    .unwrap();
    assert_removed(&docker, &id).await;
    assert_eq!(
        docker
            .inspect_container(survivor.id(), None::<InspectContainerOptions>)
            .await
            .unwrap()
            .state
            .unwrap()
            .running,
        Some(true)
    );
}

struct FakeDocker {
    server: MockServer,
    create_seen: Arc<Notify>,
    labels: Arc<Mutex<Value>>,
}

impl FakeDocker {
    async fn new(delay: Duration, mismatched_owner: bool) -> Self {
        Self::with_create_failure(delay, mismatched_owner, false).await
    }

    async fn with_create_failure(
        delay: Duration,
        mismatched_owner: bool,
        fail_create: bool,
    ) -> Self {
        let server = MockServer::start().await;
        let create_seen = Arc::new(Notify::new());
        let labels = Arc::new(Mutex::new(json!({})));
        Mock::given(method("GET"))
            .and(path_regex(".*/images/.*/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .mount(&server)
            .await;
        let captured = labels.clone();
        let notify = create_seen.clone();
        Mock::given(method("POST"))
            .and(path_regex(".*/containers/create"))
            .respond_with(move |request: &Request| {
                let body: Value = request.body_json().unwrap();
                *captured.lock().unwrap() = body["Labels"].clone();
                notify.notify_one();
                // The daemon has accepted the create side effect, but the client
                // cannot know its ID until this delayed response arrives.
                ResponseTemplate::new(if fail_create { 500 } else { 201 })
                    .set_delay(delay)
                    .set_body_json(json!({"Id":"fixture-id", "Warnings":[]}))
            })
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path_regex(".*/containers/fixture-id/start"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let inspected_labels = labels.clone();
        Mock::given(method("GET"))
            .and(path_regex(".*/containers/[^/]+/json"))
            .respond_with(move |_: &Request| {
                let labels = if mismatched_owner {
                    json!({"cronduit.test.owner":"someone-else"})
                } else {
                    inspected_labels.lock().unwrap().clone()
                };
                ResponseTemplate::new(200).set_body_json(json!({
                    "Id":"fixture-id", "Config":{"Labels":labels}, "State":{"Running":true},
                    "NetworkSettings":{"Ports":{"5432/tcp":[{"HostIp":"0.0.0.0","HostPort":"9"}]}}
                }))
            })
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .and(path_regex(".*/containers/fixture-id"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        Self {
            server,
            create_seen,
            labels,
        }
    }

    async fn deletes(&self) -> usize {
        self.server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .filter(|r| r.method == "DELETE")
            .count()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancellation_during_create_waits_for_late_response_then_removes() {
    let fake = FakeDocker::new(Duration::from_millis(200), false).await;
    let endpoint = fake.server.uri();
    let task = tokio::spawn(TestContainer::start_at(
        Image::Alpine,
        endpoint,
        Duration::from_secs(1),
    ));
    fake.create_seen.notified().await;
    task.abort();
    assert!(task.await.err().unwrap().is_cancelled());
    assert!(
        fake.labels.lock().unwrap()["cronduit.test.owner"]
            .as_str()
            .unwrap()
            .len()
            >= 26
    );
    assert_eq!(
        fake.deletes().await,
        1,
        "accepted create must be reaped after cancellation"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn readiness_timeout_removes_created_postgres() {
    let fake = FakeDocker::new(Duration::ZERO, false).await;
    let result = TestContainer::start_at(
        Image::Postgres,
        fake.server.uri(),
        Duration::from_millis(20),
    )
    .await;
    assert!(
        result
            .err()
            .unwrap()
            .to_string()
            .contains("readiness timed out")
    );
    assert_eq!(fake.deletes().await, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cleanup_refuses_a_different_owner() {
    let fake = FakeDocker::new(Duration::ZERO, true).await;
    let guard = TestContainer::start_at(Image::Alpine, fake.server.uri(), Duration::from_secs(1))
        .await
        .unwrap();
    let refusal = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(guard)));
    assert!(
        refusal.is_err(),
        "ownership mismatch must visibly fail cleanup"
    );
    assert_eq!(
        fake.deletes().await,
        0,
        "never delete a resource owned by someone else"
    );
}

#[test]
fn postgres_urls_preserve_remote_and_ipv6_hosts() {
    assert_eq!(
        containers::postgres_url("docker.example", 15432).unwrap(),
        "postgres://postgres:postgres@docker.example:15432/postgres"
    );
    assert_eq!(
        containers::postgres_url("2001:db8::7", 15432).unwrap(),
        "postgres://postgres:postgres@[2001:db8::7]:15432/postgres"
    );
    assert_eq!(
        containers::postgres_url("[2001:db8::7]", 15432).unwrap(),
        "postgres://postgres:postgres@[2001:db8::7]:15432/postgres"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancellation_of_unready_postgres_does_not_wait_for_readiness_deadline() {
    let fake = FakeDocker::new(Duration::ZERO, false).await;
    let task = tokio::spawn(TestContainer::start_at(
        Image::Postgres,
        fake.server.uri(),
        Duration::from_secs(60),
    ));
    // Wait until startup has actually progressed beyond the protected create RPC.
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if fake
                .server
                .received_requests()
                .await
                .unwrap()
                .iter()
                .any(|r| r.url.path().ends_with("/start"))
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(
        tokio::time::timeout(Duration::from_secs(3), task)
            .await
            .unwrap()
            .err()
            .unwrap()
            .is_cancelled()
    );
    assert_eq!(fake.deletes().await, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_create_response_reaps_only_the_pre_named_owned_container() {
    let fake = FakeDocker::with_create_failure(Duration::ZERO, false, true).await;
    let result =
        TestContainer::start_at(Image::Alpine, fake.server.uri(), Duration::from_secs(1)).await;
    assert!(result.err().unwrap().to_string().contains("create fixture"));
    let requests = fake.server.received_requests().await.unwrap();
    assert!(
        requests
            .iter()
            .any(|r| r.url.path().contains("/containers/cronduit-test-")),
        "lost-ID fallback must inspect the unique pre-known name"
    );
    assert_eq!(fake.deletes().await, 1);
}
