#!/bin/sh
# Execute only from the checksum-verified, root-owned copy made by the printed command.
set -eu
if [ "$(/usr/bin/id -u)" -ne 0 ]; then
    echo 'Run ./scripts/boot-service-command.sh as your normal account, then use its printed command.' >&2
    exit 1
fi
SERVICE_USER=${1:?Missing account}
SERVICE_UID=${2:?Missing account ID}
MODE=${3:?Missing install/remove mode}
case "$MODE" in install|remove) ;; *) exit 1 ;; esac
STAGE=$(CDPATH='' cd -- "$(/usr/bin/dirname -- "$0")" && pwd)
PLIST=/Library/LaunchDaemons/com.albovsky.ministat.collector.plist
# Account details come from the OS. Never load a user-writable plist or owner file as root.
/usr/bin/python3 -I - "$SERVICE_USER" "$SERVICE_UID" "$STAGE/collector.plist" <<'PY'
import grp, pathlib, plistlib, pwd, sys
account = pwd.getpwnam(sys.argv[1])
if account.pw_uid != int(sys.argv[2]) or account.pw_uid < 501:
    raise SystemExit('Expected a normal local account with the specified UID')
home = pathlib.Path(account.pw_dir)
if not home.is_absolute():
    raise SystemExit('Account home must be absolute')
state = home / 'Library/Application Support/Ministat'
daemon = {
    'Label': 'com.albovsky.ministat.collector',
    'UserName': account.pw_name, 'GroupName': grp.getgrgid(account.pw_gid).gr_name,
    'ProgramArguments': [str(state/'bin/ministat'), 'collect', '--state-dir', str(state)],
    'EnvironmentVariables': {'HOME':str(home), 'PATH':'/usr/bin:/bin:/usr/sbin:/sbin'},
    'RunAtLoad':True, 'KeepAlive':True, 'ThrottleInterval':10,
    'StandardOutPath':str(state/'collector.log'), 'StandardErrorPath':str(state/'collector.log'),
    'ProcessType':'Background', 'LowPriorityIO':True,
}
pathlib.Path(sys.argv[3]).write_bytes(plistlib.dumps(daemon))
PY
/usr/bin/plutil -lint "$STAGE/collector.plist"
if [ "$MODE" = remove ]; then
    /bin/launchctl bootout system/com.albovsky.ministat.collector 2>/dev/null || true
    /bin/launchctl bootout "gui/$SERVICE_UID/com.albovsky.ministat.preview" 2>/dev/null || true
    if [ -f "$PLIST" ]; then /bin/mv "$PLIST" "$PLIST.disabled"; fi
    /bin/launchctl bootout "gui/$SERVICE_UID/com.albovsky.ministat.menu" 2>/dev/null || true
    # Disable login startup with that user's permissions, even when paths are symlinks.
    SERVICE_HOME=$(/usr/bin/python3 -I -c 'import pwd,sys; print(pwd.getpwnam(sys.argv[1]).pw_dir)' "$SERVICE_USER")
    MENU_PLIST="$SERVICE_HOME/Library/LaunchAgents/com.albovsky.ministat.menu.plist"
    if [ -f "$MENU_PLIST" ]; then
        /usr/bin/sudo -u "$SERVICE_USER" /bin/mv "$MENU_PLIST" "$MENU_PLIST.disabled"
    fi
    /usr/bin/sudo -u "$SERVICE_USER" /usr/bin/pkill -f "^$SERVICE_HOME/Applications/Ministat.app/Contents/MacOS/ministat" 2>/dev/null || true
    echo 'Boot collector and menu startup disabled. App, snapshots and rollback candidates retained.'
    exit 0
fi
/bin/launchctl bootout "gui/$SERVICE_UID/com.albovsky.ministat.preview" 2>/dev/null || true
/bin/launchctl bootout system/com.albovsky.ministat.collector 2>/dev/null || true
/usr/bin/install -o root -g wheel -m 644 "$STAGE/collector.plist" "$PLIST"
/bin/launchctl bootstrap system "$PLIST"
/bin/launchctl print system/com.albovsky.ministat.collector
printf '\nBoot collector installed; it runs as %s.\n' "$SERVICE_USER"
