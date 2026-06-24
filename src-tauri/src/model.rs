// Public model types and helpers. Several items are referenced from
// later units (commands, webview manager) and are intentionally allowed
// to appear unused until those units land.
#![allow(dead_code)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct AppState {
    pub websites: Vec<Website>,
    pub instances: HashMap<Uuid, Instance>,
    pub active_website_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Website {
    pub id: Uuid,
    pub url_root: String,
    pub display_title: String,
    pub root_instance_ids: Vec<Uuid>,
    pub active_instance_id: Option<Uuid>,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Instance {
    pub id: Uuid,
    pub website_id: Uuid,
    pub parent_instance_id: Option<Uuid>,
    pub user_name: Option<String>,
    pub page_title: Option<String>,
    pub current_url: String,
    pub created_at_ms: i64,
    /// Optional emoji/short string displayed before the name in the
    /// sidebar. Defaulted to None so older persisted Instances without
    /// this field still deserialize cleanly.
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoreSchema {
    pub version: u32,
    pub app: AppState,
}

impl StoreSchema {
    pub fn current(app: AppState) -> Self {
        Self {
            version: SCHEMA_VERSION,
            app,
        }
    }
}

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("incompatible store schema version: found {found}, supported {supported}")]
    IncompatibleVersion { found: u32, supported: u32 },

    #[error("serde_json: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn serialize(state: &AppState) -> Result<String, ModelError> {
    let schema = StoreSchema::current(state.clone());
    Ok(serde_json::to_string_pretty(&schema)?)
}

pub fn deserialize(raw: &str) -> Result<AppState, ModelError> {
    let schema: StoreSchema = serde_json::from_str(raw)?;
    if schema.version != SCHEMA_VERSION {
        return Err(ModelError::IncompatibleVersion {
            found: schema.version,
            supported: SCHEMA_VERSION,
        });
    }
    Ok(schema.app)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstanceTreeNode {
    pub instance: Instance,
    pub children: Vec<InstanceTreeNode>,
}

pub fn project_tree(state: &AppState, website_id: Uuid) -> Vec<InstanceTreeNode> {
    let mut children_of: HashMap<Option<Uuid>, Vec<Uuid>> = HashMap::new();
    for inst in state.instances.values() {
        if inst.website_id != website_id {
            continue;
        }
        children_of
            .entry(inst.parent_instance_id)
            .or_default()
            .push(inst.id);
    }
    for ids in children_of.values_mut() {
        ids.sort_by_key(|id| state.instances.get(id).map(|i| i.created_at_ms));
    }
    let roots = state
        .websites
        .iter()
        .find(|w| w.id == website_id)
        .map(|w| w.root_instance_ids.clone())
        .unwrap_or_default();
    roots
        .into_iter()
        .filter_map(|id| build_node(id, state, &children_of))
        .collect()
}

fn build_node(
    id: Uuid,
    state: &AppState,
    children_of: &HashMap<Option<Uuid>, Vec<Uuid>>,
) -> Option<InstanceTreeNode> {
    let instance = state.instances.get(&id)?.clone();
    let children = children_of
        .get(&Some(id))
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|cid| build_node(cid, state, children_of))
        .collect();
    Some(InstanceTreeNode { instance, children })
}

