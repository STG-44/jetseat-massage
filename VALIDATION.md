# Release validation

Version 0.4.0, November 1, 2026. Local environment: Intel x86_64, macOS 15.8.2, Rust 1.96.0.

## Automated checks

- Formatting and Clippy with all targets/features and warnings denied.
- 16 tests passed locally; the real-audio test is intentionally opt-in.
- Six packet vectors match independent execution of the original Gametrix encoder.
- Software watchdog, session expiry, stop-on-error, zone masks, and bounded signal levels are covered.
- Updater tests cover semver ordering, platform/CPU matching for all five installers, rejected URLs/drafts/missing hashes, and corrupt, truncated, or oversized files.
- Localization tests cover system locale mapping, fallback, every translated placeholder, dynamic version/level labels, and nested errors.
- Native CI covers Windows x64, Windows ARM64, macOS Intel, macOS ARM64, and Linux x86_64. Windows packaging checks executable architecture/version and exercises installation, installed diagnostics, and removal. See the [workflow results](https://github.com/STG-44/jetseat-massage/actions).

## Hardware evidence

A real GTX JetSeat, VID 231D/PID 2002, interface 0, passed eight repeated zero-force USB cycles in version 0.3, including the executable extracted from the final Mac installer. These exercise temporary-caller enumeration, descriptor validation, open, zero write, and close without activating massage. They establish USB communication and stability, not perceived massage quality.

The version 0.2 macOS crash was SIGILL in CoreFoundation/IOKit after reuse of a HID manager tied to an exited thread. All HID calls now belong to one permanent owner thread. The USB protocol and owner-thread lifecycle remain unchanged in version 0.4.

Physical massage, the mapping of all six zones, Windows seat operation, and Linux seat operation require testing on those systems. Windows/macOS ARM64 CI verifies native binaries and software behavior, not a connected seat.

## Packaging and update boundaries

The icon is embedded as ICNS on macOS, an EXE/installer ICO resource on Windows, and hicolor PNGs on Linux. CJK fonts and their SIL OFL notice are bundled. Windows uses the Microsoft HID driver already present in Windows, with no third-party driver download.

macOS builds are ad-hoc signed, without Developer ID or notarization. Linux portable builds need the libraries listed in the Arch package. Package inspection is distinct from installing onto the user’s system.

Updates use HTTPS GitHub Releases. The GitHub API supplies the expected size and SHA-256; both are checked during download and before installer launch. This verifies integrity relative to the repository, not an independent developer signature. Installation requires an explicit click with massage stopped. New sessions are blocked while installing. Windows Setup waits for the previous process to exit before replacing the executable; CachyOS uses pkexec/pacman and reports cancellation/failure in the GUI.

The actual public release download and SHA-256 path were exercised with the version 0.3 Mac installer. No installer was launched by that probe. GitHub credentials are not stored in the application.
