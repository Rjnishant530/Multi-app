import { useEffect, useRef } from "react";

import { setViewportBounds } from "../ipc/commands";
import { useAppStore } from "../state/store";

// Layout constants — kept in sync with App.css.
// Computing bounds from window.innerWidth/innerHeight directly is more
// reliable than getBoundingClientRect on the placeholder <section>:
// the rect can be wrong during transitions, before layout settles, or
// after window mode changes (fullscreen, maximize), all of which we
// observed in dev. Hard-coding the layout's known constants avoids
// those failure modes entirely.
export const TOP_BAR_HEIGHT = 44;
export const SIDEBAR_FULL_WIDTH = 220;
export const SIDEBAR_MINI_WIDTH = 52;

// macOS coordinate-system compensation: Tauri's add_child on macOS
// positions the child webview in OS WINDOW FRAME coordinates (which
// include the title bar area), NOT in content-view coordinates. The
// default macOS title bar in this version is ~40px tall, so a position
// of y=40 lands ON the title bar's bottom edge instead of below our
// 40px top tab row. We add the title bar's height to y so the webview
// clears both the OS title bar and our top tabs.
//
// height stays referenced to inner content (window.innerHeight) minus
// the React top bar — NOT minus this offset — because the offset only
// shifts the top edge into frame coords; the bottom edge is still
// measured from the same frame-coord origin and naturally lands at the
// OS frame bottom when height covers the full inner area + title bar
// difference. See git log for the empirical derivation.
const MACOS_TITLE_BAR_HEIGHT = 40;

export function Viewport() {
  const ref = useRef<HTMLDivElement | null>(null);
  const activeWebsite = useAppStore((s) =>
    s.websites.find((w) => w.id === s.activeWebsiteId),
  );
  const sidebarCollapsed = useAppStore((s) => s.sidebarCollapsed);
  const activeInstanceId = activeWebsite?.active_instance_id ?? null;

  useEffect(() => {
    const report = () => {
      const sidebarWidth = sidebarCollapsed
        ? SIDEBAR_MINI_WIDTH
        : SIDEBAR_FULL_WIDTH;
      // y is in OS frame coords (title bar included). height is in
      // content-area coords (title bar excluded). The mismatch is by
      // design — together they make the webview span from below our
      // unified top bar to the bottom of the visible OS window.
      // BOTTOM_MARGIN reserves a small gutter under the webview so it
      // doesn't sit flush against the OS window's bottom edge.
      const BOTTOM_MARGIN = 5;
      const y = MACOS_TITLE_BAR_HEIGHT + TOP_BAR_HEIGHT;
      const height = Math.max(
        1,
        window.innerHeight - TOP_BAR_HEIGHT - BOTTOM_MARGIN,
      );
      const bounds = {
        x: sidebarWidth,
        y,
        width: Math.max(1, window.innerWidth - sidebarWidth),
        height,
      };
      // eslint-disable-next-line no-console
      console.debug(
        "[viewport] reporting bounds",
        bounds,
        "window=",
        window.innerWidth,
        "x",
        window.innerHeight,
        "sidebarCollapsed=",
        sidebarCollapsed,
      );
      void setViewportBounds(bounds);
    };

    // Boot-time layout race: during the first ~hundreds of ms after
    // mount, window.innerHeight can be wrong (we saw 288 instead of
    // ~792). Report on every animation frame for the first second so
    // any late-settling layout gets the correct bounds applied.
    let stopBootLoop = false;
    const bootStart = performance.now();
    const bootLoop = () => {
      report();
      if (stopBootLoop) return;
      if (performance.now() - bootStart > 1000) return;
      requestAnimationFrame(bootLoop);
    };
    requestAnimationFrame(bootLoop);

    // Steady-state: the documentElement's resize observer catches every
    // viewport-size change (window resize, fullscreen toggle, devtools
    // open/close) — more reliable than window.resize alone.
    const ro = new ResizeObserver(report);
    ro.observe(document.documentElement);
    window.addEventListener("resize", report);

    return () => {
      stopBootLoop = true;
      ro.disconnect();
      window.removeEventListener("resize", report);
    };
  }, [sidebarCollapsed]);

  return (
    <section className="viewport" ref={ref}>
      {!activeInstanceId && (
        <div className="viewport-placeholder">
          {activeWebsite
            ? "Pick an instance from the sidebar."
            : "No instance active"}
        </div>
      )}
    </section>
  );
}
