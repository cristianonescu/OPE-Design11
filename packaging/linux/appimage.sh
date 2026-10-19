#!/bin/sh
# Builds OpenStitch-<version>-<arch>.AppImage from the editor (and, when built next to it, the
# thumbnailer): an AppDir with AppRun, the desktop entry, icons and MIME types, packed by
# appimagetool with the static type2 runtime.
#   packaging/linux/appimage.sh <path/to/openstitch> <version> <out-dir>
# appimagetool and the runtime are downloaded from their versioned GitHub releases and checked
# against the SHA-256 sums below; set APPIMAGETOOL / APPIMAGE_RUNTIME to use local copies.
set -eu
bin=$1
version=$2
out=$3
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
arch=$(uname -m)

TOOL_VERSION=1.9.1        # https://github.com/AppImage/appimagetool/releases
RUNTIME_VERSION=20251108  # https://github.com/AppImage/type2-runtime/releases
case "$arch" in
  x86_64)
    tool_sha=ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0
    runtime_sha=2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d ;;
  aarch64)
    tool_sha=f0837e7448a0c1e4e650a93bb3e85802546e60654ef287576f46c71c126a9158
    runtime_sha=00cbdfcf917cc6c0ff6d3347d59e0ca1f7f45a6df1a428a0d6d8a78664d87444 ;;
  *) echo "no pinned appimagetool for $arch" >&2; exit 1 ;;
esac

mkdir -p "$out"
out=$(cd "$out" && pwd)
tools="$out/.appimage-tools"
mkdir -p "$tools"

fetch() { # <url> <sha256> <dest>
  if [ ! -f "$3" ] || ! echo "$2  $3" | sha256sum -c --status; then
    curl -fsSL --retry 3 -o "$3.part" "$1"
    echo "$2  $3.part" | sha256sum -c --quiet
    mv "$3.part" "$3"
  fi
}

tool=${APPIMAGETOOL:-}
if [ -z "$tool" ]; then
  tool="$tools/appimagetool-$TOOL_VERSION-$arch.AppImage"
  fetch "https://github.com/AppImage/appimagetool/releases/download/$TOOL_VERSION/appimagetool-$arch.AppImage" \
    "$tool_sha" "$tool"
  chmod +x "$tool"
fi
runtime=${APPIMAGE_RUNTIME:-}
if [ -z "$runtime" ]; then
  runtime="$tools/runtime-$RUNTIME_VERSION-$arch"
  fetch "https://github.com/AppImage/type2-runtime/releases/download/$RUNTIME_VERSION/runtime-$arch" \
    "$runtime_sha" "$runtime"
fi

app="$out/OpenStitch.AppDir"
rm -rf "$app"
install -Dm755 "$bin" "$app/usr/bin/openstitch"
thumb="$(dirname "$bin")/openstitch-thumbnailer"
if [ -x "$thumb" ]; then
  install -Dm755 "$thumb" "$app/usr/bin/openstitch-thumbnailer"
  install -Dm644 "$here/openstitch.thumbnailer" "$app/usr/share/thumbnailers/openstitch.thumbnailer"
fi
install -Dm644 "$here/openstitch.desktop" "$app/usr/share/applications/openstitch.desktop"
install -Dm644 "$here/openstitch-mime.xml" "$app/usr/share/mime/packages/openstitch.xml"
for s in 16 32 48 64 128 256 512; do
  install -Dm644 "$root/assets/icons/openstitch-$s.png" "$app/usr/share/icons/hicolor/${s}x$s/apps/openstitch.png"
done
install -Dm644 "$root/assets/icons/openstitch.svg" "$app/usr/share/icons/hicolor/scalable/apps/openstitch.svg"
install -Dm644 "$root/README.md" "$app/usr/share/doc/openstitch/README.md"
install -Dm644 "$root/THIRD_PARTY_NOTICES.md" "$app/usr/share/doc/openstitch/THIRD_PARTY_NOTICES.md"
# AppImage layout: AppRun, the desktop entry and its icon at the top of the AppDir.
install -Dm755 "$here/AppRun" "$app/AppRun"
cp "$here/openstitch.desktop" "$app/openstitch.desktop"
cp "$root/assets/icons/openstitch-256.png" "$app/openstitch.png"
ln -s openstitch.png "$app/.DirIcon"

image="$out/OpenStitch-$version-$arch.AppImage"
rm -f "$image"
# The runner has no FUSE, so appimagetool (itself an AppImage) runs extracted.
ARCH=$arch APPIMAGE_EXTRACT_AND_RUN=1 "$tool" --no-appstream --runtime-file "$runtime" "$app" "$image"
rm -rf "$app"
echo "$image"
