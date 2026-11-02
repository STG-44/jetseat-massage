# Release validation

Version 0.5.0, November 2, 2026. Local environment: Intel x86_64, macOS 15.8.2, Rust 1.96.0.

## Automated checks

- Formatting and Clippy with all targets/features and warnings denied.
- The local suite passes 25 tests, with one opt-in real-audio test ignored in the default run. That real-audio test was also run separately and passed with zero signal.
- Six packet vectors match independent execution of the original Gametrix encoder.
- Software watchdog, session expiry, stop-on-error, zone masks, and bounded signal levels are covered.
- Updater tests cover semver ordering, platform/CPU matching for every operating system, CPU and Linux package format, rejected URLs/drafts/missing hashes, and corrupt, truncated, or oversized files.
- Localization tests cover system locale mapping, fallback, every translated placeholder, dynamic version/level labels, and nested errors.
- Native CI covers Windows x64, Windows ARM64, macOS Intel, macOS ARM64, and Linux x86_64. Windows packaging checks executable architecture/version and exercises installation, installed diagnostics, and removal. See the [workflow results](https://github.com/STG-44/jetseat-massage/actions).

## Hardware evidence

A real GTX JetSeat, VID 231D/PID 2002, interface 0, passed eight repeated zero-force USB cycles in version 0.3, including the executable extracted from the final Mac installer. These exercise temporary-caller enumeration, descriptor validation, open, zero write, and close without activating massage. They establish USB communication and stability, not perceived massage quality.

The version 0.2 macOS crash was SIGILL in CoreFoundation/IOKit after reuse of a HID manager tied to an exited thread. All HID calls now belong to one permanent owner thread. The USB protocol and owner-thread lifecycle remain unchanged in version 0.5.

Physical massage, the mapping of all six zones, Windows seat operation, and Linux seat operation require testing on those systems. Windows/macOS ARM64 CI verifies native binaries and software behavior, not a connected seat.

## Packaging and update boundaries

The icon is embedded as ICNS on macOS, an EXE/installer ICO resource on Windows, and hicolor PNGs on Linux. CJK fonts and their SIL OFL notice are bundled. Windows uses the Microsoft HID driver already present in Windows, with no third-party driver download.

macOS builds are ad-hoc signed, without Developer ID or notarization. Linux portable builds need the libraries listed in the Arch package. Package inspection is distinct from installing onto the user’s system.

Updates use HTTPS GitHub Releases. The GitHub API supplies the expected size and SHA-256; both are checked during download and before installer launch. This verifies integrity relative to the repository, not an independent developer signature. Installation requires an explicit click with massage stopped. New sessions are blocked while installing. Windows Setup waits for the previous process to exit before replacing the executable; CachyOS uses pkexec/pacman and reports cancellation/failure in the GUI.

The actual public release download and SHA-256 path were exercised with the version 0.3 Mac installer. No installer was launched by that probe. GitHub credentials are not stored in the application.

## Linux packaging checks in 0.5

The Linux release workflow targets a glibc 2.34 baseline on AlmaLinux 9 and rejects a binary with a newer requirement. It creates DEB, RPM, Arch, AppImage, and portable archives. Native package tests install using the distribution package manager, run the installed HID diagnostics, inspect the icon/USB rule/font notice, and uninstall the package. Debian-family jobs also start the actual packaged GUI as a regular user with Xvfb and software OpenGL. AppImage is extracted and run without FUSE, including its GUI.

Updater tests cover distribution-family detection without shell evaluation, exact installer arguments with paths containing spaces or shell characters, and atomic AppImage replacement with executable permissions and symlink rejection. Package installations use fixed manager paths through pkexec; no command string is evaluated by a shell.

Source ZIP and tar.gz archives are generated from the same committed source and checked for the lockfile, build instructions, translations and font license. CI results distinguish tested base distributions from compatible derivatives. No container test exercises a connected seat.

Native installation/runtime/removal checks passed on Ubuntu 22.04, 24.04 and 26.04; Debian 12 and 13; Fedora 44 and 45; AlmaLinux 9 and 10; openSUSE Leap 16.0 and Tumbleweed; and Arch Linux. Ubuntu/Debian packaged GUI checks also passed under Xvfb. These are isolated container checks, not full desktop or physical-device certification.

## Background session and UI regression checks

The new supervisor is exercised beyond the former 1.5-second GUI timeout without any GUI callbacks. A fake USB transport runs a live session with background pulses and confirms that the tail reports are zero. A simulated 30-minute timeline exercises the same expiry decision used by the supervisor. Suspension and stale writer supervision latch output off. Device ownership is tested both in one process and through a real child process, including release after Drop.

The previous GUI heartbeat writes and 3-second redraw-pause stop have been removed. The macOS process-activity token is retained until final zero reports and HID close. The UI is split from the engine and uses ordinary egui controls; startup screenshots check compact layout and settings placement above Stop. On the connected Intel Mac, the 30-minute GUI session remained active after more than four minutes minimized, with zero strength forced by the opt-in QA build. Diagnostics showed more than 11,700 writes and six zero motor commands; Stop remained accessible with settings/diagnostics open. A second process was rejected while the device was owned, and eight more zero-force open/write/close cycles passed after Stop and GUI exit. macOS reported the session process-activity assertions while active; those assertions disappeared after Stop. This verifies background transport and ownership, not perceived massage quality or a full physical 30-minute massage.
