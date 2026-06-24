//! Lifecycle of per-instance child webviews mounted in the main window.
//!
//! One webview per active `Instance`, identified by a stable label
//! `instance-<uuid>`. Only one webview is visible at a time; the rest
//! are hidden via `Webview::hide()` but stay spawned so re-activation
//! is instant. The currently-visible webview is positioned to the
//! viewport rect that the React layer reports from its placeholder div.
//!
//! Session isolation is platform-specific and centralized in
//! `apply_isolation` below.

#![allow(dead_code)] // referenced by command surface in U4.

use std::collections::HashMap;

use anyhow::{Context, Result, anyhow};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl,
    webview::{NewWindowResponse, WebviewBuilder},
};
use tokio::sync::mpsc;
use tracing::warn;
use url::Url;
use uuid::Uuid;

use crate::model::Instance;
use crate::nav_guard;
#[cfg(not(target_os = "macos"))]
use crate::paths;

/// Request from a webview closure asking the orchestrator to spawn a
/// new child instance rooted at `target_url`, attached as a child of
/// `parent_instance_id`.
#[derive(Debug, Clone)]
pub struct ForkRequest {
    pub parent_instance_id: Uuid,
    pub target_url: String,
}

pub type ForkSender = mpsc::UnboundedSender<ForkRequest>;
pub type ForkReceiver = mpsc::UnboundedReceiver<ForkRequest>;

