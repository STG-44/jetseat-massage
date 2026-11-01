#!/usr/bin/env python3
"""Package an existing x86_64 Linux binary; no massage or privileged install occurs."""
import argparse, gzip, hashlib, io, json, os, re, shutil, stat, subprocess, tarfile, tempfile, time, urllib.request
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
VERSION = re.search(r'^version = "([^"]+)"', (ROOT / 'Cargo.toml').read_text(), re.M).group(1)
DIST = ROOT / 'dist'

def run(*args, **kwargs):
    return subprocess.run([str(a) for a in args], check=True, **kwargs)

def copy(source, destination, executable=False):
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, destination)
    destination.chmod(0o755 if executable else 0o644)

def common(stage, binary):
    copy(binary, stage / 'usr/bin/jetseat-massage', True)
    copy(ROOT/'assets/jetseat-massage.desktop', stage/'usr/share/applications/jetseat-massage.desktop')
    for size in [16,32,48,64,128,256,512]:
        copy(ROOT/f'assets/icons/app-{size}.png', stage/f'usr/share/icons/hicolor/{size}x{size}/apps/jetseat-massage.png')
    copy(ROOT/'assets/70-jetseat.rules', stage/'usr/lib/udev/rules.d/70-jetseat.rules')
    copy(ROOT/'scripts/install-usb-access.sh', stage/'usr/share/jetseat-massage/install-usb-access.sh', True)
    for source, target in [('LICENSE','LICENSE'), ('assets/fonts/OFL.txt','OFL-NotoSansCJK.txt')]:
        copy(ROOT/source, stage/'usr/share/licenses/jetseat-massage'/target)
    for name in ['README.md','PROTOCOL.md','VALIDATION.md','BUILDING.md']:
        copy(ROOT/name, stage/'usr/share/doc/jetseat-massage'/name)

def glibc_version(binary):
    result = subprocess.check_output(['objdump','-p',str(binary)], text=True)
    if 'RPATH' in result or 'RUNPATH' in result:
        raise RuntimeError('Release binary must not contain a build-machine RPATH')
    versions = re.findall(r'GLIBC_(\d+\.\d+)', result)
    return max(versions, key=lambda s: tuple(map(int, s.split('.'))))

def deb(stage, glibc):
    control = stage/'DEBIAN';control.mkdir()
    size = sum(p.stat().st_size for p in stage.rglob('*') if p.is_file()) // 1024 + 1
    (control/'control').write_text(f'''Package: jetseat-massage
Version: {VERSION}
Section: utils
Priority: optional
Architecture: amd64
Maintainer: JetSeat Massage contributors <55913434+STG-44@users.noreply.github.com>
Installed-Size: {size}
Depends: libc6 (>= {glibc}), libgcc-s1, libasound2 (>= 1.2.2) | libasound2t64, libxkbcommon0, libxkbcommon-x11-0, libx11-6, libxcursor1, libxi6, libxrandr2, libwayland-client0, libgl1, udev, pkexec | policykit-1
Homepage: https://github.com/STG-44/jetseat-massage
Description: USB and audio massage controller for Gametrix KW-901
 Native Rust GUI with six USB zones, audio output, eleven languages and updates.
''')
    hook = '#!/bin/sh\nset -e\nif command -v udevadm >/dev/null 2>&1; then udevadm control --reload-rules || true; fi\nexit 0\n'
    for name in ['postinst','postrm']:
        (control/name).write_text(hook);(control/name).chmod(0o755)
    sums = ''.join(hashlib.md5(p.read_bytes()).hexdigest()+'  '+str(p.relative_to(stage))+'\n' for p in sorted(stage.rglob('*')) if p.is_file() and control not in p.parents)
    (control/'md5sums').write_text(sums)
    run('dpkg-deb','--root-owner-group','--build',stage,DIST/f'jetseat-massage_{VERSION}_amd64.deb')

