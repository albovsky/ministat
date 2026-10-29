#!/bin/sh
# The only administrator step: install a non-root collector as a system boot job.
set -eu
if [ "$(id -u)" -ne 0 ]; then echo 'Run this exact installer with sudo.' >&2; exit 1; fi
STATE=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
SERVICE_USER=$(sed -n '1p' "$STATE/install/owner")
SERVICE_UID=$(sed -n '2p' "$STATE/install/owner")
test "$(id -u "$SERVICE_USER")" = "$SERVICE_UID"
test "$SERVICE_UID" -ge 501
APP=$(/usr/libexec/PlistBuddy -c 'Print :ProgramArguments:2' "$STATE/install/menu.plist")
PLIST=/Library/LaunchDaemons/com.albovsky.ministat.collector.plist
if [ "${1:-}" = --remove ]; then
    /bin/launchctl bootout system/com.albovsky.ministat.collector 2>/dev/null || true
    /bin/launchctl bootout "gui/$SERVICE_UID/com.albovsky.ministat.preview" 2>/dev/null || true
    if [ -f "$PLIST" ]; then /bin/mv "$PLIST" "$PLIST.disabled"; fi
    /bin/launchctl bootout "gui/$SERVICE_UID/com.albovsky.ministat.menu" 2>/dev/null || true
    SERVICE_HOME=$(dirname "$(dirname "$APP")")
    MENU_PLIST="$SERVICE_HOME/Library/LaunchAgents/com.albovsky.ministat.menu.plist"
    if [ -f "$MENU_PLIST" ]; then /bin/mv "$MENU_PLIST" "$STATE/install/menu-disabled.plist"; fi
    /usr/bin/sudo -u "$SERVICE_USER" /usr/bin/pkill -f "^$APP/Contents/MacOS/ministat" 2>/dev/null || true
    echo 'Boot collector and menu startup disabled. App, snapshots and rollback candidates retained.'
    exit 0
fi
/usr/bin/plutil -lint "$STATE/install/collector.plist"
test "$(/usr/libexec/PlistBuddy -c 'Print :UserName' "$STATE/install/collector.plist")" = "$SERVICE_USER"
test -x "$STATE/bin/ministat"
/bin/launchctl bootout "gui/$SERVICE_UID/com.albovsky.ministat.preview" 2>/dev/null || true
/bin/launchctl bootout system/com.albovsky.ministat.collector 2>/dev/null || true
/usr/bin/install -o root -g wheel -m 644 "$STATE/install/collector.plist" "$PLIST"
/bin/launchctl bootstrap system "$PLIST"
/bin/launchctl print system/com.albovsky.ministat.collector
printf '\nBoot collector installed; it runs as %s, not root.\n' "$SERVICE_USER"
