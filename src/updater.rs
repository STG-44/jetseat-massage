use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::process::Command;
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

pub const RELEASES_URL: &str = "https://github.com/STG-44/jetseat-massage/releases";
const API: &str = "https://api.github.com/repos/STG-44/jetseat-massage/releases?per_page=20";
const MAX_SIZE: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Release {
    pub version: Version,
    pub name: String,
    pub url: String,
    pub size: u64,
    pub digest: String,
}

#[derive(Clone, Debug)]
pub struct Prepared {
    pub release: Release,
    pub path: PathBuf,
}

#[derive(Debug)]
pub enum Status {
    Idle,
    Checking,
    Current,
    Available(Release),
    Downloading,
    Ready(Prepared),
    Installing,
    Error(String),
}

enum Event {
    Checked(Option<Release>),
    Downloaded(Prepared),
    InstallerOpened,
}

pub struct Updater {
    pub status: Status,
    rx: Option<Receiver<Result<Event, String>>>,
    checked: Option<Instant>,
    pub close_requested: bool,
}

impl Default for Updater {
    fn default() -> Self {
        Self {
            status: Status::Idle,
            rx: None,
            checked: None,
            close_requested: false,
        }
    }
}

impl Updater {
    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }
    pub fn due(&self) -> bool {
        !self.busy()
            && matches!(
                self.status,
                Status::Idle | Status::Current | Status::Error(_)
            )
            && self
                .checked
                .is_none_or(|time| time.elapsed() >= Duration::from_secs(6 * 3600))
    }
    pub fn check(&mut self) {
        if self.busy() {
            return;
        }
        self.checked = Some(Instant::now());
        self.status = Status::Checking;
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(check().map(Event::Checked));
        });
    }
    pub fn download(&mut self) {
        let Status::Available(release) = &self.status else {
            return;
        };
        let release = release.clone();
        self.status = Status::Downloading;
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(download(release).map(Event::Downloaded));
        });
    }
    pub fn poll(&mut self) {
        let Some(rx) = &self.rx else {
            return;
        };
        let result = match rx.try_recv() {
            Ok(value) => value,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err("Поток обновления завершился".into()),
        };
        self.rx = None;
        self.status = match result {
            Ok(Event::Checked(Some(release))) => Status::Available(release),
            Ok(Event::Checked(None)) => Status::Current,
            Ok(Event::Downloaded(prepared)) => Status::Ready(prepared),
            Ok(Event::InstallerOpened) => {
                self.close_requested = true;
                Status::Installing
            }
            Err(error) => Status::Error(error),
        };
    }

    pub fn install(&mut self) {
        let Status::Ready(prepared) = &self.status else {
            return;
        };
        let prepared = prepared.clone();
        self.status = Status::Installing;
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(launch_installer(&prepared).map(|_| Event::InstallerOpened));
        });
    }
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}

fn asset_name(version: &Version, os: &str, arch: &str) -> Option<String> {
    match (os, arch) {
        ("macos", "x86_64") => Some(format!("JetSeat-Massage-{version}-macOS-Intel.pkg")),
        ("macos", "aarch64") => Some(format!("JetSeat-Massage-{version}-macOS-arm64.pkg")),
        ("linux-deb", "x86_64") => Some(format!("jetseat-massage_{version}_amd64.deb")),
        ("linux-rpm", "x86_64") => Some(format!("jetseat-massage-{version}-1.x86_64.rpm")),
        ("linux-appimage", "x86_64") => Some(format!("JetSeat-Massage-{version}-x86_64.AppImage")),
        ("linux", "x86_64") => Some(format!("jetseat-massage-{version}-1-x86_64.pkg.tar.zst")),
        ("windows", "x86_64") => Some(format!("JetSeat-Massage-{version}-Windows-x64-Setup.exe")),
        ("windows", "aarch64") => {
            Some(format!("JetSeat-Massage-{version}-Windows-arm64-Setup.exe"))
        }
        _ => None,
    }
}