def rpm(stage, work):
    top = work/'rpmbuild'
    for name in ['BUILD','BUILDROOT','RPMS','SOURCES','SPECS','SRPMS']: (top/name).mkdir(parents=True)
    spec = top/'SPECS/jetseat.spec'
    # RPM resolves SONAME/file capabilities across Fedora, RHEL and openSUSE.
    spec.write_text(f'''Name: jetseat-massage
Version: {VERSION}
Release: 1
Summary: USB and audio massage controller for Gametrix KW-901
License: MIT AND OFL-1.1
URL: https://github.com/STG-44/jetseat-massage
BuildArch: x86_64
Requires: libasound.so.2()(64bit), libxkbcommon.so.0()(64bit), libxkbcommon-x11.so.0()(64bit)
Requires: libX11.so.6()(64bit), libXcursor.so.1()(64bit), libXi.so.6()(64bit), libXrandr.so.2()(64bit)
Requires: libwayland-client.so.0()(64bit), libGL.so.1()(64bit), /usr/bin/pkexec, /usr/bin/udevadm
%description
Native Rust GUI with six USB zones, audio output, eleven languages and updates.
%prep
%build
%install
mkdir -p %{{buildroot}}
cp -a "{stage}/." %{{buildroot}}/
%post
if command -v udevadm >/dev/null 2>&1; then udevadm control --reload-rules || :; fi
%postun
if command -v udevadm >/dev/null 2>&1; then udevadm control --reload-rules || :; fi
%files
/usr/bin/jetseat-massage
/usr/lib/udev/rules.d/70-jetseat.rules
/usr/share/applications/jetseat-massage.desktop
/usr/share/icons/hicolor/*/apps/jetseat-massage.png
/usr/share/jetseat-massage
%license /usr/share/licenses/jetseat-massage/*
%doc /usr/share/doc/jetseat-massage
''')
    run('rpmbuild','-bb','--define',f'_topdir {top}','--define','_build_id_links none','--define','debug_package %{nil}',spec)
    copy(next((top/'RPMS/x86_64').glob('*.rpm')), DIST/f'jetseat-massage-{VERSION}-1.x86_64.rpm')

def arch(stage, glibc, work):
    now = int(time.time())
    dependencies = [f'glibc>={glibc}','gcc-libs','alsa-lib','libxkbcommon','libxkbcommon-x11','libx11','libxcursor','libxi','libxrandr','wayland','mesa','polkit','systemd']
    size = sum(p.stat().st_size for p in stage.rglob('*') if p.is_file())
    (stage/'.PKGINFO').write_text('\n'.join(['pkgname = jetseat-massage','pkgbase = jetseat-massage',f'pkgver = {VERSION}-1','pkgdesc = USB and audio massage controller for Gametrix KW-901','url = https://github.com/STG-44/jetseat-massage',f'builddate = {now}','packager = JetSeat Massage release build',f'size = {size}','arch = x86_64','license = MIT','license = OFL-1.1','xdata = pkgtype=pkg']+['depend = '+s for s in dependencies])+'\n')
    copy(ROOT/'packaging/arch/jetseat-massage.install', stage/'.INSTALL')
    lines=['#mtree','/set type=file uid=0 gid=0 mode=644',f'/set time={now}','. type=dir mode=755']
    for p in sorted(stage.rglob('*')):
        name='./'+str(p.relative_to(stage));mode=p.stat().st_mode & 0o777
        if p.is_dir(): lines.append(f'{name} type=dir mode={mode:o}')
        else: lines.append(f'{name} type=file mode={mode:o} size={p.stat().st_size} sha256digest={hashlib.sha256(p.read_bytes()).hexdigest()}')
    (stage/'.MTREE').write_bytes(gzip.compress(('\n'.join(lines)+'\n').encode(),mtime=0))
    archive=work/'arch.tar'
    with tarfile.open(archive,'w',format=tarfile.PAX_FORMAT) as tar:
        for p in sorted(stage.rglob('*')):
            item=tar.gettarinfo(str(p),str(p.relative_to(stage)));item.uid=item.gid=0;item.uname=item.gname='root';item.mtime=now
            if p.is_file():
                with p.open('rb') as f:tar.addfile(item,f)
            else:tar.addfile(item)
    run('zstd','-T0','-10','--force',archive,'-o',DIST/f'jetseat-massage-{VERSION}-1-x86_64.pkg.tar.zst')

def portable(stage, work):
    package=work/f'JetSeat-Massage-{VERSION}-Linux-x86_64';package.mkdir()
    shutil.copytree(stage/'usr',package/'usr')
    copy(ROOT/'scripts/install-usb-access.sh',package/'install-usb-access.sh',True)
    (package/'INSTALL.txt').write_text('Portable Linux x86_64 build. Requires glibc 2.34+ and the desktop libraries listed in BUILDING.md.\nRun ./usr/bin/jetseat-massage\nFor USB access, run sudo ./install-usb-access.sh once, then reconnect USB.\nNative .deb/.rpm/Arch packages or AppImage are recommended for normal installation.\n')
    with tarfile.open(DIST/f'JetSeat-Massage-{VERSION}-Linux-x86_64.tar.gz','w:gz') as tar:tar.add(package,arcname=package.name)

