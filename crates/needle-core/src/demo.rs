use anyhow::Result;
use lofty::{
    config::WriteOptions,
    prelude::{Accessor, ItemKey, TagExt},
    tag::{Tag, TagType},
};
use std::{f32::consts::TAU, path::Path};

pub fn create(directory: &Path) -> Result<()> {
    std::fs::create_dir_all(directory)?;
    for (index, (name, root, bpm)) in [
        ("A room with a view", 130.81f32, 92.0),
        ("Slow orbit", 110.0, 78.0),
        ("Last light", 146.83, 104.0),
    ]
    .iter()
    .enumerate()
    {
        let path = directory.join(format!("{:02} - {name}.wav", index + 1));
        if path.exists() {
            continue;
        }
        let rate = 44100;
        let duration = 24.0;
        let mut writer = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 2,
                sample_rate: rate,
                bits_per_sample: 16,
                sample_format: hound::SampleFormat::Int,
            },
        )?;
        for sample in 0..(rate as f32 * duration) as u32 {
            let t = sample as f32 / rate as f32;
            let fade = (t / 1.5).min(1.0) * ((duration - t) / 2.0).clamp(0.0, 1.0);
            let beat = t * bpm / 60.0;
            let pulse = (-7.0 * beat.fract()).exp();
            let chord = (TAU * root * t).sin() * 0.05
                + (TAU * root * 1.5 * t).sin() * 0.035
                + (TAU * root * 2.5 * t).sin() * 0.025;
            let melody_note =
                [2.0, 2.5, 3.0, 3.75, 3.0, 2.5, 2.25, 1.5][((beat / 2.0) as usize) % 8];
            let melody =
                (TAU * root * melody_note * t).sin() * (-2.0 * (beat / 2.0).fract()).exp() * 0.07;
            let kick = (TAU * 48.0 * t).sin() * pulse * 0.06;
            let left = (chord + melody + kick) * fade;
            let right = (chord + melody * (0.85 + 0.15 * (t * 0.5).sin()) + kick) * fade;
            writer.write_sample((left * i16::MAX as f32) as i16)?;
            writer.write_sample((right * i16::MAX as f32) as i16)?;
        }
        writer.finalize()?;
        let mut tag = Tag::new(TagType::Id3v2);
        tag.set_title(name.to_string());
        tag.set_artist("Needle Studio".into());
        tag.set_album("First listening · Demo recordings".into());
        tag.set_track(index as u32 + 1);
        tag.insert_text(ItemKey::RecordingDate, "2026".into());
        tag.set_genre("Ambient".into());
        tag.insert_text(ItemKey::IntegerBpm, bpm.to_string());
        tag.save_to_path(&path, WriteOptions::default())?;
    }
    Ok(())
}
