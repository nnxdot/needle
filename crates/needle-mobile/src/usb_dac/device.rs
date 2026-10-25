//! Speaking to a USB DAC through Linux's usbfs, on the file Android opens for the app
//! (`UsbDeviceConnection.getFileDescriptor`): control requests (the rate, the volume), the
//! alternate setting, and isochronous transfers for the music and the DAC's feedback.
use anyhow::{Context, Result, bail};
use std::ffi::c_void;

#[repr(C)]
struct CtrlTransfer {
    request_type: u8,
    request: u8,
    value: u16,
    index: u16,
    length: u16,
    timeout: u32,
    data: *mut c_void,
}

#[repr(C)]
struct SetInterface {
    interface: u32,
    alt: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct IsoPacket {
    length: u32,
    actual: u32,
    status: u32,
}

#[repr(C)]
struct Urb {
    kind: u8,
    endpoint: u8,
    status: i32,
    flags: u32,
    buffer: *mut c_void,
    buffer_length: i32,
    actual_length: i32,
    start_frame: i32,
    packets: i32,
    error_count: i32,
    signr: u32,
    context: *mut c_void,
    // The packets follow (`iso_frame_desc`).
}

const IOC_NONE: u32 = 0;
const IOC_WRITE: u32 = 1;
const IOC_READ: u32 = 2;

const fn ioc(dir: u32, nr: u32, size: usize) -> u32 {
    (dir << 30) | ((size as u32) << 16) | ((b'U' as u32) << 8) | nr
}

const CONTROL: u32 = ioc(IOC_READ | IOC_WRITE, 0, std::mem::size_of::<CtrlTransfer>());
const SETINTERFACE: u32 = ioc(IOC_READ, 4, std::mem::size_of::<SetInterface>());
const SUBMITURB: u32 = ioc(IOC_READ, 10, std::mem::size_of::<Urb>());
const DISCARDURB: u32 = ioc(IOC_NONE, 11, 0);
const REAPURB: u32 = ioc(IOC_WRITE, 12, std::mem::size_of::<*mut c_void>());
const REAPURBNDELAY: u32 = ioc(IOC_WRITE, 13, std::mem::size_of::<*mut c_void>());
const CLAIMINTERFACE: u32 = ioc(IOC_READ, 15, std::mem::size_of::<u32>());
const GET_SPEED: u32 = ioc(IOC_NONE, 31, 0);

const URB_TYPE_ISO: u8 = 0;
const URB_ISO_ASAP: u32 = 0x02;
/// Linux's speed numbers: 3 is high speed, 5 and up super speed.
const SPEED_HIGH: i32 = 3;

fn ioctl(fd: i32, request: u32, arg: *mut c_void) -> Result<i32> {
    // SAFETY: each caller passes a pointer to the structure `request` names, alive for the call.
    let r = unsafe { libc::ioctl(fd, request as _, arg) };
    if r < 0 {
        Err(std::io::Error::last_os_error().into())
    } else {
        Ok(r)
    }
}

/// The DAC, through its own copy of Android's file.
pub struct Device {
    fd: i32,
}

impl Drop for Device {
    fn drop(&mut self) {
        // SAFETY: the copy is this Device's alone.
        unsafe { libc::close(self.fd) };
    }
}

impl Device {
    /// A copy of Android's file for the device, so Android may close its own.
    pub fn open(fd: i32) -> Result<Self> {
        // SAFETY: dup of a file the caller holds open.
        let own = unsafe { libc::dup(fd) };
        if own < 0 {
            bail!("The DAC's USB connection could not be used ({})", std::io::Error::last_os_error());
        }
        Ok(Self { fd: own })
    }

    pub fn high_speed(&self) -> bool {
        ioctl(self.fd, GET_SPEED, std::ptr::null_mut()).is_ok_and(|s| s >= SPEED_HIGH)
    }

    /// Takes an interface for Needle (Android has already let it go with `claimInterface(force)`).
    pub fn claim(&self, interface: u8) {
        let mut n = interface as u32;
        let _ = ioctl(self.fd, CLAIMINTERFACE, &mut n as *mut u32 as *mut c_void);
    }

    pub fn set_interface(&self, interface: u8, alt: u8) -> Result<()> {
        let mut s = SetInterface { interface: interface as u32, alt: alt as u32 };
        ioctl(self.fd, SETINTERFACE, &mut s as *mut SetInterface as *mut c_void)
            .with_context(|| format!("The DAC refused setting {alt} of interface {interface}"))?;
        Ok(())
    }

