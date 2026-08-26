use std::{
    env,
    path::{Path, PathBuf},
    process::{Command as StdCommand, Stdio},
    sync::{atomic::Ordering, Arc},
};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};

use crate::models::RpcEnvelope;

use super::{AppServer, AppServerError};

impl AppServer {
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

    pub(super) async fn diagnostic_details(&self) -> String {
        self.diagnostics
            .lock()
            .await
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("; ")
    }
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
