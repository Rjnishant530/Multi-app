import { useEffect, useRef, useState } from "react";

import { useAppStore } from "../state/store";
import type { InstanceTreeNode } from "../types";
import { Favicon } from "./Favicon";
import { IconPicker } from "./IconPicker";
import { InstanceIcon } from "./InstanceIcon";
import { SidebarNode } from "./SidebarNode";

// Stable reference for the empty case. Zustand v5 + React 18's
// useSyncExternalStore compares snapshots by identity — returning a
// fresh `[]` literal on every render would trigger an infinite update
// loop. Reuse this constant whenever no tree is cached.
const EMPTY_TREE: InstanceTreeNode[] = [];

// Pre-selected icon for new instances so the add-form's icon slot
// doesn't show a dot. Users can change it via the picker before they
// submit, or leave it as the default. Slug matches an entry in
// ICON_CATALOG.
const DEFAULT_NEW_INSTANCE_ICON = "user";

export function Sidebar() {
  const activeWebsiteId = useAppStore((s) => s.activeWebsiteId);
  const tree = useAppStore((s) =>
    activeWebsiteId
      ? (s.treesByWebsiteId[activeWebsiteId] ?? EMPTY_TREE)
      : EMPTY_TREE,
  );
  const websites = useAppStore((s) => s.websites);
  const collapsed = useAppStore((s) => s.sidebarCollapsed);
  const addInstance = useAppStore((s) => s.addInstance);
  const toggleSidebar = useAppStore((s) => s.toggleSidebar);

  const activeWebsite = websites.find((w) => w.id === activeWebsiteId);
  const activeInstanceId = activeWebsite?.active_instance_id ?? null;

  const nameInputRef = useRef<HTMLInputElement | null>(null);
  // When the user clicks the mini-rail's + button we expand the
  // sidebar and set this flag; the effect below then moves focus to
  // the name input so they can type immediately.
  const focusNameOnNextOpen = useRef(false);

  useEffect(() => {
    if (!collapsed && focusNameOnNextOpen.current && nameInputRef.current) {
      nameInputRef.current.focus();
      focusNameOnNextOpen.current = false;
    }
  }, [collapsed]);

  const [pendingName, setPendingName] = useState("");
  const [pendingIcon, setPendingIcon] = useState<string | null>(
    DEFAULT_NEW_INSTANCE_ICON,
  );
  const [pending, setPending] = useState(false);
  const [iconPickerOpen, setIconPickerOpen] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!activeWebsiteId || pending) return;
    setPending(true);
    await addInstance(
      activeWebsiteId,
      pendingName.trim() || undefined,
      pendingIcon ?? undefined,
    );
    setPending(false);
    setPendingName("");
    setPendingIcon(DEFAULT_NEW_INSTANCE_ICON);
  }

  function openAddForm() {
    if (!collapsed) {
      nameInputRef.current?.focus();
      return;
    }
    focusNameOnNextOpen.current = true;
    toggleSidebar();
  }

  if (!activeWebsite) {
    if (collapsed) {
      // Empty mini-rail when nothing is selected.
      return <aside className="sidebar sidebar-mini" />;
    }
    return (
      <aside className="sidebar">
        <div className="sidebar-empty">Add a website to get started.</div>
      </aside>
    );
  }

  if (collapsed) {
    return (
      <aside className="sidebar sidebar-mini">
        <div className="sidebar-mini-tree">
          {tree.map((node) => (
            <SidebarNode
              key={node.instance.id}
              node={node}
              depth={0}
              activeInstanceId={activeInstanceId}
              miniMode
            />
          ))}
        </div>
        <button
          type="button"
          className="sidebar-mini-add"
          onClick={openAddForm}
          disabled={pending}
          title="Add instance"
          aria-label="Add instance"
        >
          +
        </button>
      </aside>
    );
  }

  return (
    <aside className="sidebar">
      <header className="sidebar-header">
        <div className="sidebar-title-row">
          <Favicon domain={activeWebsite.url_root} size={14} />
          <span className="sidebar-title">{activeWebsite.url_root}</span>
        </div>
        <span className="sidebar-subtitle">instances</span>
      </header>
      <div className="sidebar-tree">
        {tree.length === 0 ? (
          <div className="sidebar-empty">
            No instances yet. Add one below.
          </div>
        ) : (
          tree.map((node) => (
            <SidebarNode
              key={node.instance.id}
              node={node}
              depth={0}
              activeInstanceId={activeInstanceId}
            />
          ))
        )}
      </div>
      <form className="sidebar-add-form" onSubmit={submit}>
        <div className="sidebar-add-form-row">
          <div className="sidebar-add-icon-wrap">
            <button
              type="button"
              className="sidebar-add-icon-button"
              onClick={() => setIconPickerOpen((open) => !open)}
              title="Pick an icon"
              aria-label="Pick icon for new instance"
            >
              <InstanceIcon icon={pendingIcon} size={14} />
            </button>
            {iconPickerOpen && (
              <IconPicker
                selected={pendingIcon}
                onSelect={(slug) => {
                  setPendingIcon(slug);
                  setIconPickerOpen(false);
                }}
                onClose={() => setIconPickerOpen(false)}
              />
            )}
          </div>
          <input
            ref={nameInputRef}
            type="text"
            className="sidebar-add-name-input"
            placeholder="Name (optional)"
            value={pendingName}
            onChange={(e) => setPendingName(e.target.value)}
            disabled={pending}
          />
        </div>
        <button type="submit" disabled={pending}>
          + Instance
        </button>
      </form>
    </aside>
  );
}
