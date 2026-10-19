#!/bin/sh
# Builds OpenStitch.app from a release binary and zips it.
#   packaging/macos/bundle.sh <path/to/openstitch> <version> <out-dir>
# The bundle is ad-hoc signed only (no Developer ID): Gatekeeper asks before the first launch.
set -eu

bin=$1
version=$2
out=$3
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)

app="$out/OpenStitch.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/openstitch"
cp "$root/assets/icons/openstitch.icns" "$app/Contents/Resources/openstitch.icns"
sed "s/@VERSION@/$version/g" "$here/Info.plist" > "$app/Contents/Info.plist"
printf 'APPL????' > "$app/Contents/PkgInfo"
cp "$root/THIRD_PARTY_NOTICES.md" "$app/Contents/Resources/"
plutil -lint "$app/Contents/Info.plist"
codesign --force --sign - "$app"
(cd "$out" && ditto -c -k --keepParent OpenStitch.app "OpenStitch-$version-macos-$(uname -m).zip")
echo "$out/OpenStitch-$version-macos-$(uname -m).zip"
