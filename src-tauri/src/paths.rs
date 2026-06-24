use std::path::PathBuf;

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};

pub fn app_data_dir(app: &AppHandle) -> Result<PathBuf> {
    app.path()
        .app_data_dir()
        .context("failed to resolve app data directory")
}

pub fn metadata_dir(app: &AppHandle) -> Result<PathBuf> {
    let dir = app_data_dir(app)?.join("metadata");
    std::fs::create_dir_all(&dir).with_context(|| {
        format!("failed to create metadata directory at {}", dir.display())
    })?;
    Ok(dir)
}

pub fn webviews_dir(app: &AppHandle) -> Result<PathBuf> {
    let dir = app_data_dir(app)?.join("webviews");
    std::fs::create_dir_all(&dir).with_context(|| {
        format!("failed to create webviews directory at {}", dir.display())
    })?;
    Ok(dir)
}

#[allow(dead_code)]
pub fn webview_data_dir(app: &AppHandle, instance_id: &uuid::Uuid) -> Result<PathBuf> {
    let dir = webviews_dir(app)?.join(instance_id.to_string());
    std::fs::create_dir_all(&dir).with_context(|| {
        format!("failed to create webview data dir at {}", dir.display())
    })?;
    Ok(dir)
}
