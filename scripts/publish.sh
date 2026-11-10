#!/usr/bin/env bash
# Publie une version de PluralHome sur updates.aragonite.fr.
#
#   scripts/publish.sh            build signé + dépôt du .deb + manifeste updater
#   scripts/publish.sh --no-build réutilise le dernier build
#
# Prérequis : version dans apps/desktop/package.json, section « ## X.Y.Z » dans
# CHANGELOG.md, clé de signature dans ~/.config/pluralhome/updater.key (600).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export PATH="/home/clem/.nvm/versions/node/v22.23.4/bin:$PATH"
KEY_FILE="${PLURALHOME_UPDATER_KEY:-$HOME/.config/pluralhome/updater.key}"
PUBLIC_BASE="https://updates.aragonite.fr/pluralhome"
FILES_DIR="/var/lib/aragonite-updates/pluralhome"
MANIFEST_DIR="/var/www/updates.aragonite.fr/pluralhome"

VERSION="$(node -p "require('$ROOT/apps/desktop/package.json').version")"
TAG="v$VERSION"
log() { printf '\033[1m%s\033[0m\n' "$*"; }
die() { printf 'erreur : %s\n' "$*" >&2; exit 1; }

grep -q "^## $VERSION " "$ROOT/CHANGELOG.md" || die "pas de section « ## $VERSION » dans CHANGELOG.md"
[[ -r "$KEY_FILE" ]] || die "clé de signature introuvable : $KEY_FILE"

if [[ "${1:-}" != "--no-build" ]]; then
  log "Build signé $TAG"
  (cd "$ROOT/apps/desktop" &&
    TAURI_SIGNING_PRIVATE_KEY="$(cat "$KEY_FILE")" \
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${PLURALHOME_UPDATER_KEY_PASSWORD:-}" \
    pnpm tauri build)
fi

BUNDLE="$ROOT/target/release/bundle/deb"
SRC_DEB="$(ls "$BUNDLE"/PluralHome_"${VERSION}"_amd64.deb)"
[[ -s "$SRC_DEB.sig" ]] || die "signature absente : $SRC_DEB.sig"
DEB_NAME="pluralhome_${VERSION}_amd64.deb"

log "Dépôt de $DEB_NAME"
sudo install -D -m 0644 "$SRC_DEB" "$FILES_DIR/$TAG/$DEB_NAME"
sudo install -D -m 0644 "$SRC_DEB.sig" "$FILES_DIR/$TAG/$DEB_NAME.sig"
(cd "$FILES_DIR/$TAG" && sha256sum "$DEB_NAME" | sudo tee SHA256SUMS >/dev/null)

log "Manifeste updater"
mkdir -p "$MANIFEST_DIR"
TMP="$(mktemp "$MANIFEST_DIR/.latest.XXXXXX")"
node - "$ROOT/CHANGELOG.md" "$VERSION" "$SRC_DEB.sig" "$PUBLIC_BASE/download/$TAG/$DEB_NAME" > "$TMP" <<'JS'
const fs = require("node:fs");
const [changelog, version, sigFile, url] = process.argv.slice(2);
const lines = fs.readFileSync(changelog, "utf8").split("\n");
const start = lines.findIndex((l) => l.startsWith(`## ${version} `));
const end = lines.findIndex((l, i) => i > start && l.startsWith("## "));
const notes = lines.slice(start + 1, end === -1 ? undefined : end).join("\n").trim();
const entry = { signature: fs.readFileSync(sigFile, "utf8").trim(), url };
process.stdout.write(JSON.stringify({
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: { "linux-x86_64-deb": entry, "linux-x86_64": entry },
}, null, 2) + "\n");
JS
chmod 644 "$TMP"
mv "$TMP" "$MANIFEST_DIR/latest.json"

log "Vérification en ligne"
curl -fsS "$PUBLIC_BASE/latest.json" | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>console.log("latest.json :",JSON.parse(s).version))'
curl -fsSI "$PUBLIC_BASE/download/$TAG/$DEB_NAME" | head -1
log "Publié : $TAG"
