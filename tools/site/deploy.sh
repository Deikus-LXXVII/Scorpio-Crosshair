#!/bin/sh
# Builds the website and copies it to the web server.
# Usage: SCORPIO_SITE_HOST=<ssh host> tools/site/deploy.sh
# The server serves /var/www/scorpio as https://scorpio.lxxvll.com (Caddy).
set -eu
: "${SCORPIO_SITE_HOST:?set SCORPIO_SITE_HOST to the ssh host of the web server}"
cd "$(dirname "$0")/../.."
node tools/site/build.js
# tar over ssh: works with any rsync (or none) on either side.
COPYFILE_DISABLE=1 tar --no-xattrs -C docs/site --exclude '._*' --exclude '.DS_Store' -czf - . |
	ssh "$SCORPIO_SITE_HOST" 'set -e; new=$(mktemp -d /var/www/.scorpio-XXXXXX); tar -xzf - -C "$new"; chmod -R u=rwX,go=rX "$new"; rm -rf /var/www/scorpio.old; mv /var/www/scorpio /var/www/scorpio.old; mv "$new" /var/www/scorpio; rm -rf /var/www/scorpio.old'
echo "deployed to https://scorpio.lxxvll.com/"
