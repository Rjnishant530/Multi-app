use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use parking_lot::RwLock;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;
use tokio::sync::mpsc;
use tracing::warn;

use crate::model::{self, AppState};

pub const STORE_FILENAME: &str = "metadata/store.json";
pub const STATE_KEY: &str = "state";

const DEBOUNCE_INTERVAL: Duration = Duration::from_millis(250);

pub type SharedState = Arc<RwLock<AppState>>;

pub trait StateWriter: Send + 'static {
    fn write(&self, state: &AppState);
}

impl<F> StateWriter for F
where
    F: Fn(&AppState) + Send + 'static,
{
    fn write(&self, state: &AppState) {
        (self)(state)
    }
}

pub struct Persister {
    tx: mpsc::UnboundedSender<()>,
}

impl Persister {
    /// Build the persister and its async loop, but DO NOT spawn it.
    /// The caller chooses the runtime — production uses Tauri's runtime
    /// (`tauri::async_runtime::spawn`); tests use the `#[tokio::test]`
    /// runtime so paused-time semantics work.
    pub fn build<W: StateWriter>(
        state: SharedState,
        writer: W,
        debounce: Duration,
    ) -> (Self, std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>) {
        let (tx, mut rx) = mpsc::unbounded_channel::<()>();
        let fut = async move {
            loop {
                if rx.recv().await.is_none() {
                    return;
                }
                // Coalesce: keep extending the wait window as long as new
                // notifications keep arriving within the debounce interval.
                loop {
                    match tokio::time::timeout(debounce, rx.recv()).await {
                        Ok(Some(_)) => continue,
                        Ok(None) => return,
                        Err(_) => break,
                    }
                }
                let snapshot = state.read().clone();
                writer.write(&snapshot);
            }
        };
        (Self { tx }, Box::pin(fut))
    }

    /// Production entry: build and spawn on Tauri's async runtime.
    pub fn start<W: StateWriter>(state: SharedState, writer: W) -> Self {
        Self::start_with_interval(state, writer, DEBOUNCE_INTERVAL)
    }

    pub fn start_with_interval<W: StateWriter>(
        state: SharedState,
        writer: W,
        debounce: Duration,
    ) -> Self {
        let (this, fut) = Self::build(state, writer, debounce);
        tauri::async_runtime::spawn(fut);
        this
    }

    pub fn schedule(&self) {
        let _ = self.tx.send(());
    }
}

pub fn load_initial_state(app: &AppHandle) -> Result<AppState> {
    let store = app
        .store(STORE_FILENAME)
        .context("failed to open metadata store")?;
    match store.get(STATE_KEY) {
        Some(value) => {
            let raw = value.to_string();
            match model::deserialize(&raw) {
                Ok(state) => Ok(state),
                Err(err) => {
                    warn!(error = %err, "could not parse persisted state; starting fresh");
                    Ok(AppState::default())
                }
            }
        }
        None => Ok(AppState::default()),
    }
}

pub fn make_app_writer(app: AppHandle) -> impl StateWriter {
    move |state: &AppState| {
        let raw = match model::serialize(state) {
            Ok(s) => s,
            Err(err) => {
                warn!(error = %err, "failed to serialize state");
                return;
            }
        };
        let value: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(err) => {
                warn!(error = %err, "failed to reparse serialized state");
                return;
            }
        };
        let store = match app.store(STORE_FILENAME) {
            Ok(s) => s,
            Err(err) => {
                warn!(error = %err, "failed to open store for write");
                return;
            }
        };
        store.set(STATE_KEY, value);
        if let Err(err) = store.save() {
            warn!(error = %err, "failed to flush store to disk");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::*;

    fn shared(state: AppState) -> SharedState {
        Arc::new(RwLock::new(state))
    }

    struct CountingWriter {
        count: Arc<AtomicUsize>,
        snapshots: Arc<Mutex<Vec<AppState>>>,
    }

    impl StateWriter for CountingWriter {
        fn write(&self, state: &AppState) {
            self.count.fetch_add(1, Ordering::SeqCst);
            self.snapshots.lock().unwrap().push(state.clone());
        }
    }

    fn counting() -> (
        CountingWriter,
        Arc<AtomicUsize>,
        Arc<Mutex<Vec<AppState>>>,
    ) {
        let count = Arc::new(AtomicUsize::new(0));
        let snapshots = Arc::new(Mutex::new(Vec::new()));
        let writer = CountingWriter {
            count: count.clone(),
            snapshots: snapshots.clone(),
        };
        (writer, count, snapshots)
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn single_schedule_writes_once_after_debounce() {
        let state = shared(AppState::default());
        let (writer, count, _) = counting();
        let (persister, fut) =
            Persister::build(state, writer, Duration::from_millis(250));
        tokio::spawn(fut);

        persister.schedule();
        // Less than the debounce — no write yet.
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(count.load(Ordering::SeqCst), 0);
        // Past the debounce — exactly one write.
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn ten_rapid_schedules_collapse_into_at_most_two_writes() {
        let state = shared(AppState::default());
        let (writer, count, _) = counting();
        let (persister, fut) =
            Persister::build(state, writer, Duration::from_millis(250));
        tokio::spawn(fut);

        for _ in 0..10 {
            persister.schedule();
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        let writes = count.load(Ordering::SeqCst);
        assert!(
            (1..=2).contains(&writes),
            "expected 1..=2 writes from a tight burst, got {writes}"
        );
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn separated_schedules_produce_separate_writes() {
        let state = shared(AppState::default());
        let (writer, count, _) = counting();
        let (persister, fut) =
            Persister::build(state, writer, Duration::from_millis(100));
        tokio::spawn(fut);

        persister.schedule();
        tokio::time::sleep(Duration::from_millis(200)).await;
        persister.schedule();
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn writer_observes_latest_state_snapshot() {
        use uuid::Uuid;

        use crate::model::Website;

        let state = shared(AppState::default());
        let (writer, _, snapshots) = counting();
        let (persister, fut) =
            Persister::build(state.clone(), writer, Duration::from_millis(100));
        tokio::spawn(fut);

        persister.schedule();
        // Mutate during the debounce window — the eventual write must observe
        // the LATEST state, not the state at schedule() time.
        tokio::time::sleep(Duration::from_millis(20)).await;
        {
            let mut w = state.write();
            w.websites.push(Website {
                id: Uuid::nil(),
                url_root: "https://added-after-schedule.test".into(),
                display_title: "added".into(),
                root_instance_ids: vec![],
                active_instance_id: None,
                created_at_ms: 7,
            });
        }
        tokio::time::sleep(Duration::from_millis(250)).await;

        let snaps = snapshots.lock().unwrap();
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].websites.len(), 1);
        assert_eq!(snaps[0].websites[0].url_root, "https://added-after-schedule.test");
    }
}
