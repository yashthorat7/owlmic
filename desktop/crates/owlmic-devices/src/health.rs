//! The device check behind the Repair tile (SYSTEM_DESIGN sections 13.4 and 19): Owlmic Mic's
//! endpoints, Owlmic Cam's registration and the firewall rules. Audio devices coming and going
//! trigger a new check.

use crate::DeviceHealth;
use crate::audio::{enumerator, find};
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::{
    DEVICE_STATE, EDataFlow, ERole, IMMDeviceEnumerator, IMMNotificationClient,
    IMMNotificationClient_Impl, eConsole, eRender,
};
use windows::core::{PCWSTR, Result, implement};

pub fn check(win11: bool) -> DeviceHealth {
    let mic = enumerator().is_ok_and(|e| find(&e, eRender, &crate::render::ENDPOINTS).is_some());
    let cam = if win11 {
        owlmic_vcam::register::is_registered()
    } else {
        crate::softcam::is_registered()
    };
    DeviceHealth {
        mic,
        cam,
        net: crate::firewall::rules_present(),
    }
}

#[implement(IMMNotificationClient)]
struct Changes {
    devices: Box<dyn Fn() + Send + Sync>,
    default_output: Box<dyn Fn() + Send + Sync>,
}

impl IMMNotificationClient_Impl for Changes_Impl {
    fn OnDeviceStateChanged(&self, _: &PCWSTR, _: DEVICE_STATE) -> Result<()> {
        (self.devices)();
        Ok(())
    }
    fn OnDeviceAdded(&self, _: &PCWSTR) -> Result<()> {
        (self.devices)();
        Ok(())
    }
    fn OnDeviceRemoved(&self, _: &PCWSTR) -> Result<()> {
        (self.devices)();
        Ok(())
    }
    fn OnDefaultDeviceChanged(&self, flow: EDataFlow, role: ERole, _: &PCWSTR) -> Result<()> {
        if flow == eRender && role == eConsole {
            (self.default_output)();
        }
        Ok(())
    }
    fn OnPropertyValueChanged(&self, _: &PCWSTR, _: &PROPERTYKEY) -> Result<()> {
        Ok(())
    }
}

/// Calls back on audio device changes, and when the default output moves, until dropped.
pub struct Watch {
    devices: IMMDeviceEnumerator,
    client: IMMNotificationClient,
}

pub fn watch(
    on_change: impl Fn() + Send + Sync + 'static,
    on_default_output: impl Fn() + Send + Sync + 'static,
) -> Option<Watch> {
    let devices = enumerator().ok()?;
    let client: IMMNotificationClient = Changes {
        devices: Box::new(on_change),
        default_output: Box::new(on_default_output),
    }
    .into();
    unsafe { devices.RegisterEndpointNotificationCallback(&client).ok()? };
    Some(Watch { devices, client })
}

impl Drop for Watch {
    fn drop(&mut self) {
        unsafe {
            let _ = self
                .devices
                .UnregisterEndpointNotificationCallback(&self.client);
        }
    }
}
