use std::sync::Arc;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::models::parse_account;

use super::{AppServer, AppServerError};

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
            let refresh_server = Arc::clone(self);
            let refresh_app = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = refresh_server.refresh(&refresh_app).await;
            });
        }
    }

    pub(super) async fn refresh(&self, app: &AppHandle) -> Result<(), AppServerError> {
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
}
