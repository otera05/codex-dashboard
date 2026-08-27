use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::models::parse_account;

use super::{AppServer, AppServerError};

const REFRESH_DEBOUNCE: Duration = Duration::from_millis(75);

impl AppServer {
    pub(super) async fn handle_notification(
        self: &Arc<Self>,
        app: &AppHandle,
        method: &str,
        params: Value,
    ) {
        if method == "serverRequest/resolved" {
            if let Some(request_id) = params.get("requestId") {
                self.snapshot
                    .write()
                    .await
                    .approvals
                    .retain(|item| item.request_id != *request_id);
                let _ = app.emit(
                    "dashboard-event",
                    json!({ "type": "approval.resolved", "requestId": request_id }),
                );
            }
        }
        if Self::notification_requires_refresh(method) {
            let revision = self.next_refresh_revision();
            let refresh_server = Arc::clone(self);
            let refresh_app = app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(REFRESH_DEBOUNCE).await;
                if !refresh_server.is_current_refresh_revision(revision) {
                    return;
                }
                let _guard = refresh_server.refresh_lock.lock().await;
                if refresh_server.is_current_refresh_revision(revision) {
                    let _ = refresh_server.refresh_snapshot(&refresh_app).await;
                }
            });
        }
    }

    pub(super) async fn refresh(&self, app: &AppHandle) -> Result<(), AppServerError> {
        let _guard = self.refresh_lock.lock().await;
        self.refresh_snapshot(app).await
    }

    async fn refresh_snapshot(&self, app: &AppHandle) -> Result<(), AppServerError> {
        self.reload_sessions().await?;
        let account_result = self.read_account(false).await;
        let rates = self.read_rate_limits().await;
        let mut snapshot = self.snapshot.write().await;
        snapshot.account = parse_account(&account_result, &rates);
        snapshot.connected = true;
        let _ = app.emit(
            "dashboard-event",
            json!({ "type": "snapshot", "snapshot": &*snapshot }),
        );
        Ok(())
    }

    fn next_refresh_revision(&self) -> u64 {
        self.refresh_revision.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn is_current_refresh_revision(&self, revision: u64) -> bool {
        self.refresh_revision.load(Ordering::Relaxed) == revision
    }

    fn notification_requires_refresh(method: &str) -> bool {
        matches!(
            method,
            "thread/started"
                | "thread/archived"
                | "thread/deleted"
                | "thread/unarchived"
                | "thread/status/changed"
                | "thread/name/updated"
                | "thread/tokenUsage/updated"
                | "turn/started"
                | "turn/completed"
                | "item/completed"
                | "account/updated"
                | "account/rateLimits/updated"
                | "account/login/completed"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::AppServer;

    #[test]
    fn refreshes_for_state_changing_notifications() {
        assert!(AppServer::notification_requires_refresh("thread/started"));
        assert!(AppServer::notification_requires_refresh("turn/completed"));
        assert!(AppServer::notification_requires_refresh("account/updated"));
        assert!(!AppServer::notification_requires_refresh("item/started"));
    }

    #[test]
    fn only_the_latest_refresh_revision_remains_current() {
        let server = AppServer::new();
        let first = server.next_refresh_revision();
        let second = server.next_refresh_revision();

        assert!(!server.is_current_refresh_revision(first));
        assert!(server.is_current_refresh_revision(second));
    }
}
