# Architecture

The hardware backend does not depend on egui window repainting. The UI chooses configuration and sends Start/Stop; it does not keep a physical session alive.

- `signal.rs`: the existing six program envelopes, zone mapping and session fades.
- `protocol.rs`: the original packet encoder and HID descriptor validation. Packet vectors remain unchanged.
- `usb.rs`: one device writer, command progress, stop reports and device ownership.
- `hid_service.rs`: the permanent owner of every HIDAPI operation. This preserves the macOS CFRunLoop lifetime.
- `session.rs`: independent session supervision, expiry and suspend detection.
- `power.rs`: bounded user-initiated activity on macOS and a Windows execution-state request while a session runs. No additional crate is needed.
- `device_lock.rs`: cooperative ownership across JetSeat processes, released only after the HID handle closes. macOS additionally requests exclusive HID access. Arbitrary third-party tools on Windows/Linux may not honor JetSeat's cooperative lock.
- `audio.rs`: CPAL output and the existing waveform, with background supervision.
- `ui/mod.rs`: presentation and view state. Settings, advanced controls and developer diagnostics are separate egui windows constrained above the Stop controls.
- `ui/theme.rs`: the dark palette, typography and widget style.
- `i18n.rs`, `locales/messages.json`: runtime language selection and eleven embedded languages.
- `updater.rs`, `linux_update.rs`: verified downloads and OS/package-aware installation.

## Device lifecycle

Start first stops the previous output, claims ownership, then validates and opens the selected device. Only the device writer sends motor reports. Stop, expiry and background faults latch output off, send repeated zero reports, close the handle, and release ownership. A delayed native I/O call keeps the device busy and the Stop control available; another session cannot start over it. Normal window closure is cancelled while USB shutdown is still pending.

The GUI never restarts a stopped session automatically after reconnect or system sleep. Auto-connect discovers/selects a device; it does not start massage. The configured intensity and the instantaneous motor command are different values; instantaneous commands are shown only in developer diagnostics.

## Profiles and future game integration

Current persistent settings are implemented. Named profiles, import/export, a versioned public profile schema and game integrations are **not implemented** yet.

A future profile module should define a versioned JSON document with explicit units and validation, convert it into the existing typed configuration, and keep display preferences separate. A future game adapter should provide effects/configuration to the single device owner, rather than opening HID independently. Neither feature requires replacing the packet protocol or program algorithms.

## Limits

The software watchdog requires a running process and OS. There is no confirmed controller-side timeout in the implemented output-report protocol. A process crash, disconnected USB or full system sleep may prevent a stop report from reaching the seat. Stop the session before deliberately sleeping the computer or disconnecting USB; use the controller's power switch if vibration continues. Firmware and persistent controller settings are not changed.
