#!/bin/sh
set -eu
PROJECT_ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
MODE=${1:-run}
"$PROJECT_ROOT/scripts/build.sh"
APP="$PROJECT_ROOT/dist/Ministat.app"
# Match only this GUI executable, never the independent collector.
/usr/bin/pkill -f "^$APP/Contents/MacOS/ministat" 2>/dev/null || true
case "$MODE" in
  run) /usr/bin/open -n "$APP" ;;
  --preview) /usr/bin/open -n "$APP" --args menu --preview ;;
  --verify) /usr/bin/open -n "$APP" --args menu --preview
    sleep 1; /usr/bin/pgrep -f "^$APP/Contents/MacOS/ministat" >/dev/null ;;
  --debug) /usr/bin/lldb -- "$APP/Contents/MacOS/ministat" menu --preview ;;
  --logs|--telemetry) /usr/bin/open -n "$APP"
    /usr/bin/log stream --style compact --predicate 'process == "ministat"' ;;
  *) echo 'Usage: build_and_run.sh [--preview|--verify|--debug|--logs|--telemetry]' >&2; exit 2 ;;
esac