pub fn fork_channel() -> (ForkSender, ForkReceiver) {
    mpsc::unbounded_channel()
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct ViewportRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub struct WebviewManager {
    inner: RwLock<Inner>,
    fork_tx: ForkSender,
}

struct Inner {
    spawned: HashMap<Uuid, String>,
    active: Option<Uuid>,
    viewport: ViewportRect,
}

impl WebviewManager {
    pub fn new(fork_tx: ForkSender) -> Self {
        Self {
            inner: RwLock::new(Inner {
                spawned: HashMap::new(),
                active: None,
                viewport: ViewportRect::default(),
            }),
            fork_tx,
        }
    }

    pub fn label_for(instance_id: &Uuid) -> String {
        // The capability file wildcards `instance-*`; keep this prefix
        // in sync with `capabilities/default.json`.
        format!("instance-{}", instance_id.simple())
    }

    pub fn ensure_spawned(&self, app: &AppHandle, instance: &Instance) -> Result<String> {
        {
            let inner = self.inner.read();
            if let Some(label) = inner.spawned.get(&instance.id) {
                return Ok(label.clone());
            }
        }

        let label = Self::label_for(&instance.id);
        let url = url::Url::parse(&instance.current_url)
            .with_context(|| format!("invalid url for instance: {}", instance.current_url))?;
        // Per-webview "trust boundary": the eTLD+1 the webview was
        // spawned on. Same-site navigations stay inside this webview;
        // cross-site navigations fork into a child instance.
        let parent_root = nav_guard::registered_domain(&url).ok_or_else(|| {
            anyhow!(
                "cannot determine registered domain for instance url: {}",
                instance.current_url
            )
        })?;
        let parent_id = instance.id;

        let builder: WebviewBuilder<tauri::Wry> =
            WebviewBuilder::new(label.clone(), WebviewUrl::External(url));
        let builder = apply_isolation(app, builder, &instance.id)?;
        let builder = attach_navigation_handlers(
            builder,
            parent_id,
            parent_root,
            self.fork_tx.clone(),
        );

        let window = app
            .get_window("main")
            .ok_or_else(|| anyhow!("main window not yet initialized"))?;
        let (position, size) = {
            let inner = self.inner.read();
            (
                LogicalPosition::new(inner.viewport.x, inner.viewport.y),
                LogicalSize::new(
                    inner.viewport.width.max(1.0),
                    inner.viewport.height.max(1.0),
                ),
            )
        };
        let webview = window
            .add_child(builder, position, size)
            .with_context(|| format!("failed to add child webview {label}"))?;
        webview.hide().ok();

        let mut inner = self.inner.write();
        inner.spawned.insert(instance.id, label.clone());
        Ok(label)
    }

    pub fn activate(&self, app: &AppHandle, id: Uuid) -> Result<()> {
        let (prev_label, target_label, rect) = {
            let mut inner = self.inner.write();
            let prev_label = inner
                .active
                .filter(|prev| *prev != id)
                .and_then(|prev| inner.spawned.get(&prev).cloned());
            let target_label = inner
                .spawned
                .get(&id)
                .cloned()
                .ok_or_else(|| anyhow!("instance not spawned: {id}"))?;
            let rect = inner.viewport;
            inner.active = Some(id);
            (prev_label, target_label, rect)
        };

        if let Some(prev) = prev_label {
            if let Some(wv) = app.get_webview(&prev) {
                wv.hide().ok();
            }
        }
        let wv = app
            .get_webview(&target_label)
            .ok_or_else(|| anyhow!("webview missing: {target_label}"))?;
        wv.set_position(LogicalPosition::new(rect.x, rect.y))?;
        wv.set_size(LogicalSize::new(
            rect.width.max(1.0),
            rect.height.max(1.0),
        ))?;
        wv.show()?;
        Ok(())
    }

    pub fn deactivate(&self, app: &AppHandle) {
        let prev = {
            let mut inner = self.inner.write();
            let label = inner
                .active
                .and_then(|id| inner.spawned.get(&id).cloned());
            inner.active = None;
            label
        };
        if let Some(label) = prev {
            if let Some(wv) = app.get_webview(&label) {
                wv.hide().ok();
            }
        }
    }

    pub fn set_viewport(&self, app: &AppHandle, rect: ViewportRect) -> Result<()> {
        let label = {
            let mut inner = self.inner.write();
            inner.viewport = rect;
            inner.active.and_then(|id| inner.spawned.get(&id).cloned())
        };
        if let Some(label) = label {
            if let Some(wv) = app.get_webview(&label) {
                tracing::debug!(
                    x = rect.x,
                    y = rect.y,
                    width = rect.width,
                    height = rect.height,
                    label = %label,
                    "set_viewport applied to active webview"
                );
                wv.set_position(LogicalPosition::new(rect.x, rect.y))?;
                wv.set_size(LogicalSize::new(
                    rect.width.max(1.0),
                    rect.height.max(1.0),
                ))?;
            }
        }
        Ok(())
    }

    pub fn destroy(&self, app: &AppHandle, id: Uuid) -> Result<()> {
        let label = {
            let mut inner = self.inner.write();
            if inner.active == Some(id) {
                inner.active = None;
            }
            inner.spawned.remove(&id)
        };
        if let Some(label) = label {
            if let Some(wv) = app.get_webview(&label) {
                wv.close().ok();
            }
        }
        Ok(())
    }

    pub fn active(&self) -> Option<Uuid> {
        self.inner.read().active
    }

    pub fn is_spawned(&self, id: &Uuid) -> bool {
        self.inner.read().spawned.contains_key(id)
    }

    pub fn current_viewport(&self) -> ViewportRect {
        self.inner.read().viewport
    }
}

fn attach_navigation_handlers(
    builder: WebviewBuilder<tauri::Wry>,
    parent_id: Uuid,
    parent_root: String,
    fork_tx: ForkSender,
) -> WebviewBuilder<tauri::Wry> {
    let nav_root = parent_root.clone();
    let nav_tx = fork_tx.clone();
    let builder = builder.on_navigation(move |target: &Url| {
        if !nav_guard::should_fork(&nav_root, target) {
            // Same-site or no-registered-domain (about:blank, data:, etc.)
            // → let the webview proceed normally.
            return true;
        }
        // Real cross-site: hand off and cancel the parent navigation.
        if let Err(err) = nav_tx.send(ForkRequest {
            parent_instance_id: parent_id,
            target_url: target.to_string(),
        }) {
            warn!(error = ?err, "fork channel closed; cross-site nav dropped");
        }
        false
    });

    let popup_root = parent_root;
    let popup_tx = fork_tx;
    builder.on_new_window(move |target: Url, _features| {
        // window.open / target=_blank: we never let the OS spawn a
        // separate window we don't control. If the target is a real
        // distinct site, fork; if it's same-site or schemeless
        // (about:blank handoffs are common in OAuth/SSO flows), deny
        // without forking — the parent webview continues to own it.
        if nav_guard::should_fork(&popup_root, &target) {
            if let Err(err) = popup_tx.send(ForkRequest {
                parent_instance_id: parent_id,
                target_url: target.to_string(),
            }) {
                warn!(error = ?err, "fork channel closed; popup nav dropped");
            }
        }
        NewWindowResponse::Deny
    })
}

// ---- Platform-conditional session isolation ----
//
// macOS 14+ : WKWebsiteDataStore(forIdentifier:) — Wry exposes this as
// `data_store_identifier([u8; 16])`. The UUID bytes ARE the identifier;
// WebKit owns the on-disk store under ~/Library/WebKit/WebsiteDataStore/.
//
// Windows / Linux : the unit of isolation is a directory on disk
// (WebView2 user data folder, WebKitGTK WebsiteDataManager base dir).
// `data_directory(path)` drives both.

#[cfg(target_os = "macos")]
fn apply_isolation(
    _app: &AppHandle,
    builder: WebviewBuilder<tauri::Wry>,
    instance_id: &Uuid,
) -> Result<WebviewBuilder<tauri::Wry>> {
    Ok(builder.data_store_identifier(*instance_id.as_bytes()))
}

#[cfg(not(target_os = "macos"))]
fn apply_isolation(
    app: &AppHandle,
    builder: WebviewBuilder<tauri::Wry>,
    instance_id: &Uuid,
) -> Result<WebviewBuilder<tauri::Wry>> {
    let dir = paths::webview_data_dir(app, instance_id)?;
    Ok(builder.data_directory(dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager_for_tests() -> (WebviewManager, ForkReceiver) {
        let (tx, rx) = fork_channel();
        (WebviewManager::new(tx), rx)
    }

    #[test]
    fn label_for_uses_instance_uuid_simple_form() {
        let id = Uuid::from_u128(0xDEAD_BEEF_DEAD_BEEF_DEAD_BEEF_DEAD_BEEF);
        let label = WebviewManager::label_for(&id);
        assert!(label.starts_with("instance-"));
        assert_eq!(label.len(), "instance-".len() + 32);
        assert_eq!(label, "instance-deadbeefdeadbeefdeadbeefdeadbeef");
    }

    #[test]
    fn label_collisions_are_impossible_for_distinct_uuids() {
        let a = WebviewManager::label_for(&Uuid::from_u128(1));
        let b = WebviewManager::label_for(&Uuid::from_u128(2));
        assert_ne!(a, b);
    }

    #[test]
    fn manager_starts_empty() {
        let (mgr, _rx) = manager_for_tests();
        assert_eq!(mgr.active(), None);
        assert!(!mgr.is_spawned(&Uuid::nil()));
        let v = mgr.current_viewport();
        assert_eq!(v.width, 0.0);
        assert_eq!(v.height, 0.0);
    }
}