#[allow(dead_code)]
pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_uuid(n: u8) -> Uuid {
        let mut bytes = [0u8; 16];
        bytes[0] = n;
        Uuid::from_bytes(bytes)
    }

    fn website(id_byte: u8, title: &str) -> Website {
        Website {
            id: fixed_uuid(id_byte),
            url_root: format!("https://{title}.test"),
            display_title: title.into(),
            root_instance_ids: vec![],
            active_instance_id: None,
            created_at_ms: 1_000_000,
        }
    }

    fn instance(id_byte: u8, website_id: Uuid, parent: Option<Uuid>, created_ms: i64) -> Instance {
        Instance {
            id: fixed_uuid(id_byte),
            website_id,
            parent_instance_id: parent,
            user_name: None,
            page_title: None,
            current_url: "https://example.test".into(),
            created_at_ms: created_ms,
            icon: None,
        }
    }

    #[test]
    fn round_trip_default_state() {
        let state = AppState::default();
        let raw = serialize(&state).unwrap();
        let parsed = deserialize(&raw).unwrap();
        assert_eq!(state, parsed);
    }

    #[test]
    fn round_trip_complex_tree() {
        let mut state = AppState::default();

        let w_google = website(1, "google");
        let w_github = website(2, "github");

        let i_root_a = instance(10, w_google.id, None, 1);
        let i_root_b = instance(11, w_google.id, None, 2);
        let i_child_of_a_1 = instance(12, w_google.id, Some(i_root_a.id), 3);
        let i_child_of_a_2 = instance(13, w_google.id, Some(i_root_a.id), 4);
        let i_grandchild = instance(14, w_google.id, Some(i_child_of_a_1.id), 5);
        let i_gh_root = instance(20, w_github.id, None, 6);

        let mut w_google = w_google;
        w_google.root_instance_ids = vec![i_root_a.id, i_root_b.id];
        w_google.active_instance_id = Some(i_root_a.id);

        let mut w_github = w_github;
        w_github.root_instance_ids = vec![i_gh_root.id];

        state.websites = vec![w_google.clone(), w_github.clone()];
        state.active_website_id = Some(w_google.id);

        for inst in [
            &i_root_a,
            &i_root_b,
            &i_child_of_a_1,
            &i_child_of_a_2,
            &i_grandchild,
            &i_gh_root,
        ] {
            state.instances.insert(inst.id, inst.clone());
        }

        let raw = serialize(&state).unwrap();
        let parsed = deserialize(&raw).unwrap();
        assert_eq!(state, parsed);
    }

    #[test]
    fn rejects_incompatible_schema_version() {
        let blob = serde_json::json!({
            "version": 99,
            "app": AppState::default(),
        })
        .to_string();
        let err = deserialize(&blob).unwrap_err();
        match err {
            ModelError::IncompatibleVersion { found, supported } => {
                assert_eq!(found, 99);
                assert_eq!(supported, SCHEMA_VERSION);
            }
            other => panic!("expected IncompatibleVersion, got {other:?}"),
        }
    }

    #[test]
    fn project_tree_shapes_nested_children_in_chronological_order() {
        let mut state = AppState::default();

        let mut w = website(1, "google");
        let root = instance(10, w.id, None, 100);
        let child_old = instance(11, w.id, Some(root.id), 200);
        let child_new = instance(12, w.id, Some(root.id), 300);
        let grand = instance(13, w.id, Some(child_old.id), 400);

        w.root_instance_ids = vec![root.id];
        state.websites = vec![w.clone()];
        for inst in [&root, &child_old, &child_new, &grand] {
            state.instances.insert(inst.id, inst.clone());
        }

        let tree = project_tree(&state, w.id);
        assert_eq!(tree.len(), 1);
        let root_node = &tree[0];
        assert_eq!(root_node.instance.id, root.id);
        assert_eq!(root_node.children.len(), 2);

        assert_eq!(root_node.children[0].instance.id, child_old.id);
        assert_eq!(root_node.children[1].instance.id, child_new.id);
        assert_eq!(root_node.children[0].children.len(), 1);
        assert_eq!(root_node.children[0].children[0].instance.id, grand.id);
        assert!(root_node.children[1].children.is_empty());
    }

    #[test]
    fn project_tree_scopes_by_website() {
        let mut state = AppState::default();

        let mut w_a = website(1, "google");
        let mut w_b = website(2, "github");

        let a_root = instance(10, w_a.id, None, 1);
        let b_root = instance(20, w_b.id, None, 1);

        w_a.root_instance_ids = vec![a_root.id];
        w_b.root_instance_ids = vec![b_root.id];

        state.websites = vec![w_a.clone(), w_b.clone()];
        state.instances.insert(a_root.id, a_root.clone());
        state.instances.insert(b_root.id, b_root.clone());

        let tree_a = project_tree(&state, w_a.id);
        let tree_b = project_tree(&state, w_b.id);
        assert_eq!(tree_a.len(), 1);
        assert_eq!(tree_b.len(), 1);
        assert_eq!(tree_a[0].instance.id, a_root.id);
        assert_eq!(tree_b[0].instance.id, b_root.id);
    }

    #[test]
    fn project_tree_for_unknown_website_returns_empty() {
        let state = AppState::default();
        assert!(project_tree(&state, fixed_uuid(99)).is_empty());
    }
}
