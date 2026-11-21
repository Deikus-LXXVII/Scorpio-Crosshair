#!/bin/sh
# Builds the website and copies it to the web server.
# Usage: SCORPIO_SITE_HOST=<ssh host> tools/site/deploy.sh
# The server serves /var/www/scorpio as https://scorpio.lxxvll.com (Caddy).
set -eu
: "${SCORPIO_SITE_HOST:?set SCORPIO_SITE_HOST to the ssh host of the web server}"
cd "$(dirname "$0")/../.."
node tools/site/build.js
rsync -rlt --delete --chmod=D755,F644 --exclude '._*' --exclude '.DS_Store' docs/site/ "$SCORPIO_SITE_HOST:/var/www/scorpio/"
echo "deployed to https://scorpio.lxxvll.com/"
