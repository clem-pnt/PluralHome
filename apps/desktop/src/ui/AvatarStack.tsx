import type { CSSProperties } from "react";
import { Avatar } from "./Avatar";

/**
 * Photos empilées en diagonale, comme les groupes de messages privés : deux
 * visages qui se chevauchent dans un carré de la taille d'un avatar.
 */
export function AvatarStack({
  people,
  size = 36,
}: {
  people: { name: string; avatar: string | null }[];
  size?: number;
}) {
  const [a, b] = people;
  if (!a) {
    return (
      <span
        className="avatar avatar--blurry"
        style={{ "--size": `${size}px` } as CSSProperties}
        aria-hidden
      />
    );
  }
  if (!b) return <Avatar name={a.name} src={a.avatar} size={size} />;
  const small = Math.round(size * 0.68);
  return (
    <span className="avatar-stack" aria-hidden style={{ "--size": `${size}px` } as CSSProperties}>
      <Avatar name={a.name} src={a.avatar} size={small} />
      <Avatar name={b.name} src={b.avatar} size={small} />
    </span>
  );
}
