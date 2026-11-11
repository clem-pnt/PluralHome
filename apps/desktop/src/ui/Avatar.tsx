import type { CSSProperties } from "react";

export function Avatar({
  name,
  color,
  src,
  size = 40,
}: {
  name: string;
  color: string;
  src?: string | null | undefined;
  size?: number;
}) {
  return (
    <span
      className="avatar"
      aria-hidden
      style={{ "--member": color, "--size": `${size}px` } as CSSProperties}
    >
      {src ? (
        <img src={src} alt="" className="avatar__img" />
      ) : (
        name.slice(0, 1).toLocaleUpperCase()
      )}
    </span>
  );
}
