mod rpc;

use std::{
    collections::{HashSet, VecDeque},
    env,
    path::{Path, PathBuf},
    process::{Command as StdCommand, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::{Mutex, RwLock},
};

pub use rpc::AppServerError;
use rpc::Pending;

use crate::models::{
    apply_turn_history, merge_turn_history, parse_account, parse_sessions, Account,
    ApprovalRequest, CodexModel, DashboardSnapshot, RpcEnvelope, Session,
};

fn is_unmaterialized_thread_error(error: &AppServerError) -> bool {
    matches!(error, AppServerError::Rpc(message) if message.contains("is not materialized yet") && message.contains("thread/turns/list"))
}

fn requires_thread_resume(pending_threads: &HashSet<String>, thread_id: &str) -> bool {
    !pending_threads.contains(thread_id)
}

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

    pub async fn connect(self: &Arc<Self>, app: AppHandle) -> Result<(), AppServerError> {
        let generation = self.connection_generation.fetch_add(1, Ordering::Relaxed) + 1;
        self.disconnect().await;
        self.clear_diagnostics().await;
        let codex_path = resolve_codex_binary().map_err(AppServerError::Rpc)?;
        self.record_diagnostic("launch", &format!("using {}", codex_path.display()))
            .await;
        let mut command = Command::new(codex_path);
        command
            .args(["app-server", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(AppServerError::disconnected)?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(AppServerError::disconnected)?;
        let stderr = child.stderr.take();
        *self.stdin.lock().await = Some(stdin);
        *self.child.lock().await = Some(child);

        let pending = Arc::clone(&self.pending);
        let server = Arc::clone(self);
        let event_app = app.clone();
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let Ok(envelope) = serde_json::from_str::<RpcEnvelope>(&line) else {
                    server.record_diagnostic("stdout", &line).await;
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
            if server.connection_generation.load(Ordering::Relaxed) == generation {
                server.record_diagnostic("stdio", "stdout closed").await;
                server.mark_disconnected(&event_app).await;
            }
        });

        if let Some(stderr) = stderr {
            let server = Arc::clone(self);
            tauri::async_runtime::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    server.record_diagnostic("stderr", &line).await;
                }
            });
        }

        self.request("initialize", json!({ "clientInfo": { "name": "codex-dashboard", "title": "Codex Dashboard", "version": env!("CARGO_PKG_VERSION") }, "capabilities": { "experimentalApi": true } })).await?;
        self.write_notification("initialized", json!({})).await?;
        self.refresh(&app).await?;
        Ok(())
    }

    async fn disconnect(&self) {
        *self.stdin.lock().await = None;
        if let Some(mut child) = self.child.lock().await.take() {
            let _ = child.start_kill();
        }
        self.reject_pending_as_disconnected().await;
    }

    async fn mark_disconnected(&self, app: &AppHandle) {
        *self.stdin.lock().await = None;
        *self.child.lock().await = None;
        self.reject_pending_with_diagnostics().await;
        self.snapshot.write().await.connected = false;
        let _ = app.emit(
            "dashboard-event",
            json!({ "type": "connection.changed", "connected": false }),
        );
    }

    async fn clear_diagnostics(&self) {
        self.diagnostics.lock().await.clear();
    }

    async fn record_diagnostic(&self, source: &str, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }
        eprintln!("Codex App Server {source}: {line}");
        let mut diagnostics = self.diagnostics.lock().await;
        if diagnostics.len() >= 12 {
            diagnostics.pop_front();
        }
        diagnostics.push_back(format!("{source}: {line}"));
    }

    async fn diagnostic_details(&self) -> String {
        self.diagnostics
            .lock()
            .await
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("; ")
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

    pub async fn load_session(&self, thread_id: &str) -> Result<Session, AppServerError> {
        let mut turns = Vec::new();
        let mut cursor: Option<String> = None;

        for _ in 0..10 {
            let response = match self
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
                .await
            {
                Ok(response) => response,
                Err(error) if is_unmaterialized_thread_error(&error) => json!({ "data": [] }),
                Err(error) => return Err(error),
            };
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
        let session = if let Some(index) = snapshot
            .sessions
            .iter()
            .position(|session| session.id == thread_id)
        {
            &mut snapshot.sessions[index]
        } else if let Some(index) = snapshot
            .archived_sessions
            .iter()
            .position(|session| session.id == thread_id)
        {
            &mut snapshot.archived_sessions[index]
        } else {
            return Err(AppServerError::Rpc(format!(
                "Session {thread_id} was not found"
            )));
        };
        apply_turn_history(session, &turns);
        Ok(session.clone())
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

    pub async fn create_thread(
        &self,
        cwd: &str,
        model: Option<&str>,
        prompt: &str,
    ) -> Result<Session, AppServerError> {
        let mut params = json!({ "cwd": cwd, "threadSource": "codex-dashboard" });
        if let Some(model) = model.filter(|value| !value.is_empty()) {
            params["model"] = json!(model);
        }
        let response = self.request("thread/start", params).await?;
        let thread_id = response
            .pointer("/thread/id")
            .and_then(Value::as_str)
            .ok_or_else(|| AppServerError::Rpc("thread/start returned no thread id".to_owned()))?
            .to_owned();
        if !prompt.trim().is_empty() {
            self.request(
                "turn/start",
                json!({ "threadId": thread_id, "input": [{ "type": "text", "text": prompt, "text_elements": [] }] }),
            )
            .await?;
        }
        let thread = response
            .get("thread")
            .cloned()
            .ok_or_else(|| AppServerError::Rpc("thread/start returned no thread".to_owned()))?;
        let mut session = parse_sessions(&json!({ "data": [thread] }))
            .pop()
            .ok_or_else(|| AppServerError::Rpc("Could not parse the new session".to_owned()))?;
        session.model = response
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("Codex")
            .to_owned();
        if session.title.trim().is_empty() || session.title == "Untitled session" {
            session.title = prompt
                .lines()
                .find(|line| !line.trim().is_empty())
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .unwrap_or("New session")
                .chars()
                .take(80)
                .collect();
        }
        if !prompt.trim().is_empty() {
            session.status = "working".to_owned();
        } else {
            session.history_loaded = true;
            self.pending_threads.lock().await.insert(thread_id.clone());
        }
        let mut snapshot = self.snapshot.write().await;
        snapshot.sessions.retain(|item| item.id != thread_id);
        snapshot.sessions.insert(0, session.clone());
        Ok(session)
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

    pub async fn refresh_session(&self, thread_id: &str) -> Result<Session, AppServerError> {
        let response = match self
            .request(
                "thread/turns/list",
                json!({
                    "threadId": thread_id,
                    "limit": 20,
                    "sortDirection": "desc",
                    "itemsView": "full"
                }),
            )
            .await
        {
            Ok(response) => response,
            Err(error) if is_unmaterialized_thread_error(&error) => json!({ "data": [] }),
            Err(error) => return Err(error),
        };
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

    pub async fn send_turn(&self, thread_id: &str, text: &str) -> Result<(), AppServerError> {
        let requires_resume = {
            let pending_threads = self.pending_threads.lock().await;
            requires_thread_resume(&pending_threads, thread_id)
        };
        if requires_resume {
            self.request("thread/resume", json!({ "threadId": thread_id }))
                .await?;
        }
        self.request(
            "turn/start",
            json!({ "threadId": thread_id, "input": [{ "type": "text", "text": text, "text_elements": [] }] }),
        )
        .await?;
        Ok(())
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

    async fn read_account(&self, refresh_token: bool) -> Value {
        self.request("account/read", json!({ "refreshToken": refresh_token }))
            .await
            .unwrap_or(Value::Null)
    }

    async fn read_rate_limits(&self) -> Value {
        self.request("account/rateLimits/read", json!({}))
            .await
            .unwrap_or(Value::Null)
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

fn resolve_codex_binary() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("CODEX_DASHBOARD_CODEX").filter(|value| !value.is_empty()) {
        let path = PathBuf::from(path);
        return if supports_app_server(&path) {
            Ok(path)
        } else {
            Err(format!(
                "CODEX_DASHBOARD_CODEX does not support `codex app-server`: {}",
                path.display()
            ))
        };
    }

    let mut rejected = Vec::new();
    for candidate in codex_candidates() {
        if supports_app_server(&candidate) {
            return Ok(candidate);
        }
        rejected.push(candidate.display().to_string());
    }

    Err(if rejected.is_empty() {
        "No codex binary found on PATH".to_owned()
    } else {
        format!(
            "No codex binary on PATH supports `codex app-server`. Checked: {}",
            rejected.join(", ")
        )
    })
}

fn codex_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(paths) = env::var_os("PATH") {
        for directory in env::split_paths(&paths) {
            let candidate = directory.join(executable_name("codex"));
            if is_executable_file(&candidate) && !candidates.iter().any(|item| item == &candidate) {
                candidates.push(candidate);
            }
        }
    }
    candidates
}

fn supports_app_server(path: &Path) -> bool {
    let Ok(output) = StdCommand::new(path)
        .args(["app-server", "--help"])
        .stdin(Stdio::null())
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.contains("Usage: codex app-server") && stdout.contains("--stdio")
}

fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_unmaterialized_thread_history_errors() {
        let error = AppServerError::Rpc(
            r#"{"code":-32600,"message":"thread abc is not materialized yet; thread/turns/list is unavailable before first user message"}"#.to_owned(),
        );
        assert!(is_unmaterialized_thread_error(&error));
        assert!(!is_unmaterialized_thread_error(&AppServerError::Rpc(
            r#"{"code":-32600,"message":"another request failed"}"#.to_owned(),
        )));
    }

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

    #[test]
    fn skips_resume_for_a_pending_unmaterialized_thread() {
        let pending_threads = HashSet::from(["pending".to_owned()]);
        assert!(!requires_thread_resume(&pending_threads, "pending"));
        assert!(requires_thread_resume(&pending_threads, "existing"));
    }
}

fn executable_name(name: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        format!("{name}.exe")
    }
    #[cfg(not(target_os = "windows"))]
    {
        name.to_owned()
    }
}
