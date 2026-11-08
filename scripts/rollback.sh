#!/bin/sh
set -eu
if [ "$(id -u)" -eq 0 ]; then echo 'Run rollback as the normal account.' >&2; exit 1; fi
STATE="$HOME/Library/Application Support/Ministat"
BACKUP=${1:?Pass the absolute rollback directory printed by ls "$HOME/Library/Application Support/Ministat/rollback"}
case "$BACKUP" in "$STATE"/rollback/*) ;; *) echo 'Choose a Ministat rollback directory.' >&2; exit 1 ;; esac
test -x "$BACKUP/Ministat.app/Contents/MacOS/ministat"
APP="$HOME/Applications/Ministat.app"
/usr/bin/ditto --norsrc --noextattr "$BACKUP/Ministat.app" "$HOME/Applications/Ministat.rollback.app"
/usr/bin/xattr -cr "$HOME/Applications/Ministat.rollback.app"
/usr/bin/codesign --verify --strict "$HOME/Applications/Ministat.rollback.app"
cp "$HOME/Applications/Ministat.rollback.app/Contents/MacOS/ministat" "$STATE/bin/.ministat.rollback"
CURRENT="$STATE/rollback/before-rollback-$(date +%Y%m%d-%H%M%S)-$$"
mkdir -p "$CURRENT"
mv "$APP" "$CURRENT/Ministat.app"
cp -p "$STATE/bin/ministat" "$CURRENT/ministat"
mv "$HOME/Applications/Ministat.rollback.app" "$APP"
mv "$STATE/bin/.ministat.rollback" "$STATE/bin/ministat"
/usr/bin/pkill -f "^$APP/Contents/MacOS/ministat" 2>/dev/null || true
/usr/bin/pkill -f "^$STATE/bin/ministat collect" 2>/dev/null || true
/usr/bin/open -a "$APP"
echo 'Previous release restored. The supervised collector will restart automatically.'
