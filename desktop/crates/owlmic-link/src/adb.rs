//! USB debugging (SYSTEM_DESIGN section 14.3): when a phone's ADB interface appears, run adb
//! with `reverse tcp:7653 tcp:7653`, so the phone reaches this PC at 127.0.0.1. The adb server
//! runs only while a phone is plugged in, and only one Owlmic started is stopped. Nothing polls
//! once reversed: device arrivals come from Windows, and authorisations and adbd restarts from
//! `adb track-devices`.

use std::io::Read;
use std::net::Ipv4Addr;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, mpsc};
use std::time::Duration;
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    CM_GET_DEVICE_INTERFACE_LIST_PRESENT, CM_Get_Device_Interface_List_SizeW, CM_NOTIFY_ACTION,
    CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL, CM_NOTIFY_ACTION_DEVICEINTERFACEREMOVAL,
    CM_NOTIFY_EVENT_DATA, CM_NOTIFY_FILTER, CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE,
    CM_Register_Notification, CM_Unregister_Notification, CR_SUCCESS, HCMNOTIFICATION,
};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
};
use windows::Win32::Networking::WinSock::AF_INET;
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_BREAKAWAY_OK,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectExtendedLimitInformation, SetInformationJobObject,
};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    PROCESS_TERMINATE, QueryFullProcessImageNameW, TerminateProcess, WaitForSingleObject,
};
use windows::core::{GUID, PWSTR};

/// The interface Android's ADB function exposes on Windows.
const ADB_INTERFACE: GUID = GUID::from_u128(0xF72FE0D4_CBCB_407D_8814_9ED673D0DD6B);
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const ADB_SERVER_PORT: u16 = 5037;
const RETRY: Duration = Duration::from_millis(800);
const SLOW_RETRY: Duration = Duration::from_secs(3);
const RETRIES: u32 = 8;
const ADB_TIMEOUT: Duration = Duration::from_secs(10);
const ADB_START_TIMEOUT: Duration = Duration::from_secs(15);
const ADB_KILL_TIMEOUT: Duration = Duration::from_secs(3);
const TIMEOUTS_BEFORE_KILL: u32 = 2;
const SHUTDOWN_WAIT: Duration = Duration::from_millis(600);

static SHUTDOWN: AtomicBool = AtomicBool::new(false);
static WAKE: Mutex<Option<mpsc::Sender<()>>> = Mutex::new(None);
static DONE: (Mutex<bool>, Condvar) = (Mutex::new(false), Condvar::new());

/// Stops the ADB watcher and any ADB server Owlmic started before the process exits.
pub fn shutdown() {
    SHUTDOWN.store(true, Ordering::Release);
    if let Some(tx) = WAKE.lock().unwrap_or_else(|p| p.into_inner()).as_ref() {
        let _ = tx.send(());
    } else {
        return;
    }
    let (lock, cv) = &DONE;
    let guard = lock.lock().unwrap_or_else(|p| p.into_inner());
    if !*guard {
        let _ = cv.wait_timeout(guard, SHUTDOWN_WAIT);
    }
}

/// Keeps spawned `adb` clients from outliving Owlmic on crashes or forced kills.
struct Job(HANDLE);

