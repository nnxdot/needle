#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    use wasapi::*;
    initialize_mta().ok()?;
    let enumerator = DeviceEnumerator::new()?;
    let device = enumerator.get_default_device(&Direction::Render)?;
    println!("Default: {}", device.get_friendlyname()?);
    let client = device.get_iaudioclient()?;
    for rate in [44100, 48000, 96000] {
        for channels in [1, 2] {
            for (bits, valid, kind) in [
                (32, 24, SampleType::Int),
                (24, 24, SampleType::Int),
                (16, 16, SampleType::Int),
                (32, 32, SampleType::Float),
            ] {
                let format = WaveFormat::new(bits, valid, &kind, rate, channels, None);
                println!(
                    "{rate} Hz {channels} ch {bits}/{valid} {kind:?}: {:?}",
                    client
                        .is_supported_exclusive_with_quirks(&format)
                        .map(|f| format!(
                            "{} / {} bits {:?}",
                            f.get_bitspersample(),
                            f.get_validbitspersample(),
                            f.get_subformat()
                        ))
                );
            }
        }
    }
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    println!("WASAPI diagnostics require Windows.");
}
