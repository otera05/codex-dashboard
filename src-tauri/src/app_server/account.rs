use std::sync::Arc;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::models::{parse_account, Account};

use super::{AppServer, AppServerError};

impl AppServer {
    pub async fn refresh_account(
        self: &Arc<Self>,
        app: &AppHandle,
    ) -> Result<Account, AppServerError> {
        let account_result = self
            .request_with_reconnect(app, "account/read", json!({ "refreshToken": true }))
            .await
            .unwrap_or(Value::Null);
        let rates = self
            .request_with_reconnect(app, "account/rateLimits/read", json!({}))
            .await
            .unwrap_or(Value::Null);
        let account = parse_account(&account_result, &rates);
        self.snapshot.write().await.account = account.clone();
        let _ = app.emit(
            "dashboard-event",
            json!({ "type": "account.updated", "account": account }),
        );
        Ok(account)
    }

    pub async fn logout_account(
        self: &Arc<Self>,
        app: &AppHandle,
    ) -> Result<Account, AppServerError> {
        self.request_with_reconnect(app, "account/logout", Value::Null)
            .await?;
        let account = Account::default();
        self.snapshot.write().await.account = account.clone();
        let _ = app.emit(
            "dashboard-event",
            json!({ "type": "account.updated", "account": account }),
        );
        Ok(account)
    }

    pub(super) async fn read_account(&self, refresh_token: bool) -> Value {
        self.request("account/read", json!({ "refreshToken": refresh_token }))
            .await
            .unwrap_or(Value::Null)
    }

    pub(super) async fn read_rate_limits(&self) -> Value {
        self.request("account/rateLimits/read", json!({}))
            .await
            .unwrap_or(Value::Null)
    }
}
