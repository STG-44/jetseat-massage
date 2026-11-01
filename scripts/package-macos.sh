#!/usr/bin/env bash
set -euo pipefail
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
version="$(awk -F '\"' '/^version = / { print $2; exit }' Cargo.toml)"
arch="$(uname -m)"
if [ "$arch" = x86_64 ]; then arch=Intel; fi
MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-12.0}" cargo build --release --locked
app="$project_root/dist/JetSeat Massage.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "${CARGO_TARGET_DIR:-target}/release/jetseat-massage" "$app/Contents/MacOS/jetseat-massage"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>jetseat-massage</string>
<key>CFBundleIdentifier</key><string>dev.jetseat.massage</string>
<key>CFBundleName</key><string>JetSeat Massage</string>
<key>CFBundleDisplayName</key><string>JetSeat Massage</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleVersion</key><string>$version</string>
<key>LSMinimumSystemVersion</key><string>12.0</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
chmod +x "$app/Contents/MacOS/jetseat-massage"
xattr -cr "$app"
codesign --force --sign - "$app"
codesign --verify --deep --strict "$app"
ditto --norsrc --noextattr --noqtn -c -k --keepParent "$app" "$project_root/dist/JetSeat-Massage-macOS-$arch.zip"
stage="$(mktemp -d "${TMPDIR:-/tmp}/jetseat-installer.XXXXXX")"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/root/Applications"
ditto --norsrc --noextattr --noqtn "$app" "$stage/root/Applications/JetSeat Massage.app"
pkgbuild --analyze --root "$stage/root" "$stage/components.plist"
/usr/libexec/PlistBuddy -c 'Set :0:BundleIsRelocatable false' "$stage/components.plist"
pkgbuild --root "$stage/root" --component-plist "$stage/components.plist" \
    --identifier dev.jetseat.massage --version "$version" --install-location / \
    "$project_root/dist/JetSeat-Massage-$version-macOS-$arch.pkg"
printf 'Готово: приложение, ZIP и установщик .pkg в %s/dist\n' "$project_root"

