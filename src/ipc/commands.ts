// Typed wrappers over Tauri's `invoke()`. Each function corresponds 1:1
// with a #[tauri::command] in src-tauri/src/commands.rs.

import { invoke } from "@tauri-apps/api/core";
import type {
  Instance,
  InstanceTreeNode,
  Uuid,
  ViewportRect,
  Website,
} from "../types";

export function listWebsites(): Promise<Website[]> {
  return invoke("list_websites");
}

export function addWebsite(url: string): Promise<Website> {
  return invoke("add_website", { url });
}

export function removeWebsite(id: Uuid): Promise<void> {
  return invoke("remove_website", { id });
}

export function listInstanceTree(websiteId: Uuid): Promise<InstanceTreeNode[]> {
  return invoke("list_instance_tree", { websiteId });
}

export function addInstance(
  websiteId: Uuid,
  name?: string,
  icon?: string,
): Promise<Instance> {
  return invoke("add_instance", {
    websiteId,
    name: name ?? null,
    icon: icon ?? null,
  });
}

export function renameInstance(id: Uuid, name: string): Promise<Instance> {
  return invoke("rename_instance", { id, name });
}

export function setInstanceIcon(
  id: Uuid,
  icon: string | null,
): Promise<Instance> {
  return invoke("set_instance_icon", { id, icon });
}

export function removeInstance(id: Uuid): Promise<Uuid[]> {
  return invoke("remove_instance", { id });
}

export function activateWebsite(id: Uuid): Promise<void> {
  return invoke("activate_website", { id });
}

export function activateInstance(id: Uuid): Promise<void> {
  return invoke("activate_instance", { id });
}

export function setViewportBounds(rect: ViewportRect): Promise<void> {
  return invoke("set_viewport_bounds", { rect });
}

export function setActiveWebviewVisibility(visible: boolean): Promise<void> {
  return invoke("set_active_webview_visibility", { visible });
}

export function instanceBack(id: Uuid): Promise<void> {
  return invoke("instance_back", { id });
}

export function instanceForward(id: Uuid): Promise<void> {
  return invoke("instance_forward", { id });
}

export function instanceReload(id: Uuid): Promise<void> {
  return invoke("instance_reload", { id });
}
