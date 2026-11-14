//! USB debugging (SYSTEM_DESIGN section 14.3): when a phone's ADB interface appears, run adb
//! with `reverse tcp:7653 tcp:7653`, so the phone reaches this PC at 127.0.0.1. The adb server
//! runs only while a phone is plugged in, and only one Owlmic started is stopped. Nothing polls:
//! device arrivals come from Windows, and authorisations and adbd restarts from
//! `adb track-devices`.

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_GET_DEVICE_INTERFACE_LIST_PRESENT, CM_Get_Device_Interface_List_SizeW, CM_NOTIFY_ACTION,
    CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL, CM_NOTIFY_ACTION_DEVICEINTERFACEREMOVAL,
    CM_NOTIFY_EVENT_DATA, CM_NOTIFY_FILTER, CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE,
    CM_Register_Notification, CM_Unregister_Notification, CR_SUCCESS, HCMNOTIFICATION,
};
use windows::core::GUID;

/// The interface Android's ADB function exposes on Windows.
const ADB_INTERFACE: GUID = GUID::from_u128(0xF72FE0D4_CBCB_407D_8814_9ED673D0DD6B);
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const ADB_SERVER_PORT: u16 = 5037;
/// A reverse that failed (adbd still starting, say) is tried again this often, a few times.
const RETRY: Duration = Duration::from_secs(2);
const RETRIES: u32 = 5;

/// adb isn't bundled: phones with USB debugging belong to people who already have it, from the
/// Android SDK (ANDROID_HOME, ANDROID_SDK_ROOT or Android Studio's default folder) or on the PATH.
pub fn adb_path() -> PathBuf {
    let sdk_roots = ["ANDROID_HOME", "ANDROID_SDK_ROOT"]
        .into_iter()
        .filter_map(|var| std::env::var_os(var).map(PathBuf::from))
        .chain(
            std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Android").join("Sdk")),
        );
    sdk_roots
        .map(|root| root.join("platform-tools").join("adb.exe"))
        .find(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("adb"))
}

fn adb(args: &[&str]) -> Option<std::process::Output> {
    Command::new(adb_path())
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .output()
        .ok()
}

fn adb_interfaces_present() -> bool {
    let mut len = 0u32;
    let r = unsafe {
        CM_Get_Device_Interface_List_SizeW(
            &mut len,
            &ADB_INTERFACE,
            None,
            CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
        )
    };
    // An empty list is a single terminating null.
    r == CR_SUCCESS && len > 1
}

fn server_already_running() -> bool {
    std::net::TcpStream::connect_timeout(
        &([127, 0, 0, 1], ADB_SERVER_PORT).into(),
        Duration::from_millis(200),
    )
    .is_ok()
}

/// Authorised phones from `adb devices`.
fn authorised_serials() -> Vec<String> {
    adb(&["devices"])
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
        .lines()
        .skip(1)
        .filter_map(|l| l.split_once('\t'))
        .filter(|(_, state)| state.trim() == "device")
        .map(|(serial, _)| serial.to_owned())
        .collect()
}

fn reverse(serial: &str) -> bool {
    adb(&["-s", serial, "reverse", "tcp:7653", "tcp:7653"]).is_some_and(|o| o.status.success())
}

unsafe extern "system" fn on_interface(
    _h: HCMNOTIFICATION,
    context: *const core::ffi::c_void,
    action: CM_NOTIFY_ACTION,
    _data: *const CM_NOTIFY_EVENT_DATA,
    _size: u32,
) -> u32 {
    if action == CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL
        || action == CM_NOTIFY_ACTION_DEVICEINTERFACEREMOVAL
    {
        let tx = unsafe { &*(context as *const mpsc::Sender<()>) };
        let _ = tx.send(());
    }
    0
}

/// Windows tells `tx` when an ADB interface comes or goes, until dropped.
struct Registration {
    handle: HCMNOTIFICATION,
    context: *mut mpsc::Sender<()>,
}

impl Registration {
    fn new(tx: mpsc::Sender<()>) -> Option<Self> {
        let context = Box::into_raw(Box::new(tx));
        let mut filter = CM_NOTIFY_FILTER {
            cbSize: size_of::<CM_NOTIFY_FILTER>() as u32,
            FilterType: CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE,
            ..Default::default()
        };
        filter.u.DeviceInterface.ClassGuid = ADB_INTERFACE;
        let mut handle = HCMNOTIFICATION::default();
        let r = unsafe {
            CM_Register_Notification(
                &filter,
                Some(context as *const _),
                Some(on_interface),
                &mut handle,
            )
        };
        if r != CR_SUCCESS {
            drop(unsafe { Box::from_raw(context) });
            return None;
        }
        Some(Self { handle, context })
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        // Waits for a callback in progress, so the context can go after it.
        unsafe {
            let _ = CM_Unregister_Notification(self.handle);
            drop(Box::from_raw(self.context));
        }
    }
}

/// `adb track-devices`, which also keeps the adb server up while phones are plugged in. Every
/// change it prints is an event. Dropping it stops it, and the server if Owlmic started that.
struct Tracker {
    child: Child,
    started_server: bool,
}

impl Tracker {
    fn start(tx: mpsc::Sender<()>) -> Option<Self> {
        let started_server = !server_already_running();
        let mut child = Command::new(adb_path())
            .arg("track-devices")
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .ok()?;
        if let Some(mut out) = child.stdout.take() {
            let _ = std::thread::Builder::new()
                .name("owlmic-adb-track".into())
                .spawn(move || {
                    let mut buf = [0u8; 512];
                    while matches!(std::io::Read::read(&mut out, &mut buf), Ok(n) if n > 0) {
                        if tx.send(()).is_err() {
                            return;
                        }
                    }
                });
        }
        Some(Self {
            child,
            started_server,
        })
    }
}

impl Drop for Tracker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if self.started_server {
            let _ = adb(&["kill-server"]);
        }
    }
}

/// Watches for phones with USB debugging until something fails. Runs on its own supervised
/// thread.
pub fn watch(up: &dyn Fn()) -> Result<(), String> {
    let (tx, rx) = mpsc::channel();
    let _registration = Registration::new(tx.clone()).ok_or("device notifications")?;
    up();
    let mut tracker: Option<Tracker> = None;
    let mut reversed: Vec<String> = Vec::new();
    let mut retries = 0;
    let _ = tx.send(());
    loop {
        let event = if retries > 0 {
            rx.recv_timeout(RETRY).or_else(|e| match e {
                mpsc::RecvTimeoutError::Timeout => Ok(()),
                mpsc::RecvTimeoutError::Disconnected => Err(()),
            })
        } else {
            rx.recv().map_err(|_| ())
        };
        if event.is_err() {
            return Err("device notifications stopped".into());
        }
        while rx.try_recv().is_ok() {}
        if !adb_interfaces_present() {
            tracker = None;
            reversed.clear();
            retries = 0;
            continue;
        }
        if tracker.is_none() {
            tracker = Tracker::start(tx.clone());
        }
        // A phone that went offline (adbd restarted, debugging toggled) needs its reverse again.
        let authorised = authorised_serials();
        reversed.retain(|s| authorised.contains(s));
        let mut failed = false;
        for serial in authorised {
            if !reversed.contains(&serial) {
                if reverse(&serial) {
                    reversed.push(serial);
                } else {
                    failed = true;
                }
            }
        }
        retries = match (failed, retries) {
            (false, _) => 0,
            (true, 0) => RETRIES,
            (true, n) => n - 1,
        };
    }
}
