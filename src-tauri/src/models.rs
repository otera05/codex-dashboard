use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub cached: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TimelineItem {
    Message {
        id: String,
        role: String,
        text: String,
        created_at: i64,
        #[serde(skip_serializing_if = "Option::is_none")]
        streaming: Option<bool>,
    },
    Command {
        id: String,
        command: String,
        cwd: String,
        status: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        exit_code: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ms: Option<i64>,
        created_at: i64,
    },
    FileChange {
        id: String,
        status: String,
        changes: Vec<FileChange>,
        created_at: i64,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    path: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    move_path: Option<String>,
    diff: String,
    additions: usize,
    deletions: usize,
}

impl TimelineItem {
    fn id(&self) -> &str {
        match self {
            Self::Message { id, .. } | Self::Command { id, .. } | Self::FileChange { id, .. } => id,
        }
    }

    fn created_at(&self) -> i64 {
        match self {
            Self::Message { created_at, .. }
            | Self::Command { created_at, .. }
            | Self::FileChange { created_at, .. } => *created_at,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub title: String,
    pub cwd: String,
    pub status: String,
    pub updated_at: i64,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_turn_id: Option<String>,
    pub messages: Vec<TimelineItem>,
    pub token_usage: TokenUsage,
    pub history_loaded: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<i64>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSnapshot {
    pub sessions: Vec<Session>,
    pub account: Account,
    pub connected: bool,
}

#[derive(Debug, Deserialize)]
pub struct RpcEnvelope {
    pub id: Option<Value>,
    pub method: Option<String>,
    pub result: Option<Value>,
    pub error: Option<Value>,
}

pub fn parse_account(account_result: &Value, rate_result: &Value) -> Account {
    let account = account_result.pointer("/account");
    let primary = rate_result.pointer("/rateLimits/primary");
    Account {
        connected: account.is_some_and(|value| !value.is_null()),
        email: account
            .and_then(|value| value.get("email"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        plan: account
            .and_then(|value| value.get("planType"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        used_percent: primary
            .and_then(|value| value.get("usedPercent"))
            .and_then(Value::as_f64),
        resets_at: primary
            .and_then(|value| value.get("resetsAt"))
            .and_then(Value::as_i64)
            .map(|seconds| seconds * 1000),
    }
}

pub fn parse_sessions(value: &Value) -> Vec<Session> {
    value
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|thread| {
            let id = thread
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let preview = thread
                .get("preview")
                .and_then(Value::as_str)
                .unwrap_or("Untitled session");
            let title = thread
                .get("name")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .unwrap_or(preview)
                .to_owned();
            let status = thread
                .pointer("/status/type")
                .or_else(|| thread.get("status"))
                .and_then(Value::as_str)
                .unwrap_or("idle");
            let mapped_status = match status {
                "active" | "running" => "working",
                "waiting" => "waiting",
                "systemError" | "error" => "error",
                _ => "idle",
            };
            Session {
                id,
                title,
                cwd: thread
                    .get("cwd")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                status: mapped_status.to_owned(),
                updated_at: thread
                    .get("updatedAt")
                    .and_then(Value::as_i64)
                    .unwrap_or_default()
                    * 1000,
                model: thread
                    .get("model")
                    .and_then(Value::as_str)
                    .unwrap_or("Codex")
                    .to_owned(),
                active_turn_id: None,
                messages: Vec::new(),
                token_usage: TokenUsage::default(),
                history_loaded: false,
            }
        })
        .collect()
}

pub fn apply_turn_history(session: &mut Session, turns: &[Value]) {
    let (messages, active_turn_id) = messages_from_turns(session.updated_at, turns);
    session.messages = messages;
    session.active_turn_id = active_turn_id;
    session.history_loaded = true;
}

pub fn merge_turn_history(session: &mut Session, turns: &[Value]) {
    let (messages, active_turn_id) = messages_from_turns(session.updated_at, turns);
    let refreshed_ids: HashSet<&str> = messages.iter().map(TimelineItem::id).collect();
    session
        .messages
        .retain(|message| !refreshed_ids.contains(message.id()));
    session.messages.extend(messages);
    session.messages.sort_by(|left, right| {
        left.created_at()
            .cmp(&right.created_at())
            .then(left.id().cmp(right.id()))
    });
    session.active_turn_id = active_turn_id;
    session.history_loaded = true;
}

fn messages_from_turns(updated_at: i64, turns: &[Value]) -> (Vec<TimelineItem>, Option<String>) {
    let mut messages = Vec::new();
    let mut active_turn_id = None;

    for turn in turns {
        let turn_id = turn.get("id").and_then(Value::as_str).unwrap_or_default();
        let started_at = turn
            .get("startedAt")
            .and_then(Value::as_i64)
            .unwrap_or(updated_at / 1000)
            * 1000;

        if turn.get("status").and_then(Value::as_str) == Some("inProgress") {
            active_turn_id = Some(turn_id.to_owned());
        }

        for (index, item) in turn
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            let item_id = item
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| index.to_string());
            let id = format!("{turn_id}:{item_id}");
            let created_at = started_at + index as i64;
            match item.get("type").and_then(Value::as_str) {
                Some("userMessage") => push_message(
                    &mut messages,
                    id,
                    "user",
                    user_message_text(item),
                    created_at,
                ),
                Some("agentMessage") => push_message(
                    &mut messages,
                    id,
                    "assistant",
                    item.get("text")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    created_at,
                ),
                Some("plan") => push_message(
                    &mut messages,
                    id,
                    "assistant",
                    item.get("text")
                        .and_then(Value::as_str)
                        .map(|text| format!("Plan\n{text}"))
                        .unwrap_or_default(),
                    created_at,
                ),
                Some("commandExecution") => messages.push(TimelineItem::Command {
                    id,
                    command: item
                        .get("command")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    cwd: item
                        .get("cwd")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    status: item
                        .get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("completed")
                        .to_owned(),
                    output: item
                        .get("aggregatedOutput")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    exit_code: item.get("exitCode").and_then(Value::as_i64),
                    duration_ms: item.get("durationMs").and_then(Value::as_i64),
                    created_at,
                }),
                Some("fileChange") => messages.push(TimelineItem::FileChange {
                    id,
                    status: item
                        .get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("completed")
                        .to_owned(),
                    changes: file_changes(item),
                    created_at,
                }),
                _ => {}
            }
        }
    }

    (messages, active_turn_id)
}

fn file_changes(item: &Value) -> Vec<FileChange> {
    item.get("changes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|change| {
            let diff = change
                .get("diff")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let (additions, deletions) = diff_stats(&diff);
            FileChange {
                path: change
                    .get("path")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                kind: change
                    .pointer("/kind/type")
                    .and_then(Value::as_str)
                    .unwrap_or("update")
                    .to_owned(),
                move_path: change
                    .pointer("/kind/move_path")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                diff,
                additions,
                deletions,
            }
        })
        .collect()
}

fn diff_stats(diff: &str) -> (usize, usize) {
    diff.lines().fold((0, 0), |(additions, deletions), line| {
        if line.starts_with('+') && !line.starts_with("+++") {
            (additions + 1, deletions)
        } else if line.starts_with('-') && !line.starts_with("---") {
            (additions, deletions + 1)
        } else {
            (additions, deletions)
        }
    })
}

fn push_message(
    items: &mut Vec<TimelineItem>,
    id: String,
    role: &str,
    text: String,
    created_at: i64,
) {
    if text.trim().is_empty() {
        return;
    }
    items.push(TimelineItem::Message {
        id,
        role: role.to_owned(),
        text,
        created_at,
        streaming: None,
    });
}

fn user_message_text(item: &Value) -> String {
    item.get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|input| match input.get("type").and_then(Value::as_str) {
            Some("text") => input.get("text").and_then(Value::as_str).map(str::to_owned),
            Some("image") | Some("localImage") => Some("[Image]".to_owned()),
            Some("audio") | Some("localAudio") => Some("[Audio]".to_owned()),
            Some("skill") => input
                .get("name")
                .and_then(Value::as_str)
                .map(|name| format!("[Skill: {name}]")),
            Some("mention") => input
                .get("name")
                .and_then(Value::as_str)
                .map(|name| format!("@{name}")),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn session() -> Session {
        Session {
            id: "thread-1".into(),
            title: "Test".into(),
            cwd: "/tmp".into(),
            status: "idle".into(),
            updated_at: 1_700_000_000_000,
            model: "Codex".into(),
            active_turn_id: None,
            messages: Vec::new(),
            token_usage: TokenUsage::default(),
            history_loaded: false,
        }
    }

    #[test]
    fn converts_full_turn_items_to_messages() {
        let mut session = session();
        let turns = json!([{
            "id": "turn-1",
            "status": "inProgress",
            "startedAt": 1_700_000_000,
            "items": [
                { "type": "userMessage", "id": "user-1", "content": [
                    { "type": "text", "text": "Hello", "text_elements": [] },
                    { "type": "localImage", "path": "/tmp/image.png" }
                ]},
                { "type": "reasoning", "id": "reasoning-1", "summary": [], "content": [] },
                { "type": "commandExecution", "id": "command-1", "command": "npm test", "cwd": "/tmp", "status": "completed", "aggregatedOutput": "1 passed", "exitCode": 0, "durationMs": 420 },
                { "type": "fileChange", "id": "change-1", "status": "completed", "changes": [{
                    "path": "src/App.tsx", "kind": { "type": "update", "move_path": null },
                    "diff": "--- a/src/App.tsx\n+++ b/src/App.tsx\n@@ -1 +1 @@\n-old\n+new"
                }]},
                { "type": "agentMessage", "id": "agent-1", "text": "Hi there" }
            ]
        }]);

        apply_turn_history(&mut session, turns.as_array().unwrap());

        assert!(session.history_loaded);
        assert_eq!(session.active_turn_id.as_deref(), Some("turn-1"));
        assert_eq!(session.messages.len(), 4);
        assert!(
            matches!(&session.messages[0], TimelineItem::Message { text, .. } if text == "Hello\n[Image]")
        );
        assert!(
            matches!(&session.messages[1], TimelineItem::Command { command, output, exit_code: Some(0), .. } if command == "npm test" && output.as_deref() == Some("1 passed"))
        );
        assert!(
            matches!(&session.messages[2], TimelineItem::FileChange { changes, .. } if changes.len() == 1 && changes[0].additions == 1 && changes[0].deletions == 1)
        );
        assert!(
            matches!(&session.messages[3], TimelineItem::Message { role, .. } if role == "assistant")
        );
        let serialized = serde_json::to_value(&session.messages[2]).unwrap();
        assert_eq!(serialized["changes"][0]["kind"], "update");
        assert_eq!(serialized["changes"][0]["additions"], 1);
    }

    #[test]
    fn merges_refreshed_turn_items_without_losing_older_history() {
        let mut session = session();
        session.history_loaded = true;
        session.messages = vec![TimelineItem::Message {
            id: "old-agent".into(),
            role: "assistant".into(),
            text: "Older response".into(),
            created_at: 1_699_999_000_000,
            streaming: None,
        }];
        let turns = json!([{
            "id": "turn-2",
            "status": "completed",
            "startedAt": 1_700_000_001,
            "items": [
                { "type": "userMessage", "id": "user-2", "content": [{ "type": "text", "text": "Next", "text_elements": [] }] },
                { "type": "agentMessage", "id": "agent-2", "text": "Latest response" }
            ]
        }]);

        merge_turn_history(&mut session, turns.as_array().unwrap());
        merge_turn_history(&mut session, turns.as_array().unwrap());

        assert_eq!(session.messages.len(), 3);
        assert_eq!(session.messages[0].id(), "old-agent");
        assert!(
            matches!(&session.messages[2], TimelineItem::Message { text, .. } if text == "Latest response")
        );
        assert_eq!(session.active_turn_id, None);
    }

    #[test]
    fn updates_a_running_command_in_place() {
        let mut session = session();
        let running = json!([{
            "id": "turn-command",
            "status": "inProgress",
            "startedAt": 1_700_000_001,
            "items": [{
                "type": "commandExecution", "id": "command-1", "command": "cargo test",
                "cwd": "/tmp", "status": "inProgress", "aggregatedOutput": "running", "exitCode": null, "durationMs": null
            }]
        }]);
        let completed = json!([{
            "id": "turn-command",
            "status": "completed",
            "startedAt": 1_700_000_001,
            "items": [{
                "type": "commandExecution", "id": "command-1", "command": "cargo test",
                "cwd": "/tmp", "status": "completed", "aggregatedOutput": "ok", "exitCode": 0, "durationMs": 500
            }]
        }]);

        apply_turn_history(&mut session, running.as_array().unwrap());
        merge_turn_history(&mut session, completed.as_array().unwrap());

        assert_eq!(session.messages.len(), 1);
        assert!(
            matches!(&session.messages[0], TimelineItem::Command { status, output, exit_code: Some(0), .. } if status == "completed" && output.as_deref() == Some("ok"))
        );
        let serialized = serde_json::to_value(&session.messages[0]).unwrap();
        assert_eq!(
            serialized.get("type").and_then(Value::as_str),
            Some("command")
        );
        assert_eq!(serialized.get("exitCode").and_then(Value::as_i64), Some(0));
    }
}
