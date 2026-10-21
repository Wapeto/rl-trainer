//! The install counter: once a day the app tells the relay a random id and its version. Nothing
//! else is sent, and Settings can turn it off.

use std::time::Duration;

use rl_core::ingest::now_ms;

use crate::app::{App, AppError};
use crate::settings;

const PING_EVERY_MS: i64 = 24 * 60 * 60 * 1000;
const PING_TIMEOUT: Duration = Duration::from_secs(10);

/// The relay's `/ping` address, from its `/report` address.
pub fn ping_url(report_url: &str) -> Option<String> {
    report_url
        .strip_suffix("/report")
        .map(|base| format!("{base}/ping"))
}

fn due(last_ping_at: Option<i64>, now: i64) -> bool {
    last_ping_at.is_none_or(|last| now - last >= PING_EVERY_MS)
}

impl App {
    pub(crate) fn spawn_usage_ping(&self) {
        // Release builds only: a dev build must not count as an install.
        if cfg!(debug_assertions) {
            return;
        }
        let app = self.clone();
        let task = tokio::spawn(async move {
            if let Err(err) = app.send_usage_ping().await {
                tracing::debug!(error = %err, "usage ping failed");
            }
        });
        crate::app::push_background(self, task);
    }

    /// Pings the relay when it is on, set up and a day has passed; returns whether it did.
    /// A failed ping is simply tried again at the next start.
    pub async fn send_usage_ping(&self) -> Result<bool, AppError> {
        let Some(url) = self.inner.report_url.as_deref().and_then(ping_url) else {
            return Ok(false);
        };
        let (id, now) = {
            let conn = self.read_db()?;
            let now = now_ms();
            if !settings::load(&conn)?.usage_ping || !due(settings::last_ping_at(&conn)?, now) {
                return Ok(false);
            }
            drop(conn);
            (settings::install_id(&*self.write_db()?)?, now)
        };
        let version = env!("CARGO_PKG_VERSION");
        let sent = async {
            reqwest::Client::builder()
                .timeout(PING_TIMEOUT)
                .build()?
                .post(url)
                .header("X-RL-Trainer-Version", version)
                .json(&serde_json::json!({ "id": id, "version": version }))
                .send()
                .await?
                .error_for_status()
        }
        .await;
        match sent {
            Ok(_) => {
                settings::set_last_ping_at(&*self.write_db()?, now)?;
                Ok(true)
            }
            Err(err) => {
                tracing::debug!(error = %err.without_url(), "usage ping was not accepted");
                Ok(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ping_address_comes_from_the_report_address() {
        assert_eq!(
            ping_url("https://r.example.workers.dev/report").as_deref(),
            Some("https://r.example.workers.dev/ping")
        );
        assert_eq!(ping_url("https://r.example.workers.dev/other"), None);
    }

    #[test]
    fn a_ping_is_due_after_a_day() {
        assert!(due(None, 5));
        assert!(!due(Some(1_000), 1_000 + PING_EVERY_MS - 1));
        assert!(due(Some(1_000), 1_000 + PING_EVERY_MS));
    }
}
