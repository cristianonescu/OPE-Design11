#!/bin/sh
# Installs OpenStitch for the current user (or system-wide with PREFIX=/usr/local, as root):
# the binary, the desktop entry, icons, the .osd / .pes MIME types (double-clicking a project
# or PES file opens it in OpenStitch) and the thumbnailer (file managers show stitch previews).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
prefix=${PREFIX:-$HOME/.local}
share="$prefix/share"

install -Dm755 "$here/bin/openstitch" "$prefix/bin/openstitch"
if [ -x "$here/bin/openstitch-thumbnailer" ]; then
  install -Dm755 "$here/bin/openstitch-thumbnailer" "$prefix/bin/openstitch-thumbnailer"
  install -Dm644 "$here/share/thumbnailers/openstitch.thumbnailer" "$share/thumbnailers/openstitch.thumbnailer"
fi
install -Dm644 "$here/share/applications/openstitch.desktop" "$share/applications/openstitch.desktop"
install -Dm644 "$here/share/mime/packages/openstitch.xml" "$share/mime/packages/openstitch.xml"
for d in "$here"/share/icons/hicolor/*/apps; do
  size=$(basename "$(dirname "$d")")
  for f in "$d"/*; do
    install -Dm644 "$f" "$share/icons/hicolor/$size/apps/$(basename "$f")"
  done
done
command -v update-mime-database >/dev/null && update-mime-database "$share/mime" || true
command -v update-desktop-database >/dev/null && update-desktop-database "$share/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$share/icons/hicolor" || true
command -v xdg-mime >/dev/null && \
  xdg-mime default openstitch.desktop application/x-openstitch-design application/x-pes-embroidery || true
echo "OpenStitch installed in $prefix (make sure $prefix/bin is on PATH)"
