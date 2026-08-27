use std::sync::Arc;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::models::ApprovalRequest;

use super::{AppServer, AppServerError};

impl AppServer {
    pub(super) async fn handle_server_request(
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
