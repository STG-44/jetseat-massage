# JetSeat Massage

A Rust desktop application with a native egui interface for the **Gametrix KW-901**. It controls six motors directly over USB, supports 3.5 mm audio output, and includes a device-free demo.

[Download installers](https://github.com/STG-44/jetseat-massage/releases) · [USB protocol](PROTOCOL.md) · [Validation](VALIDATION.md)

## Installation

| System | Installer |
| --- | --- |
| Windows 10/11, Intel or AMD x64 | `JetSeat-Massage-0.5.0-Windows-x64-Setup.exe` |
| Windows 11, ARM64 | `JetSeat-Massage-0.5.0-Windows-arm64-Setup.exe` |
| macOS 12+, Intel | `JetSeat-Massage-0.5.0-macOS-Intel.pkg` |
| macOS 12+, Apple Silicon (M series) | `JetSeat-Massage-0.5.0-macOS-arm64.pkg` |
| Ubuntu / Debian / Mint / Pop!_OS / Zorin, x86_64 | `jetseat-massage_0.5.0_amd64.deb` |
| Fedora / RHEL / Rocky / AlmaLinux / openSUSE, x86_64 | `jetseat-massage-0.5.0-1.x86_64.rpm` |
| CachyOS / Arch / Manjaro / EndeavourOS, x86_64 | `jetseat-massage-0.5.0-1-x86_64.pkg.tar.zst` |
| Portable Linux, x86_64 | `JetSeat-Massage-0.5.0-x86_64.AppImage` |

Rust is not needed to install a ready-made package. Portable ZIP archives are also provided for Windows and macOS. This is a prerelease: physical vibration and zone placement still need user confirmation.

**Windows:** run Setup. It installs for the current user, adds a Start menu shortcut, and appears in Installed apps. USB uses the **Microsoft HID driver already included with Windows**; no vendor driver, external DLL download, or separate Gametrix software is required. HIDAPI and the application runtime are compiled into the executable. This follows the [Windows HID over USB driver model](https://learn.microsoft.com/en-us/windows-hardware/drivers/hid/hid-over-usb). Actual seat operation on Windows has not yet been tested.

**macOS:** open the `.pkg`; the application is installed in `/Applications`. Intel and Apple Silicon packages contain native binaries for their respective CPUs. The app is ad-hoc signed; the installer has no Developer ID signature or Apple notarization. macOS may require approval in Privacy & Security. To remove it, move the app to Trash.

**Linux:** choose the format for your distribution. Native packages install the menu entry, icons, font notices and USB access rule. Reconnect the seat USB afterwards and run the app as a regular desktop user.

```sh
# Ubuntu / Debian / Linux Mint / Pop!_OS / Zorin OS
sudo apt install ./jetseat-massage_0.5.0_amd64.deb

# Fedora / RHEL / Rocky / AlmaLinux
sudo dnf install ./jetseat-massage-0.5.0-1.x86_64.rpm

# openSUSE Leap / Tumbleweed
sudo zypper install --allow-unsigned-rpm ./jetseat-massage-0.5.0-1.x86_64.rpm

# Arch / CachyOS / Manjaro / EndeavourOS
sudo pacman -U ./jetseat-massage-0.5.0-1-x86_64.pkg.tar.zst
```

**AppImage:** download, make executable, and run. It can run without FUSE using `--appimage-extract-and-run`:

```sh
chmod +x JetSeat-Massage-0.5.0-x86_64.AppImage
./JetSeat-Massage-0.5.0-x86_64.AppImage
# Once, to enable USB for the active desktop user:
./JetSeat-Massage-0.5.0-x86_64.AppImage --install-usb-rule
```

The USB rule operation asks for system authorization; it does not activate massage. Native packages already include the rule. AppImage uses the host OpenGL/audio stack and bundles selected keyboard libraries and fonts.

Published Linux binaries require x86_64 and glibc 2.34+. Intended desktop families include Ubuntu 22.04+, Debian 12+, Mint 21+, Pop!_OS 22.04+, Zorin 17+, Fedora, RHEL/Rocky/AlmaLinux 9+, openSUSE Leap 16/Tumbleweed, and current Arch derivatives. Native installation, diagnostics and removal were tested in containers on Ubuntu 22.04/24.04/26.04, Debian 12/13, Fedora 44/45, AlmaLinux 9/10, openSUSE Leap 16/Tumbleweed, and Arch Linux. Mint, Pop!_OS, Zorin, Manjaro and other derivatives inherit their base-family compatibility; they were not individually tested. Alpine/musl and NixOS need distribution-specific handling; see [BUILDING.md](BUILDING.md).

**Source archives:** both `JetSeat-Massage-0.5.0-source.zip` and `JetSeat-Massage-0.5.0-source.tar.gz` include the full application source, lockfile, translations, font and icon assets, licenses, tests, and packaging scripts. [BUILDING.md](BUILDING.md) has instructions for each Linux family, macOS and Windows.

## Main window and settings

The compact main window contains the six massage programs, configured intensity, pace, duration presets (including 30 minutes), remaining time and Start/Stop. The gear opens connection, language and update settings. Advanced settings contain the zone switches and audio frequency. Developer mode unlocks HID diagnostics, raw commands and logs; these details are hidden in normal use. Stop remains outside the settings windows and is available while USB shutdown is pending.

The USB protocol and program envelopes are unchanged. Session supervision is independent of GUI rendering. macOS claims exclusive HID access; all platforms also coordinate JetSeat instances with a per-device lock. Stop closes the device before releasing ownership. Arbitrary third-party applications on Windows/Linux do not necessarily honor this cooperative lock.

The design and future profile/game integration boundaries are documented in [ARCHITECTURE.md](ARCHITECTURE.md). Named profiles and game adapters are not part of this release.

## Languages

Choose the language in Settings. Changes take effect immediately and are saved. The first launch follows the system language; unsupported languages fall back to English.

Russian, Ukrainian, English, German, French, Spanish, Brazilian Portuguese, Italian, Simplified Chinese, Japanese, and Korean are included. Help, diagnostics, update screens, and application error messages are translated. Device names and native operating system errors retain their original text. Noto Sans CJK is embedded, so Asian text does not depend on installed fonts.

## Programs

| Program | Behavior |
| --- | --- |
| Relax | Smooth overall rise and fall |
| Wave | Moves from seat to back over USB; varies overall strength over audio |
| Pulse | Pulses separated by rest |
| Steady | Constant selected strength |
| Snake | Visits all six zones: up the left side, down the right |
| Left / right | Smoothly alternates sides |

USB and Demo support all six programs and individual zone switches: left/right seat, lower back, and mid-back. Audio supports the first four programs. Strength, pace, and a 1–30 minute timer are adjustable. Indicators display commanded levels, not measured vibration.

## Connect by USB

1. Connect the original KW-901 power supply and switch on its controller.
2. Connect USB to the computer. No audio cable is needed.
3. Open JetSeat. A single supported seat is selected automatically. If several are connected, choose one in Settings.
4. Choose a program, intensity and duration, then click **Start**. Start at 25%; optional zone switches are in Advanced settings.

The app validates both the device ID and a compatible HID descriptor before writing. If the device is detected but does not vibrate, check power and the controller’s USB mode. The app does not change firmware or saved controller settings.

Stop, Escape, session expiry, and normal exit attempt to send zero strength. An independent background supervisor monitors the writer and timer; minimizing or hiding the window does not interrupt its heartbeat. On macOS, a user-initiated process activity keeps the session out of App Nap and prevents automatic idle system sleep until shutdown finishes. This is a software watchdog: stopping after a process crash, USB failure, or computer shutdown cannot be guaranteed. Use the controller’s power button if vibration continues.

## Connect by audio

Connect the 3.5 mm cable to the computer or a sound card and enable audio capture on the seat controller. In Settings, choose **Audio · 3.5 mm** and the matching output. Begin with low system volume and 25% strength. Audio controls the overall response, while USB controls individual zones. The 40–120 Hz setting is an audio frequency, not motor speed.

Selecting speakers makes the signal audible. Other applications on the same output may also cause vibration. Stop disables this application’s signal only.

Every launch restores the chosen connection mode with no active session. Strength, program, pace, duration, zones, language, connection mode and update preferences are saved. Auto-connect discovers a single seat, but never starts massage automatically.

## Updates

Version 0.3 and later can check and download updates at startup and every six hours while massage is stopped. This can be disabled in Settings. A public GitHub release is selected by version, operating system, and CPU. File size and SHA-256 are checked after download and again before installation. No GitHub login is required.

Installation begins with **Install update**, after the session is stopped. macOS opens Installer; Windows launches the bundled Setup and waits for the old process to exit; Linux uses apt, dnf, zypper or pacman through system authorization with pkexec. AppImage updates replace the original writable AppImage atomically, preserving executable permissions. Keep the `.AppImage` extension and launch the application from that file. Relaunch the application after installation. New sessions are blocked while installing. Version 0.2 users must install once manually.

## Diagnostics and development

Enable developer mode in Advanced settings to open Diagnostics. It can enumerate devices and copy a JSON report without sending commands. Name matches are hints; known VID/PID matches are marked separately.

```sh
jetseat-massage --diagnose kw901-usb.json
jetseat-massage --list-audio
jetseat-massage --usb-self-test
jetseat-massage --check-updates
```

`--usb-self-test` requires exactly one compatible connected seat and performs eight zero-force open/write/close cycles. It does not test massage strength.

Rust 1.88 or later is required to build. macOS requires Xcode Command Line Tools. Windows requires the Visual Studio C++ build tools and NSIS. Choose the matching native Rust target.

```sh
bash scripts/package-macos.sh
bash scripts/package-macos.sh aarch64-apple-darwin
```

```powershell
./scripts/package-windows.ps1 -Architecture x64
./scripts/package-windows.ps1 -Architecture arm64
```

On CachyOS / Arch:

```sh
sudo pacman -S --needed base-devel rust pkgconf
cd packaging/arch
makepkg -si
```

On other Linux systems, `bash scripts/package-linux.sh` creates a portable archive. All packaging scripts place results in `dist/`.

```sh
cargo fmt --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --locked
```

CI checks native Windows x64/ARM64, macOS Intel/ARM64, and Linux x86_64 builds. Linux release binaries are built on AlmaLinux 9 with glibc 2.34; separate jobs install, run and remove packages on Ubuntu, Debian, Fedora, AlmaLinux, openSUSE and Arch. AppImage is exercised without FUSE on Ubuntu 22.04. Windows CI also installs the package, runs the installed diagnostics executable, and uninstalls it.

Translations are embedded in `locales/messages.json`; translation columns follow `Language::ALL` after Russian (the source column). Keep numeric placeholders intact. `cargo test` validates all translated placeholders. The generated icon source and prompt are under `assets/icons/`.

MIT license. Noto Sans CJK is distributed under the SIL Open Font License in `assets/fonts/OFL.txt`; packages include this notice. This is an independent project, not an official Gametrix product. Original Windows programs, proprietary DLLs, and audio files are not redistributed.
