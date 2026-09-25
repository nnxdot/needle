//! Updates: ask needle.nnx.fyi for Needle's latest release, and install a newer one after
//! checking the download against the release's SHA-256 list. Each system takes only its own
//! kind of download: the installer on Windows; on Linux the .deb or .rpm, for the package
//! system that installed Needle and this computer's processor.
use anyhow::{Context, Result, bail};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Describes the latest release in the shape of a GitHub release (`tag_name`, `assets`).
pub const LATEST: &str = "https://needle.nnx.fyi/latest.json";

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: String,
    pub page: String,
    /// The download Needle installs by itself; `None` when this copy of Needle was not
    /// installed in a way it can update (then the download page is offered).
    pub package: Option<Package>,
    pub installer: String,
    pub installer_url: String,
    /// The release's SHA256SUMS.txt, if it has one.
    pub checksums_url: Option<String>,
}

/// How this copy of Needle was installed, which decides what it downloads to update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Package {
    /// Windows: `Needle-Setup-<version>.exe`.
    WindowsInstaller,
    /// Debian, Ubuntu, Mint: `needle_<version>_amd64.deb` (or `_arm64`).
    Deb,
    /// Fedora, openSUSE: `needle-<version>-1.x86_64.rpm` (or `.aarch64`).
    Rpm,
}

impl Package {
    /// Whether `name` is this kind of download for a computer with `arch` (as in
    /// `std::env::consts::ARCH`).
    fn fits(self, name: &str, arch: &str) -> bool {
        let name = name.to_lowercase();
        match self {
            Self::WindowsInstaller => name.contains("setup") && name.ends_with(".exe"),
            Self::Deb => {
                let arch = match arch {
                    "x86_64" => "amd64",
                    "aarch64" => "arm64",
                    other => other,
                };
                name.ends_with(&format!("_{arch}.deb"))
            }
            Self::Rpm => name.ends_with(&format!(".{arch}.rpm")),
        }
    }
}

/// How this copy of Needle was installed: `None` for a copy run from a folder (or a build
/// from source), which Needle does not update by itself.
pub fn installed_package() -> Option<Package> {
    #[cfg(windows)]
    {
        Some(Package::WindowsInstaller)
    }
    #[cfg(target_os = "linux")]
    {
        static FOUND: std::sync::OnceLock<Option<Package>> = std::sync::OnceLock::new();
        *FOUND.get_or_init(|| {
            let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
            let owns = |program: &str, flag: &str| {
                std::process::Command::new(program)
                    .arg(flag)
                    .arg(&exe)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .is_ok_and(|s| s.success())
            };
            if owns("dpkg-query", "-S") {
                Some(Package::Deb)
            } else if owns("rpm", "-qf") {
                Some(Package::Rpm)
            } else {
                None
            }
        })
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        None
    }
}

fn parts(version: &str) -> Vec<u64> {
    version
        .trim_start_matches(['v', 'V'])
        .split(['.', '-', '+'])
        .map_while(|p| p.parse().ok())
        .collect()
}

/// Whether `tag` is a later version than `current` ("v1.2.0" > "1.1.9").
pub fn newer(current: &str, tag: &str) -> bool {
    let (mut a, mut b) = (parts(current), parts(tag));
    let length = a.len().max(b.len());
    a.resize(length, 0);
    b.resize(length, 0);
    b > a
}

/// The release, if it is newer than `current`, with the download for `package` on `arch`.
/// With no package (or none for it in this release) it is offered for the download page.
pub(crate) fn from_json(
    current: &str,
    release: &Value,
    package: Option<Package>,
    arch: &str,
) -> Option<Release> {
    if release["draft"].as_bool() == Some(true) || release["prerelease"].as_bool() == Some(true) {
        return None;
    }
    let tag = release["tag_name"].as_str()?;
    if !newer(current, tag) {
        return None;
    }
    let assets = release["assets"].as_array()?;
    let url = |a: &Value| a["browser_download_url"].as_str().map(str::to_string);
    let found = package.and_then(|package| {
        let asset = assets.iter().find(|a| {
            a["name"]
                .as_str()
                .is_some_and(|name| package.fits(name, arch))
        })?;
        Some((package, asset["name"].as_str()?.to_string(), url(asset)?))
    });
    let (package, installer, installer_url) = match found {
        Some((package, name, link)) => (Some(package), name, link),
        None => (None, String::new(), String::new()),
    };
    Some(Release {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        page: release["html_url"].as_str().unwrap_or_default().to_string(),
        package,
        installer,
        installer_url,
        checksums_url: assets
            .iter()
            .find(|a| a["name"].as_str() == Some("SHA256SUMS.txt"))
            .and_then(url),
    })
}

