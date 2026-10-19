#!/bin/zsh
# Smoke-tests a macOS package from CI before a release.
#
#   tools/smoke-macos.sh <macos-universal artifact folder or OpenStitch-*-macos-universal.zip>
#
# Download the artifact first, for example:
#   gh run download <run-id> -n openstitch-main-<sha>-macos-universal -D artifact
#
# Checks: universal binary, bundle metadata, ad-hoc signature, licence files, the editor
# starting headless on the demo, a PES file and the Design Database (screenshots), and the CLI
# writing PES v6 and DST. Everything runs in a temporary folder; nothing is installed.
set -euo pipefail

src=${1:?usage: tools/smoke-macos.sh <artifact folder or zip>}
root=${0:A:h:h}
work=$(mktemp -d -t openstitch-smoke)
trap 'echo "work folder: $work"' EXIT

zip=$src
[[ -d $src ]] && zip=$(print -l "$src"/OpenStitch-*-macos-universal.zip | head -1)
cli=${zip:h}/$(cd "${zip:h}" && print -l openstitch-cli-*macos*.tar.gz 2>/dev/null | head -1)

fail() { echo "FAIL: $*"; exit 1; }
ok() { echo "ok:   $*"; }

# Extract like Finder (Archive Utility) does; plain unzip writes AppleDouble entries into the
# bundle as files, which breaks the signature seal.
ditto -x -k "$zip" "$work/app"
app=$work/app/OpenStitch.app
bin=$app/Contents/MacOS/openstitch

archs=$(lipo -archs "$bin")
[[ $archs == *x86_64* && $archs == *arm64* ]] || fail "binary is not universal ($archs)"
ok "universal binary ($archs)"

plist=$app/Contents/Info.plist
for key in CFBundleIdentifier CFBundleShortVersionString LSMinimumSystemVersion; do
  value=$(/usr/libexec/PlistBuddy -c "Print :$key" "$plist") || fail "Info.plist has no $key"
  ok "$key = $value"
done

codesign --verify --deep --strict "$app" || fail "signature does not verify"
ok "signature verifies (ad-hoc; Gatekeeper will still ask on first open)"

for f in THIRD_PARTY_NOTICES.md LICENSE-MIT LICENSE-APACHE; do
  [[ -f $app/Contents/Resources/$f ]] || fail "$f missing from Resources"
done
ok "notices and licences bundled"

# The editor must start, render and exit by itself within 60 s for each scene.
cp "$root/crates/openstitch-formats/tests/fixtures/oracle.pes" "$work/"
mkdir -p "$work/db" && cp "$work/oracle.pes" "$work/db/"
scene() {
  local out=$1; shift
  (cd "$work" && "$bin" "$@" --screenshot "$out") &
  local pid=$!
  for _ in {1..60}; do kill -0 $pid 2>/dev/null || break; sleep 1; done
  if kill -0 $pid 2>/dev/null; then kill $pid; fail "$out: editor did not exit within 60 s"; fi
  wait $pid || fail "$out: editor exited with an error"
  [[ -s $work/$out ]] || fail "$out: no screenshot written"
  ok "$out rendered"
}
scene demo.png --demo
scene pes.png oracle.pes --ribbon view
scene database.png --database db --db-view large

if [[ -f $cli ]]; then
  mkdir -p "$work/cli" && tar -xzf "$cli" -C "$work/cli"
  tool=$(find "$work/cli" -name stitchtool -type f | head -1)
  print -r -- '[{"command":"design.new","params":{"name":"smoke","width":50,"height":50}},
    {"command":"shape.rectangle","params":{"x":10,"y":10,"width":20,"height":10,"fill":"Red"}},
    {"command":"design.export","params":{"path":"smoke.pes"}},
    {"command":"design.export","params":{"path":"smoke.dst"}}]' > "$work/smoke.json"
  (cd "$work" && "$tool" exec smoke.json > /dev/null) || fail "stitchtool exec failed"
  [[ $(head -c 8 "$work/smoke.pes") == "#PES0060" ]] || fail "default PES is not v6"
  [[ -s $work/smoke.dst ]] || fail "no DST written"
  ok "CLI writes PES v6 and DST"
else
  echo "skip: no openstitch-cli archive next to the app zip"
fi

echo "PASS: open the screenshots in $work to check them by eye"
