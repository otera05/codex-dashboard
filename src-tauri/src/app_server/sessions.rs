use serde_json::{json, Value};

use std::collections::HashSet;

use crate::models::{
    apply_turn_history, merge_turn_history, parse_sessions, DashboardSnapshot, Session,
};

use super::{AppServer, AppServerError};

fn is_unmaterialized_thread_error(error: &AppServerError) -> bool {
    matches!(error, AppServerError::Rpc(message) if message.contains("is not materialized yet") && message.contains("thread/turns/list"))
}

fn requires_thread_resume(pending_threads: &HashSet<String>, thread_id: &str) -> bool {
    !pending_threads.contains(thread_id)
}

impl AppServer {
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
    fn skips_resume_for_a_pending_unmaterialized_thread() {
        let pending_threads = HashSet::from(["pending".to_owned()]);
        assert!(!requires_thread_resume(&pending_threads, "pending"));
        assert!(requires_thread_resume(&pending_threads, "existing"));
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
}
