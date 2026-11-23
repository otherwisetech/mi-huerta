#!/usr/bin/env bash
# Wraps the `mi-huerta` app binary in "Mi Huerta.app" (bundle id com.otherwisetech.mi-huerta),
# with the headless command line next to it (Contents/MacOS/mi-huerta-cli: the MCP server and
# Otherwise Helper's jobs) and Otherwise Helper's job manifest in Contents/Resources.
# Usage: script/bundle-macos.sh [debug|release]   (default: release; builds first)
#        UNIVERSAL=1 script/bundle-macos.sh       (release for Apple silicon + Intel, joined with lipo)
# Honours CARGO_TARGET_DIR (binaries read from, and the .app written to, that directory).
set -euo pipefail
profile="${1:-release}"
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
target="${CARGO_TARGET_DIR:-target}"
if [ "${UNIVERSAL:-0}" = 1 ]; then
  profile=release
  for t in aarch64-apple-darwin x86_64-apple-darwin; do
    rustup target add "$t" >/dev/null
    cargo build -p mi-huerta-app -p mi-huerta-cli --release --target "$t"
  done
  mkdir -p "$target/release"
  for b in mi-huerta mi-huerta-cli; do
    lipo -create -output "$target/release/$b" "$target/aarch64-apple-darwin/release/$b" "$target/x86_64-apple-darwin/release/$b"
  done
elif [ "$profile" = release ]; then cargo build -p mi-huerta-app -p mi-huerta-cli --release; else cargo build -p mi-huerta-app -p mi-huerta-cli; fi
version="$(cargo pkgid -p mi-huerta-app | sed 's/.*[#@]//')"
app="$target/$profile/Mi Huerta.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$target/$profile/mi-huerta" "$app/Contents/MacOS/mi-huerta"
cp "$target/$profile/mi-huerta-cli" "$app/Contents/MacOS/mi-huerta-cli"
cp crates/app/assets/logo/AppIcon.icns "$app/Contents/Resources/AppIcon.icns"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Mi Huerta</string>
  <key>CFBundleDisplayName</key><string>Mi Huerta</string>
  <key>CFBundleIdentifier</key><string>com.otherwisetech.mi-huerta</string>
  <key>CFBundleExecutable</key><string>mi-huerta</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>CFBundleDevelopmentRegion</key><string>es</string>
  <key>CFBundleLocalizations</key><array><string>es</string><string>en</string><string>pt</string><string>fr</string></array>
  <key>CFBundleURLTypes</key>
  <array>
    <dict>
      <key>CFBundleURLName</key><string>com.otherwisetech.mi-huerta</string>
      <key>CFBundleTypeRole</key><string>Viewer</string>
      <key>CFBundleURLSchemes</key><array><string>otherwise-mi-huerta</string></array>
    </dict>
  </array>
  <key>NSHumanReadableCopyright</key><string>Otherwise Technologies LLC · MIT OR Apache-2.0</string>
</dict>
</plist>
PLIST
plutil -lint -s "$app/Contents/Info.plist"
# Otherwise Helper's jobs (daily reminders, rain forecast), found here without registering.
"$app/Contents/MacOS/mi-huerta-cli" background manifest > "$app/Contents/Resources/otherwise-background.json"
touch "$app"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$app" || true
echo "$app"
