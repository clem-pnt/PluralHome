import type { CSSProperties } from "react";

export function Avatar({ name, color, size = 40 }: { name: string; color: string; size?: number }) {
  return (
    <span
      className="avatar"
      aria-hidden
      style={{ "--member": color, "--size": `${size}px` } as CSSProperties}
    >
      {name.slice(0, 1).toLocaleUpperCase()}
    </span>
  );
}
