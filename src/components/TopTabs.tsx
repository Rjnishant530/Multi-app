import { useState } from "react";

import { ArrowLeft, ArrowRight, RotateCw } from "lucide-react";

import { instanceBack, instanceForward, instanceReload } from "../ipc/commands";
import { useAppStore } from "../state/store";
import { Favicon } from "./Favicon";

export function TopTabs() {
  const websites = useAppStore((s) => s.websites);
  const activeWebsiteId = useAppStore((s) => s.activeWebsiteId);
  const setActiveWebsite = useAppStore((s) => s.setActiveWebsite);
  const addWebsite = useAppStore((s) => s.addWebsite);
  const removeWebsite = useAppStore((s) => s.removeWebsite);
  const requestConfirm = useAppStore((s) => s.requestConfirm);
  const sidebarCollapsed = useAppStore((s) => s.sidebarCollapsed);
  const toggleSidebar = useAppStore((s) => s.toggleSidebar);
  const activeWebsite = useAppStore((s) =>
    s.websites.find((w) => w.id === s.activeWebsiteId),
  );
  const activeInstanceId = activeWebsite?.active_instance_id ?? null;
  const navDisabled = !activeInstanceId;

  const [adding, setAdding] = useState(false);
  const [value, setValue] = useState("");
  const [pending, setPending] = useState(false);

  function confirmRemoveWebsite(label: string, id: string) {
    requestConfirm({
      message: `Remove ${label}? All its instances and their cookies/storage will be permanently deleted.`,
      destructiveLabel: "Remove website",
      onConfirm: () => removeWebsite(id),
    });
  }

  async function submit() {
    if (!value.trim() || pending) return;
    setPending(true);
    const result = await addWebsite(value.trim());
    setPending(false);
    if (result) {
      setValue("");
      setAdding(false);
    }
  }

  return (
    <header className="top-tabs">
      <button
        type="button"
        className="sidebar-toggle"
        onClick={toggleSidebar}
        aria-label={sidebarCollapsed ? "Show sidebar" : "Hide sidebar"}
        title={sidebarCollapsed ? "Show sidebar" : "Hide sidebar"}
      >
        {sidebarCollapsed ? "☰" : "⇤"}
      </button>
      <div className="top-tabs-nav">
        <button
          type="button"
          className="nav-btn"
          onClick={() => activeInstanceId && void instanceBack(activeInstanceId)}
          disabled={navDisabled}
          title="Back"
          aria-label="Back"
        >
          <ArrowLeft size={14} strokeWidth={1.8} />
        </button>
        <button
          type="button"
          className="nav-btn"
          onClick={() =>
            activeInstanceId && void instanceForward(activeInstanceId)
          }
          disabled={navDisabled}
          title="Forward"
          aria-label="Forward"
        >
          <ArrowRight size={14} strokeWidth={1.8} />
        </button>
        <button
          type="button"
          className="nav-btn"
          onClick={() =>
            activeInstanceId && void instanceReload(activeInstanceId)
          }
          disabled={navDisabled}
          title="Reload"
          aria-label="Reload"
        >
          <RotateCw size={14} strokeWidth={1.8} />
        </button>
      </div>
      <nav className="tabs">
        {websites.map((w) => (
          <span
            key={w.id}
            className={`tab${w.id === activeWebsiteId ? " active" : ""}`}
            title={w.url_root}
          >
            <button
              type="button"
              className="tab-label"
              onClick={() => void setActiveWebsite(w.id)}
              onAuxClick={(e) => {
                if (e.button === 1) {
                  confirmRemoveWebsite(w.url_root, w.id);
                }
              }}
            >
              <Favicon domain={w.url_root} size={14} />
              <span className="tab-label-text">{w.display_title}</span>
            </button>
            <button
              type="button"
              className="tab-close"
              aria-label={`Close ${w.display_title}`}
              onClick={(e) => {
                e.stopPropagation();
                confirmRemoveWebsite(w.url_root, w.id);
              }}
            >
              ×
            </button>
          </span>
        ))}
      </nav>
      <div className="top-tabs-actions">
        {adding ? (
          <form
            className="tab-add-form"
            onSubmit={(e) => {
              e.preventDefault();
              void submit();
            }}
          >
            <input
              autoFocus
              type="text"
              value={value}
              placeholder="google.com"
              onChange={(e) => setValue(e.target.value)}
              onBlur={() => {
                if (!pending && !value) setAdding(false);
              }}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  setAdding(false);
                  setValue("");
                }
              }}
              disabled={pending}
            />
          </form>
        ) : (
          <button
            type="button"
            className="tab-add"
            onClick={() => setAdding(true)}
            aria-label="Add website"
            title="Add website"
          >
            +
          </button>
        )}
      </div>
    </header>
  );
}
