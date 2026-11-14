//! Bluetooth (SYSTEM_DESIGN section 14.1, level 4): an RFCOMM server that publishes the Owlmic
//! service record, so a bonded phone finds it by UUID. One stream carries control and audio.

use crate::hub::BtAddr;
use crate::server::{self, Shared};
use crate::wire::Closer;
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use windows::Win32::Devices::Bluetooth::{AF_BTH, BTHPROTO_RFCOMM, NS_BTH, SOCKADDR_BTH};
use windows::Win32::Networking::WinSock::{
    CSADDR_INFO, RNRSERVICE_DELETE, RNRSERVICE_REGISTER, SD_BOTH, SEND_RECV_FLAGS, SO_RCVTIMEO,
    SO_SNDTIMEO, SOCK_STREAM, SOCKADDR, SOCKET, SOCKET_ADDRESS, SOL_SOCKET, WSADATA, WSAQUERYSETW,
    WSASetServiceW, WSAStartup, accept, bind, closesocket, getsockname, listen, recv, send,
    setsockopt, shutdown, socket,
};
use windows::core::{GUID, PWSTR};

/// "owlmic" in its first six bytes (SYSTEM_DESIGN section 14.1).
pub const SERVICE: GUID = GUID::from_u128(0x6f776c6d_6963_4000_8000_00805f9b34fb);
const BT_PORT_ANY: u32 = u32::MAX;
/// A phone walking out of range must not hold a write forever.
const SEND_TIMEOUT_MS: u32 = 5_000;
/// The same backstop as a TCP control connection's.
const RECEIVE_TIMEOUT_MS: u32 = 150_000;

/// A socket closed when the last handle to it goes.
struct Socket(SOCKET);

impl Drop for Socket {
    fn drop(&mut self) {
        unsafe {
            closesocket(self.0);
        }
    }
}

/// One end of a connection: reading, writing and closing share the socket.
#[derive(Clone)]
struct Stream(Arc<Socket>);

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = unsafe { recv(self.0.0, buf, SEND_RECV_FLAGS(0)) };
        if n < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(n as usize)
        }
    }
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = unsafe { send(self.0.0, buf, SEND_RECV_FLAGS(0)) };
        if n < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(n as usize)
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn set_timeout(s: SOCKET, option: i32, ms: u32) {
    unsafe {
        setsockopt(s, SOL_SOCKET, option, Some(&ms.to_ne_bytes()));
    }
}

/// The local radio's address as "AA:BB:CC:DD:EE:FF", for HELLO_ACK.
pub fn format_addr(addr: u64) -> String {
    (0..6)
        .rev()
        .map(|i| format!("{:02X}", (addr >> (i * 8)) & 0xFF))
        .collect::<Vec<_>>()
        .join(":")
}

/// The published service record, removed when dropped.
struct Record(SOCKADDR_BTH);

impl Record {
    fn publish(local: SOCKADDR_BTH) -> Option<Self> {
        let mut r = Self(local);
        unsafe { set_service(&mut r.0, RNRSERVICE_REGISTER) }.then_some(r)
    }
}

impl Drop for Record {
    fn drop(&mut self) {
        unsafe {
            set_service(&mut self.0, RNRSERVICE_DELETE);
        }
    }
}

/// Registers or removes the record that lets phones find this server's channel by the Owlmic
/// UUID.
unsafe fn set_service(
    local: &mut SOCKADDR_BTH,
    op: windows::Win32::Networking::WinSock::WSAESETSERVICEOP,
) -> bool {
    let mut name: Vec<u16> = "Owlmic\0".encode_utf16().collect();
    let mut class = SERVICE;
    let mut addr = CSADDR_INFO {
        LocalAddr: SOCKET_ADDRESS {
            lpSockaddr: local as *mut _ as *mut SOCKADDR,
            iSockaddrLength: size_of::<SOCKADDR_BTH>() as i32,
        },
        RemoteAddr: SOCKET_ADDRESS::default(),
        iSocketType: SOCK_STREAM.0,
        iProtocol: BTHPROTO_RFCOMM as i32,
    };
    let set = WSAQUERYSETW {
        dwSize: size_of::<WSAQUERYSETW>() as u32,
        lpszServiceInstanceName: PWSTR(name.as_mut_ptr()),
        lpServiceClassId: &mut class,
        dwNameSpace: NS_BTH,
        dwNumberOfCsAddrs: 1,
        lpcsaBuffer: &mut addr,
        ..Default::default()
    };
    unsafe { WSASetServiceW(&set, op, 0) == 0 }
}

