# Gametrix USB protocol: implementation evidence

The packet encoder was independently reconstructed and verified against the original `uwApi.dll`. A connected GTX JetSeat (231D:2002, interface 0) passed eight zero-force open/write/close cycles. Physical vibration and zone placement have not yet been confirmed.

## Sources

The control-panel installer was downloaded from the [Gametrix KW-901 page](https://gametrix.ru/catalog/gaming/vibration-pads/kw901-jetseat-live-sense) and extracted without running Windows programs. The examined file was `{app}/uwAPI/uwApi.dll`, PE32, 203776 bytes, base address `0x10000000`.

DLL SHA-256:

```
ac6b2b72d8254878741c7bf694ee1f9ed9a97ec8c1909a77a0f490d15bc31b29
```

Disassembly and execution of the original encoder in an x86 emulator produced `tests/usb-golden-vectors.json`. `WriteFile` was intercepted; no hardware was accessed. The original encoder was not replaced with the Rust implementation. The stack-cookie check was skipped and `IsDebuggerPresent` returned zero.

Zone masks were also compared with [irFFB’s IUberwoorf.h](https://github.com/nlp80/irFFB/blob/master/irFFB/IUberwoorf.h).

## Device IDs and report format

The DLL’s checks at `0x100098ea`–`0x10009942` accept:

| VID | PID |
| --- | --- |
| 16C0 | 0613 |
| 16C0 | 0125 |
| 231D | 2001 |
| 231D | 2002 |

These identify a device family; compatibility with every firmware has not been physically tested. The application also requires an unnumbered HID Output report of exactly 64 bits. VID/PID alone is insufficient.

The write function at `0x10003df0` submits nine bytes: `00` as the HID API Report ID, then eight encoded bytes. [hidapi::write](https://docs.rs/hidapi/2.6.7/hidapi/struct.HidDevice.html#method.write) requires this leading byte even for unnumbered reports.

The input consists of six motor levels (0–255) and two service bytes. The original library can vary service bytes; this application uses zeros. The order from `0x10006030` is:

| Byte | Zone | SDK mask (hex) |
| --- | --- | --- |
| 0 | Left seat | 0200 |
| 1 | Right seat | 0002 |
| 2 | Left lower back | 0400 |
| 3 | Right lower back | 0004 |
| 4 | Left mid-back | 0800 |
| 5 | Right mid-back | 0008 |

Left/right placement requires confirmation on the seat.

## Encoding

Function `0x10003970` produces eight keys:

```
9f3be6c9 4502ab6f 80944295 e0f578ec
1907fa1f 439e493a 6401749d 99447f2b
```

The eight input bytes form two little-endian 32-bit words. Perform 64 rounds with arithmetic modulo 2³²:

```
input = low on even rounds, high on odd rounds
term = ((input >> 8) XOR (input << 6)) + key[round % 8] + round + input
high += term on even rounds; low += term on odd rounds
report = 00 || little_endian(low) || little_endian(high)
```

Zero strength matches the stop function at `0x10004260`:

```
00 4d 60 a7 a6 ed 2b 34 db
```

`packets_match_original_dll_execution` tests six vectors: zeros, distinct motor levels, maximum levels, and nonzero service bytes. These are encoder inputs; unit tests do not send hardware commands.

To reproduce the independent oracle, use Python, `pefile`, `unicorn`, and your own original DLL with the SHA-256 above:

```sh
python3 tools/usb_oracle.py /path/to/uwApi.dll /tmp/usb-vectors.json
```

The script does not open USB devices and rejects a different DLL before emulation. Compare its output with `tests/usb-golden-vectors.json`.

## Runtime and limits

`usb.rs` opens the selected HID path, rechecks the ID and descriptor, sends zeros, then updates levels at 50 Hz. Completion, errors, and worker exit attempt repeated zero strength before closing. The timer is limited to 30 minutes. A 1.5-second watchdog now observes background supervision and successful writer progress, independent of GUI rendering. Feature reports, persistent controller mode changes, and firmware updates are not implemented. Stop depends on a working process, USB connection, and OS HID call.

Since version 0.3, `hid_service.rs` owns every HID operation on one permanent thread. On macOS the HID manager retains the initializing thread’s CFRunLoop. Allowing that thread to exit between enumeration and Start caused SIGILL. Evidence includes private system crash reports from November 1, 2026 and the [HIDAPI multithreading notes](https://github.com/libusb/hidapi/wiki/Multi%E2%80%90threading-Notes). Crash reports are not included in this public repository.

Windows uses the native HIDAPI backend with the [in-box Microsoft HID USB driver](https://learn.microsoft.com/en-us/windows-hardware/drivers/hid/hid-over-usb). No replacement kernel driver or proprietary Gametrix DLL is installed.

The proprietary DLL is not redistributed. The repository contains independent Rust code and test vectors; physical compatibility remains a separate validation step.

## Minimized-window regression in 0.5

Previously, a missing GUI heartbeat after 1.5 seconds or a redraw pause after 3 seconds stopped the session. A minimized macOS window can stop repainting, so these guards confused window visibility with an unhealthy output worker. Version 0.5 moves supervision to a dedicated thread and uses [NSProcessInfo user-initiated activity](https://developer.apple.com/library/archive/documentation/Performance/Conceptual/power_efficiency_guidelines_osx/PrioritizeWorkAtTheAppLevel.html) during the session, retaining the activity through final zero reports and HID close. The supervisor detects actual process/sleep gaps before refreshing its heartbeat; waking cannot resume output.

A cooperative per-device lock prevents competing JetSeat processes, with explicit unlock after HID close. macOS additionally requests exclusive device opening through HIDAPI. Native HID on Windows/Linux can still be shared by non-cooperating third-party software; JetSeat cannot impose its lock on those applications. No undocumented firmware/watchdog command has been added.
