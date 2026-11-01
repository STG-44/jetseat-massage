# JetSeat Massage

A Rust desktop application with a native egui interface for the **Gametrix KW-901**. It controls six motors directly over USB, supports 3.5 mm audio output, and includes a device-free demo.

[Download installers](https://github.com/STG-44/jetseat-massage/releases) · [USB protocol](PROTOCOL.md) · [Validation](VALIDATION.md)

## Installation

| System | Installer |
| --- | --- |
| Windows 10/11, Intel or AMD x64 | `JetSeat-Massage-0.4.0-Windows-x64-Setup.exe` |
| Windows 11, ARM64 | `JetSeat-Massage-0.4.0-Windows-arm64-Setup.exe` |
| macOS 12+, Intel | `JetSeat-Massage-0.4.0-macOS-Intel.pkg` |
| macOS 12+, Apple Silicon (M series) | `JetSeat-Massage-0.4.0-macOS-arm64.pkg` |
| CachyOS / Arch Linux, x86_64 | `jetseat-massage-0.4.0-1-x86_64.pkg.tar.zst` |

Rust is not needed to install a ready-made package. Portable ZIP archives are also provided for Windows and macOS. This is a prerelease: physical vibration and zone placement still need user confirmation.

**Windows:** run Setup. It installs for the current user, adds a Start menu shortcut, and appears in Installed apps. USB uses the **Microsoft HID driver already included with Windows**; no vendor driver, external DLL download, or separate Gametrix software is required. HIDAPI and the application runtime are compiled into the executable. This follows the [Windows HID over USB driver model](https://learn.microsoft.com/en-us/windows-hardware/drivers/hid/hid-over-usb). Actual seat operation on Windows has not yet been tested.

**macOS:** open the `.pkg`; the application is installed in `/Applications`. Intel and Apple Silicon packages contain native binaries for their respective CPUs. The app is ad-hoc signed; the installer has no Developer ID signature or Apple notarization. macOS may require approval in Privacy & Security. To remove it, move the app to Trash.

**CachyOS / Arch:** install the downloaded package:

```sh
sudo pacman -U ./jetseat-massage-0.4.0-1-x86_64.pkg.tar.zst
```

The package includes a udev rule. Reconnect the seat USB after installation and run the GUI as a regular user. For other distributions, the portable archive includes installation instructions; dependencies are listed in `packaging/arch/PKGBUILD`.

## Languages

Choose the language at the top of the window. Changes take effect immediately and are saved. The first launch follows the system language; unsupported languages fall back to English.

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
3. Choose **USB · 6 zones**, click **Find seat**, and select the device.
4. Choose a program, zones, and strength, then click **Start massage**. Start at 25%.

The app validates both the device ID and a compatible HID descriptor before writing. If the device is detected but does not vibrate, check power and the controller’s USB mode. The app does not change firmware or saved controller settings.

Stop, Escape, session expiry, and normal exit attempt to send zero strength. A 1.5-second GUI heartbeat watchdog also stops output. This is a software watchdog: stopping after a process crash, USB failure, or computer shutdown cannot be guaranteed. Use the controller’s power button if vibration continues.

## Connect by audio

Connect the 3.5 mm cable to the computer or a sound card and enable audio capture on the seat controller. Choose **Audio · 3.5 mm** and the matching output. Begin with low system volume and 25% strength. Audio controls the overall response, while USB controls individual zones. The 40–120 Hz setting is an audio frequency, not motor speed.

Selecting speakers makes the signal audible. Other applications on the same output may also cause vibration. Stop disables this application’s signal only.

Every launch starts in Demo with no active session. Strength, program, pace, duration, zones, language, and update preferences are saved.

## Updates

Version 0.3 and later can check and download updates at startup and every six hours while massage is stopped. This can be disabled. A public GitHub release is selected by version, operating system, and CPU. File size and SHA-256 are checked after download and again before installation. No GitHub login is required.

Installation begins with **Install update**, after the session is stopped. macOS opens Installer; Windows launches the bundled Setup and waits for the old process to exit; CachyOS uses pacman through system authorization with pkexec. Relaunch the application after installation. New sessions are blocked while installing. Version 0.2 users must install once manually.

## Diagnostics and development

The USB diagnostics page can enumerate devices and copy a JSON report without sending commands. Name matches are hints; known VID/PID matches are marked separately.

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

CI checks and packages native Windows x64/ARM64, macOS Intel/ARM64, and Linux x86_64 builds. Windows CI also installs the package, runs the installed diagnostics executable, and uninstalls it.

Translations are embedded in `locales/messages.json`; translation columns follow `Language::ALL` after Russian (the source column). Keep numeric placeholders intact. `cargo test` validates all translated placeholders. The generated icon source and prompt are under `assets/icons/`.

MIT license. Noto Sans CJK is distributed under the SIL Open Font License in `assets/fonts/OFL.txt`; packages include this notice. This is an independent project, not an official Gametrix product. Original Windows programs, proprietary DLLs, and audio files are not redistributed.
