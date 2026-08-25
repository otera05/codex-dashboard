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
            }
        })
        .collect()
}