/// Ask needle.nnx.fyi whether a newer Needle is out. Sends nothing but the request itself.
pub fn check(current: &str) -> Result<Option<Release>> {
    let latest: Value = crate::integrations::client()?
        .get(LATEST)
        .send()?
        .error_for_status()?
        .json()?;
    Ok(from_json(
        current,
        &latest,
        installed_package(),
        std::env::consts::ARCH,
    ))
}

/// The expected hash for `name` in a SHA256SUMS list (`<hash>  <name>` per line).
pub(crate) fn expected_hash(list: &str, name: &str) -> Option<String> {
    list.lines().find_map(|line| {
        let (hash, file) = line.split_once(char::is_whitespace)?;
        (file.trim().trim_start_matches('*') == name).then(|| hash.trim().to_lowercase())
    })
}

pub(crate) fn sha256_file(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut file = std::fs::File::open(path)?;
    std::io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// Download the installer into `directory` and check it. The release must list a checksum for
/// it; an unchecked installer is never run.
pub fn download(release: &Release, directory: &Path) -> Result<PathBuf> {
    if release.package.is_none() {
        bail!(
            "This copy of Needle cannot update itself. Download the new version from the release page."
        );
    }
    let client = crate::integrations::client()?;
    let list_url = release.checksums_url.as_ref().context("This release has no checksum list, so Needle will not install it by itself. Download it from the release page instead.")?;
    let list = client.get(list_url).send()?.error_for_status()?.text()?;
    let expected = expected_hash(&list, &release.installer)
        .context("The checksum list does not include the installer.")?;
    std::fs::create_dir_all(directory)?;
    let path = directory.join(&release.installer);
    let mut response = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()?
        .get(&release.installer_url)
        .send()?
        .error_for_status()?;
    let mut file = std::fs::File::create(&path)?;
    std::io::copy(&mut response, &mut file)?;
    drop(file);
    let actual = sha256_file(&path)?;
    if actual != expected {
        let _ = std::fs::remove_file(&path);
        bail!("The download did not match its checksum, so it was deleted.");
    }
    Ok(path)
}

/// Install the checked download. On Windows the installer runs quietly: it closes Needle,
/// updates it, and starts it again. On Linux the system's package tool installs it, after
/// the system asks for the password; once that worked, Needle starts again a moment after it
/// closes. When the password is not given, nothing changes.
pub fn install(release: &Release, installer: &Path) -> Result<()> {
    match release.package {
        Some(Package::WindowsInstaller) => {
            std::process::Command::new(installer)
                .args([
                    "/SILENT",
                    "/SUPPRESSMSGBOXES",
                    "/NORESTART",
                    "/CLOSEAPPLICATIONS",
                ])
                .spawn()
                .context("Could not start the installer")?;
            Ok(())
        }
        Some(Package::Deb | Package::Rpm) => install_linux(release, installer),
        None => bail!("This copy of Needle cannot update itself."),
    }
}

fn install_linux(release: &Release, package: &Path) -> Result<()> {
    let on_path = |program: &str| {
        std::env::var_os("PATH").is_some_and(|paths| {
            std::env::split_paths(&paths).any(|dir| dir.join(program).is_file())
        })
    };
    let package = package.to_string_lossy().to_string();
    let command: Vec<&str> = match release.package {
        Some(Package::Deb) => vec!["apt-get", "install", "-y", "--allow-downgrades", &package],
        Some(Package::Rpm) if on_path("dnf") => vec!["dnf", "install", "-y", &package],
        Some(Package::Rpm) if on_path("zypper") => vec![
            "zypper",
            "--non-interactive",
            "install",
            "--allow-unsigned-rpm",
            &package,
        ],
        _ => bail!("No package tool was found to install the update with."),
    };
    if !on_path("pkexec") {
        bail!(
            "Installing needs pkexec (polkit), which this system does not have. Install the update from the release page instead."
        );
    }
    let status = std::process::Command::new("pkexec")
        .args(&command)
        .status()
        .context("Could not start pkexec")?;
    if !status.success() {
        bail!(
            "The update was not installed (the password was not given, or the package tool stopped)."
        );
    }
    // Start the new Needle once this one has closed, so it does not find this one running.
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new("sh")
            .args(["-c", "sleep 1; exec \"$0\""])
            .arg(exe.to_string_lossy().trim_end_matches(" (deleted)"))
            .spawn();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions() {
        assert!(newer("0.9.1", "v1.0.0"));
        assert!(newer("1.0.0", "1.0.1"));
        assert!(newer("1.0", "1.0.1"));
        assert!(!newer("1.0.0", "v1.0.0"));
        assert!(!newer("1.2.0", "1.1.9"));
        assert!(!newer("1.0.0", "nightly"));
    }

    #[test]
    fn each_system_takes_only_its_own_download() {
        let json: Value = serde_json::from_str(
            r#"{"tag_name":"v1.5.0","html_url":"https://needle.nnx.fyi/","assets":[
                {"name":"Needle-Setup-1.5.0.exe","browser_download_url":"https://d/setup"},
                {"name":"Needle-1.5.0-windows-x64.zip","browser_download_url":"https://d/zip"},
                {"name":"needle_1.5.0_amd64.deb","browser_download_url":"https://d/deb"},
                {"name":"needle_1.5.0_arm64.deb","browser_download_url":"https://d/deb-arm"},
                {"name":"needle-1.5.0-1.x86_64.rpm","browser_download_url":"https://d/rpm"},
                {"name":"SHA256SUMS.txt","browser_download_url":"https://d/sums"}]}"#,
        )
        .unwrap();
        let pick = |package, arch| {
            from_json("1.4.3", &json, package, arch).map(|r| (r.package, r.installer_url))
        };
        assert_eq!(
            pick(Some(Package::WindowsInstaller), "x86_64"),
            Some((Some(Package::WindowsInstaller), "https://d/setup".into()))
        );
        assert_eq!(
            pick(Some(Package::Deb), "x86_64"),
            Some((Some(Package::Deb), "https://d/deb".into()))
        );
        assert_eq!(
            pick(Some(Package::Deb), "aarch64"),
            Some((Some(Package::Deb), "https://d/deb-arm".into()))
        );
        assert_eq!(
            pick(Some(Package::Rpm), "x86_64"),
            Some((Some(Package::Rpm), "https://d/rpm".into()))
        );
        // No .rpm for ARM in this release, and a copy run from a folder: the download page.
        assert_eq!(
            pick(Some(Package::Rpm), "aarch64"),
            Some((None, String::new()))
        );
        assert_eq!(pick(None, "x86_64"), Some((None, String::new())));
        // A release with only Windows downloads never hands Linux the installer.
        let windows_only: Value = serde_json::from_str(
            r#"{"tag_name":"v1.5.0","assets":[{"name":"Needle-Setup-1.5.0.exe","browser_download_url":"https://d/setup"}]}"#,
        )
        .unwrap();
        let release = from_json("1.4.3", &windows_only, Some(Package::Deb), "x86_64").unwrap();
        assert_eq!(release.package, None);
        assert!(release.installer_url.is_empty());
    }

    #[test]
    fn reads_releases_and_checksums() {
        let json: Value = serde_json::from_str(
            r#"{"tag_name":"v1.1.0","html_url":"https://github.com/x/y/releases/v1.1.0","draft":false,"prerelease":false,
            "assets":[{"name":"Needle-1.1.0-windows-x64.zip","browser_download_url":"https://d/zip"},
                      {"name":"Needle-Setup-1.1.0.exe","browser_download_url":"https://d/setup"},
                      {"name":"SHA256SUMS.txt","browser_download_url":"https://d/sums"}]}"#,
        )
        .unwrap();
        let windows = Some(Package::WindowsInstaller);
        let release = from_json("1.0.0", &json, windows, "x86_64").unwrap();
        assert_eq!(release.version, "1.1.0");
        assert_eq!(release.installer_url, "https://d/setup");
        assert_eq!(release.checksums_url.as_deref(), Some("https://d/sums"));
        assert!(from_json("1.1.0", &json, windows, "x86_64").is_none());
        let list = "ABC123  Needle-Setup-1.1.0.exe\ndef  other.zip\n";
        assert_eq!(
            expected_hash(list, "Needle-Setup-1.1.0.exe").as_deref(),
            Some("abc123")
        );
        assert_eq!(expected_hash(list, "missing.exe"), None);
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f");
        std::fs::write(&file, b"abc").unwrap();
        assert_eq!(
            sha256_file(&file).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
