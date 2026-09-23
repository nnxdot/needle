//! Updates: ask needle.nnx.fyi for Needle's latest release, and install a newer one with its
//! installer after checking the download against the release's SHA-256 list.
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
    pub installer: String,
    pub installer_url: String,
    /// The release's SHA256SUMS.txt, if it has one.
    pub checksums_url: Option<String>,
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

/// The release, if it is newer than `current` and has a Windows installer.
pub(crate) fn from_json(current: &str, release: &Value) -> Option<Release> {
    if release["draft"].as_bool() == Some(true) || release["prerelease"].as_bool() == Some(true) {
        return None;
    }
    let tag = release["tag_name"].as_str()?;
    if !newer(current, tag) {
        return None;
    }
    let assets = release["assets"].as_array()?;
    let url = |a: &Value| a["browser_download_url"].as_str().map(str::to_string);
    let installer = assets.iter().find(|a| {
        a["name"].as_str().is_some_and(|n| {
            n.to_lowercase().contains("setup") && n.to_lowercase().ends_with(".exe")
        })
    })?;
    Some(Release {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        page: release["html_url"].as_str().unwrap_or_default().to_string(),
        installer: installer["name"].as_str()?.to_string(),
        installer_url: url(installer)?,
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
    Ok(from_json(current, &latest))
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

/// Run the checked installer quietly; it closes Needle, updates it, and starts it again.
pub fn install(installer: &Path) -> Result<()> {
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
    fn reads_releases_and_checksums() {
        let json: Value = serde_json::from_str(
            r#"{"tag_name":"v1.1.0","html_url":"https://github.com/x/y/releases/v1.1.0","draft":false,"prerelease":false,
            "assets":[{"name":"Needle-1.1.0-windows-x64.zip","browser_download_url":"https://d/zip"},
                      {"name":"Needle-Setup-1.1.0.exe","browser_download_url":"https://d/setup"},
                      {"name":"SHA256SUMS.txt","browser_download_url":"https://d/sums"}]}"#,
        )
        .unwrap();
        let release = from_json("1.0.0", &json).unwrap();
        assert_eq!(release.version, "1.1.0");
        assert_eq!(release.installer_url, "https://d/setup");
        assert_eq!(release.checksums_url.as_deref(), Some("https://d/sums"));
        assert!(from_json("1.1.0", &json).is_none());
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
