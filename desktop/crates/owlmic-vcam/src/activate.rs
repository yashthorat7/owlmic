//! The class factory the camera service creates, and the IMFActivate it hands out, which builds
//! the media source on demand. The activate's attributes are a plain store kept for the service.

use crate::exports::LIVE;
use crate::source;
use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use windows::Win32::Foundation::{CLASS_E_NOAGGREGATION, E_POINTER};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::IClassFactory;
use windows::Win32::System::Com::IClassFactory_Impl;
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::core::{BOOL, GUID, IUnknown, Interface, PCWSTR, PWSTR, Ref, Result, implement};

#[implement(IClassFactory)]
pub struct Factory;

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> Result<()> {
        if out.is_null() || iid.is_null() {
            return Err(E_POINTER.into());
        }
        unsafe { *out = std::ptr::null_mut() };
        if !outer.is_null() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let activate: IMFActivate = Activate::new()?.into();
        // The camera service asks for IMFActivate; anything else gets the source itself.
        if unsafe { activate.query(iid, out) }.is_ok() {
            return Ok(());
        }
        unsafe { source::create()?.query(iid, out).ok() }
    }

    fn LockServer(&self, lock: BOOL) -> Result<()> {
        LIVE.fetch_add(if lock.as_bool() { 1 } else { -1 }, Ordering::AcqRel);
        Ok(())
    }
}

#[implement(IMFActivate)]
pub struct Activate {
    attributes: IMFAttributes,
    source: Mutex<Option<IMFMediaSourceEx>>,
}

impl Activate {
    fn new() -> Result<Self> {
        let mut attributes = None;
        unsafe { MFCreateAttributes(&mut attributes, 1)? };
        LIVE.fetch_add(1, Ordering::AcqRel);
        Ok(Self {
            attributes: attributes.ok_or(windows::Win32::Foundation::E_UNEXPECTED)?,
            source: Mutex::new(None),
        })
    }
}

impl Drop for Activate {
    fn drop(&mut self) {
        LIVE.fetch_sub(1, Ordering::AcqRel);
    }
}

impl IMFActivate_Impl for Activate_Impl {
    fn ActivateObject(&self, iid: *const GUID, out: *mut *mut c_void) -> Result<()> {
        let mut slot = self.source.lock().unwrap_or_else(|p| p.into_inner());
        if slot.is_none() {
            *slot = Some(source::create()?);
        }
        unsafe { slot.as_ref().unwrap().query(iid, out).ok() }
    }

    fn ShutdownObject(&self) -> Result<()> {
        if let Some(s) = self.source.lock().unwrap_or_else(|p| p.into_inner()).take() {
            unsafe { s.Shutdown()? };
        }
        Ok(())
    }

    fn DetachObject(&self) -> Result<()> {
        self.source.lock().unwrap_or_else(|p| p.into_inner()).take();
        Ok(())
    }
}

