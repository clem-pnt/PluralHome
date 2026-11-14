import type { CSSProperties } from "react";

/** Photo de profil du membre, ou son initiale sur fond neutre à défaut. */
export function Avatar({
  name,
  src,
  size = 40,
}: {
  name: string;
  src?: string | null | undefined;
  size?: number;
}) {
  return (
    <span className="avatar" aria-hidden style={{ "--size": `${size}px` } as CSSProperties}>
      {src ? (
        <img src={src} alt="" className="avatar__img" />
      ) : (
        name.slice(0, 1).toLocaleUpperCase()
      )}
    </span>
  );
}
