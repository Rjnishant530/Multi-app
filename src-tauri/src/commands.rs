//! IPC command surface and the pure state operations that back it.
//!
//! The `state_ops` submodule contains every mutation that ce-work
//! considers business logic — they take `&mut AppState` and return
//! either the affected DTO or an `OpError`. The `#[tauri::command]`
//! handlers below them are thin wrappers that:
//!   1. Lock the shared state.
//!   2. Call into a `state_ops` function.
//!   3. Drive side effects on the webview manager when relevant.
//!   4. Schedule a debounced persist on success.
//!
//! This split keeps the load-bearing logic 100% unit-testable without
//! needing a Tauri runtime.

#![allow(dead_code)] // fork_instance, update_instance_url/title used by U5+U6.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State, Wry};
use thiserror::Error;
use uuid::Uuid;

use crate::AppContext;
use crate::model::{self, AppState, Instance, InstanceTreeNode, Website, project_tree};
use crate::nav_guard;
use crate::webview_manager::ViewportRect;

// ---- Error & result shape exposed to JS ----

#[derive(Debug, Error)]
pub enum OpError {
    #[error("invalid url: {0}")]
    InvalidUrl(String),
    #[error("url is missing a host: {0}")]
    UrlMissingHost(String),
    #[error("url has no resolvable registered domain: {0}")]
    UrlNoRegisteredDomain(String),
    #[error("website not found: {0}")]
    WebsiteNotFound(Uuid),
    #[error("instance not found: {0}")]
    InstanceNotFound(Uuid),
    #[error("name cannot be empty")]
    EmptyName,
    #[error("internal: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for OpError {
    fn from(err: anyhow::Error) -> Self {
        OpError::Internal(format!("{err:#}"))
    }
}

impl serde::Serialize for OpError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type OpResult<T> = Result<T, OpError>;

// ---- Pure mutations (testable without Tauri runtime) ----

pub mod state_ops {
    use super::*;

