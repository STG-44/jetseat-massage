#!/usr/bin/env bash
set -euo pipefail
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
version="$(awk -F '\"' '/^version = / { print $2; exit }' Cargo.toml)"
target="${1:-${CARGO_BUILD_TARGET:-}}"
host="$(rustc -vV | sed -n 's/^host: //p')"
case "${target:-$host}" in
    x86_64-apple-darwin) arch=Intel ;;
    aarch64-apple-darwin) arch=arm64 ;;
    *) printf 'Unsupported macOS target: %s\n' "${target:-$host}" >&2; exit 1 ;;
esac
args=(--release --locked)
binary="${CARGO_TARGET_DIR:-target}/release/jetseat-massage"
if [ -n "$target" ]; then
    args+=(--target "$target")
    binary="${CARGO_TARGET_DIR:-target}/$target/release/jetseat-massage"
fi
MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-12.0}" cargo build "${args[@]}"
app="$project_root/dist/macos-$arch/JetSeat Massage.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$binary" "$app/Contents/MacOS/jetseat-massage"
cp assets/icons/app.icns "$app/Contents/Resources/AppIcon.icns"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>jetseat-massage</string>
<key>CFBundleIdentifier</key><string>dev.jetseat.massage</string>
<key>CFBundleName</key><string>JetSeat Massage</string>
<key>CFBundleDisplayName</key><string>JetSeat Massage</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleIconFile</key><string>AppIcon</string>
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

