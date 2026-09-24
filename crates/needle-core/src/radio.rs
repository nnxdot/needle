//! Offline radio: each song's sound (tempo, key, energy, brightness, pulse, and tone colour),
//! measured on this computer, and radio stations built from songs that sound alike.
use crate::{database::Library, model::Track};
use anyhow::{Result, bail};
use realfft::RealFftPlanner;
use rodio::Source;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Bump when the measurements change, so songs are measured again.
pub const VERSION: u32 = 1;
const BANDS: usize = 16;
/// How much of a song to listen to, from a fifth of the way in.
const LISTEN_SECONDS: f64 = 60.;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Features {
    pub version: u32,
    /// Beats per minute; 0 when no steady beat was found.
    pub tempo: f32,
    /// How clear the beat is, 0 to 1.
    pub pulse: f32,
    /// 0 = C … 11 = B.
    pub key: u8,
    pub minor: bool,
    /// How sure the key is, 0 to 1.
    pub key_strength: f32,
    /// Loudness, 0 (quiet) to 1 (loud).
    pub energy: f32,
    /// Where the sound's weight sits, 0 (dark) to 1 (bright).
    pub brightness: f32,
    /// The spectrum's shape across bands: zero mean, unit length. Empty when the song could
    /// not be measured.
    pub timbre: Vec<f32>,
}

const KEYS: [&str; 12] = [
    "C", "C♯", "D", "E♭", "E", "F", "F♯", "G", "A♭", "A", "B♭", "B",
];

impl Features {
    pub fn usable(&self) -> bool {
        self.timbre.len() == BANDS
    }
    pub fn key_name(&self) -> String {
        format!(
            "{} {}",
            KEYS[self.key as usize % 12],
            if self.minor { "minor" } else { "major" }
        )
    }
    /// A word for the feel: how lively crossed with how bright or dark.
    pub fn mood(&self) -> &'static str {
        let tempo = if self.tempo > 0. {
            ((self.tempo - 70.) / 90.).clamp(0., 1.)
        } else {
            0.4
        };
        let lively = 0.65 * self.energy + 0.35 * tempo;
        let bright = 0.55 * self.brightness + if self.minor { 0.1 } else { 0.35 };
        match (lively > 0.55, bright > 0.5) {
            (true, true) => "Upbeat",
            (true, false) => "Intense",
            (false, true) => "Mellow",
            (false, false) => "Melancholy",
        }
    }
    /// "124 BPM · A minor · Upbeat".
    pub fn summary(&self) -> String {
        let mut parts = vec![];
        if self.tempo > 0. {
            parts.push(format!("{:.0} BPM", self.tempo));
        }
        if self.key_strength > 0.3 {
            parts.push(self.key_name());
        }
        parts.push(self.mood().to_string());
        parts.join(" · ")
    }
}

/// How different two songs sound: 0 for the same, about 1 for unrelated, more for opposites.
pub fn distance(a: &Features, b: &Features) -> f32 {
    let timbre = (1.
        - a.timbre
            .iter()
            .zip(&b.timbre)
            .map(|(x, y)| x * y)
            .sum::<f32>())
        / 2.;
    let tempo = if a.tempo > 0. && b.tempo > 0. {
        // Double and half time feel alike.
        let ratio = (a.tempo / b.tempo).log2().abs();
        (ratio.min((ratio - 1.).abs()) / 0.25).min(1.)
    } else {
        0.5
    };
    // Around the circle of fifths, minor keys with their relative major.
    let fifths = |f: &Features| ((f.key as usize + if f.minor { 3 } else { 0 }) * 7 % 12) as i32;
    let steps = (fifths(a) - fifths(b)).abs();
    let key = steps.min(12 - steps) as f32 / 6. * a.key_strength.min(b.key_strength);
    1.0 * timbre
        + 1.2 * (a.energy - b.energy).abs()
        + 0.6 * (a.brightness - b.brightness).abs()
        + 0.5 * tempo
        + 0.3 * key
        + 0.3 * (a.pulse - b.pulse).abs()
}

