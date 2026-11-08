#!/bin/sh
# Run on the administration Mac, never on the monitored host.
#
# The root helper comes from GitHub at a full commit SHA you have reviewed, not from
# the monitored account's writable checkout. Its bytes travel inside the SSH command,
# root writes them to a private root-owned directory, checks the digest computed here,
# and runs that copy for the account sudo reports (SUDO_USER/SUDO_UID).
set -eu
MODE=install
MERGED_ONLY=1
while [ $# -gt 0 ]; do
    case "$1" in
        --remove) MODE=remove; shift ;;
        --unmerged) MERGED_ONLY=0; shift ;;
        --) shift; break ;;
        -*) echo "Unknown option: $1" >&2; exit 2 ;;
        *) break ;;
    esac
done
if [ $# -ne 2 ]; then
    echo 'Usage: admin-boot-service.sh [--remove] [--unmerged] <ssh-host> <full-commit-sha>' >&2
    exit 2
fi
HOST=$1
REF=$2
REPO=${MINISTAT_REPO:-albovsky/ministat}
GH=${MINISTAT_GH:-gh}
SSH=${MINISTAT_SSH:-ssh}
case "$REF" in
    *[!0-9a-f]*) echo 'The commit must be a full lowercase 40-character SHA.' >&2; exit 2 ;;
esac
if [ "${#REF}" -ne 40 ]; then
    echo 'The commit must be a full lowercase 40-character SHA.' >&2
    exit 2
fi
case "$HOST" in -*) echo 'The SSH host must not start with "-".' >&2; exit 2 ;; esac

if [ "$MERGED_ONLY" -eq 1 ]; then
    BRANCH=$("$GH" api "repos/$REPO" --jq .default_branch)
    STATUS=$("$GH" api "repos/$REPO/compare/$BRANCH...$REF" --jq .status)
    case "$STATUS" in
        identical|behind) ;;
        *) echo "Commit $REF is not on $BRANCH (compare status: $STATUS). Pass --unmerged to deploy it anyway." >&2; exit 1 ;;
    esac
else
    echo "Warning: deploying $REF without checking that it is merged." >&2
fi

WORK=$(/usr/bin/mktemp -d "${TMPDIR:-/tmp}/ministat-admin.XXXXXXXX")
trap '/bin/rm -rf "$WORK"' EXIT HUP INT TERM
"$GH" api -H 'Accept: application/vnd.github.raw' \
    "repos/$REPO/contents/scripts/install-boot-service.sh?ref=$REF" > "$WORK/helper.sh"
if [ ! -s "$WORK/helper.sh" ]; then
    echo 'GitHub returned an empty helper.' >&2
    exit 1
fi
DIGEST=$(/usr/bin/shasum -a 256 "$WORK/helper.sh")
DIGEST=${DIGEST%% *}
printf 'Helper: %s/blob/%s/scripts/install-boot-service.sh\nSHA-256: %s\nMode: %s on %s\n' \
    "https://github.com/$REPO" "$REF" "$DIGEST" "$MODE" "$HOST" >&2

# This root script is fixed text; only the base64 payload, digest and mode vary.
# sudo resets the environment and sets SUDO_USER/SUDO_UID from the invoking account.
REMOTE=$(/usr/bin/python3 -I - "$WORK/helper.sh" "$DIGEST" "$MODE" <<'PY'
import base64, pathlib, shlex, sys
wrapper = '''set -eu
umask 077
stage=$(/usr/bin/mktemp -d /private/var/root/ministat.XXXXXXXX)
trap '/bin/rm -rf "$stage"' EXIT HUP INT TERM
printf '%s' "$1" | /usr/bin/base64 -D > "$stage/installer.sh"
digest=$(/usr/bin/shasum -a 256 "$stage/installer.sh")
if [ "${digest%% *}" != "$2" ]; then
    echo 'Ministat helper did not arrive intact; nothing was run.' >&2
    exit 1
fi
/bin/chmod 500 "$stage/installer.sh"
/bin/sh "$stage/installer.sh" "${SUDO_USER:?}" "${SUDO_UID:?}" "$3"'''
payload = base64.b64encode(pathlib.Path(sys.argv[1]).read_bytes()).decode()
print(shlex.join(['/usr/bin/sudo', '/bin/sh', '-c', wrapper, 'ministat-admin',
                  payload, sys.argv[2], sys.argv[3]]))
PY
)
exec "$SSH" -t -- "$HOST" "$REMOTE"