    /// Normalize a user-supplied URL into (parsed_url, etld_plus_one, display_title).
    /// Adds `https://` when no scheme is present so users can paste raw hostnames.
    pub fn normalize_website_url(input: &str) -> OpResult<(url::Url, String, String)> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(OpError::InvalidUrl(input.into()));
        }
        let with_scheme = if trimmed.contains("://") {
            trimmed.to_string()
        } else {
            format!("https://{trimmed}")
        };
        let url =
            url::Url::parse(&with_scheme).map_err(|_| OpError::InvalidUrl(input.into()))?;
        let host = url
            .host_str()
            .ok_or_else(|| OpError::UrlMissingHost(input.into()))?
            .to_lowercase();
        let etld1 = nav_guard::registered_domain(&url)
            .ok_or_else(|| OpError::UrlNoRegisteredDomain(input.into()))?;
        Ok((url, etld1, host))
    }

    pub fn add_website(state: &mut AppState, input: &str, now_ms: i64) -> OpResult<Website> {
        let (url, etld1, host) = normalize_website_url(input)?;
        // Avoid silent duplicates: re-adding an existing eTLD+1 returns the
        // existing website rather than spawning a parallel one.
        if let Some(existing) = state
            .websites
            .iter()
            .find(|w| w.url_root.eq_ignore_ascii_case(&etld1))
        {
            return Ok(existing.clone());
        }
        let website = Website {
            id: Uuid::new_v4(),
            url_root: etld1,
            display_title: host,
            root_instance_ids: vec![],
            active_instance_id: None,
            created_at_ms: now_ms,
        };
        let _ = url;
        state.websites.push(website.clone());
        if state.active_website_id.is_none() {
            state.active_website_id = Some(website.id);
        }
        Ok(website)
    }

    /// Returns the list of (website_id, instance_id) removed so the caller
    /// can clean up webviews/data dirs.
    pub fn remove_website(state: &mut AppState, website_id: Uuid) -> OpResult<Vec<Uuid>> {
        let idx = state
            .websites
            .iter()
            .position(|w| w.id == website_id)
            .ok_or(OpError::WebsiteNotFound(website_id))?;
        let removed_instances: Vec<Uuid> = state
            .instances
            .values()
            .filter(|i| i.website_id == website_id)
            .map(|i| i.id)
            .collect();
        for id in &removed_instances {
            state.instances.remove(id);
        }
        state.websites.remove(idx);
        if state.active_website_id == Some(website_id) {
            state.active_website_id = state.websites.first().map(|w| w.id);
        }
        Ok(removed_instances)
    }

    pub fn add_instance(
        state: &mut AppState,
        website_id: Uuid,
        name: Option<String>,
        icon: Option<String>,
        now_ms: i64,
    ) -> OpResult<Instance> {
        let website = state
            .websites
            .iter_mut()
            .find(|w| w.id == website_id)
            .ok_or(OpError::WebsiteNotFound(website_id))?;
        let starting_url = if website.url_root.starts_with("http") {
            website.url_root.clone()
        } else {
            format!("https://{}", website.url_root)
        };
        let trimmed_name = name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let trimmed_icon = icon
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let instance = Instance {
            id: Uuid::new_v4(),
            website_id,
            parent_instance_id: None,
            user_name: trimmed_name,
            page_title: None,
            current_url: starting_url,
            created_at_ms: now_ms,
            icon: trimmed_icon,
        };
        website.root_instance_ids.push(instance.id);
        if website.active_instance_id.is_none() {
            website.active_instance_id = Some(instance.id);
        }
        state.instances.insert(instance.id, instance.clone());
        Ok(instance)
    }

    pub fn set_instance_icon(
        state: &mut AppState,
        id: Uuid,
        icon: Option<String>,
    ) -> OpResult<Instance> {
        let instance = state
            .instances
            .get_mut(&id)
            .ok_or(OpError::InstanceNotFound(id))?;
        instance.icon = icon
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        Ok(instance.clone())
    }

    /// Spawn a child instance under `parent` for a cross-domain URL.
    /// The new instance shares the parent's website_id; it appears in the
    /// sidebar tree as a sub-node of the parent.
    pub fn fork_instance(
        state: &mut AppState,
        parent_id: Uuid,
        target_url: &str,
        now_ms: i64,
    ) -> OpResult<Instance> {
        // Defense in depth: never persist a fork to a URL that has no
        // registered domain. The navigation handler already filters
        // these out via nav_guard::should_fork, but if anything else
        // ever calls fork_instance directly we must not pollute state.
        let parsed = url::Url::parse(target_url)
            .map_err(|_| OpError::InvalidUrl(target_url.into()))?;
        if crate::nav_guard::registered_domain(&parsed).is_none() {
            return Err(OpError::UrlNoRegisteredDomain(target_url.into()));
        }
        let _ = parsed;
        let parent = state
            .instances
            .get(&parent_id)
            .ok_or(OpError::InstanceNotFound(parent_id))?
            .clone();
        let instance = Instance {
            id: Uuid::new_v4(),
            website_id: parent.website_id,
            parent_instance_id: Some(parent_id),
            user_name: None,
            page_title: None,
            current_url: target_url.to_string(),
            created_at_ms: now_ms,
            icon: None,
        };
        state.instances.insert(instance.id, instance.clone());
        Ok(instance)
    }

    pub fn rename_instance(
        state: &mut AppState,
        id: Uuid,
        name: &str,
    ) -> OpResult<Instance> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(OpError::EmptyName);
        }
        let instance = state
            .instances
            .get_mut(&id)
            .ok_or(OpError::InstanceNotFound(id))?;
        instance.user_name = Some(trimmed.to_string());
        Ok(instance.clone())
    }

    /// Returns the full set of removed instance UUIDs (the target plus all
    /// descendants) so the caller can clean up webviews + data dirs.
    pub fn remove_instance(state: &mut AppState, id: Uuid) -> OpResult<Vec<Uuid>> {
        if !state.instances.contains_key(&id) {
            return Err(OpError::InstanceNotFound(id));
        }
        // Build the descendant set.
        let mut to_remove = vec![id];
        let mut i = 0;
        while i < to_remove.len() {
            let parent = to_remove[i];
            let children: Vec<Uuid> = state
                .instances
                .values()
                .filter(|inst| inst.parent_instance_id == Some(parent))
                .map(|inst| inst.id)
                .collect();
            to_remove.extend(children);
            i += 1;
        }
        let website_id = state.instances.get(&id).map(|i| i.website_id);
        for r in &to_remove {
            state.instances.remove(r);
        }
        if let Some(wid) = website_id {
            if let Some(website) = state.websites.iter_mut().find(|w| w.id == wid) {
                website.root_instance_ids.retain(|i| !to_remove.contains(i));
                if let Some(active) = website.active_instance_id {
                    if to_remove.contains(&active) {
                        website.active_instance_id = website.root_instance_ids.first().copied();
                    }
                }
            }
        }
        Ok(to_remove)
    }

    pub fn set_active_website(state: &mut AppState, id: Uuid) -> OpResult<()> {
        if !state.websites.iter().any(|w| w.id == id) {
            return Err(OpError::WebsiteNotFound(id));
        }
        state.active_website_id = Some(id);
        Ok(())
    }

    pub fn set_active_instance(state: &mut AppState, id: Uuid) -> OpResult<Uuid> {
        let instance = state
            .instances
            .get(&id)
            .ok_or(OpError::InstanceNotFound(id))?;
        let website_id = instance.website_id;
        if let Some(website) = state.websites.iter_mut().find(|w| w.id == website_id) {
            website.active_instance_id = Some(id);
        }
        state.active_website_id = Some(website_id);
        Ok(website_id)
    }

    pub fn update_instance_url(state: &mut AppState, id: Uuid, url: &str) {
        if let Some(inst) = state.instances.get_mut(&id) {
            inst.current_url = url.to_string();
        }
    }

    pub fn update_instance_title(state: &mut AppState, id: Uuid, title: &str) {
        if let Some(inst) = state.instances.get_mut(&id) {
            inst.page_title = Some(title.to_string());
        }
    }

    /// Drop dangling references that an externally-edited or partially-
    /// truncated `store.json` could leave behind. Called once at startup,
    /// after load_initial_state. Returns the number of references cleaned.
    pub fn sanitize(state: &mut AppState) -> usize {
        let mut fixed = 0;
        // Drop instances persisted by an earlier buggy nav handler with
        // junk URLs (about:blank, data:, etc. — no registered domain).
        // Those can never be spawned and just clutter the sidebar.
        let bad_url_instances: Vec<Uuid> = state
            .instances
            .values()
            .filter(|i| {
                url::Url::parse(&i.current_url)
                    .ok()
                    .and_then(|u| crate::nav_guard::registered_domain(&u))
                    .is_none()
            })
            .map(|i| i.id)
            .collect();
        for id in bad_url_instances {
            state.instances.remove(&id);
            fixed += 1;
        }
        // Clear active_instance_id refs that point to missing instances.
        for website in &mut state.websites {
            if let Some(active) = website.active_instance_id {
                if !state.instances.contains_key(&active) {
                    website.active_instance_id = None;
                    fixed += 1;
                }
            }
            // Prune root_instance_ids of any UUID whose instance is gone.
            let original = website.root_instance_ids.len();
            website
                .root_instance_ids
                .retain(|id| state.instances.contains_key(id));
            fixed += original - website.root_instance_ids.len();
            // If we cleared the active instance, fall back to first root.
            if website.active_instance_id.is_none() {
                website.active_instance_id = website.root_instance_ids.first().copied();
            }
        }
        // Drop instances whose parent_instance_id or website_id is gone.
        let website_ids: std::collections::HashSet<Uuid> =
            state.websites.iter().map(|w| w.id).collect();
        let to_drop: Vec<Uuid> = state
            .instances
            .values()
            .filter(|i| !website_ids.contains(&i.website_id))
            .map(|i| i.id)
            .collect();
        for id in to_drop {
            state.instances.remove(&id);
            fixed += 1;
        }
        // active_website_id may point to a gone website.
        if let Some(active) = state.active_website_id {
            if !state.websites.iter().any(|w| w.id == active) {
                state.active_website_id = state.websites.first().map(|w| w.id);
                fixed += 1;
            }
        }
        fixed
    }
}