impl Job {
    fn new() -> Option<Self> {
        let handle = unsafe { CreateJobObjectW(None, None) }.ok()?;
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags =
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_BREAKAWAY_OK;
        let ok = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&raw const info).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if ok.is_err() {
            unsafe {
                let _ = CloseHandle(handle);
            }
            return None;
        }
        Some(Self(handle))
    }

    fn assign(&self, child: &Child) {
        let process = HANDLE(child.as_raw_handle());
        unsafe {
            let _ = AssignProcessToJobObject(self.0, process);
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

enum AdbOutcome {
    Exited { status: ExitStatus, stdout: String },
    Timeout,
    SpawnFailed,
}

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
        .chain(
            std::env::var_os("USERPROFILE")
                .map(|d| PathBuf::from(d).join("platform-tools").join("adb.exe")),
        )
        .find(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("adb"))
}

/// Runs `adb` with a hard timeout; `capture` is false when stdout isn't needed so a forked ADB
/// daemon cannot inherit pipe handles.
fn run_adb(args: &[&str], capture: bool, timeout: Duration, job: Option<&Job>) -> AdbOutcome {
    let mut cmd = Command::new(adb_path());
    cmd.args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .stdout(if capture {
            Stdio::piped()
        } else {
            Stdio::null()
        });
    let Ok(mut child) = cmd.spawn() else {
        return AdbOutcome::SpawnFailed;
    };
    if let Some(j) = job {
        j.assign(&child);
    }
    let rx = if capture {
        child.stdout.take().map(|mut out| {
            let (tx, rx) = mpsc::channel();
            let _ = std::thread::Builder::new()
                .name("owlmic-adb-out".into())
                .spawn(move || {
                    let mut buf = Vec::new();
                    let _ = out.read_to_end(&mut buf);
                    let _ = tx.send(String::from_utf8_lossy(&buf).into_owned());
                });
            rx
        })
    } else {
        None
    };
    let wait = unsafe {
        WaitForSingleObject(
            HANDLE(child.as_raw_handle()),
            timeout.as_millis().min(u32::MAX as u128) as u32,
        )
    };
    if wait != WAIT_OBJECT_0 {
        let _ = child.kill();
        let _ = child.wait();
        return AdbOutcome::Timeout;
    }
    let Ok(status) = child.wait() else {
        return AdbOutcome::SpawnFailed;
    };
    let stdout = rx
        .and_then(|r| r.recv_timeout(Duration::from_millis(200)).ok())
        .unwrap_or_default();
    AdbOutcome::Exited { status, stdout }
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

fn parse_authorised_serials(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .skip(1)
        .filter_map(|l| l.split_once('\t'))
        .filter(|(_, state)| state.trim() == "device")
        .map(|(serial, _)| serial.trim().to_owned())
        .collect()
}

/// Authorised phones from `adb devices`, or `None` when ADB timed out.
fn authorised_serials(job: Option<&Job>) -> Option<Vec<String>> {
    match run_adb(&["devices"], true, ADB_TIMEOUT, job) {
        AdbOutcome::Exited { status, stdout } if status.success() => {
            Some(parse_authorised_serials(&stdout))
        }
        AdbOutcome::Exited { .. } | AdbOutcome::SpawnFailed => Some(Vec::new()),
        AdbOutcome::Timeout => None,
    }
}

enum ReverseResult {
    Ok,
    Failed,
    Timeout,
}

fn reverse(serial: &str, job: Option<&Job>) -> ReverseResult {
    match run_adb(
        &["-s", serial, "reverse", "tcp:7653", "tcp:7653"],
        false,
        ADB_TIMEOUT,
        job,
    ) {
        AdbOutcome::Exited { status, .. } if status.success() => ReverseResult::Ok,
        AdbOutcome::Exited { .. } | AdbOutcome::SpawnFailed => ReverseResult::Failed,
        AdbOutcome::Timeout => ReverseResult::Timeout,
    }
}

fn listener_pids(port: u16) -> Vec<u32> {
    let mut size = 0u32;
    let af = AF_INET.0 as u32;
    unsafe {
        let _ = GetExtendedTcpTable(None, &mut size, false, af, TCP_TABLE_OWNER_PID_LISTENER, 0);
    }
    if size == 0 {
        return Vec::new();
    }
    let mut buf = vec![0u64; (size as usize).div_ceil(8)];
    let rc = unsafe {
        GetExtendedTcpTable(
            Some(buf.as_mut_ptr().cast()),
            &mut size,
            false,
            af,
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        )
    };
    if rc != 0 {
        return Vec::new();
    }
    let table = unsafe { &*(buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID) };
    let rows =
        unsafe { std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize) };
    rows.iter()
        .filter(|r| {
            let addr = Ipv4Addr::from(u32::from_be(r.dwLocalAddr));
            let local_port = u16::from_be(r.dwLocalPort as u16);
            local_port == port && (addr.is_loopback() || addr.is_unspecified())
        })
        .map(|r| r.dwOwningPid)
        .collect()
}

fn terminate_wedged_adb_server() {
    for pid in listener_pids(ADB_SERVER_PORT) {
        let Ok(proc) = (unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE,
                false,
                pid,
            )
        }) else {
            continue;
        };
        let mut buf = [0u16; 260];
        let mut len = buf.len() as u32;
        let ok = unsafe {
            QueryFullProcessImageNameW(proc, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len)
        };
        if ok.is_ok() {
            let path = String::from_utf16_lossy(&buf[..len as usize]);
            let is_adb = Path::new(&path)
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case("adb.exe"));
            if is_adb {
                unsafe {
                    let _ = TerminateProcess(proc, 1);
                    let _ = WaitForSingleObject(proc, 1000);
                }
            }
        }
        unsafe {
            let _ = CloseHandle(proc);
        }
    }
}

