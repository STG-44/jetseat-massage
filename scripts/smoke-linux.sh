#!/usr/bin/env bash
# Used only in disposable CI containers, never against the host system.
set -euo pipefail
family="$1"
case "$family" in
  deb)
    apt-get update
    DEBIAN_FRONTEND=noninteractive apt-get install -y /work/packages/*.deb python3 xvfb xauth libgl1-mesa-dri
    ;;
  rpm)
    dnf install -y /work/packages/*.rpm python3
    ;;
  suse)
    zypper --non-interactive refresh
    zypper --non-interactive install --allow-unsigned-rpm /work/packages/*.rpm python3
    ;;
  arch)
    pacman -Syu --noconfirm
    pacman -S --noconfirm python
    pacman -U --noconfirm /work/packages/*.pkg.tar.zst
    ;;
esac
/usr/bin/jetseat-massage --diagnose /tmp/jetseat-diagnostic.json
python3 - <<'PY'
import json,os
from pathlib import Path
r=json.loads(Path('/tmp/jetseat-diagnostic.json').read_text())
assert r['os']=='linux' and r['arch']=='x86_64' and r['version']=='0.5.0'
assert Path('/usr/lib/udev/rules.d/70-jetseat.rules').is_file()
assert Path('/usr/share/icons/hicolor/256x256/apps/jetseat-massage.png').is_file()
assert Path('/usr/share/licenses/jetseat-massage/OFL-NotoSansCJK.txt').is_file()
print('Installed native executable, HID diagnostics, USB rule, icon and font notice verified')
PY
if [ "$family" = deb ]; then
    # Keep audio/USB inactive; confirm that the actual packaged GUI stays alive.
    useradd --create-home jetseat-test
    set +e
    runuser -u jetseat-test -- env LIBGL_ALWAYS_SOFTWARE=1 xvfb-run -a timeout 5 /usr/bin/jetseat-massage > /tmp/jetseat-gui.log 2>&1
    result=$?
    set -e
    cat /tmp/jetseat-gui.log
    test "$result" -eq 124
fi
case "$family" in
  deb) apt-get remove -y jetseat-massage ;;
  rpm) dnf remove -y jetseat-massage ;;
  suse) zypper --non-interactive remove jetseat-massage ;;
  arch) pacman -R --noconfirm jetseat-massage ;;
esac
test ! -e /usr/bin/jetseat-massage
test ! -e /usr/lib/udev/rules.d/70-jetseat.rules
printf 'Native package installation, diagnostics and removal: OK\n'
