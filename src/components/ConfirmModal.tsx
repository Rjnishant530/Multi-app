import { useEffect } from "react";

import { setActiveWebviewVisibility } from "../ipc/commands";
import { useAppStore } from "../state/store";

export function ConfirmModal() {
  const confirm = useAppStore((s) => s.confirm);
  const clear = useAppStore((s) => s.clearConfirm);

  useEffect(() => {
    if (!confirm) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") clear();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [confirm, clear]);

  // Tauri child webviews are native OS surfaces stacked above every
  // React layer — z-index can't reach them, so a full-screen modal
  // would be entirely behind the active site. Hide the active webview
  // while the modal is open, restore it on dismiss.
  useEffect(() => {
    if (!confirm) return;
    void setActiveWebviewVisibility(false);
    return () => {
      void setActiveWebviewVisibility(true);
    };
  }, [confirm]);

  if (!confirm) return null;

  async function onConfirm() {
    if (!confirm) return;
    const fn = confirm.onConfirm;
    clear();
    await fn();
  }

  return (
    <div
      className="confirm-backdrop"
      onClick={clear}
      onContextMenu={(e) => e.preventDefault()}
    >
      <div className="confirm-modal" onClick={(e) => e.stopPropagation()}>
        <div className="confirm-message">{confirm.message}</div>
        <div className="confirm-actions">
          <button
            type="button"
            className="confirm-cancel"
            onClick={clear}
            autoFocus
          >
            Cancel
          </button>
          <button
            type="button"
            className="confirm-destructive"
            onClick={() => void onConfirm()}
          >
            {confirm.destructiveLabel ?? "Delete"}
          </button>
        </div>
      </div>
    </div>
  );
}
