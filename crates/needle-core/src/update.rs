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
    /// macOS, Apple silicon: `Needle-<version>-macos.dmg`, holding `Needle.app`.
    MacApp,
}

impl Package {
    /// Whether `name` is this kind of download of Needle `version` for a computer with `arch`
    /// (as in `std::env::consts::ARCH`). Only Needle's own file for exactly this version
    /// fits, so an older or unrelated package in a release is never taken.
    fn fits(self, name: &str, version: &str, arch: &str) -> bool {
        let name = name.to_lowercase();
        match self {
            Self::WindowsInstaller => name == format!("needle-setup-{version}.exe"),
            Self::Deb => {
                let arch = match arch {
                    "x86_64" => "amd64",
                    "aarch64" => "arm64",
                    other => other,
                };
                name == format!("needle_{version}_{arch}.deb")
            }
            Self::Rpm => {
                name.starts_with(&format!("needle-{version}-"))
                    && name.ends_with(&format!(".{arch}.rpm"))
            }
            Self::MacApp => arch == "aarch64" && name == format!("needle-{version}-macos.dmg"),
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
                let Some(program) = system_tool(program) else {
                    return false;
                };
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
    // macOS: an app bundle this user can replace (in Applications, as the disk image and the
    // install script put it).
    #[cfg(target_os = "macos")]
    {
        mac_bundle().map(|_| Package::MacApp)
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

/// The `.app` folder this Needle runs from, when this user can replace it.
#[cfg(target_os = "macos")]
fn mac_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    // <bundle>.app/Contents/MacOS/Needle
    let bundle = exe.parent()?.parent()?.parent()?;
    let folder = bundle.parent()?;
    let writable = |path: &Path| {
        use std::os::unix::ffi::OsStrExt;
        let Ok(text) = std::ffi::CString::new(path.as_os_str().as_bytes()) else {
            return false;
        };
        // SAFETY: a valid C string for the path.
        unsafe { libc::access(text.as_ptr(), libc::W_OK) == 0 }
    };
    (bundle.extension().is_some_and(|e| e == "app") && writable(bundle) && writable(folder))
        .then(|| bundle.to_path_buf())
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
    let version = tag.trim_start_matches(['v', 'V']).to_lowercase();
    let found = package.and_then(|package| {
        let asset = assets.iter().find(|a| {
            a["name"]
                .as_str()
                .is_some_and(|name| package.fits(name, &version, arch))
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

/// Download the installer into a new folder of its own inside `directory` (the system's
/// temporary folder) and check it. The release must list a checksum for it; an unchecked
/// installer is never run. The folder is made fresh, with a random name, readable and
/// writable by this user only, so no one else can swap the file before it is installed.
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
    let folder = private_folder(directory)?;
    let fetch = || -> Result<PathBuf> {
        let path = folder.join(&release.installer);
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
    };
    // A failed download leaves nothing behind.
    let fetched = fetch();
    if fetched.is_err() {
        let _ = std::fs::remove_dir_all(&folder);
    }
    fetched
}

/// A new folder inside `parent` that only this user can use. It must not exist yet, so a
/// folder (or link) someone else put there in advance is never used.
fn private_folder(parent: &Path) -> Result<PathBuf> {
    let folder = parent.join(format!("needle-update-{}", uuid::Uuid::new_v4().simple()));
    #[cfg(unix)]
    let builder = {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = std::fs::DirBuilder::new();
        builder.mode(0o700);
        builder
    };
    #[cfg(not(unix))]
    let builder = std::fs::DirBuilder::new();
    builder.create(&folder).with_context(|| {
        format!(
            "Could not make a folder for the update in {}",
            parent.display()
        )
    })?;
    Ok(folder)
}

/// A system program by its full path, from the places the system keeps its own tools, so a
/// program of the same name elsewhere on `PATH` is never run instead.
#[cfg(unix)]
fn system_tool(name: &str) -> Option<PathBuf> {
    ["/usr/bin", "/bin", "/usr/sbin", "/sbin"]
        .iter()
        .map(|dir| Path::new(dir).join(name))
        .find(|path| path.is_file())
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
                .map_err(|error| {
                    // The installer did not start: its folder goes. When it did, it runs from
                    // there, and Windows clears its temporary folder.
                    if let Some(folder) = installer.parent() {
                        let _ = std::fs::remove_dir_all(folder);
                    }
                    anyhow::anyhow!("Could not start the installer: {error}")
                })?;
            Ok(())
        }
        #[cfg(unix)]
        Some(Package::Deb | Package::Rpm) => {
            // Once the package tool is done (or the password was not given), the downloaded
            // package and its folder go, so tried updates do not pile up.
            let installed = install_linux(release, installer);
            if let Some(folder) = installer.parent() {
                let _ = std::fs::remove_dir_all(folder);
            }
            installed
        }
        #[cfg(not(unix))]
        Some(Package::Deb | Package::Rpm) => bail!("Linux packages install only on Linux."),
        #[cfg(target_os = "macos")]
        Some(Package::MacApp) => {
            let installed = install_mac(installer);
            if let Some(folder) = installer.parent() {
                let _ = std::fs::remove_dir_all(folder);
            }
            installed
        }
        #[cfg(not(target_os = "macos"))]
        Some(Package::MacApp) => bail!("The macOS app installs only on macOS."),
        None => bail!("This copy of Needle cannot update itself."),
    }
}

#[cfg(unix)]
fn install_linux(release: &Release, package: &Path) -> Result<()> {
    let tool = |name: &str| system_tool(name).map(|p| p.to_string_lossy().to_string());
    let package = package.to_string_lossy().to_string();
    let command: Vec<String> = match (
        release.package,
        tool("apt-get"),
        tool("dnf"),
        tool("zypper"),
    ) {
        (Some(Package::Deb), Some(apt), _, _) => {
            vec![
                apt,
                "install".into(),
                "-y".into(),
                "--allow-downgrades".into(),
                package,
            ]
        }
        (Some(Package::Rpm), _, Some(dnf), _) => vec![dnf, "install".into(), "-y".into(), package],
        (Some(Package::Rpm), _, None, Some(zypper)) => vec![
            zypper,
            "--non-interactive".into(),
            "install".into(),
            "--allow-unsigned-rpm".into(),
            package,
        ],
        _ => bail!("No package tool was found to install the update with."),
    };
    let Some(pkexec) = system_tool("pkexec") else {
        bail!(
            "Installing needs pkexec (polkit), which this system does not have. Install the update from the release page instead."
        );
    };
    let status = std::process::Command::new(pkexec)
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
        let _ = std::process::Command::new("/bin/sh")
            .args(["-c", "sleep 1; exec \"$0\""])
            .arg(exe.to_string_lossy().trim_end_matches(" (deleted)"))
            .spawn();
    }
    Ok(())
}

/// Swaps the running `Needle.app` for the one in the checked disk image: the new copy goes
/// next to the old one first, so a failure leaves the old one as it was. Needle starts again
/// a moment after it closes.
#[cfg(target_os = "macos")]
fn install_mac(image: &Path) -> Result<()> {
    let bundle = mac_bundle().context("Needle's app folder cannot be replaced by this user.")?;
    replace_bundle(image, &bundle)?;
    // Start the new Needle once this one has closed (up to 30 s), so `open` does not just
    // bring this one to the front.
    let _ = std::process::Command::new("/bin/sh")
        .args([
            "-c",
            "i=0; while kill -0 \"$1\" 2>/dev/null && [ $i -lt 150 ]; do sleep 0.2; i=$((i+1)); done; exec /usr/bin/open \"$0\"",
        ])
        .arg(&bundle)
        .arg(std::process::id().to_string())
        .spawn();
    Ok(())
}

/// Puts the `Needle.app` from the disk image `image` in place of `bundle`.
#[cfg(target_os = "macos")]
fn replace_bundle(image: &Path, bundle: &Path) -> Result<()> {
    let tool = |name: &str| system_tool(name).with_context(|| format!("{name} is missing"));
    let run = |program: &Path, args: &[&std::ffi::OsStr]| -> Result<()> {
        let status = std::process::Command::new(program)
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .with_context(|| format!("Could not start {}", program.display()))?;
        if !status.success() {
            bail!("{} stopped with {status}", program.display());
        }
        Ok(())
    };
    let hdiutil = tool("hdiutil")?;
    let mount = image.with_extension("mounted");
    std::fs::create_dir(&mount)?;
    let bundle = bundle.to_path_buf();
    run(
        &hdiutil,
        &[
            "attach".as_ref(),
            "-nobrowse".as_ref(),
            "-readonly".as_ref(),
            "-noautoopen".as_ref(),
            "-mountpoint".as_ref(),
            mount.as_os_str(),
            image.as_os_str(),
        ],
    )
    .context("Could not open the disk image")?;
    // Names of the update's own, never there before, so nothing else beside the app is touched.
    let unique = uuid::Uuid::new_v4().simple();
    let fresh = bundle.with_file_name(format!(".Needle-update-{unique}.app"));
    let copied = run(
        &tool("ditto")?,
        &[mount.join("Needle.app").as_os_str(), fresh.as_os_str()],
    );
    let _ = run(
        &hdiutil,
        &["detach".as_ref(), "-quiet".as_ref(), mount.as_os_str()],
    );
    if let Err(error) = copied {
        let _ = std::fs::remove_dir_all(&fresh);
        return Err(error.context("Could not copy the new Needle"));
    }
    // The two are swapped in one step, so an update stopped at any moment leaves a Needle.app
    // that opens. A running program is not disturbed when its folder moves.
    if let Err(error) = swap(&fresh, &bundle) {
        let _ = std::fs::remove_dir_all(&fresh);
        return Err(error.context("Could not put the new Needle in place"));
    }
    // The old copy is where the new one was.
    let _ = std::fs::remove_dir_all(&fresh);
    let _ = std::fs::remove_dir(&mount);
    Ok(())
}

/// Swaps two folders in one step (APFS and HFS+ can).
#[cfg(target_os = "macos")]
fn swap(a: &Path, b: &Path) -> Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let a = std::ffi::CString::new(a.as_os_str().as_bytes())?;
    let b = std::ffi::CString::new(b.as_os_str().as_bytes())?;
    // SAFETY: two valid C strings for the paths.
    if unsafe { libc::renamex_np(a.as_ptr(), b.as_ptr(), libc::RENAME_SWAP) } != 0 {
        return Err(std::io::Error::last_os_error().into());
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
                {"name":"Needle-1.5.0-macos.dmg","browser_download_url":"https://d/dmg"},
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
        // The Mac app is for Apple silicon only.
        assert_eq!(
            pick(Some(Package::MacApp), "aarch64"),
            Some((Some(Package::MacApp), "https://d/dmg".into()))
        );
        assert_eq!(
            pick(Some(Package::MacApp), "x86_64"),
            Some((None, String::new()))
        );
        // An older or unrelated package in the release is never taken.
        let odd: Value = serde_json::from_str(
            r#"{"tag_name":"v1.5.0","assets":[
                {"name":"needle_1.4.3_amd64.deb","browser_download_url":"https://d/old"},
                {"name":"other_1.5.0_amd64.deb","browser_download_url":"https://d/other"}]}"#,
        )
        .unwrap();
        let release = from_json("1.4.3", &odd, Some(Package::Deb), "x86_64").unwrap();
        assert_eq!(release.package, None);
        // A release with only Windows downloads never hands Linux the installer.
        let windows_only: Value = serde_json::from_str(
            r#"{"tag_name":"v1.5.0","assets":[{"name":"Needle-Setup-1.5.0.exe","browser_download_url":"https://d/setup"}]}"#,
        )
        .unwrap();
        let release = from_json("1.4.3", &windows_only, Some(Package::Deb), "x86_64").unwrap();
        assert_eq!(release.package, None);
        assert!(release.installer_url.is_empty());
    }

    /// macOS: an update swaps the app for the one in the disk image, and leaves nothing else.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_mac_update_replaces_the_app_from_the_disk_image() {
        let dir = tempfile::tempdir().unwrap();
        let make = |root: &Path, text: &str| {
            let macos = root.join("Needle.app/Contents/MacOS");
            std::fs::create_dir_all(&macos).unwrap();
            std::fs::write(macos.join("Needle"), text).unwrap();
        };
        let source = dir.path().join("source");
        make(&source, "new");
        let image = dir.path().join("update/Needle-9.9.9-macos.dmg");
        std::fs::create_dir_all(image.parent().unwrap()).unwrap();
        let made = std::process::Command::new("/usr/bin/hdiutil")
            .args([
                "create",
                "-quiet",
                "-fs",
                "HFS+",
                "-format",
                "UDZO",
                "-srcfolder",
            ])
            .arg(&source)
            .arg(&image)
            .status()
            .unwrap();
        assert!(made.success());
        let apps = dir.path().join("Applications");
        make(&apps, "old");
        replace_bundle(&image, &apps.join("Needle.app")).unwrap();
        let installed = std::fs::read_to_string(apps.join("Needle.app/Contents/MacOS/Needle"));
        assert_eq!(installed.unwrap(), "new");
        let left: Vec<_> = std::fs::read_dir(&apps)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(left, ["Needle.app"]);
        assert!(!image.with_extension("mounted").exists());
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
