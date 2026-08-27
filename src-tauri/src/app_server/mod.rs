mod account;
mod process;
mod rpc;
mod sessions;

use std::{
    collections::{HashSet, VecDeque},
    sync::{atomic::AtomicU64, Arc},
};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::{
    process::{Child, ChildStdin},
    sync::{Mutex, RwLock},
};

pub use rpc::AppServerError;
use rpc::Pending;

use crate::models::{parse_account, ApprovalRequest, CodexModel, DashboardSnapshot};

pub struct AppServer {
    child: Mutex<Option<Child>>,
    stdin: Mutex<Option<ChildStdin>>,
    pending: Pending,
    next_id: AtomicU64,
    connection_generation: AtomicU64,
    diagnostics: Mutex<VecDeque<String>>,
    pending_threads: Mutex<HashSet<String>>,
    pub snapshot: RwLock<DashboardSnapshot>,
}

impl AppServer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            child: Mutex::new(None),
            stdin: Mutex::new(None),
            pending: Pending::default(),
            next_id: AtomicU64::new(1),
            connection_generation: AtomicU64::new(0),
            diagnostics: Mutex::new(VecDeque::new()),
            pending_threads: Mutex::new(HashSet::new()),
            snapshot: RwLock::new(DashboardSnapshot::default()),
        })
    }

    async fn handle_server_request(
        self: &Arc<Self>,
        app: &AppHandle,
        method: String,
        request_id: Value,
        params: Value,
    ) {
        let kind = match method.as_str() {
            "item/commandExecution/requestApproval" => "command",
            "item/fileChange/requestApproval" => "fileChange",
            _ => {
                let _ = self
                    .write_response(
                        request_id,
                        Err(json!({ "code": -32601, "message": format!("Unsupported server request: {method}") })),
                    )
                    .await;
                return;
            }
        };
        let default_decisions = ["accept", "acceptForSession", "decline"];
        let available_decisions = params
            .get("availableDecisions")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .filter(|items| !items.is_empty())
            .unwrap_or_else(|| {
                default_decisions
                    .iter()
                    .map(|value| (*value).to_owned())
                    .collect()
            });
        let approval = ApprovalRequest {
            request_id,
            kind: kind.to_owned(),
            thread_id: string_field(&params, "threadId"),
            turn_id: string_field(&params, "turnId"),
            item_id: string_field(&params, "itemId"),
            started_at: params
                .get("startedAtMs")
                .and_then(Value::as_i64)
                .unwrap_or_default(),
            command: optional_string_field(&params, "command"),
            cwd: optional_string_field(&params, "cwd"),
            reason: optional_string_field(&params, "reason"),
            grant_root: optional_string_field(&params, "grantRoot"),
            available_decisions,
        };
        let mut snapshot = self.snapshot.write().await;
        snapshot
            .approvals
            .retain(|item| item.request_id != approval.request_id);
        snapshot.approvals.push(approval.clone());
        drop(snapshot);
        let _ = app.emit(
            "dashboard-event",
            json!({ "type": "approval.requested", "approval": approval }),
        );
    }

    async fn handle_notification(self: &Arc<Self>, app: &AppHandle, method: &str, params: Value) {
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

    pub async fn resolve_approval(
        &self,
        app: &AppHandle,
        request_id: Value,
        decision: &str,
    ) -> Result<(), AppServerError> {
        if !matches!(
            decision,
            "accept" | "acceptForSession" | "decline" | "cancel"
        ) {
            return Err(AppServerError::Rpc(
                "Unsupported approval decision".to_owned(),
            ));
        }
        let mut snapshot = self.snapshot.write().await;
        let Some(index) = snapshot
            .approvals
            .iter()
            .position(|approval| approval.request_id == request_id)
        else {
            return Err(AppServerError::Rpc(
                "Approval request is no longer pending".to_owned(),
            ));
        };
        let approval = snapshot.approvals.remove(index);
        drop(snapshot);
        if let Err(error) = self
            .write_response(request_id.clone(), Ok(json!({ "decision": decision })))
            .await
        {
            self.snapshot.write().await.approvals.push(approval);
            return Err(error);
        }
        let _ = app.emit(
            "dashboard-event",
            json!({ "type": "approval.resolved", "requestId": request_id }),
        );
        Ok(())
    }

    pub async fn list_models(&self) -> Result<Vec<CodexModel>, AppServerError> {
        let response = self
            .request(
                "model/list",
                json!({ "limit": 100, "includeHidden": false }),
            )
            .await?;
        Ok(response
            .get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|model| {
                let id = model.get("model").and_then(Value::as_str)?.to_owned();
                Some(CodexModel {
                    display_name: model
                        .get("displayName")
                        .and_then(Value::as_str)
                        .unwrap_or(&id)
                        .to_owned(),
                    description: model
                        .get("description")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    is_default: model
                        .get("isDefault")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    id,
                })
            })
            .collect())
    }

    async fn refresh(&self, app: &AppHandle) -> Result<(), AppServerError> {
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

fn string_field(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn optional_string_field(value: &Value, field: &str) -> Option<String> {
    value.get(field).and_then(Value::as_str).map(str::to_owned)
}
