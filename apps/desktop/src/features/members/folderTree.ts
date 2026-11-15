import type { Folder } from "../../ipc";

export type FolderNode = { folder: Folder; depth: number; path: string };

export type FolderTree = {
  byId: Map<string, Folder>;
  /** Enfants de chaque dossier ; la clé `""` donne la racine. */
  children: Map<string, Folder[]>;
  /** Parcours en profondeur, dans l'ordre d'affichage. */
  flat: FolderNode[];
  parentOf: (id: string) => string | null;
};

export const SEP = " › ";

/**
 * Arbre des dossiers. Un parent introuvable ou une boucle (possible le temps d'une
 * synchronisation entre deux appareils) ramène le dossier à la racine.
 */
export function buildTree(folders: Folder[]): FolderTree {
  const byId = new Map(folders.map((f) => [f.id, f]));
  const parentOf = (id: string): string | null => {
    const p = byId.get(id)?.parentId ?? null;
    if (!p || !byId.has(p)) return null;
    const seen = new Set([id]);
    for (let cur: string | null = p; cur; cur = byId.get(cur)?.parentId ?? null) {
      if (seen.has(cur)) return null;
      seen.add(cur);
      if (!byId.has(cur)) break;
    }
    return p;
  };
  const children = new Map<string, Folder[]>();
  for (const f of folders) {
    const key = parentOf(f.id) ?? "";
    children.set(key, [...(children.get(key) ?? []), f]);
  }
  const flat: FolderNode[] = [];
  const walk = (key: string, depth: number, prefix: string) => {
    for (const f of children.get(key) ?? []) {
      const path = prefix ? `${prefix}${SEP}${f.name}` : f.name;
      flat.push({ folder: f, depth, path });
      walk(f.id, depth + 1, path);
    }
  };
  walk("", 0, "");
  return { byId, children, flat, parentOf };
}

/** Le dossier et tous ses sous-dossiers. */
export function subtree(tree: FolderTree, id: string): string[] {
  const out: string[] = [];
  const walk = (k: string) => {
    out.push(k);
    for (const c of tree.children.get(k) ?? []) walk(c.id);
  };
  walk(id);
  return out;
}

/** Membres rangés dans le dossier ou l'un de ses sous-dossiers. */
export function membersUnder(tree: FolderTree, id: string): Set<string> {
  const out = new Set<string>();
  for (const k of subtree(tree, id)) for (const x of tree.byId.get(k)?.members ?? []) out.add(x);
  return out;
}

/** Chemin de la racine jusqu'au dossier inclus. */
export function ancestry(tree: FolderTree, id: string): string[] {
  const out: string[] = [];
  for (let cur: string | null = id; cur; cur = tree.parentOf(cur)) out.unshift(cur);
  return out;
}

export const pathOf = (tree: FolderTree, id: string) =>
  tree.flat.find((n) => n.folder.id === id)?.path ?? tree.byId.get(id)?.name ?? "";