fn recover_hung_server(
    tracker: &mut Option<Tracker>,
    reversed: &mut Vec<String>,
    owned_server: &mut bool,
    job: Option<&Job>,
) {
    *tracker = None;
    reversed.clear();
    let killed = matches!(
        run_adb(&["kill-server"], false, ADB_KILL_TIMEOUT, job),
        AdbOutcome::Exited { .. }
    );
    if !killed || server_already_running() {
        terminate_wedged_adb_server();
    }
    *owned_server = false;
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
/// change it prints is an event.
struct Tracker {
    child: Child,
}

impl Tracker {
    fn start(tx: mpsc::Sender<()>, job: Option<&Job>) -> Option<Self> {
        let mut child = Command::new(adb_path())
            .arg("track-devices")
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .ok()?;
        if let Some(j) = job {
            j.assign(&child);
        }
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
                    let _ = tx.send(());
                });
        }
        Some(Self { child })
    }

    fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for Tracker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn next_wait(
    iface_present: bool,
    tracker_alive: bool,
    need_retry: bool,
    retries: u32,
) -> (Option<Duration>, u32) {
    if !iface_present {
        return (tracker_alive.then_some(RETRY), 0);
    }
    if !tracker_alive || need_retry {
        return if retries > 0 {
            (Some(RETRY), retries - 1)
        } else {
            (Some(SLOW_RETRY), 0)
        };
    }
    (None, RETRIES)
}

