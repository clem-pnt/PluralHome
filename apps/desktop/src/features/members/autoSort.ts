/**
 * Tri automatique : lit les descriptions sur l'appareil et propose des dossiers
 * (âge, genre) et des rôles. Rien n'est appliqué sans validation, et rien ne sort
 * de l'appareil. Prudent par construction : un mot isolé (« protège l'hôte »,
 * « s'occupe des enfants ») ne suffit pas, il faut un champ (« Âge : 16 ») ou une
 * tournure qui parle du membre lui-même (« c'est une protectrice », « 16 ans »).
 */

export type SortMember = {
  id: string;
  name: string;
  description: string | null;
  roles: string[];
};

export type Hit = { id: string; name: string; reason: string };

export type Proposal = {
  kind: "age" | "gender" | "role";
  /** Nom du dossier ou du rôle proposé. */
  label: string;
  /** Pour un dossier : identifiant s'il existe déjà. */
  folderId?: string | undefined;
  hits: Hit[];
};

export type Labels = {
  age: Record<AgeGroup, string>;
  gender: Record<Gender, string>;
  roles: Record<RoleKey, string>;
};

export type AgeGroup = "child" | "teen" | "adult";
export type Gender = "female" | "male" | "nonbinary" | "agender" | "fluid";
export type RoleKey =
  | "host"
  | "cohost"
  | "protector"
  | "gatekeeper"
  | "little"
  | "caretaker"
  | "trauma"
  | "persecutor"
  | "introject"
  | "fictive"
  | "fragment";

const L = "\\p{L}\\d";
const B = `(?<![${L}])`;
const E = `(?![${L}])`;
const word = (body: string) => `${B}(?:${body})${E}`;

const ROLE_PATTERNS: Record<RoleKey, string> = {
  cohost: "co[- ]?h[oô]te(?:sse)?|co[- ]?host",
  host: "(?<!co[- ]?)(?:hote(?:sse)?|host)",
  protector: "protect(?:eur|rice|eurice|or)s?|protection|defense(?:ur|use)s?",
  gatekeeper: "gate[- ]?keepers?|guet[- ]?keepers?|gardien(?:[·.-]?ne)?s?",
  little: "(?<!a )littles?",
  caretaker: "care[- ]?(?:taker|giver)s?|soign(?:eur|euse|ant|ante)s?",
  trauma:
    "trauma[- ]?holders?|port(?:eur|euse|e)s? (?:de |des |du |le |les )?traumas?|porte[- ]trauma",
  persecutor: "persecut(?:eur|rice|or)s?",
  introject: "introje(?:c)?ts?",
  fictive: "fictive?s?|fictif|factive?s?|factif|fictives?",
  fragment: "fragments?",
};

const GENDER_PATTERNS: Record<Gender, string> = {
  nonbinary: "non[- ]?binaire|non[- ]?binary|nonbinary|enby",
  agender: "agenre|agender",
  fluid: "gender[- ]?fluid|genre[- ]?fluide|genderfluide",
  female: "fille|femme|feminine?|girl|woman|female|mtf|transfem(?:me)?",
  male: "garcon|homme|masculine?|boy|man|male|mec|ftm|transmasc",
};
/** Hors champ « Genre : », seuls ces genres se lisent n'importe où dans le texte. */
const GENDER_ANYWHERE: Gender[] = ["nonbinary", "agender", "fluid"];

const AGE_UNIT = "ans?|yo|y/o|y\\.o\\.?|years?(?: old)?|yrs?";
const AGE_NUMBER = new RegExp(
  `${B}(\\d{1,3})(?:\\s*(?:-|a|/)\\s*\\d{1,3})?\\s*(?:${AGE_UNIT})${E}`,
  "gu",
);
/** « il y a 3 ans », « depuis 2 ans » : une durée, pas un âge. */
const DURATION_BEFORE =
  /(?:il y a|y a|depuis|pendant|durant|apres|avant|since|for|during|after|in|dans|en|tous les|toutes les)\s*$/u;
