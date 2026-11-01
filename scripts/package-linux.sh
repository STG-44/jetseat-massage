#!/usr/bin/env bash
set -euo pipefail
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
mkdir -p dist
if command -v makepkg >/dev/null; then
    (cd packaging/arch && makepkg --force --cleanbuild)
    cp "$project_root"/packaging/arch/jetseat-massage-*.pkg.tar.* "$project_root/dist/"
    printf 'Ready: pacman package in %s/dist\n' "$project_root"
    exit 0
fi
cargo build --release --locked
package="$project_root/dist/jetseat-massage-linux-$(uname -m)"
mkdir -p "$package/bin" "$package/share/applications" "$package/share/icons/hicolor/scalable/apps"
cp "${CARGO_TARGET_DIR:-target}/release/jetseat-massage" "$package/bin/"
for size in 16 32 48 64 128 256 512; do
    mkdir -p "$package/share/icons/hicolor/${size}x${size}/apps"
    cp "assets/icons/app-$size.png" "$package/share/icons/hicolor/${size}x${size}/apps/jetseat-massage.png"
done
cp assets/jetseat-massage.desktop "$package/share/applications/"
mkdir -p "$package/lib/udev/rules.d"
cp assets/70-jetseat.rules "$package/lib/udev/rules.d/"
cat > "$package/INSTALL.txt" <<'INSTALL'
From the extracted package directory:
mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications" "$HOME/.local/share/icons"
cp bin/jetseat-massage "$HOME/.local/bin/"
cp -R share/icons/hicolor "$HOME/.local/share/icons/"
sed "s|Exec=jetseat-massage|Exec=\"$HOME/.local/bin/jetseat-massage\"|" share/applications/jetseat-massage.desktop > "$HOME/.local/share/applications/jetseat-massage.desktop"

For KW-901 USB access:
sudo install -Dm644 lib/udev/rules.d/70-jetseat.rules /etc/udev/rules.d/70-jetseat.rules
sudo udevadm control --reload-rules
Unplug and reconnect the seat USB. Run the GUI as a regular user.
INSTALL
cp README.md LICENSE "$package/"
cp assets/fonts/OFL.txt "$package/OFL-NotoSansCJK.txt"
tar -czf "$package.tar.gz" -C "$project_root/dist" "$(basename "$package")"
printf 'Ready: %s.tar.gz\n' "$package"