/// Watches for phones with USB debugging until something fails. Runs on its own supervised
/// thread.
pub fn watch(up: &dyn Fn()) -> Result<(), String> {
    let (tx, rx) = mpsc::channel();
    let _registration = Registration::new(tx.clone()).ok_or("device notifications")?;
    let job = Job::new();
    *WAKE.lock().unwrap_or_else(|p| p.into_inner()) = Some(tx.clone());
    *DONE.0.lock().unwrap_or_else(|p| p.into_inner()) = false;
    up();

    let mut tracker: Option<Tracker> = None;
    let mut reversed: Vec<String> = Vec::new();
    let mut wait: Option<Duration> = None;
    let mut retries = RETRIES;
    let mut timeouts = 0u32;
    let mut owned_server = false;
    let mut detach_ticks = 0u32;
    let _ = tx.send(());

    loop {
        let (closed, mut had_event) = match wait {
            Some(d) => match rx.recv_timeout(d) {
                Ok(()) => (false, true),
                Err(mpsc::RecvTimeoutError::Timeout) => (false, false),
                Err(mpsc::RecvTimeoutError::Disconnected) => (true, false),
            },
            None => (rx.recv().is_err(), true),
        };
        if closed {
            *WAKE.lock().unwrap_or_else(|p| p.into_inner()) = None;
            return Err("device notifications stopped".into());
        }
        while rx.try_recv().is_ok() {
            had_event = true;
        }
        if SHUTDOWN.load(Ordering::Acquire) {
            drop(tracker);
            if owned_server {
                let _ = run_adb(&["kill-server"], false, ADB_KILL_TIMEOUT, job.as_ref());
            }
            *WAKE.lock().unwrap_or_else(|p| p.into_inner()) = None;
            *DONE.0.lock().unwrap_or_else(|p| p.into_inner()) = true;
            DONE.1.notify_all();
            loop {
                std::thread::park();
            }
        }
        if let Some(t) = tracker.as_mut()
            && !t.alive()
        {
            tracker = None;
        }
        let iface_present = adb_interfaces_present();
        if !iface_present {
            reversed.clear();
            timeouts = 0;
            // Avoid killing adb server mid-replug when user detaches and attaches quickly.
            if tracker.is_some() {
                detach_ticks += 1;
                if detach_ticks >= 3 {
                    tracker = None;
                    if owned_server {
                        let _ = run_adb(&["kill-server"], false, ADB_KILL_TIMEOUT, job.as_ref());
                        owned_server = false;
                    }
                    detach_ticks = 0;
                }
            }
            (wait, retries) = next_wait(false, tracker.is_some(), false, retries);
            continue;
        }
        detach_ticks = 0;
        if had_event {
            // Any USB or device-tracker change may have restarted adbd (e.g. USB mode toggle).
            reversed.clear();
            retries = RETRIES;
        }
        if tracker.is_none() {
            if !server_already_running() {
                match run_adb(&["start-server"], false, ADB_START_TIMEOUT, job.as_ref()) {
                    AdbOutcome::Exited { status, .. } if status.success() => {
                        owned_server = true;
                        timeouts = 0;
                    }
                    AdbOutcome::Exited { .. } | AdbOutcome::SpawnFailed => {
                        timeouts = 0;
                        (wait, retries) = next_wait(true, false, true, retries);
                        continue;
                    }
                    AdbOutcome::Timeout => {
                        timeouts += 1;
                        if timeouts >= TIMEOUTS_BEFORE_KILL {
                            timeouts = 0;
                            recover_hung_server(
                                &mut tracker,
                                &mut reversed,
                                &mut owned_server,
                                job.as_ref(),
                            );
                        }
                        (wait, retries) = next_wait(true, false, true, retries);
                        continue;
                    }
                }
            }
            tracker = Tracker::start(tx.clone(), job.as_ref());
        }
        let Some(authorised) = authorised_serials(job.as_ref()) else {
            timeouts += 1;
            if timeouts >= TIMEOUTS_BEFORE_KILL {
                timeouts = 0;
                recover_hung_server(&mut tracker, &mut reversed, &mut owned_server, job.as_ref());
            }
            (wait, retries) = next_wait(true, tracker.is_some(), true, retries);
            continue;
        };
        timeouts = 0;
        reversed.retain(|s| authorised.contains(s));
        let mut need_retry = authorised.is_empty();
        for serial in authorised {
            if !reversed.contains(&serial) {
                match reverse(&serial, job.as_ref()) {
                    ReverseResult::Ok => {
                        timeouts = 0;
                        reversed.push(serial);
                    }
                    ReverseResult::Failed => need_retry = true,
                    ReverseResult::Timeout => {
                        timeouts += 1;
                        if timeouts >= TIMEOUTS_BEFORE_KILL {
                            timeouts = 0;
                            recover_hung_server(
                                &mut tracker,
                                &mut reversed,
                                &mut owned_server,
                                job.as_ref(),
                            );
                        }
                        need_retry = true;
                        break;
                    }
                }
            }
        }
        (wait, retries) = next_wait(true, tracker.is_some(), need_retry, retries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_authorised_serials_with_crlf_and_ignores_other_states() {
        let out = "List of devices attached\r\nabc123\tdevice\r\ndef456\tunauthorized\r\nghi789\toffline\r\n";
        assert_eq!(parse_authorised_serials(out), vec!["abc123"]);
    }

    #[test]
    fn retries_keep_going_slowly_while_phone_is_plugged_in_and_stop_once_reversed() {
        let (wait, r) = next_wait(true, true, true, 1);
        assert_eq!((wait, r), (Some(RETRY), 0));
        let (wait, r) = next_wait(true, true, true, 0);
        assert_eq!((wait, r), (Some(SLOW_RETRY), 0));
        let (wait, r) = next_wait(true, true, false, 0);
        assert_eq!((wait, r), (None, RETRIES));
        let (wait, r) = next_wait(false, false, false, 0);
        assert_eq!((wait, r), (None, 0));
    }
}