    /// A control request; for one that reads (`request_type` & 0x80), `data` is filled.
    pub fn control(&self, request_type: u8, request: u8, value: u16, index: u16, data: &mut [u8]) -> Result<usize> {
        let mut t = CtrlTransfer {
            request_type,
            request,
            value,
            index,
            length: data.len() as u16,
            timeout: 1000,
            data: data.as_mut_ptr() as *mut c_void,
        };
        Ok(ioctl(self.fd, CONTROL, &mut t as *mut CtrlTransfer as *mut c_void)? as usize)
    }

    pub fn fd(&self) -> i32 {
        self.fd
    }
}

/// One isochronous transfer: the request the kernel reads, its packets, and its buffer, kept
/// in place while the kernel holds them.
pub struct Transfer {
    /// The request and its packets, in memory aligned for them.
    mem: Vec<u64>,
    pub buffer: Vec<u8>,
    pub packets: usize,
    pub endpoint: u8,
    pub busy: bool,
}

impl Transfer {
    pub fn new(endpoint: u8, packets: usize, packet_size: usize, id: usize) -> Self {
        let bytes = std::mem::size_of::<Urb>() + packets * std::mem::size_of::<IsoPacket>();
        let mut t = Self {
            mem: vec![0u64; bytes.div_ceil(8)],
            buffer: vec![0u8; packets * packet_size],
            packets,
            endpoint,
            busy: false,
        };
        let urb = t.urb();
        urb.context = id as *mut c_void;
        t
    }

    fn urb(&mut self) -> &mut Urb {
        // SAFETY: `mem` is large enough and aligned for a Urb and its packets.
        unsafe { &mut *(self.mem.as_mut_ptr() as *mut Urb) }
    }

    fn packet_list(&mut self) -> &mut [IsoPacket] {
        let packets = self.packets;
        // SAFETY: the packets follow the Urb in `mem`, `packets` of them.
        unsafe {
            std::slice::from_raw_parts_mut(
                (self.mem.as_mut_ptr() as *mut u8).add(std::mem::size_of::<Urb>()) as *mut IsoPacket,
                packets,
            )
        }
    }

    /// Hand it to the kernel with each packet `lengths[i]` bytes long.
    pub fn submit(&mut self, device: &Device, lengths: &[usize]) -> Result<()> {
        let total: usize = lengths.iter().sum();
        for (p, len) in self.packet_list().iter_mut().zip(lengths) {
            *p = IsoPacket { length: *len as u32, ..Default::default() };
        }
        let buffer = self.buffer.as_mut_ptr() as *mut c_void;
        let (endpoint, packets) = (self.endpoint, lengths.len() as i32);
        let urb = self.urb();
        urb.kind = URB_TYPE_ISO;
        urb.endpoint = endpoint;
        urb.status = 0;
        urb.flags = URB_ISO_ASAP;
        urb.buffer = buffer;
        urb.buffer_length = total as i32;
        urb.actual_length = 0;
        urb.start_frame = 0;
        urb.packets = packets;
        urb.error_count = 0;
        let ptr = self.mem.as_mut_ptr() as *mut c_void;
        ioctl(device.fd, SUBMITURB, ptr).context("The DAC took no more music")?;
        self.busy = true;
        Ok(())
    }

    /// Ask the kernel to give it back now (it is then reaped).
    pub fn discard(&mut self, device: &Device) {
        if self.busy {
            let _ = ioctl(device.fd, DISCARDURB, self.mem.as_mut_ptr() as *mut c_void);
        }
    }

    /// The status of a reaped transfer, and the first packet's bytes received (feedback).
    pub fn result(&mut self) -> (i32, usize) {
        let first = self.packet_list().first().map_or(0, |p| p.actual as usize);
        (self.urb().status, first)
    }
}

/// Waits for the next transfer to come back and says which (its id); with `wait` false, only
/// one already back. Error when the device is gone.
pub fn reap(device: &Device, wait: bool) -> Result<Option<usize>> {
    let mut back: *mut c_void = std::ptr::null_mut();
    let request = if wait { REAPURB } else { REAPURBNDELAY };
    match ioctl(device.fd, request, &mut back as *mut *mut c_void as *mut c_void) {
        Ok(_) => {
            // SAFETY: the kernel hands back one of the Urbs submitted, unchanged in place.
            let id = unsafe { (*(back as *mut Urb)).context as usize };
            Ok(Some(id))
        }
        Err(e) => {
            let code = e.downcast_ref::<std::io::Error>().and_then(|e| e.raw_os_error());
            if code == Some(libc::EAGAIN) || code == Some(libc::EINTR) {
                Ok(None)
            } else {
                Err(e.context("The DAC is gone"))
            }
        }
    }
}
