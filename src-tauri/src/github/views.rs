use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewWindow};

/// A saved pull-request "view": a named repo + filter preset.
/// `filters` is stored opaquely as JSON so the backend never needs to track
/// the frontend's filter shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrView {
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub filters: serde_json::Value,
}

/// On-disk shape of `pr-views.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PrViewsStore {
    #[serde(default)]
    pub views: Vec<PrView>,
    #[serde(default)]
    pub active_view_id: Option<String>,
}

fn store_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("pr-views.json"))
}

fn load(path: &Path) -> Result<PrViewsStore, String> {
    if !path.exists() {
        return Ok(PrViewsStore::default());
    }
    let data = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    if data.trim().is_empty() {
        return Ok(PrViewsStore::default());
    }
    serde_json::from_str(&data).map_err(|e| e.to_string())
}

fn save(path: &Path, store: &PrViewsStore) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    std::fs::write(path, data).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pr_views_list(window: WebviewWindow, app: AppHandle) -> Result<PrViewsStore, String> {
    crate::security::require_main(&window)?;
    load(&store_path(&app)?)
}

/// Upsert a view. A blank `id` creates a new one; an existing `id` overwrites
/// that view (used for rename and update-to-current-filters). Returns the saved
/// view (with its assigned id).
#[tauri::command]
pub fn pr_views_save(
    window: WebviewWindow,
    app: AppHandle,
    mut view: PrView,
) -> Result<PrView, String> {
    crate::security::require_main(&window)?;
    let path = store_path(&app)?;
    let mut store = load(&path)?;
    if view.id.is_empty() {
        view.id = format!("view:{}", uuid::Uuid::new_v4());
    }
    match store.views.iter_mut().find(|v| v.id == view.id) {
        Some(existing) => *existing = view.clone(),
        None => store.views.push(view.clone()),
    }
    save(&path, &store)?;
    Ok(view)
}

#[tauri::command]
pub fn pr_views_delete(window: WebviewWindow, app: AppHandle, id: String) -> Result<(), String> {
    crate::security::require_main(&window)?;
    let path = store_path(&app)?;
    let mut store = load(&path)?;
    store.views.retain(|v| v.id != id);
    if store.active_view_id.as_deref() == Some(id.as_str()) {
        store.active_view_id = None;
    }
    save(&path, &store)
}

#[tauri::command]
pub fn pr_views_set_active(
    window: WebviewWindow,
    app: AppHandle,
    id: Option<String>,
) -> Result<(), String> {
    crate::security::require_main(&window)?;
    let path = store_path(&app)?;
    let mut store = load(&path)?;
    store.active_view_id = id.filter(|s| !s.is_empty());
    save(&path, &store)
}
