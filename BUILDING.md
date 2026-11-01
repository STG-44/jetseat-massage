# Building and installing from source

The source archives contain the Rust code, Cargo.lock, all eleven translations, fonts and licenses, icon sources, packaging scripts, and protocol tests. They do not contain proprietary Gametrix software. Rust dependencies are downloaded by Cargo on the first build.

Rust 1.88 or newer, a C/C++ compiler, and pkg-config are required. Install Rust through [rustup](https://rustup.rs/) if your distribution's package is older. Then extract `JetSeat-Massage-0.5.0-source.tar.gz` or the ZIP and open its directory.

## Ubuntu, Debian, Linux Mint, Pop!_OS, Zorin OS

```sh
sudo apt update
sudo apt install build-essential pkg-config libasound2-dev libudev-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev libgl1-mesa-dev
cargo build --release --locked
```

## Fedora, Rocky Linux, AlmaLinux, RHEL

On AlmaLinux/Rocky 9, enable CRB first with `sudo dnf config-manager --set-enabled crb` (provided by `dnf-plugins-core`). On RHEL, development dependencies may require the corresponding CodeReady Builder repository.

```sh
sudo dnf install gcc gcc-c++ make pkgconf-pkg-config alsa-lib-devel systemd-devel libxkbcommon-devel libxkbcommon-x11-devel wayland-devel libX11-devel libXcursor-devel libXi-devel libXrandr-devel mesa-libGL-devel
cargo build --release --locked
```

## openSUSE Leap / Tumbleweed

```sh
sudo zypper install gcc gcc-c++ make pkg-config alsa-devel libudev-devel libxkbcommon-devel libxkbcommon-x11-devel wayland-devel libX11-devel libXcursor-devel libXi-devel libXrandr-devel Mesa-libGL-devel
cargo build --release --locked
```

## Arch, CachyOS, Manjaro, EndeavourOS

```sh
sudo pacman -S --needed base-devel rust pkgconf alsa-lib libxkbcommon libxkbcommon-x11 libx11 libxcursor libxi libxrandr wayland mesa
cargo build --release --locked
```

Alternatively, `cd packaging/arch && makepkg -si` builds and installs a native package.

## Run and enable USB

```sh
./target/release/jetseat-massage
sudo ./scripts/install-usb-access.sh
```

Install the USB rule once, then unplug and reconnect the seat. Run the GUI as a regular desktop user. Connect the original power supply and enable the controller. The rule grants access only to the active local desktop session for the four supported Gametrix device IDs.

Source builds can target other Rust-supported CPU architectures, with suitable system libraries. Published Linux binary packages currently target **x86_64**, with **glibc 2.34+**. Alpine/musl and NixOS are not directly supported by the native binary packages; NixOS users can use `appimage-run` with the AppImage or adapt a source build to their environment.

## Create packages

`bash scripts/package-linux.sh` creates a portable archive plus native formats supported by the available tools. To create every format explicitly:

```sh
bash scripts/package-linux.sh deb rpm arch appimage portable
```

This requires Python 3.9+, `dpkg-deb`, `rpmbuild`, `zstd`, `objdump`, and the desktop libraries. AppImage tools are downloaded from pinned upstream releases and verified against `packaging/linux/appimage-tools.json`. If the upstream continuous runtime changes, review it and update the pinned checksum before building.

The release CI builds Linux binaries and the AppImage in AlmaLinux 9 (glibc 2.34), then builds native packages from that binary. Building on a newer host can raise the binary's minimum glibc version; package metadata is derived from the actual binary. AppImage uses the host's OpenGL and audio stack and bundles its font and selected keyboard libraries. A desktop environment with ALSA, X11 or Wayland, and OpenGL support is required.

To create source archives from a committed checkout:

```sh
bash scripts/package-source.sh
```

## macOS and Windows

macOS needs Xcode Command Line Tools. Windows needs Visual Studio C++ build tools and NSIS. These scripts create installers and portable ZIPs:

```sh
bash scripts/package-macos.sh
bash scripts/package-macos.sh aarch64-apple-darwin
```

```powershell
./scripts/package-windows.ps1 -Architecture x64
./scripts/package-windows.ps1 -Architecture arm64
```

## Checks

```sh
cargo fmt --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --locked
```

Default tests do not activate physical massage. The opt-in audio hardware test uses silence. `--usb-self-test` requires one connected compatible seat and sends zero force only.
