//! One Needle at a time per library: a second launch (say, opening a song from File Explorer)
//! hands its files to the running Needle over a local connection and exits.
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Serialize, Deserialize)]
struct Door {
    port: u16,
    /// Only a caller that can read the library folder knows it.
    token: String,
}

fn door_file(directory: &Path) -> PathBuf {
    directory.join("instance.json")
}

/// Give `files` (maybe none: then just come to the front) to a running Needle. Returns
/// whether one took them.
pub fn hand_over(directory: &Path, files: &[PathBuf]) -> bool {
    let Some(door) = std::fs::read_to_string(door_file(directory))
        .ok()
        .and_then(|t| serde_json::from_str::<Door>(&t).ok())
    else {
        return false;
    };
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, door.port));
    let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(500)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let mut message = format!("{}\n", door.token);
    for file in files {
        let full = std::path::absolute(file).unwrap_or_else(|_| file.clone());
        message.push_str(&format!("{}\n", full.to_string_lossy()));
    }
    message.push('\n');
    if stream.write_all(message.as_bytes()).is_err() {
        return false;
    }
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).is_ok() && reply.trim() == "ok"
}

/// Listen for later launches; `arrived` gets each launch's files.
pub fn listen(directory: &Path, arrived: impl Fn(Vec<PathBuf>) + Send + 'static) -> Result<()> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    let token = format!("{:032x}", rand::random::<u128>());
    let door = Door {
        port: listener.local_addr()?.port(),
        token: token.clone(),
    };
    std::fs::write(door_file(directory), serde_json::to_string(&door)?)?;
    std::thread::Builder::new()
        .name("needle-door".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                let Ok(mut writer) = stream.try_clone() else {
                    continue;
                };
                let mut lines = BufReader::new(stream).lines();
                if lines.next().and_then(Result::ok).as_deref() != Some(token.as_str()) {
                    continue;
                }
                let files: Vec<PathBuf> = lines
                    .map_while(Result::ok)
                    .take_while(|l| !l.is_empty())
                    .map(PathBuf::from)
                    .collect();
                let _ = writer.write_all(b"ok\n");
                arrived(files);
            }
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_launch_hands_over_its_files() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!hand_over(dir.path(), &[]), "nobody is listening yet");
        let (tx, rx) = std::sync::mpsc::channel();
        listen(dir.path(), move |files| tx.send(files).unwrap()).unwrap();
        let song = dir.path().join("a song.flac");
        assert!(hand_over(dir.path(), std::slice::from_ref(&song)));
        assert_eq!(rx.recv_timeout(Duration::from_secs(3)).unwrap(), vec![song]);
        assert!(hand_over(dir.path(), &[]));
        assert!(rx.recv_timeout(Duration::from_secs(3)).unwrap().is_empty());
        // A caller without the token is ignored.
        let door: Door =
            serde_json::from_str(&std::fs::read_to_string(door_file(dir.path())).unwrap()).unwrap();
        let mut stranger = TcpStream::connect((Ipv4Addr::LOCALHOST, door.port)).unwrap();
        stranger.write_all(b"wrong\nC:\\evil.flac\n\n").unwrap();
        assert!(rx.recv_timeout(Duration::from_millis(500)).is_err());
    }
}