// ---------------------------------------------------------------- measuring

fn hann(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos())
        .collect()
}

/// Measure mono samples at `rate`.
pub fn measure(mono: &[f32], rate: u32) -> Features {
    let size = if rate >= 32_000 { 4096 } else { 2048 };
    let hop = size / 4;
    if mono.len() < size * 8 {
        return Features {
            version: VERSION,
            ..Default::default()
        };
    }
    let window = hann(size);
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(size);
    let mut input = fft.make_input_vec();
    let mut spectrum = fft.make_output_vec();
    let bin_hz = rate as f32 / size as f32;
    let bins = size / 2 + 1;

    // Which pitch class and band each bin belongs to.
    let pitch: Vec<Option<usize>> = (0..bins)
        .map(|k| {
            let f = k as f32 * bin_hz;
            (60. ..5000.)
                .contains(&f)
                .then(|| ((12. * (f / 440.).log2() + 69.).round() as i64).rem_euclid(12) as usize)
        })
        .collect();
    let (low, high) = (60f32, (rate as f32 / 2.).min(16_000.));
    let band: Vec<Option<usize>> = (0..bins)
        .map(|k| {
            let f = k as f32 * bin_hz;
            (f >= low && f < high)
                .then(|| (((f / low).ln() / (high / low).ln()) * BANDS as f32) as usize)
                .map(|b| b.min(BANDS - 1))
        })
        .collect();
    let flux_top = ((8000. / bin_hz) as usize).min(bins);

    let mut chroma = [0f64; 12];
    let mut bands = [0f64; BANDS];
    let mut onsets = vec![];
    let (mut rms_sum, mut centroid_sum, mut frames) = (0f64, 0f64, 0usize);
    let mut previous: Vec<f32> = vec![0.; flux_top];
    let mut start = 0;
    while start + size <= mono.len() {
        let frame = &mono[start..start + size];
        rms_sum += (frame.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / size as f64).sqrt();
        for ((slot, x), w) in input.iter_mut().zip(frame).zip(&window) {
            *slot = x * w;
        }
        if fft.process(&mut input, &mut spectrum).is_err() {
            break;
        }
        let magnitude: Vec<f32> = spectrum.iter().map(|c| c.norm()).collect();
        let total: f32 = magnitude.iter().sum();
        if total > 1e-6 {
            centroid_sum += (magnitude
                .iter()
                .enumerate()
                .map(|(k, m)| k as f32 * bin_hz * m)
                .sum::<f32>()
                / total) as f64;
        }
        let mut flux = 0f32;
        for k in 1..flux_top {
            let level = (1. + 100. * magnitude[k]).ln();
            flux += (level - previous[k]).max(0.);
            previous[k] = level;
        }
        onsets.push(flux);
        for k in 0..bins {
            let power = (magnitude[k] as f64).powi(2);
            if let Some(p) = pitch[k] {
                chroma[p] += magnitude[k] as f64;
            }
            if let Some(b) = band[k] {
                bands[b] += power;
            }
        }
        frames += 1;
        start += hop;
    }
    if frames == 0 {
        return Features {
            version: VERSION,
            ..Default::default()
        };
    }

    let rms = rms_sum / frames as f64;
    let db = 20. * rms.max(1e-9).log10();
    let energy = ((db + 35.) / 27.).clamp(0., 1.) as f32;
    let centroid = (centroid_sum / frames as f64) as f32;
    let brightness = ((centroid / 300.).max(1.).log2() / (6000f32 / 300.).log2()).clamp(0., 1.);

    let (tempo, pulse) = tempo_of(&onsets, rate as f32 / hop as f32);
    let (key, minor, key_strength) = key_of(&chroma);

    let mut timbre: Vec<f32> = bands
        .iter()
        .map(|b| ((b / frames as f64) + 1e-12).log10() as f32)
        .collect();
    let mean = timbre.iter().sum::<f32>() / BANDS as f32;
    timbre.iter_mut().for_each(|t| *t -= mean);
    let length = timbre.iter().map(|t| t * t).sum::<f32>().sqrt();
    if length > 1e-6 {
        timbre.iter_mut().for_each(|t| *t /= length);
    }
    Features {
        version: VERSION,
        tempo,
        pulse,
        key,
        minor,
        key_strength,
        energy,
        brightness,
        timbre,
    }
}

