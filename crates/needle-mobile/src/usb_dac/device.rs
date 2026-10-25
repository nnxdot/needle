//! Speaking to a USB DAC through Linux's usbfs, on the file Android opens for the app
//! (`UsbDeviceConnection.getFileDescriptor`): control requests (the rate, the volume), the
//! alternate setting, and isochronous transfers for the music and the DAC's feedback. Every
//! wait has a limit: a DAC that stops answering is an error, never a hang.
use super::bus::{Bus, Reaped};
use anyhow::{Context, Result, bail};
use std::{collections::HashMap, ffi::c_void, sync::Mutex};

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
const REAPURBNDELAY: u32 = ioc(IOC_WRITE, 13, std::mem::size_of::<*mut c_void>());
const CLAIMINTERFACE: u32 = ioc(IOC_READ, 15, std::mem::size_of::<u32>());
const GET_SPEED: u32 = ioc(IOC_NONE, 31, 0);

const URB_TYPE_ISO: u8 = 0;
const URB_ISO_ASAP: u32 = 0x02;
/// Linux's speed numbers: 3 is high speed, 5 and up super speed.
const SPEED_HIGH: i32 = 3;

fn ioctl(fd: i32, request: u32, arg: *mut c_void) -> std::io::Result<i32> {
    // SAFETY: each caller passes a pointer to the structure `request` names, alive for the call.
    let r = unsafe { libc::ioctl(fd, request as _, arg) };
    if r < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(r)
    }
}

/// One isochronous transfer: the request the kernel reads, its packets, and its buffer, kept in
/// place while the kernel holds them (the heap memory does not move with the struct).
struct Transfer {
    mem: Vec<u64>,
    buffer: Vec<u8>,
    endpoint: u8,
    busy: bool,
}

impl Transfer {
    fn urb(&mut self) -> &mut Urb {
        // SAFETY: `mem` is large enough and aligned for a Urb and its packets.
        unsafe { &mut *(self.mem.as_mut_ptr() as *mut Urb) }
    }

    fn packets(&mut self, count: usize) -> &mut [IsoPacket] {
        // SAFETY: room for `count` packets after the Urb was made when `mem` was sized.
        unsafe {
            std::slice::from_raw_parts_mut(
                (self.mem.as_mut_ptr() as *mut u8).add(std::mem::size_of::<Urb>())
                    as *mut IsoPacket,
                count,
            )
        }
    }
}

/// The DAC, through Needle's own copy of Android's file.
pub struct Usbfs {
    fd: i32,
    transfers: Mutex<HashMap<usize, Transfer>>,
}

impl Usbfs {
    /// A copy of Android's file for the device, so Android may close its own.
    pub fn open(fd: i32) -> Result<Self> {
        // SAFETY: dup of a file the caller holds open.
        let own = unsafe { libc::dup(fd) };
        if own < 0 {
            bail!(
                "The DAC's USB connection could not be used ({})",
                std::io::Error::last_os_error()
            );
        }
        Ok(Self {
            fd: own,
            transfers: Mutex::new(HashMap::new()),
        })
    }
}

impl Drop for Usbfs {
    fn drop(&mut self) {
        // Every transfer back from the kernel before its memory goes; one the kernel keeps
        // (it should not) is let go rather than freed under it.
        let mut transfers =
            std::mem::take(&mut *self.transfers.lock().unwrap_or_else(|e| e.into_inner()));
        for t in transfers.values_mut().filter(|t| t.busy) {
            let _ = ioctl(self.fd, DISCARDURB, t.mem.as_mut_ptr() as *mut c_void);
        }
        for _ in 0..250 {
            if transfers.values().all(|t| !t.busy) {
                break;
            }
            let mut back: *mut c_void = std::ptr::null_mut();
            match ioctl(
                self.fd,
                REAPURBNDELAY,
                &mut back as *mut *mut c_void as *mut c_void,
            ) {
                Ok(_) => {
                    // SAFETY: the kernel hands back one of the Urbs submitted.
                    let id = unsafe { (*(back as *mut Urb)).context as usize };
                    if let Some(t) = transfers.get_mut(&id) {
                        t.busy = false;
                    }
                }
                Err(e) if e.raw_os_error() == Some(libc::EAGAIN) => {
                    std::thread::sleep(std::time::Duration::from_millis(2))
                }
                Err(_) => break,
            }
        }
        if transfers.values().any(|t| t.busy) {
            std::mem::forget(transfers);
        }
        // SAFETY: the copy is this Usbfs's alone.
        unsafe { libc::close(self.fd) };
    }
}

impl Bus for Usbfs {
    fn high_speed(&self) -> bool {
        ioctl(self.fd, GET_SPEED, std::ptr::null_mut()).is_ok_and(|s| s >= SPEED_HIGH)
    }

    fn claim(&self, interface: u8) {
        let mut n = interface as u32;
        let _ = ioctl(self.fd, CLAIMINTERFACE, &mut n as *mut u32 as *mut c_void);
    }

