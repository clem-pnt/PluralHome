import { Input, Label, SearchField } from "react-aria-components";
import { Icon } from "./Icon";

/** Champ de recherche avec loupe ; Échap l'efface. */
export function Search({
  label,
  placeholder,
  value,
  onChange,
  autoFocus,
}: {
  label: string;
  placeholder?: string;
  value: string;
  onChange: (v: string) => void;
  autoFocus?: boolean;
}) {
  return (
    <SearchField
      className="search"
      value={value}
      onChange={onChange}
      aria-label={label}
      {...(autoFocus && { autoFocus })}
    >
      <Label className="sr-only">{label}</Label>
      <span className="search__icon">
        <Icon name="search" size={16} />
      </span>
      <Input className="input search__input" placeholder={placeholder ?? label} />
    </SearchField>
  );
}

/** Texte replié pour chercher sans tenir compte des accents ni de la casse. */
export const foldText = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();

/** Le membre correspond-il à la recherche ? Seul le prénom compte. */
export function matches(q: string, x: { name: string }): boolean {
  const needle = foldText(q.trim());
  if (!needle) return true;
  const hay = foldText(x.name);
  return needle.split(/\s+/u).every((w) => hay.includes(w));
}