const DURATION_AFTER = /^\s*(?:ago|plus tot|plus tard|auparavant)/u;

const FIELD_NAMES = {
  age: "age|ages|age physique|age apparent",
  gender: "genre|gender|sexe|sex|identite de genre|identite",
  role: "roles?|role principal|roles principaux|fonctions?|role dans le systeme",
};

/** Texte en minuscules sans accents, avec la position d'origine de chaque caractère. */
type Folded = { text: string; source: string; at: number[] };

function fold(source: string): Folded {
  let text = "";
  const at: number[] = [];
  let i = 0;
  for (const ch of source) {
    const f = ch.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase().replace(/[’`]/g, "'");
    for (let k = 0; k < f.length; k++) at.push(i);
    text += f;
    i += ch.length;
  }
  at.push(source.length);
  return { text, source, at };
}

/** Extrait du texte d'origine correspondant à [start, end) du texte replié. */
const excerpt = (f: Folded, start: number, end: number) =>
  f.source
    .slice(f.at[start], f.at[end] ?? f.source.length)
    .trim()
    .replace(/\s+/g, " ");

/** Valeurs des champs « Nom : valeur » (une ligne, ou jusqu'à un séparateur). */
function fields(f: Folded, names: string): { value: string; start: number }[] {
  const re = new RegExp(
    `(?:^|[\\n|•;])[ \\t]*[-*•>]?[ \\t]*[*_]*[ \\t]*(?:${names})[ \\t]*[*_]*[ \\t]*(?:[:=–—]|\\s-\\s)[ \\t]*[*_]*[ \\t]*([^\\n|•;]*)`,
    "gu",
  );
  const out: { value: string; start: number }[] = [];
  for (const mt of f.text.matchAll(re)) {
    const value = mt[1] ?? "";
    const start = (mt.index ?? 0) + mt[0].length - value.length;
    if (value.trim()) out.push({ value, start });
  }
  return out;
}

function groupOf(age: number): AgeGroup | null {
  if (age > 150) return null;
  if (age < 13) return "child";
  if (age < 18) return "teen";
  return "adult";
}

function detectAge(f: Folded): { group: AgeGroup; reason: string } | null {
  for (const { value, start } of fields(f, FIELD_NAMES.age)) {
    if (/fluide?|fluid|slider|inconnu|unknown|variable|\?/u.test(value)) return null;
    const n = /\d{1,3}/u.exec(value);
    if (n) {
      const group = groupOf(Number(n[0]));
      if (group) return { group, reason: excerpt(f, start, start + value.trimEnd().length) };
    }
    const kw = ageKeyword(value);
    if (kw) return { group: kw, reason: excerpt(f, start, start + value.trimEnd().length) };
  }
  for (const mt of f.text.matchAll(AGE_NUMBER)) {
    const idx = mt.index ?? 0;
    if (DURATION_BEFORE.test(f.text.slice(Math.max(0, idx - 20), idx))) continue;
    if (DURATION_AFTER.test(f.text.slice(idx + mt[0].length, idx + mt[0].length + 15))) continue;
    const group = groupOf(Number(mt[1]));
    if (group) return { group, reason: excerpt(f, idx, idx + mt[0].length) };
  }
  return null;
}

function ageKeyword(value: string): AgeGroup | null {
  if (new RegExp(word("enfants?|child|kid|bebe|baby|toddler|littles?"), "u").test(value))
    return "child";
  if (new RegExp(word("ados?|adolescente?s?|teen(?:ager)?s?"), "u").test(value)) return "teen";
  if (new RegExp(word("adultes?|adults?"), "u").test(value)) return "adult";
  return null;
}

function genderIn(value: string): Gender[] {
  return (Object.keys(GENDER_PATTERNS) as Gender[]).filter((g) =>
    new RegExp(word(GENDER_PATTERNS[g]), "u").test(value),
  );
}

function detectGender(f: Folded): { gender: Gender; reason: string } | null {
  for (const { value, start } of fields(f, FIELD_NAMES.gender)) {
    const found = genderIn(value);
    // Un genre non binaire l'emporte sur les mots binaires qui l'accompagnent.
    const special = found.filter((g) => GENDER_ANYWHERE.includes(g));
    const pick = special.length === 1 ? special : found;
    if (pick.length === 1 && pick[0])
      return { gender: pick[0], reason: excerpt(f, start, start + value.trimEnd().length) };
  }
  const found: { gender: Gender; reason: string }[] = [];
  for (const g of GENDER_ANYWHERE) {
    const mt = new RegExp(word(GENDER_PATTERNS[g]), "u").exec(f.text);
    if (mt) found.push({ gender: g, reason: excerpt(f, mt.index, mt.index + mt[0].length) });
  }
  if (found.length === 1) return found[0] ?? null;
  if (found.length > 1) return null;
  // Binaires : seulement dans une tournure qui décrit le membre lui-même.
  const binary: { gender: Gender; reason: string }[] = [];
  for (const g of ["female", "male"] as const) {
    const w = GENDER_PATTERNS[g];
    const res = [
      `${B}(?:est|suis|c'est|is|i'm|im)\\s+(?:une?|a|an)?\\s*(?:jeune|petite?|grande?|young|little)?\\s*(?:${w})${E}`,
      `^\\s*[*_]*\\s*(?:une?\\s+)?(?:${w})${E}`,
      `${B}(?:${w})\\s*(?:de|,|-)?\\s*\\d{1,3}\\s*(?:${AGE_UNIT})${E}`,
      `${B}\\d{1,3}\\s*(?:${AGE_UNIT})\\s*(?:,|-)?\\s*(?:${w})${E}`,
      `${B}(?:${w})\\s+trans${E}`,
    ];
    for (const r of res) {
      const mt = new RegExp(r, "u").exec(f.text);
      if (mt) {
        binary.push({ gender: g, reason: excerpt(f, mt.index, mt.index + mt[0].length) });
        break;
      }
    }
  }
  return binary.length === 1 ? (binary[0] ?? null) : null;
}

const ROLE_CONTEXT = `${B}(?:est|suis|c'est|comme|en tant que|is|i'm|im|as)\\s+(?:l'|le|la|les|un|une|a|an|the|notre|our|my|mon|ma)?\\s*(?:[\\p{L}-]+\\s+)?`;

function detectRoles(
  f: Folded,
  catalog: string[],
  labels: Labels,
): { label: string; key: RoleKey | null; reason: string }[] {
  const out: { label: string; key: RoleKey | null; reason: string }[] = [];
  const add = (label: string, key: RoleKey | null, reason: string) => {
    const k = fold(label).text;
    if (!out.some((o) => (key && o.key === key) || fold(o.label).text === k))
      out.push({ label, key, reason });
  };
  const labelFor = (key: RoleKey) => {
    const re = new RegExp(`^\\s*(?:${ROLE_PATTERNS[key]})\\s*$`, "u");
    return catalog.find((c) => re.test(fold(c).text)) ?? labels.roles[key];
  };
  const knownIn = (text: string) =>
    (Object.keys(ROLE_PATTERNS) as RoleKey[]).filter((k) =>
      new RegExp(word(ROLE_PATTERNS[k]), "u").test(text),
    );

  // 1. Champ « Rôle : ... » : rôles connus, sinon le libellé tel qu'écrit.
  for (const { value, start } of fields(f, FIELD_NAMES.role)) {
    let pos = start;
    for (const part of value.split(/,|\/|&|\+|\bet\b|\band\b/u)) {
      const s = pos;
      pos += part.length + 1;
      const t = part.trim();
      if (!t) continue;
      const lead = s + part.indexOf(t);
      const reason = excerpt(f, lead, lead + t.length);
      const known = knownIn(t);
      const cat = catalog.find((c) => fold(c).text === t);
      if (cat) add(cat, null, reason);
      else if (known.length) for (const k of known) add(labelFor(k), k, reason);
      else if (t.length <= 40 && t.split(/\s+/u).length <= 4) {
        const raw = excerpt(f, lead, lead + t.length).replace(/[*_.]+$/u, "");
        add(raw.charAt(0).toLocaleUpperCase() + raw.slice(1), null, reason);
      }
    }
  }
  // 2. Tournures « c'est une protectrice », « is a gatekeeper ».
  for (const k of Object.keys(ROLE_PATTERNS) as RoleKey[]) {
    const mt = new RegExp(`${ROLE_CONTEXT}(?:${ROLE_PATTERNS[k]})${E}`, "u").exec(f.text);
    if (mt) add(labelFor(k), k, excerpt(f, mt.index, mt.index + mt[0].length));
    // Description qui commence par le rôle : « Little, aime les peluches ».
    const lead = new RegExp(
      `^\\s*[*_]*\\s*(?:un|une|a|an|le|la|l')?\\s*(?:${ROLE_PATTERNS[k]})${E}`,
      "u",
    ).exec(f.text);
    if (lead) add(labelFor(k), k, excerpt(f, lead.index, lead.index + lead[0].length));
  }
  for (const c of catalog) {
    const t = fold(c).text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    if (!t) continue;
    const mt = new RegExp(`${ROLE_CONTEXT}${t}${E}`, "u").exec(f.text);
    if (mt) add(c, null, excerpt(f, mt.index, mt.index + mt[0].length));
  }
  return out;
}

/** Le membre porte-t-il déjà ce rôle (même libellé, ou même rôle connu sous un autre nom) ? */
function hasRole(member: SortMember, label: string, key: RoleKey | null) {
  return member.roles.some((r) => {
    const t = fold(r).text;
    if (t === fold(label).text) return true;
    return key !== null && new RegExp(`^\\s*(?:${ROLE_PATTERNS[key]})\\s*$`, "u").test(t);
  });
}

export function analyze(
  members: SortMember[],
  folders: { id: string; name: string; members: string[] }[],
  catalog: string[],
  labels: Labels,
): Proposal[] {
  const props = new Map<string, Proposal>();
  const push = (kind: Proposal["kind"], label: string, hit: Hit) => {
    const key = `${kind}:${fold(label).text}`;
    const found = props.get(key);
    const existing =
      kind === "role" ? undefined : folders.find((x) => fold(x.name).text === fold(label).text);
    const p = found ?? { kind, label: existing?.name ?? label, folderId: existing?.id, hits: [] };
    if (!found) props.set(key, p);
    if (kind !== "role") {
      if (existing?.members.includes(hit.id)) return;
    }
    p.hits.push(hit);
  };

  for (const mb of members) {
    if (!mb.description?.trim()) continue;
    const f = fold(mb.description);
    const roles = detectRoles(f, catalog, labels);
    const age = detectAge(f);
    if (age) push("age", labels.age[age.group], { id: mb.id, name: mb.name, reason: age.reason });
    else {
      const little = roles.find((r) => r.key === "little");
      if (little)
        push("age", labels.age.child, { id: mb.id, name: mb.name, reason: little.reason });
    }
    const gender = detectGender(f);
    if (gender)
      push("gender", labels.gender[gender.gender], {
        id: mb.id,
        name: mb.name,
        reason: gender.reason,
      });
    for (const r of roles) {
      if (!hasRole(mb, r.label, r.key))
        push("role", r.label, { id: mb.id, name: mb.name, reason: r.reason });
    }
  }
  const order = { age: 0, gender: 1, role: 2 };
  const ageOrder = Object.values(labels.age);
  return [...props.values()]
    .filter((p) => p.hits.length > 0)
    .sort(
      (a, b) =>
        order[a.kind] - order[b.kind] ||
        (a.kind === "age" ? ageOrder.indexOf(a.label) - ageOrder.indexOf(b.label) : 0) ||
        a.label.localeCompare(b.label, "fr"),
    );
}
