//! What the driver needs from the USB connection, so the same driver runs on a real DAC
//! (usbfs, `device.rs`) and on a pretend one in tests (`fake.rs`).
use anyhow::Result;

/// A transfer the device gave back.
pub struct Reaped {
    /// The id it was submitted with.
    pub id: usize,
    /// 0, or the kernel's error (negative errno), for the whole transfer.
    pub status: i32,
    /// For a transfer from the device (feedback): the bytes of its first packet.
    pub data: Vec<u8>,
}

pub trait Bus: Send + Sync {
    /// The device runs at high speed (or faster): packets every 125 µs microframe.
    fn high_speed(&self) -> bool;
    /// Take an interface from the system's own driver.
    fn claim(&self, interface: u8);
    fn set_interface(&self, interface: u8, alt: u8) -> Result<()>;
    /// A control request; for one that reads (`request_type` & 0x80), `data` is filled.
    fn control(&self, request_type: u8, request: u8, value: u16, index: u16, data: &mut [u8]) -> Result<usize>;
    /// Hand over an isochronous transfer: `bytes` laid out packet after packet, each
    /// `lengths[i]` long (for a transfer from the device, the room for each packet).
    fn submit(&self, id: usize, endpoint: u8, bytes: &[u8], lengths: &[usize]) -> Result<()>;
    /// A transfer back within `wait_ms`, or None. An error when the device is gone.
    fn reap(&self, wait_ms: u32) -> Result<Option<Reaped>>;
    /// Ask for a transfer back now (it is then reaped).
    fn discard(&self, id: usize);
    /// Transfers handed over and not yet reaped.
    fn busy(&self) -> usize;
}
