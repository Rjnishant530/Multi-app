import type { LucideIcon } from "lucide-react";

import { ICON_CATALOG } from "./icons";

interface Props {
  icon: string | null | undefined;
  size?: number;
  className?: string;
  emptyChar?: string;
  // Rendered when icon is null/empty. Overrides the dot placeholder
  // when supplied — used for child instances to show a branch icon
  // by default while still allowing the user to override it.
  fallbackIcon?: LucideIcon;
  // Optional override size for the fallback icon — defaults to `size`.
  // Used to render the child-branch icon smaller than a user-picked
  // icon, since it's a secondary visual cue rather than the focal one.
  fallbackSize?: number;
}

// Renders an instance's icon. Resolution order:
//   1. The slug matches an entry in the curated catalog → render the
//      Lucide component.
//   2. Otherwise (older instances saved with a literal emoji, custom
//      text) → render the raw string so we never lose user input.
//   3. Null/empty + fallbackIcon → render the fallback component.
//   4. Null/empty + no fallback → faint dot.
export function InstanceIcon({
  icon,
  size = 14,
  className = "",
  emptyChar = "·",
  fallbackIcon: FallbackIcon,
  fallbackSize,
}: Props) {
  const lucideStyle: React.CSSProperties = {
    width: size,
    height: size,
    color: "currentColor",
    flexShrink: 0,
  };

  if (!icon) {
    if (FallbackIcon) {
      const fSize = fallbackSize ?? size;
      const fStyle: React.CSSProperties = {
        width: fSize,
        height: fSize,
        color: "currentColor",
        flexShrink: 0,
        opacity: 0.7,
      };
      return (
        <FallbackIcon
          size={fSize}
          width={fSize}
          height={fSize}
          strokeWidth={1.8}
          style={fStyle}
          className={className}
          aria-hidden
        />
      );
    }
    return (
      <span className={`instance-icon-empty ${className}`}>{emptyChar}</span>
    );
  }
  const Lucide = ICON_CATALOG[icon];
  if (Lucide) {
    return (
      <Lucide
        size={size}
        width={size}
        height={size}
        strokeWidth={1.8}
        style={lucideStyle}
        className={className}
        aria-hidden
      />
    );
  }
  return <span className={`instance-icon-text ${className}`}>{icon}</span>;
}
