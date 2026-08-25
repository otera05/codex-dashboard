use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub cached: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub role: String,
    pub text: String,
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streaming: Option<bool>,
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
    pub messages: Vec<Message>,
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
    let mut messages = Vec::new();
    let mut active_turn_id = None;

    for turn in turns {
        let turn_id = turn.get("id").and_then(Value::as_str).unwrap_or_default();
        let started_at = turn
            .get("startedAt")
            .and_then(Value::as_i64)
            .unwrap_or(session.updated_at / 1000)
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
            let item_type = item.get("type").and_then(Value::as_str);
            let (role, text) = match item_type {
                Some("userMessage") => ("user", user_message_text(item)),
                Some("agentMessage") => (
                    "assistant",
                    item.get("text")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                ),
                Some("plan") => (
                    "assistant",
                    item.get("text")
                        .and_then(Value::as_str)
                        .map(|text| format!("Plan\n{text}"))
                        .unwrap_or_default(),
                ),
                _ => continue,
            };

            if text.trim().is_empty() {
                continue;
            }
            messages.push(Message {
                id: item
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("{turn_id}-{index}")),
                role: role.to_owned(),
                text,
                created_at: started_at + index as i64,
                streaming: None,
            });
        }
    }

    session.messages = messages;
    session.active_turn_id = active_turn_id;
    session.history_loaded = true;
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
                { "type": "agentMessage", "id": "agent-1", "text": "Hi there" }
            ]
        }]);

        apply_turn_history(&mut session, turns.as_array().unwrap());

        assert!(session.history_loaded);
        assert_eq!(session.active_turn_id.as_deref(), Some("turn-1"));
        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[0].text, "Hello\n[Image]");
        assert_eq!(session.messages[1].role, "assistant");
    }
}