fn valid_digest(digest: &str) -> bool {
    digest
        .strip_prefix("sha256:")
        .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn select(releases: Vec<ApiRelease>, current: &Version, os: &str, arch: &str) -> Option<Release> {
    releases
        .into_iter()
        .filter(|r| !r.draft)
        .filter_map(|r| {
            let version = Version::parse(r.tag_name.strip_prefix('v')?).ok()?;
            if &version <= current {
                return None;
            }
            let name = asset_name(&version, os, arch)?;
            let url = format!("{RELEASES_URL}/download/v{version}/{name}");
            let asset = r
                .assets
                .into_iter()
                .find(|a| a.name == name && a.browser_download_url == url)?;
            let digest = asset.digest?;
            if asset.size == 0 || asset.size > MAX_SIZE || !valid_digest(&digest) {
                return None;
            }
            Some(Release {
                version,
                name,
                url,
                size: asset.size,
                digest,
            })
        })
        .max_by(|a, b| a.version.cmp(&b.version))
}

fn agent(timeout: u64) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(timeout)))
        .build()
        .into()
}

pub fn check() -> Result<Option<Release>, String> {
    #[cfg(target_os = "linux")]
    let os = crate::linux_update::runtime_format()?.target();
    #[cfg(not(target_os = "linux"))]
    let os = std::env::consts::OS;
    let mut response = agent(20)
        .get(API)
        .header(
            "User-Agent",
            concat!("JetSeat-Massage/", env!("CARGO_PKG_VERSION")),
        )
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("GitHub недоступен: {e}"))?;
    let bytes = response
        .body_mut()
        .with_config()
        .limit(2 * 1024 * 1024)
        .read_to_vec()
        .map_err(|e| format!("Не удалось прочитать список обновлений: {e}"))?;
    let releases =
        serde_json::from_slice(&bytes).map_err(|e| format!("Некорректный ответ GitHub: {e}"))?;
    let version = Version::parse(env!("CARGO_PKG_VERSION")).map_err(|e| e.to_string())?;
    Ok(select(releases, &version, os, std::env::consts::ARCH))
}

fn verify(mut reader: impl Read, release: &Release, mut output: impl Write) -> Result<(), String> {
    let mut hash = Sha256::new();
    let mut length = 0_u64;
    let mut buffer = [0_u8; 32768];
    loop {
        let size = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if size == 0 {
            break;
        }
        length += size as u64;
        if length > release.size || length > MAX_SIZE {
            return Err("Размер обновления не совпадает".into());
        }
        hash.update(&buffer[..size]);
        output
            .write_all(&buffer[..size])
            .map_err(|e| e.to_string())?;
    }
    if length != release.size
        || format!("sha256:{:x}", hash.finalize()) != release.digest.to_ascii_lowercase()
    {
        return Err("Проверка SHA-256 обновления не пройдена. Файл не будет установлен.".into());
    }
    Ok(())
}

