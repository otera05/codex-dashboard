use std::{
    collections::HashMap,
    process::Stdio,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::{oneshot, Mutex, RwLock},
};

use crate::models::{
    apply_turn_history, merge_turn_history, parse_account, parse_sessions, ApprovalRequest,
    DashboardSnapshot, RpcEnvelope, Session,
};

#[derive(Debug, Error)]
pub enum AppServerError {
    #[error("Codex App Server could not be started: {0}")]
    Io(#[from] std::io::Error),
    #[error("Codex App Server returned an error: {0}")]
    Rpc(String),
    #[error("Codex App Server disconnected")]
    Disconnected,
}

impl serde::Serialize for AppServerError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, AppServerError>>>>>;

pub struct AppServer {
    child: Mutex<Option<Child>>,
    stdin: Mutex<Option<ChildStdin>>,
    pending: Pending,
    next_id: AtomicU64,
    pub snapshot: RwLock<DashboardSnapshot>,
}

impl AppServer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            child: Mutex::new(None),
            stdin: Mutex::new(None),
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicU64::new(1),
            snapshot: RwLock::new(DashboardSnapshot::default()),
        })
    }

    pub async fn connect(self: &Arc<Self>, app: AppHandle) -> Result<(), AppServerError> {
        let mut command = Command::new("codex");
        command
            .args(["app-server", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        let mut child = command.spawn()?;
        let stdin = child.stdin.take().ok_or(AppServerError::Disconnected)?;
        let stdout = child.stdout.take().ok_or(AppServerError::Disconnected)?;
        *self.stdin.lock().await = Some(stdin);
        *self.child.lock().await = Some(child);

        let pending = Arc::clone(&self.pending);
        let server = Arc::clone(self);
        let event_app = app.clone();
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let Ok(envelope) = serde_json::from_str::<RpcEnvelope>(&line) else {
                    continue;
                };
                if let Some(method) = envelope.method {
                    if let Some(id) = envelope.id {
                        server
                            .handle_server_request(
                                &event_app,
                                method,
                                id,
                                envelope.params.unwrap_or(Value::Null),
                            )
                            .await;
                    } else {
                        server
                            .handle_notification(
                                &event_app,
                                &method,
                                envelope.params.unwrap_or(Value::Null),
                            )
                            .await;
                    }
                } else if let Some(id) = envelope.id.as_ref().and_then(Value::as_u64) {
                    if let Some(sender) = pending.lock().await.remove(&id) {
                        let result = envelope
                            .error
                            .map(|error| Err(AppServerError::Rpc(error.to_string())))
                            .unwrap_or_else(|| Ok(envelope.result.unwrap_or(Value::Null)));
                        let _ = sender.send(result);
                    }
                }
            }
            server.snapshot.write().await.connected = false;
            let _ = event_app.emit(
                "dashboard-event",
                json!({ "type": "connection.changed", "connected": false }),
            );
        });

        self.request("initialize", json!({ "clientInfo": { "name": "codex-dashboard", "title": "Codex Dashboard", "version": env!("CARGO_PKG_VERSION") }, "capabilities": { "experimentalApi": true } })).await?;
        self.write_notification("initialized", json!({})).await?;
        self.refresh(&app).await?;
        Ok(())
    }

    pub async fn request(&self, method: &str, params: Value) -> Result<Value, AppServerError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().await.insert(id, sender);
        let message = serde_json::to_string(
            &json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }),
        )
        .expect("JSON serialization cannot fail")
            + "\n";
        let result = if let Some(stdin) = self.stdin.lock().await.as_mut() {
            stdin.write_all(message.as_bytes()).await
        } else {
            return Err(AppServerError::Disconnected);
        };
        if let Err(error) = result {
            self.pending.lock().await.remove(&id);
            return Err(error.into());
        }
        receiver.await.map_err(|_| AppServerError::Disconnected)?
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

    async fn write_response(
        &self,
        id: Value,
        response: Result<Value, Value>,
    ) -> Result<(), AppServerError> {
        let envelope = match response {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
        };
        let message =
            serde_json::to_string(&envelope).expect("JSON serialization cannot fail") + "\n";
        self.stdin
            .lock()
            .await
            .as_mut()
            .ok_or(AppServerError::Disconnected)?
            .write_all(message.as_bytes())
            .await?;
        Ok(())
    }

    pub async fn load_session(&self, thread_id: &str) -> Result<Session, AppServerError> {
        let mut turns = Vec::new();
        let mut cursor: Option<String> = None;

        for _ in 0..10 {
            let response = self
                .request(
                    "thread/turns/list",
                    json!({
                        "threadId": thread_id,
                        "cursor": cursor,
                        "limit": 100,
                        "sortDirection": "asc",
                        "itemsView": "full"
                    }),
                )
                .await?;
            if let Some(page) = response.get("data").and_then(Value::as_array) {
                turns.extend(page.iter().cloned());
            }
            cursor = response
                .get("nextCursor")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if cursor.is_none() {
                break;
            }
        }

        let mut snapshot = self.snapshot.write().await;
        let session = snapshot
            .sessions
            .iter_mut()
            .find(|session| session.id == thread_id)
            .ok_or_else(|| AppServerError::Rpc(format!("Session {thread_id} was not found")))?;
        apply_turn_history(session, &turns);
        Ok(session.clone())
    }

    pub async fn refresh_session(&self, thread_id: &str) -> Result<Session, AppServerError> {
        let response = self
            .request(
                "thread/turns/list",
                json!({
                    "threadId": thread_id,
                    "limit": 20,
                    "sortDirection": "desc",
                    "itemsView": "full"
                }),
            )
            .await?;
        let mut turns = response
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        turns.sort_by_key(|turn| {
            (
                turn.get("startedAt")
                    .and_then(Value::as_i64)
                    .unwrap_or_default(),
                turn.get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            )
        });

        let mut snapshot = self.snapshot.write().await;
        let session = snapshot
            .sessions
            .iter_mut()
            .find(|session| session.id == thread_id)
            .ok_or_else(|| AppServerError::Rpc(format!("Session {thread_id} was not found")))?;
        merge_turn_history(session, &turns);
        Ok(session.clone())
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
        sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        let mut snapshot = self.snapshot.write().await;
        Self::preserve_loaded_history(&snapshot.sessions, &mut sessions);
        snapshot.sessions = sessions;
        snapshot.connected = true;
        Ok(snapshot.clone())
    }

    async fn write_notification(&self, method: &str, params: Value) -> Result<(), AppServerError> {
        let message =
            serde_json::to_string(&json!({ "jsonrpc": "2.0", "method": method, "params": params }))
                .expect("JSON serialization cannot fail")
                + "\n";
        self.stdin
            .lock()
            .await
            .as_mut()
            .ok_or(AppServerError::Disconnected)?
            .write_all(message.as_bytes())
            .await?;
        Ok(())
    }

    async fn refresh(&self, app: &AppHandle) -> Result<(), AppServerError> {
        self.reload_sessions().await?;
        let account_result = self
            .request("account/read", json!({ "refreshToken": false }))
            .await
            .unwrap_or(Value::Null);
        let rates = self
            .request("account/rateLimits/read", json!({}))
            .await
            .unwrap_or(Value::Null);
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

    fn notification_requires_refresh(method: &str) -> bool {
        matches!(
            method,
            "thread/started"
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
