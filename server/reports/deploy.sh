#!/bin/sh
# Installs or updates the report service on the VPS and reloads Caddy.
# Usage: SCORPIO_SITE_HOST=<ssh host> server/reports/deploy.sh
set -eu
: "${SCORPIO_SITE_HOST:?set SCORPIO_SITE_HOST to the ssh host of the web server}"
cd "$(dirname "$0")"
python3 -m unittest -q test_reports
COPYFILE_DISABLE=1 tar --no-xattrs -czf - reports.py reports.caddy scorpio-reports.service |
	ssh "$SCORPIO_SITE_HOST" 'set -e
		new=$(mktemp -d); tar -xzf - -C "$new"
		install -d -m 0755 /opt/scorpio-reports
		install -m 0644 "$new/reports.py" /opt/scorpio-reports/reports.py
		install -m 0644 "$new/scorpio-reports.service" /etc/systemd/system/scorpio-reports.service
		id scorpio-reports >/dev/null 2>&1 || useradd --system --no-create-home --home-dir /nonexistent --shell /usr/sbin/nologin scorpio-reports
		caddy_backup=$(mktemp); cp /etc/caddy/Caddyfile "$caddy_backup"
		had_site=no; [ -f /etc/caddy/reports.caddy ] && { had_site=yes; cp /etc/caddy/reports.caddy "$caddy_backup.site"; }
		install -m 0644 "$new/reports.caddy" /etc/caddy/reports.caddy
		grep -qx "import /etc/caddy/reports.caddy" /etc/caddy/Caddyfile || printf "\nimport /etc/caddy/reports.caddy\n" >> /etc/caddy/Caddyfile
		rm -rf "$new"
		if ! caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile >/dev/null 2>&1; then
			cp "$caddy_backup" /etc/caddy/Caddyfile
			if [ $had_site = yes ]; then cp "$caddy_backup.site" /etc/caddy/reports.caddy; else rm -f /etc/caddy/reports.caddy; fi
			echo "the Caddy configuration did not validate; the old one is back" >&2; exit 1
		fi
		rm -f "$caddy_backup" "$caddy_backup.site"
		systemctl daemon-reload
		systemctl enable scorpio-reports >/dev/null 2>&1
		systemctl restart scorpio-reports
		systemctl reload caddy
		sleep 1
		curl -fsS http://127.0.0.1:8798/v1/health'
echo
echo "deployed: https://reports.lxxvll.com/v1/reports"
