#!/bin/sh
# Print a fixed-checksum administrator command; never execute the home copy as root.
set -eu
if [ "$(/usr/bin/id -u)" -eq 0 ]; then echo 'Run this command as your normal account, without sudo.' >&2; exit 1; fi
MODE=install
case "${1:-}" in '') ;; --remove) MODE=remove ;; *) echo 'Usage: boot-service-command.sh [--remove]' >&2; exit 1 ;; esac
PROJECT_ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
/usr/bin/python3 -I - "$PROJECT_ROOT" "$HOME" "$MODE" <<'PY'
import hashlib, os, pathlib, pwd, shlex, sys
account = pwd.getpwuid(os.getuid())
if account.pw_uid < 501:
    raise SystemExit('Expected a normal local account')
state = pathlib.Path(sys.argv[2]) / 'Library/Application Support/Ministat'
destination = state / 'install/boot-service-candidate.sh'
destination.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
payload = (pathlib.Path(sys.argv[1]) / 'scripts/install-boot-service.sh').read_bytes()
destination.write_bytes(payload)
destination.chmod(0o600)
digest = hashlib.sha256(payload).hexdigest()
# Copy first, then verify and execute that same copy. A replacement or edit of the
# user-owned candidate after this command was printed must fail the checksum.
wrapper = '''set -eu
umask 077
stage=$(/usr/bin/mktemp -d /private/var/root/ministat.XXXXXXXX)
trap '/bin/rm -rf "$stage"' EXIT HUP INT TERM
/bin/cp "$1" "$stage/installer.sh"
digest=$(/usr/bin/shasum -a 256 "$stage/installer.sh")
if [ "${digest%% *}" != "$2" ]; then
    echo 'Ministat installer changed; regenerate and inspect the command.' >&2
    exit 1
fi
/bin/chmod 500 "$stage/installer.sh"
/bin/sh "$stage/installer.sh" "$3" "$4" "$5"'''
print(shlex.join(['/usr/bin/sudo', '/bin/sh', '-c', wrapper, 'ministat-admin',
    str(destination), digest, account.pw_name, str(account.pw_uid), sys.argv[3]]))
PY
