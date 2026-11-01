#!/usr/bin/env bash
set -euo pipefail
apt-get update
DEBIAN_FRONTEND=noninteractive apt-get install -y libasound2 libgl1-mesa-dri libgl1 libx11-6 libxcursor1 libxi6 libxrandr2 libwayland-client0 python3 xvfb xauth
cp /work/packages/*.AppImage /tmp/JetSeat-Massage.AppImage
chmod +x /tmp/JetSeat-Massage.AppImage
useradd --create-home jetseat-test
runuser -u jetseat-test -- /tmp/JetSeat-Massage.AppImage --appimage-extract-and-run --diagnose /tmp/jetseat-appimage.json
python3 - <<'PY'
import json
r=json.load(open('/tmp/jetseat-appimage.json'))
assert r['version']=='0.5.0' and r['os']=='linux' and r['arch']=='x86_64'
print('AppImage extracted and ran without FUSE or root installation')
PY
set +e
runuser -u jetseat-test -- env LIBGL_ALWAYS_SOFTWARE=1 xvfb-run -a timeout 5 /tmp/JetSeat-Massage.AppImage --appimage-extract-and-run > /tmp/jetseat-appimage-gui.log 2>&1
result=$?
set -e
cat /tmp/jetseat-appimage-gui.log
test "$result" -eq 124
printf 'AppImage GUI started as a regular user with software OpenGL: OK\n'
