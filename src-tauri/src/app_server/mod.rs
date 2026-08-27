mod account;
mod approvals;
mod events;
mod process;
mod rpc;
mod sessions;

use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{atomic::AtomicU64, Arc},
};

use serde_json::{json, Value};
use tokio::{
    process::{Child, ChildStdin},
    sync::{Mutex, RwLock},
};

pub use rpc::AppServerError;
use rpc::Pending;

use crate::models::{CodexModel, DashboardSnapshot};

pub struct AppServer {
    child: Mutex<Option<Child>>,
    stdin: Mutex<Option<ChildStdin>>,
    pending: Pending,
    next_id: AtomicU64,
    connection_generation: AtomicU64,
    refresh_revision: AtomicU64,
    refresh_lock: Mutex<()>,
    diagnostics: Mutex<VecDeque<String>>,
    pending_threads: Mutex<HashSet<String>>,
    session_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
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
            refresh_revision: AtomicU64::new(0),
            refresh_lock: Mutex::new(()),
            diagnostics: Mutex::new(VecDeque::new()),
            pending_threads: Mutex::new(HashSet::new()),
            session_locks: Mutex::new(HashMap::new()),
            snapshot: RwLock::new(DashboardSnapshot::default()),
        })
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
}
