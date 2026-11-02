//! Cooperative per-device ownership across JetSeat processes. The file stays in
//! the cache; the OS releases its lock automatically after exit or a crash.
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    path::Path,
};
const BUSY: &str = "Накидка уже используется другим экземпляром JetSeat. Остановите сеанс в нём и повторите подключение.";
pub struct DeviceLock {
    _file: File,
}
impl DeviceLock {
    pub fn acquire(identity: &[u8]) -> Result<Self, String> {
        let directories = directories_next::ProjectDirs::from("dev", "JetSeat", "JetSeat Massage")
            .ok_or("Не удалось найти каталог настроек")?;
        Self::in_directory(&directories.cache_dir().join("devices"), identity)
    }
    fn in_directory(directory: &Path, identity: &[u8]) -> Result<Self, String> {
        std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
        let path = directory.join(format!("{:x}.lock", Sha256::digest(identity)));
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0);
        }
        let file = options.open(path).map_err(|_| BUSY.to_owned())?;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            unsafe extern "C" {
                fn flock(fd: i32, operation: i32) -> i32;
            }
            // LOCK_EX | LOCK_NB. The descriptor owns the advisory lock.
            if unsafe { flock(file.as_raw_fd(), 2 | 4) } != 0 {
                return Err(BUSY.into());
            }
        }
        Ok(Self { _file: file })
    }
}
impl Drop for DeviceLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            unsafe extern "C" {
                fn flock(fd: i32, operation: i32) -> i32;
            }
            // Explicit unlock also covers a short-lived fork/exec duplicate of
            // this open file description. The HID handle has already closed.
            let _ = unsafe { flock(self._file.as_raw_fd(), 8) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_a_second_owner_and_releases_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let first = DeviceLock::in_directory(dir.path(), b"test seat").unwrap();
        assert!(DeviceLock::in_directory(dir.path(), b"test seat").is_err());
        assert!(DeviceLock::in_directory(dir.path(), b"another seat").is_ok());
        drop(first);
        assert!(DeviceLock::in_directory(dir.path(), b"test seat").is_ok());
    }
    #[test]
    fn child_process_respects_the_device_owner() {
        if let Some(path) = std::env::var_os("JETSEAT_DEVICE_LOCK_CHILD") {
            let result = DeviceLock::in_directory(Path::new(&path), b"shared seat");
            assert_eq!(
                result.is_ok(),
                std::env::var("JETSEAT_DEVICE_LOCK_EXPECT").as_deref() == Ok("free")
            );
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let first = DeviceLock::in_directory(dir.path(), b"shared seat").unwrap();
        let invoke = |expected: &str| {
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "device_lock::tests::child_process_respects_the_device_owner",
                ])
                .env("JETSEAT_DEVICE_LOCK_CHILD", dir.path())
                .env("JETSEAT_DEVICE_LOCK_EXPECT", expected)
                .output()
                .unwrap()
        };
        assert!(invoke("busy").status.success());
        drop(first);
        assert!(invoke("free").status.success());
    }
}
