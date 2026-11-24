#!/bin/sh
# Installs or updates the report service on the VPS and reloads Caddy.
# Usage: SCORPIO_SITE_HOST=<ssh host> server/reports/deploy.sh
# Caddy is shared with other sites: the new site is validated before the
# live configuration changes, and any failure puts the old files back.
set -eu
: "${SCORPIO_SITE_HOST:?set SCORPIO_SITE_HOST to the ssh host of the web server}"
cd "$(dirname "$0")"
python3 -B -m unittest -q test_reports
COPYFILE_DISABLE=1 tar --no-xattrs -czf - reports.py reports.caddy scorpio-reports.service |
	ssh -- "$SCORPIO_SITE_HOST" 'set -eu
		new=$(mktemp -d)
		old=$(mktemp -d)
		restore() {
			# A first deploy that failed: stop and disable while the unit file still exists.
			if [ -e "$old/_etc_systemd_system_scorpio-reports.service.none" ]; then
				systemctl disable --now scorpio-reports >/dev/null 2>&1 || true
			fi
			for file in /etc/caddy/Caddyfile /etc/caddy/reports.caddy /opt/scorpio-reports/reports.py /etc/systemd/system/scorpio-reports.service; do
				saved="$old/$(echo "$file" | tr / _)"
				if [ -e "$saved" ]; then cp -p "$saved" "$file"; elif [ -e "$saved.none" ]; then rm -f "$file"; fi
			done
			systemctl daemon-reload
			[ -e /etc/systemd/system/scorpio-reports.service ] && systemctl restart scorpio-reports 2>/dev/null || true
			systemctl reload caddy || true
			echo "deploy failed; the previous files are back" >&2
		}
		trap "rm -rf \"$new\" \"$old\"" EXIT
		tar -xzf - -C "$new"
		for file in /etc/caddy/Caddyfile /etc/caddy/reports.caddy /opt/scorpio-reports/reports.py /etc/systemd/system/scorpio-reports.service; do
			saved="$old/$(echo "$file" | tr / _)"
			if [ -e "$file" ]; then cp -p "$file" "$saved"; else touch "$saved.none"; fi
		done

		# The new site is checked with a copy of the live configuration first.
		sed "s#^import /etc/caddy/reports.caddy\$#import $new/reports.caddy#" /etc/caddy/Caddyfile > "$new/Caddyfile"
		grep -qxF "import $new/reports.caddy" "$new/Caddyfile" || printf "\nimport %s\n" "$new/reports.caddy" >> "$new/Caddyfile"
		if ! checked=$(cd /etc/caddy && caddy validate --config "$new/Caddyfile" --adapter caddyfile 2>&1); then
			echo "$checked" | tail -5 >&2
			exit 1
		fi

		# From here on the live files change; any failure puts them back.
		trap "restore; rm -rf \"$new\" \"$old\"" EXIT
		id scorpio-reports >/dev/null 2>&1 || useradd --system --no-create-home --home-dir /nonexistent --shell /usr/sbin/nologin scorpio-reports
		install -d -m 0755 /opt/scorpio-reports
		install -m 0644 "$new/reports.py" /opt/scorpio-reports/reports.py
		install -m 0644 "$new/scorpio-reports.service" /etc/systemd/system/scorpio-reports.service
		systemctl daemon-reload
		systemctl enable scorpio-reports >/dev/null 2>&1
		systemctl restart scorpio-reports
		for try in 1 2 3 4 5 6 7 8 9 10; do
			curl -fsS http://127.0.0.1:8798/v1/health >/dev/null 2>&1 && break
			[ "$try" = 10 ] && { echo "the service did not answer" >&2; exit 1; }
			sleep 0.5
		done

		install -m 0644 "$new/reports.caddy" /etc/caddy/reports.caddy
		grep -qxF "import /etc/caddy/reports.caddy" /etc/caddy/Caddyfile || printf "\nimport /etc/caddy/reports.caddy\n" >> /etc/caddy/Caddyfile
		systemctl reload caddy
		trap - EXIT
		rm -rf "$new" "$old"
		curl -fsS http://127.0.0.1:8798/v1/health'
echo
echo "deployed: https://reports.lxxvll.com/v1/reports"
