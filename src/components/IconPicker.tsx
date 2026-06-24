import { useEffect, useLayoutEffect, useRef, useState } from "react";

import { ICON_CHOICES } from "./icons";

interface Props {
  selected: string | null;
  onSelect: (slug: string | null) => void;
  onClose: () => void;
}

// Anchored popover. The parent wraps both the trigger button and this
// picker in a position:relative element; the picker positions itself
// absolutely relative to that parent. The picker measures its own
// position after mount and flips above the trigger when there isn't
// enough room below — important because Tauri's child webviews are
// native OS surfaces that render ABOVE every React layer (z-index
// can't reach them), so a picker that hangs into the viewport region
// gets visually clipped by the active site. Flipping up keeps the
// picker inside the sidebar region.
export function IconPicker({ selected, onSelect, onClose }: Props) {
  const ref = useRef<HTMLDivElement | null>(null);
  const [placement, setPlacement] = useState<"bottom" | "top">("bottom");

  // Decide direction immediately after mount, before paint, to avoid
  // a one-frame flash in the wrong position.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const overflowsBelow = rect.bottom > window.innerHeight - 8;
    if (overflowsBelow) setPlacement("top");
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    const onClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    const t = setTimeout(() => {
      window.addEventListener("mousedown", onClick);
    }, 0);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("mousedown", onClick);
      clearTimeout(t);
    };
  }, [onClose]);

  return (
    <div className={`icon-picker icon-picker-${placement}`} ref={ref}>
      <div className="icon-picker-grid">
        {ICON_CHOICES.map(({ slug, label, Icon }) => (
          <button
            key={slug}
            type="button"
            className={`icon-picker-cell${selected === slug ? " selected" : ""}`}
            title={label}
            aria-label={label}
            onClick={() => onSelect(slug)}
          >
            <Icon size={16} strokeWidth={1.8} />
          </button>
        ))}
      </div>
      <button
        type="button"
        className="icon-picker-clear"
        onClick={() => onSelect(null)}
        disabled={!selected}
      >
        Clear icon
      </button>
    </div>
  );
}