/// Runs the RFCOMM server until it fails. Runs on its own supervised thread; without a radio it
/// fails at once and is tried again later. `addr` holds this PC's Bluetooth address meanwhile.
pub fn serve(shared: &Arc<Shared>, addr: &BtAddr, up: &dyn Fn()) -> Result<(), String> {
    static WINSOCK: std::sync::Once = std::sync::Once::new();
    WINSOCK.call_once(|| unsafe {
        let mut data = WSADATA::default();
        WSAStartup(0x0202, &mut data);
    });
    let listener = unsafe {
        Socket(
            socket(AF_BTH as i32, SOCK_STREAM, BTHPROTO_RFCOMM as i32)
                .map_err(|_| "no Bluetooth radio")?,
        )
    };
    let mut local = SOCKADDR_BTH {
        addressFamily: AF_BTH,
        btAddr: 0,
        serviceClassId: GUID::zeroed(),
        port: BT_PORT_ANY,
    };
    let len = size_of::<SOCKADDR_BTH>() as i32;
    unsafe {
        if bind(listener.0, &local as *const _ as *const SOCKADDR, len) != 0
            || listen(listener.0, 2) != 0
        {
            return Err("no Bluetooth radio".into());
        }
        let mut got = len;
        getsockname(listener.0, &mut local as *mut _ as *mut SOCKADDR, &mut got);
    }
    let _record = Record::publish(local).ok_or("the service record")?;
    let set_addr = |a: Option<String>| *addr.lock().unwrap_or_else(|p| p.into_inner()) = a;
    set_addr(Some(format_addr(local.btAddr)));
    up();
    let result = accept_until_failed(&listener, shared);
    set_addr(None);
    result
}

fn accept_until_failed(listener: &Socket, shared: &Arc<Shared>) -> Result<(), String> {
    loop {
        let mut peer = SOCKADDR_BTH::default();
        let mut len = size_of::<SOCKADDR_BTH>() as i32;
        let accepted = unsafe {
            accept(
                listener.0,
                Some(&mut peer as *mut _ as *mut SOCKADDR),
                Some(&mut len),
            )
        };
        let s = Stream(Arc::new(Socket(
            accepted.map_err(|_| "the Bluetooth radio went away")?,
        )));
        let Some(slot) = server::claim(shared) else {
            continue;
        };
        set_timeout(s.0.0, SO_SNDTIMEO, SEND_TIMEOUT_MS);
        set_timeout(s.0.0, SO_RCVTIMEO, RECEIVE_TIMEOUT_MS);
        let shared = shared.clone();
        let _ = std::thread::Builder::new()
            .name("owlmic-bt-conn".into())
            .spawn(move || {
                let closer = s.clone();
                let close: Closer = Arc::new(move || unsafe {
                    shutdown(closer.0.0, SD_BOTH);
                });
                // Bluetooth has no IP address; a stable stand-in keeps per-peer limits working.
                let bytes = peer.btAddr.to_be_bytes();
                let ip = Ipv6Addr::new(
                    0xfd00,
                    0x6f77,
                    0,
                    0,
                    u16::from_be_bytes([bytes[2], bytes[3]]),
                    u16::from_be_bytes([bytes[4], bytes[5]]),
                    u16::from_be_bytes([bytes[6], bytes[7]]),
                    0,
                );
                server::run_control(
                    s.clone(),
                    Box::new(s),
                    crate::LINK_BLUETOOTH,
                    SocketAddr::new(IpAddr::V6(ip), 0),
                    close,
                    &shared,
                );
                drop(slot);
            });
    }
}
