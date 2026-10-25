//! Needle's own USB DAC driver for Android (behind Settings › Sound › Bit-perfect USB): the
//! music goes to the DAC as it is, at each song's own rate and depth, without Android's mixer.
//! USB Audio Class 1 and 2; 16, 24, and 32-bit music in 2, 3, or 4-byte slots; the DAC's
//! feedback for its own clock; its own volume; DSD as DoP for a DAC that takes 24-bit at
//! 176.4 kHz or more. Android 8 and newer. When the DAC refuses, the core plays through
//! Android again (the core's `platform_output`).
// Used only on Android; tested everywhere.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
mod bus;
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub mod descriptors;
#[cfg(target_os = "android")]
mod device;
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
mod driver;
#[cfg(test)]
pub(crate) mod fake;
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub mod packets;

use crate::{Needle, Result};
use std::sync::Mutex;

/// What the driver plays now ("96 kHz, 24-bit"), and why it last stopped, for the app.
static STATUS: Mutex<(Option<String>, Option<String>)> = Mutex::new((None, None));

pub(crate) fn status_playing(text: Option<String>) {
    STATUS.lock().unwrap_or_else(|e| e.into_inner()).0 = text;
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn status_failed(why: String) {
    let mut s = STATUS.lock().unwrap_or_else(|e| e.into_inner());
    s.0 = None;
    s.1 = Some(why);
}

/// How Needle's USB driver is doing, for Settings and for the app to fall back.
#[derive(Clone, uniffi::Record)]
pub struct UsbDacStatus {
    /// What it plays now, such as "96 kHz, 24-bit".
    pub playing: Option<String>,
    /// Why it stopped, once; Android's output plays meanwhile.
    pub problem: Option<String>,
}

/// What Needle found in a USB DAC, for Settings to say.
#[derive(Clone, uniffi::Record)]
pub struct UsbDacInfo {
    pub name: String,
    /// 1 or 2: USB Audio Class.
    pub uac: u8,
    /// The rates it plays, lowest first.
    pub rates: Vec<u32>,
    /// The deepest music it takes, in bits.
    pub bits: u8,
    /// It takes DSD as DoP.
    pub dop: bool,
    /// Needle sets its volume.
    pub volume: bool,
}

#[uniffi::export]
impl Needle {
    /// Use Needle's own driver for a USB DAC (Settings › Sound › Bit-perfect USB): `fd` is the
    /// file of Android's connection to it (its interfaces taken with `claimInterface(force)`),
    /// `descriptors` its raw descriptors. From the next moment on, music goes to it as it is.
    /// An error for a device the driver cannot play to; Android's own output goes on then.
    pub fn usb_dac_open(&self, fd: i32, descriptors: Vec<u8>, name: String) -> Result<UsbDacInfo> {
        #[cfg(target_os = "android")]
        {
            let bus = std::sync::Arc::new(device::Usbfs::open(fd)?);
            let dac = std::sync::Arc::new(driver::UsbDac::new(bus, &descriptors, name)?);
            let info = dac.info();
            needle_core::platform_output::set_platform_output(Some(dac));
            Ok(info)
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (fd, name);
            let dac = descriptors::parse(&descriptors).ok_or_else(|| {
                crate::NeedleError::from(anyhow::anyhow!("Not a USB audio device"))
            })?;
            Ok(UsbDacInfo {
                name: String::new(),
                uac: if dac.uac == descriptors::Uac::Two {
                    2
                } else {
                    1
                },
                rates: dac
                    .formats
                    .iter()
                    .flat_map(descriptors::listed_rates)
                    .collect(),
                bits: dac.formats.iter().map(|f| f.bits).max().unwrap_or(16),
                dop: false,
                volume: dac.volume.is_some(),
            })
        }
    }

    /// Stop using Needle's driver (the DAC was unplugged, or Bit-perfect USB turned off).
    pub fn usb_dac_close(&self) {
        needle_core::platform_output::set_platform_output(None);
        status_playing(None);
    }

    /// What the driver plays now, and why it stopped (said once).
    pub fn usb_dac_status(&self) -> UsbDacStatus {
        let mut s = STATUS.lock().unwrap_or_else(|e| e.into_inner());
        UsbDacStatus {
            playing: s.0.clone(),
            problem: s.1.take(),
        }
    }
}
