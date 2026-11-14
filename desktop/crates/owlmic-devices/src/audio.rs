//! What the WASAPI modules share: COM on the current thread, finding endpoints by name, and the
//! device's sample format.

use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_DeviceDesc;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Media::Audio::{
    DEVICE_STATE_ACTIVE, EDataFlow, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
    WAVEFORMATEX, WAVEFORMATEXTENSIBLE,
};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize, STGM_READ,
};
use windows::core::{GUID, Result};

/// COM for this thread, released when dropped.
pub struct Com;

impl Com {
    pub fn init() -> Self {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        Com
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

/// A kernel handle closed when dropped.
pub struct Handle(pub HANDLE);

// Kernel handles may be used from any thread.
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub fn enumerator() -> Result<IMMDeviceEnumerator> {
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
}

/// The endpoint's own name, without the adapter part ("Owlmic Bridge", "CABLE Input").
pub fn description(device: &IMMDevice) -> Option<String> {
    unsafe {
        device
            .OpenPropertyStore(STGM_READ)
            .and_then(|s| s.GetValue(&PKEY_Device_DeviceDesc))
            .ok()
            .map(|v| v.to_string())
    }
}

/// The first active endpoint whose name is one of `names`, in the order of `names`.
pub fn find(
    enumerator: &IMMDeviceEnumerator,
    flow: EDataFlow,
    names: &[&str],
) -> Option<IMMDevice> {
    let list = unsafe {
        enumerator
            .EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE)
            .ok()?
    };
    let devices: Vec<(IMMDevice, String)> = (0..unsafe { list.GetCount().ok()? })
        .filter_map(|i| unsafe { list.Item(i).ok() })
        .filter_map(|d| description(&d).map(|n| (d, n)))
        .collect();
    names.iter().find_map(|want| {
        devices
            .iter()
            .find(|(_, n)| n.eq_ignore_ascii_case(want))
            .map(|(d, _)| d.clone())
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sample {
    F32,
    I16,
    I32,
}

/// A device's mix format, as far as Owlmic cares.
#[derive(Debug, Clone, Copy)]
pub struct Format {
    pub rate: u32,
    pub channels: usize,
    pub sample: Sample,
}

const WAVE_FORMAT_PCM: u16 = 1;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 3;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;
const SUBTYPE_PCM: GUID = GUID::from_u128(0x00000001_0000_0010_8000_00aa00389b71);
const SUBTYPE_FLOAT: GUID = GUID::from_u128(0x00000003_0000_0010_8000_00aa00389b71);

impl Format {
    /// # Safety
    /// `wave` must point to a valid WAVEFORMATEX, extended when its tag says so.
    pub unsafe fn of(wave: *const WAVEFORMATEX) -> Option<Self> {
        let w = unsafe { wave.read_unaligned() };
        let tag = match w.wFormatTag {
            WAVE_FORMAT_EXTENSIBLE => {
                let sub =
                    unsafe { (wave as *const WAVEFORMATEXTENSIBLE).read_unaligned() }.SubFormat;
                if sub == SUBTYPE_FLOAT {
                    WAVE_FORMAT_IEEE_FLOAT
                } else if sub == SUBTYPE_PCM {
                    WAVE_FORMAT_PCM
                } else {
                    return None;
                }
            }
            t => t,
        };
        let sample = match (tag, w.wBitsPerSample) {
            (WAVE_FORMAT_IEEE_FLOAT, 32) => Sample::F32,
            (WAVE_FORMAT_PCM, 16) => Sample::I16,
            (WAVE_FORMAT_PCM, 32) => Sample::I32,
            _ => return None,
        };
        Some(Self {
            rate: w.nSamplesPerSec,
            channels: w.nChannels.max(1) as usize,
            sample,
        })
    }

    /// Writes float samples into a device buffer in this format.
    ///
    /// # Safety
    /// `dst` must have room for `src.len()` samples of this format.
    pub unsafe fn write(&self, src: &[f32], dst: *mut u8) {
        match self.sample {
            Sample::F32 => unsafe {
                std::ptr::copy_nonoverlapping(src.as_ptr(), dst as *mut f32, src.len())
            },
            Sample::I16 => {
                let dst = unsafe { std::slice::from_raw_parts_mut(dst as *mut i16, src.len()) };
                for (d, s) in dst.iter_mut().zip(src) {
                    *d = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                }
            }
            Sample::I32 => {
                let dst = unsafe { std::slice::from_raw_parts_mut(dst as *mut i32, src.len()) };
                for (d, s) in dst.iter_mut().zip(src) {
                    *d = (s.clamp(-1.0, 1.0) as f64 * i32::MAX as f64) as i32;
                }
            }
        }
    }

    /// Reads a device buffer in this format as float samples into `dst`.
    ///
    /// # Safety
    /// `src` must hold `dst.len()` samples of this format.
    pub unsafe fn read(&self, src: *const u8, dst: &mut [f32]) {
        match self.sample {
            Sample::F32 => unsafe {
                std::ptr::copy_nonoverlapping(src as *const f32, dst.as_mut_ptr(), dst.len())
            },
            Sample::I16 => {
                let src = unsafe { std::slice::from_raw_parts(src as *const i16, dst.len()) };
                for (d, s) in dst.iter_mut().zip(src) {
                    *d = *s as f32 / 32_768.0;
                }
            }
            Sample::I32 => {
                let src = unsafe { std::slice::from_raw_parts(src as *const i32, dst.len()) };
                for (d, s) in dst.iter_mut().zip(src) {
                    *d = (*s as f64 / 2_147_483_648.0) as f32;
                }
            }
        }
    }
}