/// Tempo from the onset strength curve (frames per second `rate`): the lag where the curve
/// best repeats, preferring tempos near 120.
fn tempo_of(onsets: &[f32], rate: f32) -> (f32, f32) {
    // Take away the slow swell so only the hits remain.
    let span = (rate * 0.5) as usize;
    let curve: Vec<f32> = (0..onsets.len())
        .map(|i| {
            let (a, b) = (i.saturating_sub(span), (i + span).min(onsets.len()));
            let local = onsets[a..b].iter().sum::<f32>() / (b - a) as f32;
            (onsets[i] - local).max(0.)
        })
        .collect();
    let at = |lag: usize| -> f32 {
        curve
            .iter()
            .zip(&curve[lag.min(curve.len())..])
            .map(|(a, b)| a * b)
            .sum()
    };
    let zero = at(0);
    if zero <= 1e-9 {
        return (0., 0.);
    }
    let (shortest, longest) = ((rate * 60. / 200.) as usize, (rate * 60. / 55.) as usize);
    if longest * 2 >= curve.len() {
        return (0., 0.);
    }
    let mut best = (0usize, 0f32);
    for lag in shortest.max(1)..=longest {
        let bpm = 60. * rate / lag as f32;
        let prefer = (-0.5 * ((bpm / 120.).log2() / 0.9).powi(2)).exp();
        let score = (at(lag) + 0.5 * at(lag * 2)) * prefer;
        if score > best.1 {
            best = (lag, score);
        }
    }
    if best.0 == 0 {
        return (0., 0.);
    }
    // Between whole frames: fit a parabola through the neighbours.
    let (l, c, r) = (at(best.0 - 1), at(best.0), at(best.0 + 1));
    let shift = if l - 2. * c + r != 0. {
        (0.5 * (l - r) / (l - 2. * c + r)).clamp(-0.5, 0.5)
    } else {
        0.
    };
    let pulse = (c / zero * 2.5).clamp(0., 1.);
    if pulse < 0.08 {
        return (0., pulse);
    }
    (60. * rate / (best.0 as f32 + shift), pulse)
}

/// The key whose Krumhansl–Schmuckler profile best matches the pitch classes heard.
fn key_of(chroma: &[f64; 12]) -> (u8, bool, f32) {
    const MAJOR: [f64; 12] = [
        6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
    ];
    const MINOR: [f64; 12] = [
        6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
    ];
    let correlate = |profile: &[f64; 12], tonic: usize| {
        let x: Vec<f64> = (0..12).map(|i| chroma[(i + tonic) % 12]).collect();
        let (mx, mp) = (
            x.iter().sum::<f64>() / 12.,
            profile.iter().sum::<f64>() / 12.,
        );
        let (mut num, mut dx, mut dp) = (0., 0., 0.);
        for i in 0..12 {
            num += (x[i] - mx) * (profile[i] - mp);
            dx += (x[i] - mx).powi(2);
            dp += (profile[i] - mp).powi(2);
        }
        if dx <= 0. { 0. } else { num / (dx * dp).sqrt() }
    };
    let mut best = (0u8, false, f64::MIN);
    for tonic in 0..12 {
        for (minor, profile) in [(false, &MAJOR), (true, &MINOR)] {
            let r = correlate(profile, tonic);
            if r > best.2 {
                best = (tonic as u8, minor, r);
            }
        }
    }
    (best.0, best.1, best.2.clamp(0., 1.) as f32)
}

