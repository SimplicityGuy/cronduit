//! PostgreSQL fixture on the current testcontainers API. Keep the image,
//! credentials, readiness checks, and fsync setting used by the existing tests.

use std::borrow::Cow;
use testcontainers::{Image, core::WaitFor};

#[derive(Debug, Default)]
pub struct Postgres {}

impl Image for Postgres {
    fn name(&self) -> &str {
        "postgres"
    }

    fn tag(&self) -> &str {
        "11-alpine"
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        vec![
            WaitFor::message_on_stderr("database system is ready to accept connections"),
            WaitFor::message_on_stdout("database system is ready to accept connections"),
        ]
    }

    fn env_vars(
        &self,
    ) -> impl IntoIterator<Item = (impl Into<Cow<'_, str>>, impl Into<Cow<'_, str>>)> {
        [
            ("POSTGRES_DB", "postgres"),
            ("POSTGRES_USER", "postgres"),
            ("POSTGRES_PASSWORD", "postgres"),
        ]
    }

    fn cmd(&self) -> impl IntoIterator<Item = impl Into<Cow<'_, str>>> {
        ["-c", "fsync=off"]
    }
}