def tool(name, work):
    entry=json.loads((ROOT/'packaging/linux/appimage-tools.json').read_text())[name]
    path=work/name
    with urllib.request.urlopen(entry['url'],timeout=120) as response:path.write_bytes(response.read())
    if hashlib.sha256(path.read_bytes()).hexdigest()!=entry['sha256']:raise RuntimeError(f'{name} SHA-256 mismatch')
    path.chmod(0o755);return path

def appimage(stage, work):
    appdir=work/'JetSeat.AppDir';shutil.copytree(stage,appdir)
    deploy=tool('linuxdeploy',work);maker=tool('appimagetool',work);runtime=tool('runtime',work)
    args=[deploy,'--appimage-extract-and-run','--appdir',appdir,'--executable',appdir/'usr/bin/jetseat-massage','--desktop-file',ROOT/'assets/jetseat-massage.desktop','--icon-file',ROOT/'assets/icons/app-256.png']
    # Desktop file Icon=jetseat-massage must match the supplied icon basename.
    icon=work/'jetseat-massage.png';copy(ROOT/'assets/icons/app-256.png',icon);args[-1]=icon
    args += ['--exclude-library', 'libasound.so.2']
    for name in ['libxkbcommon.so.0','libxkbcommon-x11.so.0']:
        candidates=[Path('/usr/lib64')/name,Path('/usr/lib/x86_64-linux-gnu')/name]
        library=next((p for p in candidates if p.is_file()),None)
        if not library:raise RuntimeError(f'Missing AppImage library: {name}')
        args+=['--library',library]
    run(*args)
    launcher=appdir/'AppRun'
    if launcher.exists() or launcher.is_symlink():launcher.unlink()
    copy(ROOT/'packaging/linux/AppRun',launcher,True)
    # Keep licenses and source-package identities for the unmodified bundled libraries.
    notice=appdir/'usr/share/doc/jetseat-massage/THIRD-PARTY-LINUX.txt'
    text=['AppImage runtime: MIT, https://github.com/AppImage/type2-runtime',
          'Bundled shared libraries are unmodified AlmaLinux 9 packages.',
          'Corresponding source RPMs: https://repo.almalinux.org/almalinux/9/',
          'Runtime build/source and linked library notices: https://github.com/AppImage/type2-runtime/blob/main/LICENSE','']
    for library in sorted((appdir/'usr/lib').glob('*.so*')):
        original=Path('/usr/lib64')/library.name
        if original.exists() and shutil.which('rpm'):
            result=subprocess.run(['rpm','-qf','--qf','%{NAME} %{VERSION}-%{RELEASE} | %{SOURCERPM} | %{LICENSE}\n',str(original)],capture_output=True,text=True)
            if result.returncode==0:text.append(library.name+': '+result.stdout.strip())
    notice.write_text('\n'.join(text)+'\n')
    copy(ROOT/'packaging/linux/AppImage-runtime-LICENSE.txt',appdir/'usr/share/licenses/jetseat-massage/AppImage-runtime-LICENSE.txt')
    output=DIST/f'JetSeat-Massage-{VERSION}-x86_64.AppImage'
    run(maker,'--appimage-extract-and-run','--runtime-file',runtime,appdir,output,env={**os.environ,'ARCH':'x86_64'})
    output.chmod(0o755)

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,required=True)
    parser.add_argument('--format',nargs='+',choices=['deb','rpm','arch','appimage','portable'],default=['portable'])
    parser.add_argument('--max-glibc',default=None)
    args=parser.parse_args();binary=args.binary.resolve()
    header=binary.read_bytes()[:20]
    if header[:4]!=b'\x7fELF' or header[18:20]!=b'\x3e\x00':raise RuntimeError('An existing x86_64 Linux ELF binary is required')
    minimum=glibc_version(binary)
    if args.max_glibc and tuple(map(int,minimum.split('.')))>tuple(map(int,args.max_glibc.split('.'))):raise RuntimeError(f'GLIBC {minimum} exceeds release baseline {args.max_glibc}')
    DIST.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='jetseat-linux-package-') as tmp:
        work=Path(tmp)
        for kind in args.format:
            stage=work/kind;stage.mkdir();common(stage,binary)
            if kind=='deb':deb(stage,minimum)
            elif kind=='rpm':rpm(stage,work)
            elif kind=='arch':arch(stage,minimum,work)
            elif kind=='portable':portable(stage,work)
            else:appimage(stage,work)
    print(f'Packaged {VERSION}: {", ".join(args.format)}; minimum glibc {minimum}')
if __name__=='__main__':main()
