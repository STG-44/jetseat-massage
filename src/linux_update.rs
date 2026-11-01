//! Distribution-aware Linux package updates. Commands never go through a shell.
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const UNKNOWN: &str =
    "Не удалось определить формат пакета Linux. Используйте AppImage или установите пакет вручную.";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Arch,
    Deb,
    Rpm,
    AppImage,
}
impl Format {
    pub fn target(self) -> &'static str {
        match self {
            Self::Arch => "linux",
            Self::Deb => "linux-deb",
            Self::Rpm => "linux-rpm",
            Self::AppImage => "linux-appimage",
        }
    }
    fn extension(self) -> &'static str {
        match self {
            Self::Arch => ".pkg.tar.zst",
            Self::Deb => ".deb",
            Self::Rpm => ".rpm",
            Self::AppImage => ".AppImage",
        }
    }
}
/// Parse only ID and ID_LIKE. No shell evaluation of os-release content.
pub fn distribution_format(release: &str) -> Option<Format> {
    let value = |key: &str| {
        release
            .lines()
            .find_map(|line| {
                let (name, value) = line.trim().split_once('=')?;
                (name == key).then(|| value.trim().trim_matches(['\'', '"']).to_ascii_lowercase())
            })
            .unwrap_or_default()
    };
    let ids = format!("{} {}", value("ID"), value("ID_LIKE"));
    for id in ids.split_whitespace() {
        match id {
            "arch" | "archlinux" | "cachyos" | "manjaro" | "endeavouros" => {
                return Some(Format::Arch);
            }
            "debian" | "ubuntu" | "linuxmint" | "pop" | "zorin" | "elementary" | "kali" => {
                return Some(Format::Deb);
            }
            "fedora"
            | "rhel"
            | "centos"
            | "almalinux"
            | "rocky"
            | "suse"
            | "opensuse"
            | "opensuse-leap"
            | "opensuse-tumbleweed" => return Some(Format::Rpm),
            _ => {}
        }
    }
    None
}
fn appimage_path() -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var_os("APPIMAGE")?);
    let directory = fs::canonicalize(PathBuf::from(std::env::var_os("APPDIR")?)).ok()?;
    let executable = fs::canonicalize(std::env::current_exe().ok()?).ok()?;
    (path.is_absolute()
        && path.extension().is_some_and(|s| s == "AppImage")
        && executable.starts_with(directory))
    .then_some(path)
}
pub fn runtime_format() -> Result<Format, String> {
    if appimage_path().is_some() {
        return Ok(Format::AppImage);
    }
    let release = fs::read_to_string("/etc/os-release").map_err(|e| e.to_string())?;
    distribution_format(&release).ok_or_else(|| UNKNOWN.into())
}
fn command(format: Format, suse: bool, path: &Path) -> Option<Vec<std::ffi::OsString>> {
    let args: &[&str] = match format {
        Format::Arch => &["/usr/bin/pacman", "-U", "--noconfirm"],
        Format::Deb => &["/usr/bin/apt-get", "install", "--yes"],
        Format::Rpm if suse => &[
            "/usr/bin/zypper",
            "--non-interactive",
            "install",
            "--allow-unsigned-rpm",
        ],
        Format::Rpm => &["/usr/bin/dnf", "install", "--assumeyes"],
        Format::AppImage => return None,
    };
    Some(
        args.iter()
            .map(std::ffi::OsString::from)
            .chain(std::iter::once(path.as_os_str().to_owned()))
            .collect(),
    )
}
#[cfg(unix)]
fn replace_appimage(source: &Path, destination: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = fs::symlink_metadata(destination).map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(UNKNOWN.into());
    }
    let parent = destination.parent().ok_or(UNKNOWN)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    let mut input = fs::File::open(source).map_err(|e| e.to_string())?;
    std::io::copy(&mut input, temporary.as_file_mut()).map_err(|e| e.to_string())?;
    temporary
        .as_file()
        .set_permissions(fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    temporary.persist(destination).map_err(|e| e.to_string())?;
    Ok(())
}
pub fn install(path: &Path) -> Result<(), String> {
    let format = runtime_format()?;
    if !path.is_absolute() || !path.to_string_lossy().ends_with(format.extension()) {
        return Err(UNKNOWN.into());
    }
    if format == Format::AppImage {
        #[cfg(unix)]
        return replace_appimage(path, &appimage_path().ok_or(UNKNOWN)?);
        #[cfg(not(unix))]
        return Err(UNKNOWN.into());
    }
    let args = command(format, Path::new("/usr/bin/zypper").is_file(), path).ok_or(UNKNOWN)?;
    let result = Command::new("/usr/bin/pkexec")
        .args(args)
        .output()
        .map_err(|e| format!("Не удалось открыть установщик: {e}"))?;
    if result.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Установка отменена или не удалась: {}",
            String::from_utf8_lossy(&result.stderr)
                .chars()
                .take(400)
                .collect::<String>()
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recognizes_distribution_families_without_shell_evaluation() {
        for (data, expected) in [
            ("ID=ubuntu\nID_LIKE=debian", Some(Format::Deb)),
            ("ID=linuxmint\nID_LIKE=\"ubuntu debian\"", Some(Format::Deb)),
            ("ID=pop\n", Some(Format::Deb)),
            ("ID=zorin", Some(Format::Deb)),
            ("ID=cachyos\nID_LIKE=arch", Some(Format::Arch)),
            ("ID=manjaro", Some(Format::Arch)),
            (
                "ID=rocky\nID_LIKE=\"rhel centos fedora\"",
                Some(Format::Rpm),
            ),
            (
                "ID=opensuse-tumbleweed\nID_LIKE=\"opensuse suse\"",
                Some(Format::Rpm),
            ),
            ("ID=nixos", None),
            ("ID=alpine", None),
            ("PRETTY_NAME=Ubuntu\nID=$(echo ubuntu)", None),
        ] {
            assert_eq!(distribution_format(data), expected, "{data}");
        }
    }
    #[test]
    fn installer_arguments_preserve_spaces_and_do_not_use_a_shell() {
        let path = Path::new("/home/user/Update files/seat ; test.deb");
        let args = command(Format::Deb, false, path).unwrap();
        assert_eq!(args[0], "/usr/bin/apt-get");
        assert_eq!(args.last().unwrap(), path.as_os_str());
        assert_eq!(args.len(), 4);
        assert!(command(Format::AppImage, false, path).is_none());
        assert!(
            command(Format::Rpm, true, path)
                .unwrap()
                .iter()
                .any(|s| s == "--allow-unsigned-rpm")
        );
    }
    #[cfg(unix)]
    #[test]
    fn appimage_updates_replace_atomically_and_reject_symlinks() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("JetSeat.AppImage");
        let new = dir.path().join("new.AppImage");
        fs::write(&old, b"old version").unwrap();
        fs::write(&new, b"verified new version").unwrap();
        replace_appimage(&new, &old).unwrap();
        assert_eq!(fs::read(&old).unwrap(), b"verified new version");
        assert_eq!(
            fs::metadata(&old).unwrap().permissions().mode() & 0o777,
            0o755
        );
        let link = dir.path().join("link.AppImage");
        symlink(&old, &link).unwrap();
        assert!(replace_appimage(&new, &link).is_err());
        assert_eq!(fs::read(&old).unwrap(), b"verified new version");
    }
}