fn download(release: Release) -> Result<Prepared, String> {
    let dirs = directories_next::ProjectDirs::from("dev", "JetSeat", "JetSeat Massage")
        .ok_or("Не удалось найти каталог обновлений")?;
    let cache = dirs.cache_dir().join("updates");
    fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let destination = cache.join(&release.name);
    if let Ok(file) = File::open(&destination)
        && verify(file, &release, std::io::sink()).is_ok()
    {
        return Ok(Prepared {
            release,
            path: destination,
        });
    }
    let mut response = agent(180)
        .get(&release.url)
        .header(
            "User-Agent",
            concat!("JetSeat-Massage/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|e| format!("Не удалось скачать обновление: {e}"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(&cache).map_err(|e| e.to_string())?;
    verify(
        response.body_mut().as_reader(),
        &release,
        temporary.as_file_mut(),
    )?;
    temporary
        .as_file_mut()
        .sync_all()
        .map_err(|e| e.to_string())?;
    temporary.persist(&destination).map_err(|e| e.to_string())?;
    Ok(Prepared {
        release,
        path: destination,
    })
}

pub fn launch_installer(prepared: &Prepared) -> Result<(), String> {
    verify(
        File::open(&prepared.path).map_err(|e| e.to_string())?,
        &prepared.release,
        std::io::sink(),
    )?;
    launch(&prepared.path)
}

fn launch(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Command::new("/usr/bin/open")
            .arg("-a")
            .arg("Installer")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Не удалось открыть установщик: {e}"))
    }
    #[cfg(target_os = "linux")]
    {
        crate::linux_update::install(path)
    }
    #[cfg(target_os = "windows")]
    {
        Command::new(path)
            .arg(format!("/UPDATEPID={}", std::process::id()))
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Не удалось открыть установщик: {e}"))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Err("Установка для этой системы не поддерживается".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release(version: &str, data: &[u8]) -> ApiRelease {
        let version = Version::parse(version).unwrap();
        let name = asset_name(&version, "macos", "x86_64").unwrap();
        ApiRelease {
            tag_name: format!("v{version}"),
            draft: false,
            assets: vec![ApiAsset {
                browser_download_url: format!("{RELEASES_URL}/download/v{version}/{name}"),
                name,
                size: data.len() as u64,
                digest: Some(format!("sha256:{:x}", Sha256::digest(data))),
            }],
        }
    }
    #[test]
    fn versions_platform_and_untrusted_metadata_are_filtered() {
        let current = Version::parse("0.3.0").unwrap();
        let mut draft = release("1.0.0", b"data");
        draft.draft = true;
        let mut hostile = release("3.0.0", b"data");
        hostile.assets[0].browser_download_url = "https://evil.invalid/update.pkg".into();
        let mut no_digest = release("4.0.0", b"data");
        no_digest.assets[0].digest = None;
        let selected = select(
            vec![
                draft,
                hostile,
                no_digest,
                release("0.2.0", b"old"),
                release("0.9.0", b"data"),
                release("0.10.0", b"data"),
            ],
            &current,
            "macos",
            "x86_64",
        )
        .unwrap();
        assert_eq!(selected.version.to_string(), "0.10.0");
        assert!(select(vec![release("0.4.0", b"data")], &current, "linux", "x86_64").is_none());
    }
    #[test]
    fn integrity_checks_reject_corrupt_truncated_and_oversized_downloads() {
        let r = select(
            vec![release("0.4.0", b"package")],
            &Version::parse("0.3.0").unwrap(),
            "macos",
            "x86_64",
        )
        .unwrap();
        assert!(verify(&b"package"[..], &r, std::io::sink()).is_ok());
        for data in [&b"corrupt"[..], &b"pack"[..], &b"package extra"[..]] {
            assert!(verify(data, &r, std::io::sink()).is_err());
        }
    }

    #[test]
    fn selects_only_the_matching_cpu_and_os_installer() {
        let version = Version::parse("0.4.0").unwrap();
        let targets = [
            ("macos", "x86_64"),
            ("macos", "aarch64"),
            ("windows", "x86_64"),
            ("windows", "aarch64"),
            ("linux", "x86_64"),
            ("linux-deb", "x86_64"),
            ("linux-rpm", "x86_64"),
            ("linux-appimage", "x86_64"),
        ];
        let assets = targets
            .iter()
            .map(|(os, arch)| {
                let name = asset_name(&version, os, arch).unwrap();
                ApiAsset {
                    browser_download_url: format!("{RELEASES_URL}/download/v{version}/{name}"),
                    name,
                    size: 7,
                    digest: Some(format!("sha256:{:x}", Sha256::digest(b"package"))),
                }
            })
            .collect::<Vec<_>>();
        let json = serde_json::json!([{"tag_name":"v0.4.0", "assets":assets.iter().map(|a|serde_json::json!({"name":a.name,"browser_download_url":a.browser_download_url,"size":a.size,"digest":a.digest})).collect::<Vec<_>>()}]);
        for (os, arch) in targets {
            let selected = select(
                serde_json::from_value(json.clone()).unwrap(),
                &Version::parse("0.3.0").unwrap(),
                os,
                arch,
            )
            .unwrap();
            assert_eq!(selected.name, asset_name(&version, os, arch).unwrap());
        }
        assert!(asset_name(&version, "windows", "arm").is_none());
    }
}
