import { useEffect } from "react";

import { ConfirmModal } from "./components/ConfirmModal";
import { Sidebar } from "./components/Sidebar";
import { TopTabs } from "./components/TopTabs";
import { Viewport } from "./components/Viewport";
import { subscribeBackendEvents, useAppStore } from "./state/store";

import "./App.css";

export default function App() {
  const hydrate = useAppStore((s) => s.hydrate);
  const loading = useAppStore((s) => s.loading);
  const sidebarCollapsed = useAppStore((s) => s.sidebarCollapsed);

  useEffect(() => {
    void subscribeBackendEvents();
    void hydrate();
  }, [hydrate]);

  return (
    <div className="app">
      <TopTabs />
      <main className={`layout${sidebarCollapsed ? " sidebar-collapsed" : ""}`}>
        <Sidebar />
        <Viewport />
      </main>
      <ConfirmModal />
      {loading && <div className="loading-overlay">Loading…</div>}
    </div>
  );
}
