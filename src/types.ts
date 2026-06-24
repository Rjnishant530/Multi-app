// Mirrors src-tauri/src/{model.rs,webview_manager.rs}. Keep field
// names and shapes in sync — Tauri serde-serializes structs as JSON
// objects with snake_case field names by default, so we use snake_case
// here too.

export type Uuid = string;

export interface Website {
  id: Uuid;
  url_root: string;
  display_title: string;
  root_instance_ids: Uuid[];
  active_instance_id: Uuid | null;
  created_at_ms: number;
}

export interface Instance {
  id: Uuid;
  website_id: Uuid;
  parent_instance_id: Uuid | null;
  user_name: string | null;
  page_title: string | null;
  current_url: string;
  created_at_ms: number;
  icon: string | null;
}

export interface InstanceTreeNode {
  instance: Instance;
  children: InstanceTreeNode[];
}

export interface ViewportRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface InstanceAddedEvent {
  instance: Instance;
}

export interface InstancesRemovedEvent {
  instance_ids: Uuid[];
}