impl IMFAttributes_Impl for Activate_Impl {
    fn GetItem(&self, key: *const GUID, value: *mut PROPVARIANT) -> Result<()> {
        unsafe {
            self.attributes
                .GetItem(key, (!value.is_null()).then_some(value))
        }
    }
    fn GetItemType(&self, key: *const GUID) -> Result<MF_ATTRIBUTE_TYPE> {
        unsafe { self.attributes.GetItemType(key) }
    }
    fn CompareItem(&self, key: *const GUID, value: *const PROPVARIANT) -> Result<BOOL> {
        unsafe { self.attributes.CompareItem(key, value) }
    }
    fn Compare(&self, theirs: Ref<IMFAttributes>, kind: MF_ATTRIBUTES_MATCH_TYPE) -> Result<BOOL> {
        unsafe { self.attributes.Compare(theirs.as_ref(), kind) }
    }
    fn GetUINT32(&self, key: *const GUID) -> Result<u32> {
        unsafe { self.attributes.GetUINT32(key) }
    }
    fn GetUINT64(&self, key: *const GUID) -> Result<u64> {
        unsafe { self.attributes.GetUINT64(key) }
    }
    fn GetDouble(&self, key: *const GUID) -> Result<f64> {
        unsafe { self.attributes.GetDouble(key) }
    }
    fn GetGUID(&self, key: *const GUID) -> Result<GUID> {
        unsafe { self.attributes.GetGUID(key) }
    }
    fn GetStringLength(&self, key: *const GUID) -> Result<u32> {
        unsafe { self.attributes.GetStringLength(key) }
    }
    fn GetString(&self, key: *const GUID, value: PWSTR, size: u32, length: *mut u32) -> Result<()> {
        let buf = unsafe { std::slice::from_raw_parts_mut(value.0, size as usize) };
        unsafe {
            self.attributes
                .GetString(key, buf, (!length.is_null()).then_some(length))
        }
    }
    fn GetAllocatedString(
        &self,
        key: *const GUID,
        value: *mut PWSTR,
        length: *mut u32,
    ) -> Result<()> {
        unsafe { self.attributes.GetAllocatedString(key, value, length) }
    }
    fn GetBlobSize(&self, key: *const GUID) -> Result<u32> {
        unsafe { self.attributes.GetBlobSize(key) }
    }
    fn GetBlob(&self, key: *const GUID, buf: *mut u8, size: u32, written: *mut u32) -> Result<()> {
        let buf = unsafe { std::slice::from_raw_parts_mut(buf, size as usize) };
        unsafe {
            self.attributes
                .GetBlob(key, buf, (!written.is_null()).then_some(written))
        }
    }
    fn GetAllocatedBlob(&self, key: *const GUID, buf: *mut *mut u8, size: *mut u32) -> Result<()> {
        unsafe { self.attributes.GetAllocatedBlob(key, buf, size) }
    }
    fn GetUnknown(&self, key: *const GUID, iid: *const GUID, out: *mut *mut c_void) -> Result<()> {
        let unknown: IUnknown = unsafe { self.attributes.GetUnknown(key)? };
        unsafe { unknown.query(iid, out).ok() }
    }
    fn SetItem(&self, key: *const GUID, value: *const PROPVARIANT) -> Result<()> {
        unsafe { self.attributes.SetItem(key, value) }
    }
    fn DeleteItem(&self, key: *const GUID) -> Result<()> {
        unsafe { self.attributes.DeleteItem(key) }
    }
    fn DeleteAllItems(&self) -> Result<()> {
        unsafe { self.attributes.DeleteAllItems() }
    }
    fn SetUINT32(&self, key: *const GUID, value: u32) -> Result<()> {
        unsafe { self.attributes.SetUINT32(key, value) }
    }
    fn SetUINT64(&self, key: *const GUID, value: u64) -> Result<()> {
        unsafe { self.attributes.SetUINT64(key, value) }
    }
    fn SetDouble(&self, key: *const GUID, value: f64) -> Result<()> {
        unsafe { self.attributes.SetDouble(key, value) }
    }
    fn SetGUID(&self, key: *const GUID, value: *const GUID) -> Result<()> {
        unsafe { self.attributes.SetGUID(key, value) }
    }
    fn SetString(&self, key: *const GUID, value: &PCWSTR) -> Result<()> {
        unsafe { self.attributes.SetString(key, *value) }
    }
    fn SetBlob(&self, key: *const GUID, buf: *const u8, size: u32) -> Result<()> {
        let buf = unsafe { std::slice::from_raw_parts(buf, size as usize) };
        unsafe { self.attributes.SetBlob(key, buf) }
    }
    fn SetUnknown(&self, key: *const GUID, value: Ref<IUnknown>) -> Result<()> {
        unsafe { self.attributes.SetUnknown(key, value.as_ref()) }
    }
    fn LockStore(&self) -> Result<()> {
        unsafe { self.attributes.LockStore() }
    }
    fn UnlockStore(&self) -> Result<()> {
        unsafe { self.attributes.UnlockStore() }
    }
    fn GetCount(&self) -> Result<u32> {
        unsafe { self.attributes.GetCount() }
    }
    fn GetItemByIndex(&self, index: u32, key: *mut GUID, value: *mut PROPVARIANT) -> Result<()> {
        unsafe {
            self.attributes
                .GetItemByIndex(index, key, (!value.is_null()).then_some(value))
        }
    }
    fn CopyAllItems(&self, dest: Ref<IMFAttributes>) -> Result<()> {
        unsafe { self.attributes.CopyAllItems(dest.as_ref()) }
    }
}