// ---- DTOs for command return types ----

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceAddedEvent {
    pub instance: Instance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstancesRemovedEvent {
    pub instance_ids: Vec<Uuid>,
}

// ---- #[tauri::command] handlers ----

#[tauri::command]
pub fn list_websites(ctx: State<'_, AppContext>) -> Vec<Website> {
    ctx.state.read().websites.clone()
}

#[tauri::command]
pub fn add_website(ctx: State<'_, AppContext>, url: String) -> OpResult<Website> {
    let result = {
        let mut state = ctx.state.write();
        state_ops::add_website(&mut state, &url, model::now_ms())
    };
    if result.is_ok() {
        ctx.persister.schedule();
    }
    result
}

#[tauri::command]
pub fn remove_website(
    ctx: State<'_, AppContext>,
    app: AppHandle<Wry>,
    id: Uuid,
) -> OpResult<()> {
    let removed = {
        let mut state = ctx.state.write();
        state_ops::remove_website(&mut state, id)?
    };
    for inst_id in &removed {
        let _ = ctx.webviews.destroy(&app, *inst_id);
    }
    cleanup_data_dirs(&app, &removed);
    ctx.persister.schedule();
    Ok(())
}

#[tauri::command]
pub fn list_instance_tree(
    ctx: State<'_, AppContext>,
    website_id: Uuid,
) -> Vec<InstanceTreeNode> {
    let state = ctx.state.read();
    project_tree(&state, website_id)
}

// NOTE: This command (and the other webview-spawning commands below) MUST be
// `async`. On Windows, synchronous commands execute inline on the UI thread
// inside WebView2's web-message-received (IPC) event handler. Creating a child
// webview there runs Wry's nested message pump (`wait_with_pump`) re-entrantly
// inside that handler, which WebView2 forbids — the call deadlocks (the env is
// created on disk but the completion callback can never be delivered). Making
// the command `async` runs its body on the async runtime, off the IPC handler,
// so the nested pump executes in the clean main-thread event loop. macOS has no
// nested pump, so the sync version happened to work there.
#[tauri::command]
pub async fn add_instance(
    ctx: State<'_, AppContext>,
    app: AppHandle<Wry>,
    website_id: Uuid,
    name: Option<String>,
    icon: Option<String>,
) -> OpResult<Instance> {
    // Create the instance AND make it the active one — matches the
    // expectation that a newly-added tab focuses itself.
    let new_instance = {
        let mut state = ctx.state.write();
        let inst = state_ops::add_instance(
            &mut state,
            website_id,
            name,
            icon,
            model::now_ms(),
        )?;
        let _ = state_ops::set_active_instance(&mut state, inst.id);
        inst
    };

    // Spawn the new instance's webview and show it. ensure_spawned is
    // idempotent so this is safe even if something else got there first.
    ctx.webviews
        .ensure_spawned(&app, &new_instance)
        .map_err(OpError::from)?;
    ctx.webviews
        .activate(&app, new_instance.id)
        .map_err(OpError::from)?;

    ctx.persister.schedule();
    Ok(new_instance)
}

#[tauri::command]
pub fn set_instance_icon(
    ctx: State<'_, AppContext>,
    id: Uuid,
    icon: Option<String>,
) -> OpResult<Instance> {
    let result = {
        let mut state = ctx.state.write();
        state_ops::set_instance_icon(&mut state, id, icon)
    };
    if result.is_ok() {
        ctx.persister.schedule();
    }
    result
}

#[tauri::command]
pub fn rename_instance(
    ctx: State<'_, AppContext>,
    id: Uuid,
    name: String,
) -> OpResult<Instance> {
    let result = {
        let mut state = ctx.state.write();
        state_ops::rename_instance(&mut state, id, &name)
    };
    if result.is_ok() {
        ctx.persister.schedule();
    }
    result
}

#[tauri::command]
pub fn remove_instance(
    ctx: State<'_, AppContext>,
    app: AppHandle<Wry>,
    id: Uuid,
) -> OpResult<Vec<Uuid>> {
    let removed = {
        let mut state = ctx.state.write();
        state_ops::remove_instance(&mut state, id)?
    };
    for inst_id in &removed {
        let _ = ctx.webviews.destroy(&app, *inst_id);
    }
    cleanup_data_dirs(&app, &removed);
    ctx.persister.schedule();
    Ok(removed)
}

// async: see the note on `add_instance` — this spawns a webview on Windows.
#[tauri::command]
pub async fn activate_website(
    ctx: State<'_, AppContext>,
    app: AppHandle<Wry>,
    id: Uuid,
) -> OpResult<()> {
    // Update state, then look up which instance (if any) was last
    // active on this website so we can swap the visible webview to
    // match — that's the user-facing expectation when switching tabs.
    let target = {
        let mut state = ctx.state.write();
        state_ops::set_active_website(&mut state, id)?;
        let active_iid = state
            .websites
            .iter()
            .find(|w| w.id == id)
            .and_then(|w| w.active_instance_id);
        active_iid.and_then(|iid| state.instances.get(&iid).cloned())
    };

    match target {
        Some(instance) => {
            ctx.webviews
                .ensure_spawned(&app, &instance)
                .map_err(OpError::from)?;
            ctx.webviews
                .activate(&app, instance.id)
                .map_err(OpError::from)?;
        }
        None => {
            // No instance on this website yet — hide whatever was visible
            // so the user sees the empty-state placeholder, not the
            // previous site's content.
            ctx.webviews.deactivate(&app);
        }
    }

    ctx.persister.schedule();
    Ok(())
}

// async: see the note on `add_instance` — this spawns a webview on Windows.
#[tauri::command]
pub async fn activate_instance(
    ctx: State<'_, AppContext>,
    app: AppHandle<Wry>,
    id: Uuid,
) -> OpResult<()> {
    let snapshot = {
        let mut state = ctx.state.write();
        state_ops::set_active_instance(&mut state, id)?;
        state.instances.get(&id).cloned()
    };
    if let Some(instance) = snapshot {
        ctx.webviews
            .ensure_spawned(&app, &instance)
            .map_err(OpError::from)?;
        ctx.webviews.activate(&app, id).map_err(OpError::from)?;
    }
    ctx.persister.schedule();
    Ok(())
}

#[tauri::command]
pub fn set_viewport_bounds(
    ctx: State<'_, AppContext>,
    app: AppHandle<Wry>,
    rect: ViewportRect,
) -> OpResult<()> {
    ctx.webviews
        .set_viewport(&app, rect)
        .map_err(OpError::from)
}

/// Show or hide the currently-active webview without changing the
/// active selection. Used by blocking React surfaces (confirm modal,
/// future popovers) that must visually sit above the active site —
/// Tauri's child webviews are native OS surfaces stacked above every
/// React layer, so the only way to ensure a React modal is visible is
/// to take the webview out of view while the modal is open.
#[tauri::command]
pub fn set_active_webview_visibility(
    ctx: State<'_, AppContext>,
    app: AppHandle<Wry>,
    visible: bool,
) -> OpResult<()> {
    if let Some(id) = ctx.webviews.active() {
        let label = crate::webview_manager::WebviewManager::label_for(&id);
        if let Some(wv) = app.get_webview(&label) {
            let _ = if visible { wv.show() } else { wv.hide() };
        }
    }
    Ok(())
}

// ---- Per-instance navigation controls ----
//
// Tauri 2 exposes `Webview::reload()` natively but not history.back /
// history.forward — we drive those via webview.eval() with the
// standard JS calls. Errors are swallowed; the buttons are
// best-effort and shouldn't bubble UI exceptions for a failed back.

#[tauri::command]
pub fn instance_back(
    _ctx: State<'_, AppContext>,
    app: AppHandle<Wry>,
    id: Uuid,
) -> OpResult<()> {
    let label = crate::webview_manager::WebviewManager::label_for(&id);
    if let Some(wv) = app.get_webview(&label) {
        let _ = wv.eval("history.back()");
    }
    Ok(())
}

#[tauri::command]
pub fn instance_forward(
    _ctx: State<'_, AppContext>,
    app: AppHandle<Wry>,
    id: Uuid,
) -> OpResult<()> {
    let label = crate::webview_manager::WebviewManager::label_for(&id);
    if let Some(wv) = app.get_webview(&label) {
        let _ = wv.eval("history.forward()");
    }
    Ok(())
}

#[tauri::command]
pub fn instance_reload(
    _ctx: State<'_, AppContext>,
    app: AppHandle<Wry>,
    id: Uuid,
) -> OpResult<()> {
    let label = crate::webview_manager::WebviewManager::label_for(&id);
    if let Some(wv) = app.get_webview(&label) {
        let _ = wv.reload();
    }
    Ok(())
}

// ---- Data-dir cleanup ----

fn cleanup_data_dirs(_app: &AppHandle<Wry>, _instance_ids: &[Uuid]) {
    // On Windows/Linux, the per-instance data directory under
    // $APP_DATA/webviews/<uuid>/ is owned by us and safe to remove.
    // On macOS, the data store is owned by WebKit at
    // ~/Library/WebKit/WebsiteDataStore/<uuid>/; cleanup is delegated to
    // the OS via WKWebsiteDataStore.remove_data — added in U8 when we
    // wire packaging + the smoke matrix. Leaving the disk cleanup as a
    // TODO here is safe: cookies stay isolated either way because the
    // data store is keyed by instance UUID, which never gets reused.
    #[cfg(not(target_os = "macos"))]
    for id in _instance_ids {
        if let Ok(dir) = crate::paths::webview_data_dir_path(_app, id) {
            schedule_dir_removal(dir);
        }
    }
}

/// Remove a per-instance webview data dir off-thread, retrying with backoff.
///
/// `Webview::close()` is asynchronous, and on Windows the WebView2 browser
/// process for the closed instance keeps file handles open on its user-data
/// folder for a short window after close. An immediate `remove_dir_all` then
/// fails with a sharing violation, which is why a naive best-effort delete
/// used to leave `webviews/<uuid>/` orphans behind. Retrying until the
/// process exits (or a cap is hit) makes deletion reliable. Runs on its own
/// thread so the blocking sleeps never touch the UI thread or async runtime.
#[cfg(not(target_os = "macos"))]
fn schedule_dir_removal(dir: std::path::PathBuf) {
    if !dir.exists() {
        return;
    }
    std::thread::spawn(move || {
        remove_dir_with_retries(&dir);
    });
}

#[cfg(not(target_os = "macos"))]
fn remove_dir_with_retries(dir: &std::path::Path) -> bool {
    // Cumulative wait of ~3.85s across attempts — comfortably longer than the
    // WebView2 browser process takes to exit and release its handles.
    const DELAYS_MS: [u64; 7] = [0, 100, 250, 500, 1000, 1000, 1000];
    for (attempt, delay) in DELAYS_MS.iter().enumerate() {
        if *delay > 0 {
            std::thread::sleep(std::time::Duration::from_millis(*delay));
        }
        if !dir.exists() {
            return true;
        }
        match std::fs::remove_dir_all(dir) {
            Ok(()) => return true,
            Err(err) => tracing::debug!(
                attempt,
                dir = %dir.display(),
                error = %err,
                "webview data dir removal failed; retrying"
            ),
        }
    }
    let still_present = dir.exists();
    if still_present {
        tracing::warn!(
            dir = %dir.display(),
            "gave up removing webview data dir after retries (harmless orphan; \
             UUID is never reused so isolation is unaffected)"
        );
    }
    !still_present
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_state() -> AppState {
        AppState::default()
    }

    #[test]
    fn add_website_canonicalizes_to_etld_plus_one() {
        let mut state = empty_state();
        let w = state_ops::add_website(&mut state, "https://mail.google.com/inbox/u/1", 100)
            .unwrap();
        assert_eq!(w.url_root, "google.com");
        assert_eq!(w.display_title, "mail.google.com");
        assert_eq!(state.active_website_id, Some(w.id));
    }

    #[test]
    fn add_website_accepts_raw_hostname_without_scheme() {
        let mut state = empty_state();
        let w = state_ops::add_website(&mut state, "google.com", 100).unwrap();
        assert_eq!(w.url_root, "google.com");
    }

    #[test]
    fn add_website_deduplicates_by_etld_plus_one() {
        let mut state = empty_state();
        let w1 = state_ops::add_website(&mut state, "https://google.com", 100).unwrap();
        let w2 =
            state_ops::add_website(&mut state, "https://mail.google.com/", 200).unwrap();
        assert_eq!(w1.id, w2.id);
        assert_eq!(state.websites.len(), 1);
    }

    #[test]
    fn add_website_rejects_garbage_input() {
        let mut state = empty_state();
        let err =
            state_ops::add_website(&mut state, "not a url at all !!!", 100).unwrap_err();
        match err {
            OpError::InvalidUrl(_) | OpError::UrlNoRegisteredDomain(_) => {}
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn add_instance_first_call_becomes_root_and_active() {
        let mut state = empty_state();
        let w = state_ops::add_website(&mut state, "google.com", 100).unwrap();
        let inst =
            state_ops::add_instance(&mut state, w.id, Some("Personal".into()), None, 200).unwrap();
        assert_eq!(inst.parent_instance_id, None);
        assert_eq!(inst.user_name.as_deref(), Some("Personal"));
        let w_after = &state.websites[0];
        assert_eq!(w_after.root_instance_ids, vec![inst.id]);
        assert_eq!(w_after.active_instance_id, Some(inst.id));
    }

    #[test]
    fn add_instance_for_missing_website_errors() {
        let mut state = empty_state();
        let err = state_ops::add_instance(&mut state, Uuid::nil(), None, None, 100).unwrap_err();
        assert!(matches!(err, OpError::WebsiteNotFound(_)));
    }

    #[test]
    fn rename_to_empty_string_errors_and_preserves_existing_name() {
        let mut state = empty_state();
        let w = state_ops::add_website(&mut state, "google.com", 0).unwrap();
        let i =
            state_ops::add_instance(&mut state, w.id, Some("original".into()), None, 0).unwrap();

        let err = state_ops::rename_instance(&mut state, i.id, "   ").unwrap_err();
        assert!(matches!(err, OpError::EmptyName));

        let still = state.instances.get(&i.id).unwrap();
        assert_eq!(still.user_name.as_deref(), Some("original"));
    }

    #[test]
    fn rename_trims_whitespace() {
        let mut state = empty_state();
        let w = state_ops::add_website(&mut state, "google.com", 0).unwrap();
        let i = state_ops::add_instance(&mut state, w.id, None, None, 0).unwrap();
        let renamed = state_ops::rename_instance(&mut state, i.id, "  Work  ").unwrap();
        assert_eq!(renamed.user_name.as_deref(), Some("Work"));
    }

    #[test]
    fn fork_creates_child_instance_under_parent() {
        let mut state = empty_state();
        let w = state_ops::add_website(&mut state, "google.com", 0).unwrap();
        let parent = state_ops::add_instance(&mut state, w.id, None, None, 100).unwrap();
        let child = state_ops::fork_instance(
            &mut state,
            parent.id,
            "https://stripe.com/checkout",
            200,
        )
        .unwrap();
        assert_eq!(child.parent_instance_id, Some(parent.id));
        assert_eq!(child.website_id, w.id);
        // Children do NOT appear in root_instance_ids — they're discovered
        // via project_tree's parent-pointer walk.
        let w_after = &state.websites[0];
        assert_eq!(w_after.root_instance_ids, vec![parent.id]);
    }

    #[test]
    fn remove_instance_cascades_through_descendants() {
        let mut state = empty_state();
        let w = state_ops::add_website(&mut state, "google.com", 0).unwrap();
        let root = state_ops::add_instance(&mut state, w.id, None, None, 1).unwrap();
        let child = state_ops::fork_instance(&mut state, root.id, "https://stripe.com", 2)
            .unwrap();
        let grand =
            state_ops::fork_instance(&mut state, child.id, "https://docs.stripe.com", 3)
                .unwrap();
        let _sibling_under_root =
            state_ops::add_instance(&mut state, w.id, Some("Other".into()), None, 4).unwrap();

        let removed = state_ops::remove_instance(&mut state, root.id).unwrap();
        assert_eq!(removed.len(), 3);
        for r in &[root.id, child.id, grand.id] {
            assert!(removed.contains(r));
            assert!(!state.instances.contains_key(r));
        }
        let w_after = &state.websites[0];
        // Sibling under root remains; removed root is gone from root_instance_ids.
        assert_eq!(w_after.root_instance_ids.len(), 1);
        assert_ne!(w_after.root_instance_ids[0], root.id);
    }

    #[test]
    fn remove_website_drops_all_its_instances_including_forks() {
        let mut state = empty_state();
        let wa = state_ops::add_website(&mut state, "google.com", 0).unwrap();
        let wb = state_ops::add_website(&mut state, "github.com", 0).unwrap();
        let ia = state_ops::add_instance(&mut state, wa.id, None, None, 0).unwrap();
        let ia_fork =
            state_ops::fork_instance(&mut state, ia.id, "https://stripe.com", 0).unwrap();
        let ib = state_ops::add_instance(&mut state, wb.id, None, None, 0).unwrap();

        let removed = state_ops::remove_website(&mut state, wa.id).unwrap();
        assert!(removed.contains(&ia.id));
        assert!(removed.contains(&ia_fork.id));
        assert!(!removed.contains(&ib.id));
        assert_eq!(state.websites.len(), 1);
        assert_eq!(state.websites[0].id, wb.id);
        assert!(state.instances.contains_key(&ib.id));
        assert!(!state.instances.contains_key(&ia.id));
        assert!(!state.instances.contains_key(&ia_fork.id));
        // active_website_id moves to the remaining website.
        assert_eq!(state.active_website_id, Some(wb.id));
    }

    #[test]
    fn set_active_instance_also_sets_active_website() {
        let mut state = empty_state();
        let wa = state_ops::add_website(&mut state, "google.com", 0).unwrap();
        let wb = state_ops::add_website(&mut state, "github.com", 0).unwrap();
        let ib = state_ops::add_instance(&mut state, wb.id, None, None, 0).unwrap();
        // start with wa active
        state.active_website_id = Some(wa.id);

        state_ops::set_active_instance(&mut state, ib.id).unwrap();
        assert_eq!(state.active_website_id, Some(wb.id));
        let w_after = state.websites.iter().find(|w| w.id == wb.id).unwrap();
        assert_eq!(w_after.active_instance_id, Some(ib.id));
    }

    #[test]
    fn sanitize_clears_dangling_active_instance() {
        let mut state = empty_state();
        let w = state_ops::add_website(&mut state, "google.com", 0).unwrap();
        let i = state_ops::add_instance(&mut state, w.id, None, None, 0).unwrap();
        // Tamper: simulate the persisted file losing the instance.
        state.instances.remove(&i.id);

        let fixed = state_ops::sanitize(&mut state);
        assert!(fixed >= 1);
        let w_after = &state.websites[0];
        assert!(w_after.root_instance_ids.is_empty());
        assert_eq!(w_after.active_instance_id, None);
    }

    #[test]
    fn sanitize_drops_orphan_instances_whose_website_is_gone() {
        let mut state = empty_state();
        let wa = state_ops::add_website(&mut state, "google.com", 0).unwrap();
        let _ia = state_ops::add_instance(&mut state, wa.id, None, None, 0).unwrap();
        // Tamper: drop the website but leave the instance.
        let orphans: Vec<Uuid> = state
            .instances
            .values()
            .filter(|i| i.website_id == wa.id)
            .map(|i| i.id)
            .collect();
        state.websites.retain(|w| w.id != wa.id);

        state_ops::sanitize(&mut state);
        for o in &orphans {
            assert!(!state.instances.contains_key(o));
        }
    }

    #[test]
    fn sanitize_falls_back_active_website_to_first_remaining() {
        let mut state = empty_state();
        let wa = state_ops::add_website(&mut state, "google.com", 0).unwrap();
        let wb = state_ops::add_website(&mut state, "github.com", 0).unwrap();
        state.active_website_id = Some(wa.id);
        // Tamper: drop wa.
        state.websites.retain(|w| w.id != wa.id);
        state_ops::sanitize(&mut state);
        assert_eq!(state.active_website_id, Some(wb.id));
    }

    #[test]
    fn sanitize_is_idempotent_on_clean_state() {
        let mut state = empty_state();
        let w = state_ops::add_website(&mut state, "google.com", 0).unwrap();
        let _i = state_ops::add_instance(&mut state, w.id, None, None, 0).unwrap();
        assert_eq!(state_ops::sanitize(&mut state), 0);
        assert_eq!(state_ops::sanitize(&mut state), 0);
    }

    #[test]
    fn set_active_instance_unknown_errors() {
        let mut state = empty_state();
        let err = state_ops::set_active_instance(&mut state, Uuid::nil()).unwrap_err();
        assert!(matches!(err, OpError::InstanceNotFound(_)));
    }

    #[cfg(not(target_os = "macos"))]
    fn unique_temp_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("multiapp-test-{}", Uuid::new_v4()))
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn remove_dir_with_retries_deletes_a_populated_dir() {
        let dir = unique_temp_dir();
        std::fs::create_dir_all(dir.join("EBWebView/Default")).unwrap();
        std::fs::write(dir.join("EBWebView/Local State"), b"{}").unwrap();
        assert!(dir.exists());

        assert!(remove_dir_with_retries(&dir));
        assert!(!dir.exists());
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn remove_dir_with_retries_is_ok_when_already_gone() {
        let dir = unique_temp_dir();
        assert!(!dir.exists());
        // Should report success immediately without erroring.
        assert!(remove_dir_with_retries(&dir));
    }

    // On Windows an open file handle blocks deletion (no FILE_SHARE_DELETE),
    // which is exactly the WebView2-process-still-holding-the-folder race.
    // Verify the retry loop waits out the lock and then succeeds. On Linux an
    // open handle does NOT block unlink, so this race only exists on Windows.
    #[cfg(target_os = "windows")]
    #[test]
    fn remove_dir_with_retries_waits_out_a_file_lock() {
        use std::io::Write;

        let dir = unique_temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let mut locked = std::fs::File::create(dir.join("locked.bin")).unwrap();
        locked.write_all(b"hold").unwrap();

        // Release the handle partway through the retry budget.
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(400));
            drop(locked);
        });

        let removed = remove_dir_with_retries(&dir);
        releaser.join().unwrap();

        assert!(removed, "retry loop should remove the dir after the lock releases");
        assert!(!dir.exists());
    }
}
