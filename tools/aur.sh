#!/bin/sh
# Bumps the AUR package to a released version and regenerates .SRCINFO.
# Usage: tools/aur.sh VERSION   (for example tools/aur.sh 0.1.6; needs docker)
set -eu
V=${1:?usage: tools/aur.sh VERSION}
case "$V" in *[!0-9.]* | "" ) echo "bad version: $V" >&2; exit 2 ;; esac
DIR=$(cd "$(dirname "$0")/../packaging/aur/scorpio-crosshair-bin" && pwd)
REL=https://github.com/Deikus-LXXVII/Scorpio-Crosshair
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
sum() { curl -fsSL "$1" | shasum -a 256 | cut -d' ' -f1; }
DEB=$(sum "$REL/releases/download/v$V/Scorpio_${V}_amd64.deb")
LIC=$(sum "https://raw.githubusercontent.com/Deikus-LXXVII/Scorpio-Crosshair/v$V/LICENSE")
sed -i.bak \
	-e "s/^pkgver=.*/pkgver=$V/" \
	-e "s/^pkgrel=.*/pkgrel=1/" \
	-e "s/^sha256sums=.*/sha256sums=('$LIC')/" \
	-e "s/^sha256sums_x86_64=.*/sha256sums_x86_64=('$DEB')/" \
	"$DIR/PKGBUILD"
rm -f "$DIR/PKGBUILD.bak"
docker run --rm --platform linux/amd64 -v "$DIR:/pkg:ro" archlinux:latest sh -c \
	'useradd -m b; cp -r /pkg /home/b/p; chown -R b /home/b/p; cd /home/b/p && su b -c "makepkg --printsrcinfo"' >"$TMP/srcinfo"
cp "$TMP/srcinfo" "$DIR/.SRCINFO"
