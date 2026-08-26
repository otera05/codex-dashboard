use std::{collections::HashMap, sync::Arc};

use serde_json::{json, Value};
use tauri::AppHandle;
use thiserror::Error;
use tokio::{
    io::AsyncWriteExt,
    sync::{oneshot, Mutex},
};

use super::AppServer;

#[derive(Debug, Error)]
pub enum AppServerError {
    #[error("Codex App Server could not be started: {0}")]
    Io(#[from] std::io::Error),
    #[error("Codex App Server returned an error: {0}")]
    Rpc(String),
    #[error("Codex App Server disconnected{0}")]
    Disconnected(String),
}

impl AppServerError {
    pub(super) fn disconnected() -> Self {
        Self::Disconnected(String::new())
    }

    fn disconnected_with_details(details: &str) -> Self {
        let details = details.trim();
        if details.is_empty() {
            Self::disconnected()
        } else {
            Self::Disconnected(format!(": {details}"))
        }
    }
}

impl serde::Serialize for AppServerError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub(super) type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, AppServerError>>>>>;

impl AppServer {
    pub(super) async fn reject_pending_as_disconnected(&self) {
        let mut pending = self.pending.lock().await;
        for (_, sender) in pending.drain() {
            let _ = sender.send(Err(AppServerError::disconnected()));
        }
    }

    pub(super) async fn reject_pending_with_diagnostics(&self) {
        let details = self.diagnostic_details().await;
        let mut pending = self.pending.lock().await;
        for (_, sender) in pending.drain() {
            let _ = sender.send(Err(AppServerError::disconnected_with_details(&details)));
        }
    }

    async fn disconnected_error(&self) -> AppServerError {
        AppServerError::disconnected_with_details(&self.diagnostic_details().await)
    }

    pub async fn request(&self, method: &str, params: Value) -> Result<Value, AppServerError> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
            return Err(self.disconnected_error().await);
        };
        if let Err(error) = result {
            self.pending.lock().await.remove(&id);
            if error.kind() == std::io::ErrorKind::BrokenPipe {
                return Err(self.disconnected_error().await);
            }
            return Err(error.into());
        }
        match receiver.await {
            Ok(result) => result,
            Err(_) => Err(self.disconnected_error().await),
        }
    }

    pub async fn request_with_reconnect(
        self: &Arc<Self>,
        app: &AppHandle,
        method: &str,
        params: Value,
    ) -> Result<Value, AppServerError> {
        match self.request(method, params.clone()).await {
            Err(AppServerError::Disconnected(_)) => {
                self.connect(app.clone()).await?;
                self.request(method, params).await
            }
            result => result,
        }
    }

    pub(super) async fn write_response(
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
            .ok_or_else(AppServerError::disconnected)?
            .write_all(message.as_bytes())
            .await?;
        Ok(())
    }

    pub(super) async fn write_notification(
        &self,
        method: &str,
        params: Value,
    ) -> Result<(), AppServerError> {
        let message =
            serde_json::to_string(&json!({ "jsonrpc": "2.0", "method": method, "params": params }))
                .expect("JSON serialization cannot fail")
                + "\n";
        self.stdin
            .lock()
            .await
            .as_mut()
            .ok_or_else(AppServerError::disconnected)?
            .write_all(message.as_bytes())
            .await?;
        Ok(())
    }
}
