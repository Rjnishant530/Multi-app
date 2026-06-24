# Cross-platform smoke matrix

This is the manual acceptance test for v1. Run each scenario on **all
three platforms** (macOS 14+, Windows 11, Ubuntu 24.04) on a fresh
install. Any failure on any platform blocks v1.

Headless tests cannot prove what matters most — that the *operating
system's* cookie jar / storage stores really are isolated between
webviews — so most of this matrix is hand-driven.

## Setup per platform

| Platform | Setup |
|---|---|
| macOS 14+ | `pnpm install && pnpm tauri dev` |
| Windows 11 | Install WebView2 runtime if absent. `pnpm install && pnpm tauri dev` |
| Ubuntu 24.04 | `sudo apt install libwebkit2gtk-4.1-dev libssl-dev`. `pnpm install && pnpm tauri dev` |

Before each scenario, wipe `$APP_DATA/dev.multiapp.app/` so the run
starts from a clean slate.

## Scenarios

### S1 — Fresh launch shows empty shell
- Launch the app on a clean profile.
- **Expect**: window opens at 1280×820, dark chrome, sidebar shows
  "Add a website to get started.", no errors in the dev console or
  Rust stderr.

### S2 — Add a website (paste raw hostname)
- Click `+` in the top tab bar, type `google.com`, press Enter.
- **Expect**: a `google.com` tab appears and is active; sidebar header
  reads `google.com / instances`; the sidebar empty-state asks for an
  instance.

### S3 — Add two instances and isolate cookies (load-bearing)
- In the sidebar, add an instance named "Personal".
- Sign in to a Google account (Account A).
- Add a second instance named "Work".
- Sign in to a different Google account (Account B) in "Work".
- Click "Personal" — **expect** still Account A.
- Click "Work" — **expect** still Account B.
- Quit and relaunch. Confirm both sessions persist independently.

### S4 — Same-eTLD+1 navigation stays in the instance
- From "Personal" Gmail, click a link to `https://drive.google.com`.
- **Expect**: the webview navigates inside the same instance (it does
  not fork). Sidebar tree is unchanged.

### S5 — Cross-domain navigation auto-forks
- From "Personal" Gmail, click an external link to `https://stripe.com`
  (e.g. paste a Stripe URL into a Gmail draft and click it).
- **Expect**: the original "Personal" webview stays on Gmail. A new
  child node appears under "Personal" in the sidebar, loaded with
  `stripe.com`. The new child becomes the active visible webview.

### S6 — `target=_blank` popup forks
- Find any page with a `target=_blank` link to an external domain.
- Click it.
- **Expect**: a new sub-instance appears in the sidebar; no OS-level
  popup window is spawned.

### S7 — Rename
- Double-click a sidebar instance, type a new name, press Enter.
- **Expect**: the label updates; the change persists across restarts.
- Try entering whitespace-only name → cancelled silently.

### S8 — Delete (cascades through children)
- Right-click a sidebar instance with children, confirm delete.
- **Expect**: the instance AND all its descendant sub-instances
  disappear. Cookies / storage for each are wiped (Windows/Linux: the
  `webviews/<uuid>/` dir is removed; macOS: WebKit's data store is
  scheduled for cleanup).

### S9 — Add a second website, switch
- Add `https://github.com` as a second website.
- **Expect**: top tab switches to GitHub; sidebar swaps to GitHub's
  (empty) instance tree. Switching back to `google.com` restores its
  tree state including the active instance.

### S10 — Persistence across restarts
- Quit the app. Relaunch.
- **Expect**: websites, instance tree, names, and last-active selection
  are identical to before quit.

### S11 — Window resize tracks viewport
- Resize the window quickly. Maximize, restore, drag the edges.
- **Expect**: the active webview repositions to fill the viewport rect
  with no flicker, no gaps, no overlap with the sidebar. On macOS,
  fullscreen + reduce works.

### S12 — Single-instance lock
- Launch the app while a copy is already running.
- **Expect**: the existing window comes to the front (focus + unminimize);
  no second process starts; no WebView2 / WKWebsiteDataStore lock error.

### S13 — Corrupted store recovery
- Quit the app.
- Edit `$APP_DATA/dev.multiapp.app/metadata/store.json`: replace one
  `active_instance_id` with a random UUID that doesn't exist.
- Relaunch.
- **Expect**: the app starts; the affected website falls back to its
  first available instance (or to no active instance if empty); a
  warning appears in stderr ("cleaned up dangling references").

### S14 — Garbage URL input
- Open the add-website input, paste `not a url at all !!!`.
- **Expect**: the request fails silently or shows an inline error;
  no website is created; no panic in stderr.
