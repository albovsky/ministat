#!/bin/sh
# Explain how to enable the boot service. Nothing in this account's checkout is run as root.
set -eu
FLAG=
case "${1:-}" in '') ;; --remove) FLAG='--remove ' ;; *) echo 'Usage: boot-service-command.sh [--remove]' >&2; exit 1 ;; esac
HOST=$(/bin/hostname -s 2>/dev/null || echo 'this-host')
cat <<EOF
The boot service is installed from your administration Mac, not from this account.
In a ministat checkout there, run:

    ./scripts/admin-boot-service.sh ${FLAG}<ssh-host-for-${HOST}> <full-commit-sha>

Use a commit you have reviewed on GitHub. The helper is fetched from GitHub at that
commit and sent over SSH; this account's files are never executed as root.
EOF
