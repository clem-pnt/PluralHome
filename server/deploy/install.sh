#!/bin/sh
# Installe/MAJ le serveur de synchro sur le VPS (à lancer avec sudo, depuis le dépôt).
set -eu
here=$(cd "$(dirname "$0")/.." && pwd)
id pluralhome-sync >/dev/null 2>&1 || useradd --system --no-create-home --shell /usr/sbin/nologin pluralhome-sync
install -d -m 0755 /opt/pluralhome-sync
install -m 0644 "$here"/index.mjs "$here"/app.mjs "$here"/store.mjs "$here"/package.json /opt/pluralhome-sync/
install -m 0644 "$here"/deploy/pluralhome-sync.service /etc/systemd/system/pluralhome-sync.service
systemctl daemon-reload
systemctl enable pluralhome-sync
systemctl restart pluralhome-sync
sleep 1
curl -fsS http://127.0.0.1:3190/health
