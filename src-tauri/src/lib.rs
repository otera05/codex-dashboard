mod app_server;
mod models;

use app_server::{AppServer, AppServerError};
use models::{Account, CodexModel, DashboardSnapshot, Session};
use serde_json::{json, Value};
use std::process::Command as ProcessCommand;
use std::sync::Arc;
use tauri::State;
use url::Url;

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
async fn refresh_account(
    app: tauri::AppHandle,
    server: State<'_, Arc<AppServer>>,
) -> Result<Account, AppServerError> {
    server.refresh_account(&app).await
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
async fn rename_thread(
    server: State<'_, Arc<AppServer>>,
    thread_id: String,
    name: String,
) -> Result<Session, AppServerError> {
    server.rename_thread(&thread_id, &name).await
}

#[tauri::command]
async fn archive_thread(
    server: State<'_, Arc<AppServer>>,
    thread_id: String,
) -> Result<(), AppServerError> {
    server.archive_thread(&thread_id).await
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
    app: tauri::AppHandle,
    server: State<'_, Arc<AppServer>>,
) -> Result<Option<String>, AppServerError> {
    let response = server
        .request_with_reconnect(
            &app,
            "account/login/start",
            json!({ "type": "chatgpt", "useHostedLoginSuccessPage": true, "appBrand": "codex" }),
        )
        .await?;
    let auth_url = response
        .get("authUrl")
        .or_else(|| response.get("url"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    if let Some(url) = auth_url.as_deref() {
        open_external_url(url)?;
    }
    Ok(auth_url)
}

fn open_external_url(url: &str) -> Result<(), AppServerError> {
    let parsed = Url::parse(url).map_err(|error| AppServerError::Rpc(error.to_string()))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppServerError::Rpc(
            "Login URL must use http or https".to_owned(),
        ));
    }

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = ProcessCommand::new("open");
        command.arg(url);
        command
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = ProcessCommand::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = ProcessCommand::new("xdg-open");
        command.arg(url);
        command
    };

    command.spawn()?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let server = AppServer::new();
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
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
            refresh_account,
            list_models,
            create_thread,
            rename_thread,
            archive_thread,
            send_turn,
            interrupt_turn,
            resolve_approval,
            start_chatgpt_login
        ])
        .run(tauri::generate_context!())
        .expect("error while running Codex Dashboard");
}