    fn set_interface(&self, interface: u8, alt: u8) -> Result<()> {
        let mut s = SetInterface {
            interface: interface as u32,
            alt: alt as u32,
        };
        ioctl(
            self.fd,
            SETINTERFACE,
            &mut s as *mut SetInterface as *mut c_void,
        )
        .with_context(|| format!("The DAC refused setting {alt} of interface {interface}"))?;
        Ok(())
    }

    fn control(
        &self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        data: &mut [u8],
    ) -> Result<usize> {
        let mut t = CtrlTransfer {
            request_type,
            request,
            value,
            index,
            length: data.len() as u16,
            // The kernel gives up after a second: a DAC that does not answer is an error.
            timeout: 1000,
            data: data.as_mut_ptr() as *mut c_void,
        };
        Ok(ioctl(self.fd, CONTROL, &mut t as *mut CtrlTransfer as *mut c_void)? as usize)
    }

    fn submit(&self, id: usize, endpoint: u8, bytes: &[u8], lengths: &[usize]) -> Result<()> {
        let mut transfers = self.transfers.lock().unwrap_or_else(|e| e.into_inner());
        let need = std::mem::size_of::<Urb>() + lengths.len() * std::mem::size_of::<IsoPacket>();
        let t = transfers.entry(id).or_insert_with(|| Transfer {
            mem: vec![],
            buffer: vec![],
            endpoint,
            busy: false,
        });
        if t.busy {
            bail!("A transfer was handed over twice");
        }
        // Grown only while not with the kernel.
        if t.mem.len() * 8 < need {
            t.mem = vec![0u64; need.div_ceil(8)];
        }
        let total: usize = lengths.iter().sum();
        if t.buffer.len() < total {
            t.buffer = vec![0u8; total];
        }
        t.buffer[..bytes.len().min(total)].copy_from_slice(&bytes[..bytes.len().min(total)]);
        t.endpoint = endpoint;
        for (p, len) in t.packets(lengths.len()).iter_mut().zip(lengths) {
            *p = IsoPacket {
                length: *len as u32,
                ..Default::default()
            };
        }
        let buffer = t.buffer.as_mut_ptr() as *mut c_void;
        let urb = t.urb();
        *urb = Urb {
            kind: URB_TYPE_ISO,
            endpoint,
            status: 0,
            flags: URB_ISO_ASAP,
            buffer,
            buffer_length: total as i32,
            actual_length: 0,
            start_frame: 0,
            packets: lengths.len() as i32,
            error_count: 0,
            signr: 0,
            context: id as *mut c_void,
        };
        let ptr = t.mem.as_mut_ptr() as *mut c_void;
        ioctl(self.fd, SUBMITURB, ptr).context("The DAC took no more music")?;
        t.busy = true;
        Ok(())
    }

    fn reap(&self, wait_ms: u32) -> Result<Option<Reaped>> {
        // usbfs says "writable" when a transfer is back to reap.
        let mut fds = libc::pollfd {
            fd: self.fd,
            events: libc::POLLOUT | libc::POLLWRNORM,
            revents: 0,
        };
        // SAFETY: one pollfd, alive for the call.
        let ready = unsafe { libc::poll(&mut fds, 1, wait_ms as i32) };
        if ready < 0 {
            let e = std::io::Error::last_os_error();
            if e.raw_os_error() == Some(libc::EINTR) {
                return Ok(None);
            }
            return Err(e).context("The DAC is gone");
        }
        if fds.revents & (libc::POLLERR | libc::POLLHUP) != 0 {
            bail!("The DAC is gone");
        }
        let mut back: *mut c_void = std::ptr::null_mut();
        match ioctl(
            self.fd,
            REAPURBNDELAY,
            &mut back as *mut *mut c_void as *mut c_void,
        ) {
            Ok(_) => {
                // SAFETY: the kernel hands back one of the Urbs submitted, unchanged in place.
                let id = unsafe { (*(back as *mut Urb)).context as usize };
                let mut transfers = self.transfers.lock().unwrap_or_else(|e| e.into_inner());
                let Some(t) = transfers.get_mut(&id) else {
                    return Ok(None);
                };
                t.busy = false;
                let status = t.urb().status;
                let data = if t.endpoint & 0x80 != 0 {
                    let got = t.packets(1)[0].actual as usize;
                    t.buffer[..got.min(t.buffer.len())].to_vec()
                } else {
                    vec![]
                };
                Ok(Some(Reaped { id, status, data }))
            }
            Err(e)
                if e.raw_os_error() == Some(libc::EAGAIN)
                    || e.raw_os_error() == Some(libc::EINTR) =>
            {
                Ok(None)
            }
            Err(e) => Err(e).context("The DAC is gone"),
        }
    }

    fn discard(&self, id: usize) {
        let mut transfers = self.transfers.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(t) = transfers.get_mut(&id).filter(|t| t.busy) {
            let _ = ioctl(self.fd, DISCARDURB, t.mem.as_mut_ptr() as *mut c_void);
        }
    }

    fn busy(&self) -> usize {
        self.transfers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .filter(|t| t.busy)
            .count()
    }
}
