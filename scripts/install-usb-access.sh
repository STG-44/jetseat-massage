#!/bin/sh
# Run explicitly with sudo/pkexec, or use AppImage --install-usb-rule.
set -eu
if [ "$(id -u)" -ne 0 ]; then
    printf '%s\n' 'Administrator authorization is needed to install the USB access rule.' >&2
    exit 1
fi
install -d -m 755 /etc/udev/rules.d
cat > /etc/udev/rules.d/70-jetseat.rules <<'RULES'
# Access for the active local desktop session. Reconnect USB after installation.
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="16c0", ATTRS{idProduct}=="0613", TAG+="uaccess"
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="16c0", ATTRS{idProduct}=="0125", TAG+="uaccess"
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="231d", ATTRS{idProduct}=="2001", TAG+="uaccess"
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="231d", ATTRS{idProduct}=="2002", TAG+="uaccess"
RULES
chmod 644 /etc/udev/rules.d/70-jetseat.rules
if command -v udevadm >/dev/null 2>&1; then udevadm control --reload-rules || true; fi
printf '%s\n' 'USB access rule installed. Unplug and reconnect the seat, then run the app as a regular desktop user.'
