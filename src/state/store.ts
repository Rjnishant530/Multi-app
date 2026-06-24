import { create } from "zustand";

import * as ipc from "../ipc/commands";
import {
  onInstanceAdded,
  onInstanceTitleChanged,
  onInstanceUrlChanged,
  onInstancesRemoved,
} from "../ipc/events";
import type { InstanceTreeNode, Uuid, Website } from "../types";

export interface ConfirmRequest {
  message: string;
  destructiveLabel?: string;
  onConfirm: () => void | Promise<void>;
}

interface AppStore {
  websites: Website[];
  activeWebsiteId: Uuid | null;
  // tree per website_id, hydrated on demand or invalidated on events
  treesByWebsiteId: Record<Uuid, InstanceTreeNode[]>;
  loading: boolean;
  error: string | null;
  sidebarCollapsed: boolean;
  confirm: ConfirmRequest | null;

  hydrate(): Promise<void>;
  setActiveWebsite(id: Uuid): Promise<void>;
  addWebsite(url: string): Promise<Website | null>;
  removeWebsite(id: Uuid): Promise<void>;

  addInstance(
    websiteId: Uuid,
    name?: string,
    icon?: string,
  ): Promise<void>;
  renameInstance(id: Uuid, name: string): Promise<void>;
  setInstanceIcon(id: Uuid, icon: string | null): Promise<void>;
  removeInstance(id: Uuid): Promise<void>;
  activateInstance(id: Uuid): Promise<void>;

  refreshTree(websiteId: Uuid): Promise<void>;
  refreshWebsites(): Promise<void>;

  toggleSidebar(): void;
  requestConfirm(req: ConfirmRequest): void;
  clearConfirm(): void;
}

export const useAppStore = create<AppStore>((set, get) => ({
  websites: [],
  activeWebsiteId: null,
  treesByWebsiteId: {},
  loading: true,
  error: null,
  sidebarCollapsed: false,
  confirm: null,

  async hydrate() {
    set({ loading: true, error: null });
    try {
      const websites = await ipc.listWebsites();
      const active =
        websites.find((w) => w.active_instance_id !== null) ?? websites[0];
      const activeWebsiteId = active?.id ?? null;
      set({ websites, activeWebsiteId, loading: false });
      if (activeWebsiteId) {
        await get().refreshTree(activeWebsiteId);
        const activeInstanceId =
          websites.find((w) => w.id === activeWebsiteId)?.active_instance_id ??
          null;
        if (activeInstanceId) {
          await ipc.activateInstance(activeInstanceId).catch(() => {
            // non-fatal: webview might not be ready yet
          });
        }
      }
    } catch (err) {
      set({ loading: false, error: String(err) });
    }
  },

  async refreshTree(websiteId) {
    try {
      const tree = await ipc.listInstanceTree(websiteId);
      set((s) => ({
        treesByWebsiteId: { ...s.treesByWebsiteId, [websiteId]: tree },
      }));
    } catch (err) {
      set({ error: String(err) });
    }
  },

  async refreshWebsites() {
    try {
      const websites = await ipc.listWebsites();
      set({ websites });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  async setActiveWebsite(id) {
    try {
      await ipc.activateWebsite(id);
      set({ activeWebsiteId: id });
      await get().refreshTree(id);
    } catch (err) {
      set({ error: String(err) });
    }
  },

  async addWebsite(url) {
    try {
      const w = await ipc.addWebsite(url);
      const existing = get().websites.find((x) => x.id === w.id);
      if (!existing) {
        set((s) => ({ websites: [...s.websites, w] }));
      }
      set({ activeWebsiteId: w.id });
      await get().refreshTree(w.id);
      return w;
    } catch (err) {
      set({ error: String(err) });
      return null;
    }
  },

  async removeWebsite(id) {
    try {
      await ipc.removeWebsite(id);
      set((s) => {
        const websites = s.websites.filter((w) => w.id !== id);
        const trees = { ...s.treesByWebsiteId };
        delete trees[id];
        const activeWebsiteId =
          s.activeWebsiteId === id
            ? (websites[0]?.id ?? null)
            : s.activeWebsiteId;
        return { websites, treesByWebsiteId: trees, activeWebsiteId };
      });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  async addInstance(websiteId, name, icon) {
    try {
      await ipc.addInstance(websiteId, name, icon);
      await Promise.all([
        get().refreshTree(websiteId),
        get().refreshWebsites(),
      ]);
    } catch (err) {
      set({ error: String(err) });
    }
  },

  async renameInstance(id, name) {
    try {
      await ipc.renameInstance(id, name);
      const wid = get().activeWebsiteId;
      if (wid) await get().refreshTree(wid);
    } catch (err) {
      set({ error: String(err) });
    }
  },

  async setInstanceIcon(id, icon) {
    try {
      await ipc.setInstanceIcon(id, icon);
      const wid = get().activeWebsiteId;
      if (wid) await get().refreshTree(wid);
    } catch (err) {
      set({ error: String(err) });
    }
  },

  async removeInstance(id) {
    try {
      await ipc.removeInstance(id);
      const wid = get().activeWebsiteId;
      if (wid) {
        await Promise.all([get().refreshTree(wid), get().refreshWebsites()]);
      }
    } catch (err) {
      set({ error: String(err) });
    }
  },

  async activateInstance(id) {
    try {
      await ipc.activateInstance(id);
      // Backend bumped active_instance_id on the owning website —
      // refetch so the sidebar's active highlight follows.
      await get().refreshWebsites();
    } catch (err) {
      set({ error: String(err) });
    }
  },

  toggleSidebar() {
    set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed }));
  },

  requestConfirm(req) {
    set({ confirm: req });
  },

  clearConfirm() {
    set({ confirm: null });
  },
}));

// Subscribe to backend events once at app boot. Each event invalidates
// the relevant tree cache and triggers a refresh.

let subscribed = false;

export async function subscribeBackendEvents(): Promise<void> {
  if (subscribed) return;
  subscribed = true;
  const refreshActive = async () => {
    const wid = useAppStore.getState().activeWebsiteId;
    const s = useAppStore.getState();
    await Promise.all([
      wid ? s.refreshTree(wid) : Promise.resolve(),
      s.refreshWebsites(),
    ]);
  };
  await onInstanceAdded(() => {
    void refreshActive();
  });
  await onInstancesRemoved(() => {
    void refreshActive();
  });
  await onInstanceUrlChanged(() => {
    void refreshActive();
  });
  await onInstanceTitleChanged(() => {
    void refreshActive();
  });
}
