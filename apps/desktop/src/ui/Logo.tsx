import logoPath from "./logo-amp.json";

/** Logo vectoriel : maison au trait, & Fraunces droit. Couleurs pilotées par les jetons. */
export function Logo({ size = 40, title }: { size?: number; title?: string }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="16 16 168 168"
      role={title ? "img" : "presentation"}
      aria-label={title}
    >
      <g fill="none" stroke="var(--ink)" strokeWidth="5" strokeLinecap="square">
        <path d="M28 96 L100 32 L172 96" />
        <path d="M46 80 V170 H154 V80" strokeLinecap="butt" />
      </g>
      <path fill="var(--accent)" transform={logoPath.transform} d={logoPath.d} />
    </svg>
  );
}
