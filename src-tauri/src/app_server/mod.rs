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

use crate::models::{
    parse_account, parse_sessions, ApprovalRequest, CodexModel, DashboardSnapshot, Session,
};

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

    pub async fn rename_thread(
        &self,
        thread_id: &str,
        name: &str,
    ) -> Result<Session, AppServerError> {
        self.request(
            "thread/name/set",
            json!({ "threadId": thread_id, "name": name }),
        )
        .await?;
        let mut snapshot = self.snapshot.write().await;
        let session = snapshot
            .sessions
            .iter_mut()
            .find(|session| session.id == thread_id)
            .ok_or_else(|| AppServerError::Rpc(format!("Session {thread_id} was not found")))?;
        session.title = name.to_owned();
        Ok(session.clone())
    }

    pub async fn archive_thread(&self, thread_id: &str) -> Result<(), AppServerError> {
        self.request("thread/archive", json!({ "threadId": thread_id }))
            .await?;
        self.pending_threads.lock().await.remove(thread_id);
        let mut snapshot = self.snapshot.write().await;
        if let Some(index) = snapshot
            .sessions
            .iter()
            .position(|session| session.id == thread_id)
        {
            let session = snapshot.sessions.remove(index);
            snapshot
                .archived_sessions
                .retain(|item| item.id != thread_id);
            snapshot.archived_sessions.insert(0, session);
        }
        snapshot
            .approvals
            .retain(|approval| approval.thread_id != thread_id);
        Ok(())
    }

    pub async fn reload_archived_sessions(&self) -> Result<Vec<Session>, AppServerError> {
        let mut threads = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..10 {
            let response = self
                .request(
                    "thread/list",
                    json!({
                        "cursor": cursor,
                        "limit": 100,
                        "sortKey": "updated_at",
                        "sortDirection": "desc",
                        "archived": true
                    }),
                )
                .await?;
            if let Some(page) = response.get("data").and_then(Value::as_array) {
                threads.extend(page.iter().cloned());
            }
            cursor = response
                .get("nextCursor")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if cursor.is_none() {
                break;
            }
        }
        let mut sessions = parse_sessions(&json!({ "data": threads }));
        sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        let mut snapshot = self.snapshot.write().await;
        Self::preserve_loaded_history(&snapshot.archived_sessions, &mut sessions);
        snapshot.archived_sessions = sessions;
        Ok(snapshot.archived_sessions.clone())
    }

    pub async fn unarchive_thread(&self, thread_id: &str) -> Result<Session, AppServerError> {
        let response = self
            .request("thread/unarchive", json!({ "threadId": thread_id }))
            .await?;
        let thread = response
            .get("thread")
            .cloned()
            .ok_or_else(|| AppServerError::Rpc("thread/unarchive returned no thread".to_owned()))?;
        let mut restored = parse_sessions(&json!({ "data": [thread] }))
            .pop()
            .ok_or_else(|| {
                AppServerError::Rpc("Could not parse the restored session".to_owned())
            })?;
        let mut snapshot = self.snapshot.write().await;
        if let Some(previous) = snapshot
            .archived_sessions
            .iter()
            .find(|session| session.id == thread_id)
        {
            restored.model.clone_from(&previous.model);
            restored.messages.clone_from(&previous.messages);
            restored.token_usage.clone_from(&previous.token_usage);
            restored.history_loaded = previous.history_loaded;
        }
        snapshot
            .archived_sessions
            .retain(|item| item.id != thread_id);
        snapshot.sessions.retain(|item| item.id != thread_id);
        snapshot.sessions.insert(0, restored.clone());
        Ok(restored)
    }

    pub async fn reload_sessions(&self) -> Result<DashboardSnapshot, AppServerError> {
        let mut threads = Vec::new();
        let mut cursor: Option<String> = None;

        for _ in 0..10 {
            let response = self
                .request(
                    "thread/list",
                    json!({
                        "cursor": cursor,
                        "limit": 100,
                        "sortKey": "updated_at",
                        "sortDirection": "desc"
                    }),
                )
                .await?;
            if let Some(page) = response.get("data").and_then(Value::as_array) {
                threads.extend(page.iter().cloned());
            }
            cursor = response
                .get("nextCursor")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if cursor.is_none() {
                break;
            }
        }

        let mut sessions = parse_sessions(&json!({ "data": threads }));
        let mut snapshot = self.snapshot.write().await;
        Self::preserve_loaded_history(&snapshot.sessions, &mut sessions);
        let mut pending_threads = self.pending_threads.lock().await;
        Self::preserve_pending_sessions(&snapshot.sessions, &mut sessions, &mut pending_threads);
        sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        snapshot.sessions = sessions;
        snapshot.connected = true;
        Ok(snapshot.clone())
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

    fn preserve_loaded_history(previous_sessions: &[Session], sessions: &mut [Session]) {
        for session in sessions {
            if let Some(previous) = previous_sessions
                .iter()
                .find(|previous| previous.id == session.id && previous.history_loaded)
            {
                session.messages.clone_from(&previous.messages);
                session.token_usage.clone_from(&previous.token_usage);
                session.active_turn_id.clone_from(&previous.active_turn_id);
                session.history_loaded = true;
            }
        }
    }

    fn preserve_pending_sessions(
        previous_sessions: &[Session],
        sessions: &mut Vec<Session>,
        pending_threads: &mut HashSet<String>,
    ) {
        let materialized_ids = sessions
            .iter()
            .map(|session| session.id.clone())
            .collect::<HashSet<_>>();
        sessions.extend(
            previous_sessions
                .iter()
                .filter(|session| {
                    pending_threads.contains(&session.id) && !materialized_ids.contains(&session.id)
                })
                .cloned(),
        );
        pending_threads.retain(|thread_id| !materialized_ids.contains(thread_id));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_pending_threads_until_they_are_materialized() {
        let pending = Session {
            id: "pending".to_owned(),
            title: "New session".to_owned(),
            cwd: "/workspace".to_owned(),
            status: "idle".to_owned(),
            updated_at: 2,
            model: "Codex".to_owned(),
            active_turn_id: None,
            messages: Vec::new(),
            token_usage: Default::default(),
            history_loaded: true,
        };
        let mut pending_threads = HashSet::from([pending.id.clone()]);
        let mut sessions = Vec::new();

        AppServer::preserve_pending_sessions(
            std::slice::from_ref(&pending),
            &mut sessions,
            &mut pending_threads,
        );
        assert_eq!(sessions[0].id, "pending");
        assert!(pending_threads.contains("pending"));

        AppServer::preserve_pending_sessions(
            std::slice::from_ref(&pending),
            &mut sessions,
            &mut pending_threads,
        );
        assert!(!pending_threads.contains("pending"));
    }
}
