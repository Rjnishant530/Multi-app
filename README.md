# Multi-app

A lightweight cross-platform desktop app for running many session-isolated
webviews of the same website (e.g. five Google accounts side by side) —
Rust + Tauri 2 + React. System webview, no Chromium bundle.

## How it works

- **Top tabs** are websites you've added (e.g. `google.com`, `github.com`).
- **Sidebar** shows a tree of session-isolated instances of the active
  website. Each instance is one logged-in identity; rename them as you
  like (Personal, Work, Client A…).
- **Cross-domain link clicks auto-fork** into a child instance under the
  parent, rendered as a sub-node in the sidebar tree. The parent webview
  stays on its current URL.
- Each instance has its own OS-managed cookie jar / storage:
  - macOS → `WKWebsiteDataStore(forIdentifier:)` keyed by instance UUID
  - Windows → WebView2 user data folder under `$APP_DATA/webviews/<uuid>/`
  - Linux → WebKitGTK WebsiteDataManager under `$APP_DATA/webviews/<uuid>/`

## Requirements

- **macOS 14+ (Sonoma)** — required floor for real per-webview session
  isolation via `WKWebsiteDataStore(forIdentifier:)`. The app refuses to
  build for older targets via `bundle.macOS.minimumSystemVersion`.
- **Windows 10+** with WebView2 runtime (the installer bundles the
  bootstrapper).
- **Linux** with WebKitGTK 4.1 (`libwebkit2gtk-4.1-0` on Debian/Ubuntu).
- **Rust 1.77+**, **Node 18+** (recommended 20+), **pnpm**.

## Install pnpm

```bash
# via corepack (ships with Node):
corepack enable pnpm

# OR via Volta:
volta install pnpm
```

## Run in dev

```bash
pnpm install
pnpm tauri dev
```

The first `cargo build` is slow (~1–2 min on a fast laptop). Subsequent
runs are incremental. The dev workflow uses Vite for HMR on the React
chrome and live-reloads the Rust binary on `.rs` changes.

## Build a release artifact

```bash
pnpm tauri build
```

Output lands in `src-tauri/target/release/bundle/`:

- `*.app` / `*.dmg` on macOS
- `*.msi` on Windows
- `*.AppImage`, `*.deb` on Linux

## Run the test suite

Rust unit tests:

```bash
cd src-tauri && cargo test
```

Frontend type-check:

```bash
pnpm exec tsc --noEmit
```

The cross-platform behavioral matrix is run manually — see
[`docs/smoke-matrix.md`](docs/smoke-matrix.md).

## Layout

```
src/                  React + Vite frontend
  components/         TopTabs, Sidebar, SidebarNode, Viewport
  ipc/                Typed wrappers around Tauri's invoke/listen
  state/store.ts      Zustand store mirroring backend state
  types.ts            DTOs that mirror Rust serde structs

src-tauri/            Rust backend
  src/
    commands.rs       #[tauri::command] surface + pure state_ops
    model.rs          Website / Instance / AppState / StoreSchema
    nav_guard.rs      Same-site (eTLD+1) classification via psl
    paths.rs          $APP_DATA path resolution
    store.rs          Debounced persister + tauri-plugin-store
    webview_manager.rs  Lifecycle, isolation, fork channel
    lib.rs            Tauri Builder, setup, fork consumer task

docs/
  plans/              Implementation plan (read this for context)
  smoke-matrix.md     Cross-OS manual test matrix
```

## Known limitations (v1)

- **SPA pushState navigations across domains** do not reliably trigger
  the navigation interceptor, so they won't fork. Same-eTLD+1 SPAs
  (e.g. Gmail) don't need to fork anyway, and cross-domain client-side
  navigations are rare in practice.
- **macOS < 14**: blocked at build time. There is no fallback for older
  macOS — per-webview WKWebsiteDataStore identifiers don't exist.
- **Icons** are placeholder solid-color blocks generated via `sips` and
  `iconutil`. Replace `src-tauri/icons/*` with real brand assets before
  shipping.
