import { useState } from "react";

import { useAppStore } from "../state/store";
import type { Instance, InstanceTreeNode, Uuid } from "../types";
import { IconPicker } from "./IconPicker";
import { InstanceIcon } from "./InstanceIcon";
import { ChildBranchIcon } from "./icons";

interface Props {
  node: InstanceTreeNode;
  depth: number;
  activeInstanceId: Uuid | null;
  miniMode?: boolean;
}

export function SidebarNode({
  node,
  depth,
  activeInstanceId,
  miniMode = false,
}: Props) {
  const activate = useAppStore((s) => s.activateInstance);
  const rename = useAppStore((s) => s.renameInstance);
  const setIcon = useAppStore((s) => s.setInstanceIcon);
  const remove = useAppStore((s) => s.removeInstance);
  const requestConfirm = useAppStore((s) => s.requestConfirm);
  const isActive = node.instance.id === activeInstanceId;

  const [editing, setEditing] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [draft, setDraft] = useState(displayName(node.instance));
  const [collapsed, setCollapsed] = useState(false);

  // Child (forked) instances show a branch icon by default, only when
  // the user hasn't picked a custom one. This visually marks "this
  // came from a cross-domain link click on its parent."
  const fallbackIcon =
    node.instance.parent_instance_id != null ? ChildBranchIcon : undefined;

  async function commitRename() {
    const value = draft.trim();
    if (value && value !== displayName(node.instance)) {
      await rename(node.instance.id, value);
    }
    setEditing(false);
  }

  function confirmDelete() {
    requestConfirm({
      message: `Delete "${displayName(node.instance)}" and all its sub-instances? Their cookies and storage will be permanently removed.`,
      destructiveLabel: "Delete instance",
      onConfirm: () => remove(node.instance.id),
    });
  }

  async function handleIconSelect(slug: string | null) {
    if ((node.instance.icon ?? null) !== slug) {
      await setIcon(node.instance.id, slug);
    }
    setPickerOpen(false);
  }

  // Mini mode: collapsed sidebar shows icons only — single clickable
  // button per node, no name, no caret, no inline edit. Right-click
  // still opens the delete confirm.
  if (miniMode) {
    return (
      <>
        <button
          type="button"
          className={`sidebar-mini-row${isActive ? " active" : ""}`}
          onClick={() => void activate(node.instance.id)}
          onContextMenu={(e) => {
            e.preventDefault();
            confirmDelete();
          }}
          title={displayName(node.instance)}
          style={{ marginLeft: depth * 6 }}
        >
          <InstanceIcon
            icon={node.instance.icon}
            size={18}
            fallbackIcon={fallbackIcon}
            fallbackSize={13}
          />
        </button>
        {node.children.map((child) => (
          <SidebarNode
            key={child.instance.id}
            node={child}
            depth={depth + 1}
            activeInstanceId={activeInstanceId}
            miniMode
          />
        ))}
      </>
    );
  }

  return (
    <div className="sidebar-node" style={{ paddingLeft: depth * 12 }}>
      <div className={`sidebar-row${isActive ? " active" : ""}`}>
        {node.children.length > 0 ? (
          <button
            type="button"
            className="caret"
            onClick={() => setCollapsed(!collapsed)}
            aria-label={collapsed ? "Expand" : "Collapse"}
          >
            {collapsed ? "▸" : "▾"}
          </button>
        ) : (
          <span className="caret-spacer" />
        )}
        <div className="sidebar-icon-wrap">
          <button
            type="button"
            className="sidebar-icon"
            title="Pick an icon"
            onClick={(e) => {
              e.stopPropagation();
              setPickerOpen((open) => !open);
            }}
          >
            <InstanceIcon
              icon={node.instance.icon}
              size={14}
              fallbackIcon={fallbackIcon}
              fallbackSize={10}
            />
          </button>
          {pickerOpen && (
            <IconPicker
              selected={node.instance.icon}
              onSelect={(slug) => void handleIconSelect(slug)}
              onClose={() => setPickerOpen(false)}
            />
          )}
        </div>
        {editing ? (
          <input
            autoFocus
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onBlur={() => void commitRename()}
            onKeyDown={(e) => {
              if (e.key === "Enter") void commitRename();
              if (e.key === "Escape") {
                setDraft(displayName(node.instance));
                setEditing(false);
              }
            }}
          />
        ) : (
          <>
            <button
              type="button"
              className="sidebar-label"
              onClick={() => void activate(node.instance.id)}
              onDoubleClick={() => {
                setDraft(displayName(node.instance));
                setEditing(true);
              }}
              onContextMenu={(e) => {
                e.preventDefault();
                confirmDelete();
              }}
              title={`${node.instance.current_url}\n(double-click to rename · right-click to delete)`}
            >
              {displayName(node.instance)}
            </button>
            <button
              type="button"
              className="sidebar-row-close"
              aria-label="Delete instance"
              title="Delete (cascades through children)"
              onClick={(e) => {
                e.stopPropagation();
                confirmDelete();
              }}
            >
              ×
            </button>
          </>
        )}
      </div>
      {!collapsed &&
        node.children.map((child) => (
          <SidebarNode
            key={child.instance.id}
            node={child}
            depth={depth + 1}
            activeInstanceId={activeInstanceId}
          />
        ))}
    </div>
  );
}

function displayName(i: Instance): string {
  if (i.user_name && i.user_name.length > 0) return i.user_name;
  if (i.page_title && i.page_title.length > 0) return i.page_title;
  try {
    return new URL(i.current_url).hostname;
  } catch {
    return i.current_url;
  }
}
