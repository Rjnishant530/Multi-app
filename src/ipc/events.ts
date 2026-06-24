// Typed event subscriptions. Each event name matches the string Rust
// emits via `app.emit("name", payload)` from the backend.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Instance,
  InstanceAddedEvent,
  InstancesRemovedEvent,
} from "../types";

export const Events = {
  InstanceAdded: "instance:added",
  InstanceRemoved: "instance:removed",
  InstanceUrlChanged: "instance:url-changed",
  InstanceTitleChanged: "instance:title-changed",
} as const;

export function onInstanceAdded(
  handler: (payload: InstanceAddedEvent) => void,
): Promise<UnlistenFn> {
  return listen<InstanceAddedEvent>(Events.InstanceAdded, (e) =>
    handler(e.payload),
  );
}

export function onInstancesRemoved(
  handler: (payload: InstancesRemovedEvent) => void,
): Promise<UnlistenFn> {
  return listen<InstancesRemovedEvent>(Events.InstanceRemoved, (e) =>
    handler(e.payload),
  );
}

export function onInstanceUrlChanged(
  handler: (payload: Pick<Instance, "id" | "current_url">) => void,
): Promise<UnlistenFn> {
  return listen<Pick<Instance, "id" | "current_url">>(
    Events.InstanceUrlChanged,
    (e) => handler(e.payload),
  );
}

export function onInstanceTitleChanged(
  handler: (payload: Pick<Instance, "id" | "page_title">) => void,
): Promise<UnlistenFn> {
  return listen<Pick<Instance, "id" | "page_title">>(
    Events.InstanceTitleChanged,
    (e) => handler(e.payload),
  );
}
