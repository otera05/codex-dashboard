use serde_json::{json, Value};

use crate::models::{apply_turn_history, merge_turn_history, Session};

use super::{AppServer, AppServerError};

fn is_unmaterialized_thread_error(error: &AppServerError) -> bool {
    matches!(error, AppServerError::Rpc(message) if message.contains("is not materialized yet") && message.contains("thread/turns/list"))
}

impl AppServer {
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
}
