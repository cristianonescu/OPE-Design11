#!/bin/sh
# Lays out the Linux tarball: bin/ (editor and, when built, the thumbnailer), share/ (desktop
# entry, MIME types, thumbnailer entry, hicolor icons) and install.sh.
#   packaging/linux/package.sh <path/to/openstitch> <version> <out-dir>
set -eu
bin=$1
version=$2
out=$3
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
name="openstitch-$version-linux-$(uname -m)"
dir="$out/$name"
rm -rf "$dir"
install -Dm755 "$bin" "$dir/bin/openstitch"
# The thumbnailer is built next to the editor (cargo build -p openstitch-render).
thumb="$(dirname "$bin")/openstitch-thumbnailer"
if [ -x "$thumb" ]; then
  install -Dm755 "$thumb" "$dir/bin/openstitch-thumbnailer"
  install -Dm644 "$here/openstitch.thumbnailer" "$dir/share/thumbnailers/openstitch.thumbnailer"
fi
install -Dm644 "$here/openstitch.desktop" "$dir/share/applications/openstitch.desktop"
install -Dm644 "$here/openstitch-mime.xml" "$dir/share/mime/packages/openstitch.xml"
for s in 16 32 48 64 128 256 512; do
  install -Dm644 "$root/assets/icons/openstitch-$s.png" "$dir/share/icons/hicolor/${s}x$s/apps/openstitch.png"
done
install -Dm644 "$root/assets/icons/openstitch.svg" "$dir/share/icons/hicolor/scalable/apps/openstitch.svg"
install -Dm755 "$here/install.sh" "$dir/install.sh"
cp "$root/README.md" "$root/THIRD_PARTY_NOTICES.md" "$dir/"
tar -C "$out" -czf "$out/$name.tar.gz" "$name"
echo "$out/$name.tar.gz"
