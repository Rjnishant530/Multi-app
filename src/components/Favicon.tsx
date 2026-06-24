import { useState } from "react";

import { Globe } from "lucide-react";

interface Props {
  domain: string;
  size?: number;
  className?: string;
}

// Loads a favicon via Google's s2 favicons service. Reliable, cached
// by the browser, returns a PNG for any domain. Falls back to a
// generic globe icon if the request fails (network down, domain has
// no favicon, etc.) so the row never collapses to empty space.
//
// The service URL contract: https://www.google.com/s2/favicons?domain=<d>&sz=<n>
// where sz is one of the supported sizes (16, 32, 64). We pass 2x
// the requested logical size so it stays crisp on retina displays.
export function Favicon({ domain, size = 14, className = "" }: Props) {
  const [error, setError] = useState(false);

  if (error || !domain) {
    return (
      <Globe
        size={size}
        width={size}
        height={size}
        strokeWidth={1.8}
        style={{
          width: size,
          height: size,
          color: "currentColor",
          opacity: 0.6,
          flexShrink: 0,
        }}
        className={className}
        aria-hidden
      />
    );
  }

  const px = size * 2;
  const src = `https://www.google.com/s2/favicons?domain=${encodeURIComponent(domain)}&sz=${px}`;

  return (
    <img
      src={src}
      width={size}
      height={size}
      alt=""
      aria-hidden
      onError={() => setError(true)}
      style={{ width: size, height: size, flexShrink: 0, borderRadius: 2 }}
      className={className}
    />
  );
}
