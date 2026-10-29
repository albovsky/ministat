#!/bin/sh
# Run as the target user. This stages the exact boot-service candidate, without sudo.
set -eu
if [ "$(id -u)" -eq 0 ]; then echo 'Run install.sh as your normal user.' >&2; exit 1; fi
PROJECT_ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
STATE="$HOME/Library/Application Support/Ministat"
APP="$HOME/Applications/Ministat.app"
SERVICE_USER=$(id -un)
SERVICE_UID=$(id -u)
test -x "$PROJECT_ROOT/dist/Ministat.app/Contents/MacOS/ministat"
umask 077
mkdir -p "$HOME/Applications" "$STATE/install" "$STATE/bin" "$HOME/.local/bin" "$HOME/Library/LaunchAgents"
chmod 700 "$STATE" "$STATE/install" "$STATE/bin"
BACKUP="$STATE/rollback/$(date +%Y%m%d-%H%M%S)-$$"
mkdir -p "$BACKUP"
/usr/bin/ditto --norsrc --noextattr "$PROJECT_ROOT/dist/Ministat.app" "$HOME/Applications/Ministat.next.app"
/usr/bin/xattr -cr "$HOME/Applications/Ministat.next.app"
/usr/bin/codesign --verify --strict "$HOME/Applications/Ministat.next.app"
cp "$HOME/Applications/Ministat.next.app/Contents/MacOS/ministat" "$STATE/bin/.ministat.next"
# Retain old executable inodes and a rollback candidate, then replace paths atomically.
if [ -d "$APP" ]; then mv "$APP" "$BACKUP/Ministat.app"; fi
if [ -f "$STATE/bin/ministat" ]; then cp -p "$STATE/bin/ministat" "$BACKUP/ministat"; fi
if [ -f "$HOME/Library/LaunchAgents/com.albovsky.ministat.menu.plist" ]; then
    cp -p "$HOME/Library/LaunchAgents/com.albovsky.ministat.menu.plist" "$BACKUP/menu.plist"
fi
mv "$HOME/Applications/Ministat.next.app" "$APP"
mv "$STATE/bin/.ministat.next" "$STATE/bin/ministat"
if [ -e "$HOME/.local/bin/ministat" ] && [ ! -L "$HOME/.local/bin/ministat" ]; then
    echo 'Existing ~/.local/bin/ministat is not a symlink; preserve it and choose another CLI path.' >&2; exit 1
fi
ln -sfn "$STATE/bin/ministat" "$HOME/.local/bin/ministat"
export MINISTAT_INSTALL_STATE="$STATE" MINISTAT_INSTALL_APP="$APP" MINISTAT_INSTALL_USER="$SERVICE_USER" MINISTAT_INSTALL_UID="$SERVICE_UID"
# plist generation uses the OS plist serializer to escape all account/path characters.
/usr/bin/python3 - <<'PY'
import os, pathlib, plistlib
state = pathlib.Path(os.environ['MINISTAT_INSTALL_STATE'])
app = os.environ['MINISTAT_INSTALL_APP']
user = os.environ['MINISTAT_INSTALL_USER']
uid = int(os.environ['MINISTAT_INSTALL_UID'])
home = str(pathlib.Path.home())
base = {
 'ProgramArguments': [str(state/'bin/ministat'), 'collect', '--state-dir', str(state)],
 'EnvironmentVariables': {'HOME':home, 'PATH':'/usr/bin:/bin:/usr/sbin:/sbin'},
 'RunAtLoad':True, 'KeepAlive':True, 'ThrottleInterval':10,
 'StandardOutPath':str(state/'collector.log'), 'StandardErrorPath':str(state/'collector.log'),
 'ProcessType':'Background', 'LowPriorityIO':True,
}
daemon = {**base, 'Label':'com.albovsky.ministat.collector', 'UserName':user, 'GroupName':'staff'}
preview = {**base, 'Label':'com.albovsky.ministat.preview'}
menu = {'Label':'com.albovsky.ministat.menu', 'RunAtLoad':True,
 'ProgramArguments':['/usr/bin/open','-a',app]}
for name, value in [('collector',daemon),('preview',preview),('menu',menu)]:
 (state/'install'/f'{name}.plist').write_bytes(plistlib.dumps(value))
(state/'install'/'owner').write_text(f'{user}\n{uid}\n')
PY
cp "$PROJECT_ROOT/scripts/install-boot-service.sh" "$STATE/install-boot-service.sh"
chmod 700 "$STATE/install-boot-service.sh"
cp "$STATE/install/menu.plist" "$HOME/Library/LaunchAgents/com.albovsky.ministat.menu.plist"
launchctl bootout "gui/$SERVICE_UID/com.albovsky.ministat.menu" 2>/dev/null || true
# Existing menu processes must exit so open launches the newly installed release.
/usr/bin/pkill -f "^$APP/Contents/MacOS/ministat" 2>/dev/null || true
launchctl bootstrap "gui/$SERVICE_UID" "$HOME/Library/LaunchAgents/com.albovsky.ministat.menu.plist"
if launchctl print system/com.albovsky.ministat.collector >/dev/null 2>&1; then
    # The process belongs to this user; launchd KeepAlive starts the new binary.
    /usr/bin/pkill -f "^$STATE/bin/ministat collect" 2>/dev/null || true
    printf '\nApp and CLI updated. The system boot collector is enabled and will restart automatically.\n'
else
    launchctl bootout "gui/$SERVICE_UID/com.albovsky.ministat.preview" 2>/dev/null || true
    launchctl bootstrap "gui/$SERVICE_UID" "$STATE/install/preview.plist"
    printf '\nApp and CLI installed. To enable collection before login, run:\n\n  sudo "%s/install-boot-service.sh"\n\n' "$STATE"
fi
