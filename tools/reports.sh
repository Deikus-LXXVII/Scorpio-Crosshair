#!/bin/sh
# Reads Scorpio problem reports on the VPS.
# Usage: SCORPIO_SITE_HOST=<ssh host> tools/reports.sh list [--all] | show SCR-XXXXXX | close SCR-XXXXXX
set -eu
: "${SCORPIO_SITE_HOST:?set SCORPIO_SITE_HOST to the ssh host of the web server}"
for arg in "$@"; do
	case "$arg" in list | show | close | --all | [Ss][Cc][Rr]-[A-Za-z0-9][A-Za-z0-9][A-Za-z0-9][A-Za-z0-9][A-Za-z0-9][A-Za-z0-9]) ;; *) echo "unknown argument: $arg" >&2; exit 2 ;; esac
done
exec ssh "$SCORPIO_SITE_HOST" runuser -u scorpio-reports -- python3 -I /opt/scorpio-reports/reports.py --db /var/lib/scorpio-reports/reports.db "$@"
