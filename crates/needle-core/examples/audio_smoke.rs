use needle_core::{
    audio::{Command, Player, QueueItem},
    database::Library,
    scan,
};
use std::{
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
fn wait(
    player: &Player,
    predicate: impl Fn(&needle_core::audio::PlaybackState) -> bool,
) -> anyhow::Result<()> {
    let start = Instant::now();
    loop {
        let s = player.state();
        if let Some(e) = s.error {
            anyhow::bail!("{e}");
        }
        if predicate(&s) {
            return Ok(());
        }
        if start.elapsed() > Duration::from_secs(5) {
            anyhow::bail!("Timed out: {s:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn main() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("quiet-48k.wav");
    let mut wav = hound::WavWriter::create(
        &path,
        hound::WavSpec {
            channels: 2,
            sample_rate: 48000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )?;
    for i in 0..48000 * 8 {
        let v = ((i as f32 * 440. * std::f32::consts::TAU / 48000.).sin() * 300.) as i16;
        wav.write_sample(v)?;
        wav.write_sample(v)?;
    }
    wav.finalize()?;
    let lib = Library::open(dir.path().join("library"))?;
    scan::import(&lib, dir.path(), Arc::new(AtomicBool::new(false)), |_| {})?;
    let track = lib.search("")?.remove(0);
    let mut settings = lib.settings()?;
    settings.exclusive = std::env::args().any(|arg| arg == "--exclusive");
    lib.save_settings(&settings)?;
    let player = Player::new(lib.clone());
    player.send(Command::Play(vec![
        QueueItem {
            track: track.clone(),
            reason: "Audio smoke test".into(),
        },
        QueueItem {
            track,
            reason: "Gapless queue smoke test".into(),
        },
    ]));
    wait(&player, |s| s.position > 0.25 && s.current.is_some())?;
    println!(
        "Started: {} Hz / {} ch / exclusive={}",
        player.state().output_rate,
        player.state().output_channels,
        player.state().exclusive
    );
    player.send(Command::Toggle);
    wait(&player, |s| !s.playing)?;
    player.send(Command::Seek(2.));
    wait(&player, |s| s.position >= 2.)?;
    println!("Paused seek: passed");
    player.send(Command::Toggle);
    wait(&player, |s| s.playing && s.position > 2.1)?;
    player.send(Command::Repeat(needle_core::audio::Repeat::One));
    wait(&player, |s| {
        s.repeat == needle_core::audio::Repeat::One && s.queue.len() == 1
    })?;
    player.send(Command::Repeat(needle_core::audio::Repeat::Off));
    wait(&player, |s| {
        s.repeat == needle_core::audio::Repeat::Off && s.queue.len() == 1
    })?;
    println!("Repeat-one preserves the upcoming queue: passed");
    player.shutdown();
    println!("Graceful shutdown: passed");
    let restored = Player::new(lib);
    wait(&restored, |s| s.current.is_some())?;
    assert!(!restored.state().playing);
    assert!(restored.state().position >= 2.);
    assert_eq!(restored.state().queue.len(), 1);
    println!("Session and queue restored paused: passed");
    restored.shutdown();
    Ok(())
}
