//! Needle's own USB DAC driver for Android (behind Settings › Sound › Bit-perfect USB): the
//! music goes to the DAC as it is, at each song's own rate and depth, without Android's mixer.
//! USB Audio Class 1 and 2; 16, 24, and 32-bit music in 2, 3, or 4-byte slots; the DAC's
//! feedback for its own clock; its own volume, or lowered samples for a DAC with none (not
//! bit-perfect below 100 %, and the status says so); DSD as DoP for a DAC with a volume that
//! takes 24-bit at 176.4 kHz or more. Android 8 and newer. When the DAC refuses, the core plays
//! through Android again (the core's `platform_output`).
// Used only on Android; tested everywhere.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
mod bus;
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub mod descriptors;
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
mod details;
#[cfg(target_os = "android")]
mod device;
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
mod driver;
#[cfg(test)]
pub(crate) mod fake;
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub mod packets;

use crate::{Needle, Result};
use std::{
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

/// What the driver plays now ("96 kHz, 24-bit"), and why it last stopped, for the app.
static STATUS: Mutex<(Option<String>, Option<String>)> = Mutex::new((None, None));

/// The driver in use: the core owns it; this is for the app's calls below.
static DRIVER: Mutex<Weak<driver::UsbDac>> = Mutex::new(Weak::new());

/// What happened when a DAC last opened, for "Copy DAC details", also when it was refused.
static LAST_OPEN: Mutex<String> = Mutex::new(String::new());

pub(crate) fn remember_open(text: String) {
    *LAST_OPEN.lock().unwrap_or_else(|e| e.into_inner()) = text;
}

pub(crate) fn last_open() -> String {
    LAST_OPEN.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The phone's music volume, 0 to 1 (f32 bits), for the next driver too. 1.0 at the start.
static PHONE_VOLUME: AtomicU32 = AtomicU32::new(0x3F80_0000);

fn driver() -> Option<Arc<driver::UsbDac>> {
    DRIVER.lock().unwrap_or_else(|e| e.into_inner()).upgrade()
}

/// `dac` plays from now on.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn use_driver(dac: Arc<driver::UsbDac>) {
    status_clear();
    *DRIVER.lock().unwrap_or_else(|e| e.into_inner()) = Arc::downgrade(&dac);
    needle_core::platform_output::set_platform_output(Some(dac));
}

pub(crate) fn status_playing(text: Option<String>) {
    STATUS.lock().unwrap_or_else(|e| e.into_inner()).0 = text;
}

/// The driver's word is for the driver it came from: an old "the DAC is gone" read by the next
/// one would end it.
fn status_clear() {
    *STATUS.lock().unwrap_or_else(|e| e.into_inner()) = (None, None);
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
    /// The samples reach the DAC as they are: false while Needle lowers them, for a DAC with no
    /// volume of its own below 100 %.
    pub bit_perfect: bool,
}

/// What Needle found in a USB DAC, for Settings to say.
#[derive(Clone, uniffi::Record)]
pub struct UsbDacInfo {
    pub name: String,
    /// 1, 2, or 3: USB Audio Class.
    pub uac: u8,
    /// The rates it plays, lowest first.
    pub rates: Vec<u32>,
    /// The deepest music it takes, in bits.
    pub bits: u8,
    /// It takes DSD as DoP.
    pub dop: bool,
    /// It has a volume Needle sets; without one, Needle lowers the samples.
    pub volume: bool,
}

#[uniffi::export]
impl Needle {
    /// Use Needle's own driver for a USB DAC (Settings › Sound › Bit-perfect USB): `fd` is the
    /// file of Android's connection to it (its interfaces taken with `claimInterface(force)`),
    /// `descriptors` its raw descriptors, `android_rates` the rates Android lists for it, for a
    /// clock that does not say its own. From the next moment on, music goes to it as it is.
    /// An error for a device the driver cannot play to; Android's own output goes on then.
    pub fn usb_dac_open(
        &self,
        fd: i32,
        descriptors: Vec<u8>,
        name: String,
        android_rates: Vec<u32>,
    ) -> Result<UsbDacInfo> {
        #[cfg(target_os = "android")]
        {
            let bus = Arc::new(device::Usbfs::open(fd)?);
            let volume = f32::from_bits(PHONE_VOLUME.load(Ordering::Relaxed));
            let dac = Arc::new(driver::UsbDac::new(
                bus,
                &descriptors,
                name,
                volume,
                &android_rates,
            )?);
            let info = dac.info();
            use_driver(dac);
            Ok(info)
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = (fd, name, android_rates);
            let dac = descriptors::parse(&descriptors).ok_or_else(|| {
                crate::NeedleError::from(anyhow::anyhow!("Not a USB audio device"))
            })?;
            Ok(UsbDacInfo {
                name: String::new(),
                uac: dac.uac.number(),
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

    /// Stop using Needle's driver (the DAC was unplugged, or Bit-perfect USB turned off), and
    /// wait for it to end, up to two seconds: not from the main thread. The DAC may go back to
    /// Android only then. The kernel gives an interface to a driver that touches it, so a last
    /// request after Android's own driver came back would take the interface from under it.
    pub fn usb_dac_close(&self) {
        let running = std::mem::take(&mut *DRIVER.lock().unwrap_or_else(|e| e.into_inner()));
        needle_core::platform_output::set_platform_output(None);
        let until = Instant::now() + Duration::from_secs(2);
        while running.strong_count() > 0 && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        status_clear();
    }

    /// The DAC is back with Android: the phone's output opens again, as one opened while it was
    /// away may play to nothing. No use while Needle's driver has a DAC.
    pub fn usb_output_again(&self) {
        if driver().is_none() {
            needle_core::platform_output::set_platform_output(None);
        }
    }

    /// The phone's music volume moved (0 to 1, as a slider): Android's volume cannot reach a DAC
    /// Needle's driver holds, so it comes here. Tell it before `usb_dac_open` too.
    pub fn usb_dac_volume(&self, volume: f32) {
        PHONE_VOLUME.store(volume.clamp(0., 1.).to_bits(), Ordering::Relaxed);
        if let Some(dac) = driver() {
            dac.set_phone_volume(volume);
        }
    }

    /// What Needle read from a USB DAC, in words and hex, for a report: its name, ids, and
    /// speed, every way through it, each format and rate (and whether it was a range), and what
    /// the driver chose. `fd` is Android's file for it; no interface need be taken.
    pub fn usb_dac_details(
        &self,
        fd: i32,
        descriptors: Vec<u8>,
        name: String,
        vendor: u16,
        product: u16,
    ) -> String {
        #[cfg(target_os = "android")]
        let (speed, active) = (device::speed(fd), device::active_configuration(fd));
        #[cfg(not(target_os = "android"))]
        let (speed, active) = {
            let _ = fd;
            (None, None)
        };
        let driver = driver().map(|dac| dac.details());
        details::text(
            &descriptors,
            &name,
            vendor,
            product,
            speed,
            active,
            driver,
            &last_open(),
        )
    }

    /// What the driver plays now, and why it stopped (said once).
    pub fn usb_dac_status(&self) -> UsbDacStatus {
        let mut s = STATUS.lock().unwrap_or_else(|e| e.into_inner());
        UsbDacStatus {
            playing: s.0.clone(),
            problem: s.1.take(),
            bit_perfect: driver().is_none_or(|dac| dac.bit_perfect()),
        }
    }
}