/// Measure a song: a minute of it, from a fifth of the way in.
pub fn analyze(track: &Track) -> Result<Features> {
    let mut source = crate::audio_file::decode_track(track)?;
    let channels = source.channels().max(1) as usize;
    let rate = source.sample_rate();
    if rate == 0 {
        bail!("No audio");
    }
    let skip = (track.duration * 0.2).min(45.);
    if skip > 1.
        && source
            .try_seek(std::time::Duration::from_secs_f64(skip))
            .is_err()
    {
        for _ in 0..(skip * rate as f64) as usize * channels {
            if source.next().is_none() {
                break;
            }
        }
    }
    // Very high rates are halved (averaging pairs) until near CD rate; the measures only need
    // what lies below 16 kHz.
    let mut factor = 1;
    while rate / factor > 50_000 {
        factor *= 2;
    }
    let wanted = (LISTEN_SECONDS * rate as f64) as usize / factor as usize;
    let mut mono = Vec::with_capacity(wanted);
    let (mut sum, mut count, mut frame, mut channel) = (0f32, 0u32, 0f32, 0usize);
    for sample in source {
        frame += sample;
        channel += 1;
        if channel == channels {
            sum += frame / channels as f32;
            count += 1;
            frame = 0.;
            channel = 0;
            if count == factor {
                mono.push(sum / factor as f32);
                sum = 0.;
                count = 0;
                if mono.len() >= wanted {
                    break;
                }
            }
        }
    }
    Ok(measure(&mono, rate / factor))
}

// ---------------------------------------------------------------- storing

