import { m } from "../../paraglide/messages.js";

/** Numéros publics (France). Pas de lien `tel:` : sur ordinateur il ne mène nulle part. */
const LINES = [
  { number: "3114", name: m.line_3114, detail: m.line_3114_detail },
  { number: "15", name: m.line_15, detail: m.line_15_detail },
  { number: "112", name: m.line_112, detail: m.line_112_detail },
  { number: "114", name: m.line_114, detail: m.line_114_detail },
] as const;

export function CrisisLines() {
  return (
    <ul className="lines">
      {LINES.map((l) => (
        <li key={l.number} className="line">
          <span className="line__number" data-selectable>
            {l.number}
          </span>
          <span className="line__text">
            <strong>{l.name()}</strong>
            <span className="muted">{l.detail()}</span>
          </span>
        </li>
      ))}
    </ul>
  );
}
