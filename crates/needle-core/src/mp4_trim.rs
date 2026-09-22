//! Read the single normal-rate edit used to signal AAC priming and padding.
//! Complex movie timelines are deliberately left to the container decoder.
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
#[derive(Clone, Copy, Debug)]
struct Atom {
    kind: [u8; 4],
    data: u64,
    end: u64,
}
fn atoms(file: &mut File, start: u64, end: u64) -> Option<Vec<Atom>> {
    let mut offset = start;
    let mut result = vec![];
    while offset + 8 <= end {
        if result.len() > 100_000 {
            return None;
        }
        file.seek(SeekFrom::Start(offset)).ok()?;
        let mut header = [0u8; 8];
        file.read_exact(&mut header).ok()?;
        let mut size = u32::from_be_bytes(header[..4].try_into().ok()?) as u64;
        let mut data = offset + 8;
        if size == 1 {
            let mut extended = [0; 8];
            file.read_exact(&mut extended).ok()?;
            size = u64::from_be_bytes(extended);
            data += 8;
        }
        if size == 0 {
            size = end - offset;
        }
        let next = offset.checked_add(size)?;
        if next < data || next > end {
            return None;
        }
        result.push(Atom {
            kind: header[4..].try_into().ok()?,
            data,
            end: next,
        });
        offset = next;
    }
    Some(result)
}
fn children(file: &mut File, atom: Atom) -> Option<Vec<Atom>> {
    atoms(file, atom.data, atom.end)
}
fn find(atoms: &[Atom], kind: &[u8; 4]) -> Option<Atom> {
    atoms.iter().find(|a| &a.kind == kind).copied()
}
fn data(file: &mut File, atom: Atom) -> Option<Vec<u8>> {
    if atom.end - atom.data > 1024 {
        return None;
    }
    file.seek(SeekFrom::Start(atom.data)).ok()?;
    let mut data = vec![0; (atom.end - atom.data) as usize];
    file.read_exact(&mut data).ok()?;
    Some(data)
}
fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        bytes.get(offset..offset + 4)?.try_into().ok()?,
    ))
}
fn timescale(file: &mut File, atom: Atom) -> Option<u32> {
    let bytes = data(file, atom)?;
    u32_at(&bytes, if *bytes.first()? == 1 { 20 } else { 12 }).filter(|v| *v > 0)
}
pub(crate) fn aac_trim(path: &Path, rate: u32) -> Option<(u64, u64)> {
    let mut file = File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    let top = atoms(&mut file, 0, length)?;
    let moov = children(&mut file, find(&top, b"moov")?)?;
    let movie_scale = timescale(&mut file, find(&moov, b"mvhd")?)?;
    for track in moov.iter().filter(|a| &a.kind == b"trak") {
        let track = children(&mut file, *track)?;
        let media = children(&mut file, find(&track, b"mdia")?)?;
        let handler = data(&mut file, find(&media, b"hdlr")?)?;
        if handler.get(8..12) != Some(b"soun") {
            continue;
        }
        let media_scale = timescale(&mut file, find(&media, b"mdhd")?)?;
        let info = children(&mut file, find(&media, b"minf")?)?;
        let table = children(&mut file, find(&info, b"stbl")?)?;
        let entry = data(&mut file, find(&table, b"stsd")?)?;
        if entry.get(12..16) != Some(b"mp4a") {
            continue;
        }
        let edits = children(&mut file, find(&track, b"edts")?)?;
        let list = data(&mut file, find(&edits, b"elst")?)?;
        if u32_at(&list, 4)? != 1 {
            return None;
        }
        let (duration, start, rate_offset) = match list[0] {
            0 => (
                u32_at(&list, 8)? as u64,
                i32::from_be_bytes(list.get(12..16)?.try_into().ok()?) as i64,
                16,
            ),
            1 => (
                u64::from_be_bytes(list.get(8..16)?.try_into().ok()?),
                i64::from_be_bytes(list.get(16..24)?.try_into().ok()?),
                24,
            ),
            _ => return None,
        };
        if start < 0 || list.get(rate_offset..rate_offset + 4) != Some(&[0, 1, 0, 0]) {
            return None;
        }
        let first = (start as u128 * rate as u128 / media_scale as u128) as u64;
        let frames = (duration as u128 * rate as u128 / movie_scale as u128)
            .try_into()
            .ok()?;
        if first > rate as u64 * 10 || frames == 0 {
            return None;
        }
        return Some((first, frames));
    }
    None
}