impl Library {
    pub fn features(&self, id: &str) -> Result<Option<Features>> {
        let data: Option<String> = self
            .connection()?
            .query_row(
                "SELECT data FROM features WHERE track_id=? AND version=?",
                params![id, VERSION],
                |r| r.get(0),
            )
            .optional()?;
        Ok(data.and_then(|d| serde_json::from_str(&d).ok()))
    }
    pub fn save_features(&self, id: &str, features: &Features) -> Result<()> {
        self.connection()?.execute(
            "INSERT OR REPLACE INTO features VALUES (?,?,?)",
            params![id, VERSION, serde_json::to_string(features)?],
        )?;
        Ok(())
    }
    /// Every measured song's features.
    pub fn all_features(&self) -> Result<HashMap<String, Features>> {
        let db = self.connection()?;
        let mut stmt = db.prepare(
            "SELECT f.track_id, f.data FROM features f JOIN tracks t ON t.id = f.track_id WHERE f.version=? AND t.missing=0",
        )?;
        let rows = stmt.query_map([VERSION], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut all = HashMap::new();
        for row in rows {
            let (id, data) = row?;
            if let Ok(f) = serde_json::from_str::<Features>(&data)
                && f.usable()
            {
                all.insert(id, f);
            }
        }
        Ok(all)
    }
    /// Songs not measured yet, at most `limit`.
    pub fn unmeasured(&self, limit: usize) -> Result<Vec<Track>> {
        let ids: Vec<String> = self
            .connection()?
            .prepare(
                "SELECT id FROM tracks WHERE missing=0 AND path NOT LIKE 'source://%' AND id NOT IN (SELECT track_id FROM features WHERE version=?) LIMIT ?",
            )?
            .query_map(params![VERSION, limit as i64], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        self.tracks_by_ids(&ids)
    }
    /// How many songs are measured, of how many.
    pub fn measured_count(&self) -> Result<(usize, usize)> {
        let db = self.connection()?;
        let total: i64 = db.query_row(
            "SELECT count(*) FROM tracks WHERE missing=0 AND path NOT LIKE 'source://%'",
            [],
            |r| r.get(0),
        )?;
        let done: i64 = db.query_row(
            "SELECT count(*) FROM features f JOIN tracks t ON t.id=f.track_id WHERE f.version=? AND t.missing=0",
            [VERSION],
            |r| r.get(0),
        )?;
        Ok((done as usize, total as usize))
    }
    /// A song's features, measuring it now if needed.
    pub fn features_now(&self, track: &Track) -> Result<Features> {
        if let Some(f) = self.features(&track.id)? {
            return Ok(f);
        }
        let f = analyze(track).unwrap_or(Features {
            version: VERSION,
            ..Default::default()
        });
        self.save_features(&track.id, &f)?;
        Ok(f)
    }
}

// ---------------------------------------------------------------- stations

/// Walk from `start` through the songs nearest `aim`: each next song close to the last one
/// and to the aim, never the same song twice, and no artist again within three songs.
fn walk(
    all: &HashMap<String, Features>,
    tracks: &HashMap<String, Track>,
    start: &Features,
    aim: impl Fn(&Features) -> f32,
    skip: &HashSet<String>,
    count: usize,
) -> Vec<String> {
    let mut pool: Vec<(&String, &Features, f32)> = all
        .iter()
        .filter(|(id, _)| !skip.contains(*id) && tracks.contains_key(*id))
        .map(|(id, f)| (id, f, aim(f)))
        .collect();
    pool.sort_by(|a, b| a.2.total_cmp(&b.2));
    pool.truncate((count * 6).max(60));
    let mut picked: Vec<String> = vec![];
    let mut recent: Vec<String> = vec![];
    let mut current = start.clone();
    while picked.len() < count {
        let choose = |strict: bool| {
            pool.iter()
                .filter(|(id, _, _)| !picked.contains(id))
                .filter(|(id, _, _)| {
                    !strict || !recent.contains(&tracks[*id].display_artist().to_lowercase())
                })
                .map(|(id, f, aim)| {
                    (
                        (*id).clone(),
                        (*f).clone(),
                        0.5 * distance(&current, f) + 0.5 * aim,
                    )
                })
                .min_by(|a, b| a.2.total_cmp(&b.2))
        };
        // Another artist when one sounds nearly as right; the sound comes first.
        let (id, features) = match (choose(true), choose(false)) {
            (Some(fresh), Some(any)) if fresh.2 <= any.2 + 0.25 => (fresh.0, fresh.1),
            (_, Some(any)) => (any.0, any.1),
            _ => break,
        };
        recent.push(tracks[&id].display_artist().to_lowercase());
        if recent.len() > 3 {
            recent.remove(0);
        }
        picked.push(id);
        current = features;
    }
    picked
}

fn nearest(f: &Features, seeds: &[Features]) -> f32 {
    seeds
        .iter()
        .map(|s| distance(f, s))
        .fold(f32::MAX, f32::min)
}

impl Library {
    fn station_tracks(&self, all: &HashMap<String, Features>) -> Result<HashMap<String, Track>> {
        let ids: Vec<String> = all.keys().cloned().collect();
        Ok(self
            .tracks_by_ids(&ids)?
            .into_iter()
            .map(|t| (t.id.clone(), t))
            .collect())
    }

    /// Songs that sound like `seeds`, in a flowing order. The seeds themselves are not included.
    pub fn radio(&self, seeds: &[Track], count: usize) -> Result<Vec<Track>> {
        let seed_features: Vec<Features> = seeds
            .iter()
            .filter_map(|t| self.features_now(t).ok())
            .filter(|f| f.usable())
            .collect();
        let Some(start) = seed_features.first().cloned() else {
            bail!("Needle could not measure how this sounds.");
        };
        let all = self.all_features()?;
        let tracks = self.station_tracks(&all)?;
        let skip: HashSet<String> = seeds.iter().map(|t| t.id.clone()).collect();
        let ids = walk(
            &all,
            &tracks,
            &start,
            |f| nearest(f, &seed_features),
            &skip,
            count,
        );
        Ok(ids
            .into_iter()
            .filter_map(|id| tracks.get(&id).cloned())
            .collect())
    }

    /// A station from one artist: their songs set the sound, and others join in.
    pub fn artist_radio(&self, artist: &str, count: usize) -> Result<Vec<Track>> {
        let songs = self.search(&format!(
            "artist = {0} or album_artist = {0}",
            crate::query::quote(artist)
        ))?;
        let mut seeds: Vec<Track> = songs.into_iter().filter(|t| !t.missing).collect();
        seeds.sort_by_key(|t| std::cmp::Reverse(t.play_count));
        seeds.truncate(12);
        let mut station = self.radio(&seeds, count)?;
        // Start with one of the artist's own most played songs.
        if let Some(first) = seeds.first() {
            station.insert(0, first.clone());
            station.truncate(count);
        }
        Ok(station)
    }

    /// Songs that sit between two artists' sounds, turn by turn leaning to one then the other.
    pub fn blend(&self, first: &str, second: &str, count: usize) -> Result<Vec<Track>> {
        let side = |artist: &str| -> Result<Vec<Features>> {
            let songs = self.search(&format!(
                "artist = {0} or album_artist = {0}",
                crate::query::quote(artist)
            ))?;
            Ok(songs
                .iter()
                .take(30)
                .filter_map(|t| self.features_now(t).ok())
                .filter(|f| f.usable())
                .collect())
        };
        let (a, b) = (side(first)?, side(second)?);
        if a.is_empty() || b.is_empty() {
            bail!("Needle could not measure how these artists sound.");
        }
        let all = self.all_features()?;
        let tracks = self.station_tracks(&all)?;
        let mut picked: Vec<String> = vec![];
        let mut skip = HashSet::new();
        let mut current = a[0].clone();
        // Build in short runs, alternating the side the aim leans to.
        let mut lean = 0;
        while picked.len() < count {
            let (near, far) = if lean % 2 == 0 { (&a, &b) } else { (&b, &a) };
            let run = walk(
                &all,
                &tracks,
                &current,
                |f| 0.6 * nearest(f, near) + 0.4 * nearest(f, near).max(nearest(f, far)),
                &skip,
                2.min(count - picked.len()),
            );
            if run.is_empty() {
                break;
            }
            for id in run {
                current = all[&id].clone();
                skip.insert(id.clone());
                picked.push(id);
            }
            lean += 1;
        }
        Ok(picked
            .into_iter()
            .filter_map(|id| tracks.get(&id).cloned())
            .collect())
    }

    /// Other artists whose songs sound most like this artist's, nearest first.
    pub fn similar_artists(&self, artist: &str, count: usize) -> Result<Vec<String>> {
        let all = self.all_features()?;
        let tracks = self.station_tracks(&all)?;
        let key = artist.to_lowercase();
        let mut by_artist: HashMap<String, (String, Vec<&Features>)> = HashMap::new();
        for (id, f) in &all {
            let name = tracks[id].display_artist();
            by_artist
                .entry(name.to_lowercase())
                .or_insert_with(|| (name.to_string(), vec![]))
                .1
                .push(f);
        }
        let Some((_, mine)) = by_artist.get(&key) else {
            return Ok(vec![]);
        };
        let mine: Vec<Features> = mine.iter().map(|f| (*f).clone()).collect();
        let mut others: Vec<(String, f32)> = by_artist
            .iter()
            .filter(|(k, _)| **k != key)
            .map(|(_, (name, songs))| {
                let mean =
                    songs.iter().map(|f| nearest(f, &mine)).sum::<f32>() / songs.len() as f32;
                (name.clone(), mean)
            })
            .collect();
        others.sort_by(|a, b| a.1.total_cmp(&b.1));
        Ok(others.into_iter().take(count).map(|o| o.0).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 22_050;

    /// A tone at `hz` with a click every beat at `bpm`, `seconds` long.
    fn song(hz: f32, bpm: f32, loud: f32, seconds: f32) -> Vec<f32> {
        let beat = (60. / bpm * RATE as f32) as usize;
        (0..(seconds * RATE as f32) as usize)
            .map(|i| {
                let t = i as f32 / RATE as f32;
                let since = i % beat;
                let click = if since < 400 {
                    (1. - since as f32 / 400.) * 0.8 * (t * 2500.).sin()
                } else {
                    0.
                };
                loud * (0.4 * (std::f32::consts::TAU * hz * t).sin() + click)
            })
            .collect()
    }

    #[test]
    fn finds_tempo_key_and_loudness() {
        let f = measure(&song(440., 120., 0.5, 30.), RATE);
        assert!((f.tempo - 120.).abs() < 3., "tempo {}", f.tempo);
        assert!(f.pulse > 0.2, "pulse {}", f.pulse);
        let quiet = measure(&song(440., 120., 0.05, 30.), RATE);
        assert!(quiet.energy < f.energy);
        let slow = measure(&song(440., 90., 0.5, 30.), RATE);
        assert!((slow.tempo - 90.).abs() < 3., "tempo {}", slow.tempo);
        assert!(f.usable());
        assert!(!f.summary().is_empty());
    }

    #[test]
    fn key_from_a_chord() {
        // A C major triad.
        let chord: Vec<f32> = (0..RATE as usize * 10)
            .map(|i| {
                let t = i as f32 / RATE as f32;
                [261.63f32, 329.63, 392.0]
                    .iter()
                    .map(|hz| (std::f32::consts::TAU * hz * t).sin())
                    .sum::<f32>()
                    * 0.2
            })
            .collect();
        let f = measure(&chord, RATE);
        assert_eq!((f.key, f.minor), (0, false), "{}", f.key_name());
    }

    #[test]
    fn like_sounds_are_near() {
        let a = measure(&song(440., 120., 0.5, 20.), RATE);
        let b = measure(&song(440., 121., 0.45, 20.), RATE);
        let c = measure(&song(90., 70., 0.05, 20.), RATE);
        assert!(distance(&a, &b) < distance(&a, &c));
        assert!(distance(&a, &a) < 1e-4);
    }

    #[test]
    fn stations_flow_and_mix_artists() {
        let dir = tempfile::tempdir().unwrap();
        let library = Library::open(dir.path()).unwrap();
        let make = |id: &str, artist: &str, hz: f32, bpm: f32, loud: f32| {
            let track = Track {
                id: id.into(),
                path: format!("{id}.flac"),
                artist: artist.into(),
                title: id.into(),
                ..Default::default()
            };
            library.upsert(&track).unwrap();
            library
                .save_features(id, &measure(&song(hz, bpm, loud, 15.), RATE))
                .unwrap();
            track
        };
        let seed = make("seed", "A", 440., 120., 0.5);
        make("a2", "A", 445., 122., 0.5);
        make("b1", "B", 450., 118., 0.45);
        make("b2", "B", 430., 124., 0.5);
        make("far", "C", 80., 60., 0.03);
        let station = library.radio(&[seed], 3).unwrap();
        let ids: Vec<&str> = station.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids.len(), 3);
        assert!(!ids.contains(&"seed"));
        assert!(!ids.contains(&"far"), "{ids:?}");
        let blend = library.blend("A", "B", 3).unwrap();
        assert_eq!(blend.len(), 3);
        assert_eq!(library.similar_artists("A", 2).unwrap()[0], "B");
        assert_eq!(library.measured_count().unwrap(), (5, 5));
    }
}

#[cfg(test)]
mod real {
    /// Measure a real file: `NEEDLE_MEASURE=path cargo test measure_a_file -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn measure_a_file() {
        let path = std::env::var("NEEDLE_MEASURE").unwrap();
        let track = crate::model::Track {
            path,
            duration: 170.,
            ..Default::default()
        };
        let f = super::analyze(&track).unwrap();
        println!("{f:?}\n{}", f.summary());
    }
}
