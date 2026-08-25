mod app_server;
mod models;

use app_server::{AppServer, AppServerError};
use models::{CodexModel, DashboardSnapshot, Session};
use serde_json::{json, Value};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
async fn get_dashboard_snapshot(
    server: State<'_, Arc<AppServer>>,
) -> Result<DashboardSnapshot, AppServerError> {
    Ok(server.snapshot.read().await.clone())
}

#[tauri::command]
async fn get_session(
    server: State<'_, Arc<AppServer>>,
    thread_id: String,
) -> Result<Session, AppServerError> {
    server.load_session(&thread_id).await
}

#[tauri::command]
async fn refresh_session(
    server: State<'_, Arc<AppServer>>,
    thread_id: String,
) -> Result<Session, AppServerError> {
    server.refresh_session(&thread_id).await
}

#[tauri::command]
async fn refresh_session_list(
    server: State<'_, Arc<AppServer>>,
) -> Result<DashboardSnapshot, AppServerError> {
    server.reload_sessions().await
}

#[tauri::command]
async fn list_models(server: State<'_, Arc<AppServer>>) -> Result<Vec<CodexModel>, AppServerError> {
    server.list_models().await
}

#[tauri::command]
async fn create_thread(
    server: State<'_, Arc<AppServer>>,
    cwd: String,
    model: Option<String>,
    prompt: String,
) -> Result<Session, AppServerError> {
    server.create_thread(&cwd, model.as_deref(), &prompt).await
}

#[tauri::command]
async fn send_turn(
    server: State<'_, Arc<AppServer>>,
    thread_id: String,
    text: String,
) -> Result<(), AppServerError> {
    server
        .request("thread/resume", json!({ "threadId": thread_id }))
        .await?;
    server.request("turn/start", json!({ "threadId": thread_id, "input": [{ "type": "text", "text": text, "text_elements": [] }] })).await?;
    Ok(())
}

#[tauri::command]
async fn interrupt_turn(
    server: State<'_, Arc<AppServer>>,
    thread_id: String,
    turn_id: String,
) -> Result<(), AppServerError> {
    server
        .request(
            "turn/interrupt",
            json!({ "threadId": thread_id, "turnId": turn_id }),
        )
        .await?;
    Ok(())
}

#[tauri::command]
async fn resolve_approval(
    app: tauri::AppHandle,
    server: State<'_, Arc<AppServer>>,
    request_id: Value,
    decision: String,
) -> Result<(), AppServerError> {
    server.resolve_approval(&app, request_id, &decision).await
}

#[tauri::command]
async fn start_chatgpt_login(
    server: State<'_, Arc<AppServer>>,
) -> Result<Option<String>, AppServerError> {
    let response = server
        .request(
            "account/login/start",
            json!({ "type": "chatgpt", "useHostedLoginSuccessPage": true, "appBrand": "codex" }),
        )
        .await?;
    Ok(response
        .get("authUrl")
        .or_else(|| response.get("url"))
        .and_then(Value::as_str)
        .map(str::to_owned))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let server = AppServer::new();
    tauri::Builder::default()
        .manage(Arc::clone(&server))
        .setup(move |app| {
            let handle = app.handle().clone();
            let server = Arc::clone(&server);
            tauri::async_runtime::spawn(async move {
                if let Err(error) = server.connect(handle).await {
                    eprintln!("{error}");
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_dashboard_snapshot,
            get_session,
            refresh_session,
            refresh_session_list,
            list_models,
            create_thread,
            send_turn,
            interrupt_turn,
            resolve_approval,
            start_chatgpt_login
        ])
        .run(tauri::generate_context!())
        .expect("error while running Codex Dashboard");
}
