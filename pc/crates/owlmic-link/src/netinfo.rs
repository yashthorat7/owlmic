//! Which link a peer arrived over, and where to send announcements. On Windows this reads the
//! network adapters: an address on a USB tethering adapter (RNDIS or NCM) means USB tethering.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

pub trait NetInfo: Send + Sync {
    /// USB tethering or Wi-Fi, for a peer on the local network.
    fn link_for_peer(&self, peer: IpAddr) -> u8;
    /// Where announcements go: one broadcast address per network.
    fn broadcasts(&self, port: u16) -> Vec<SocketAddr>;
}

/// Without adapter information: every peer is Wi-Fi and announcements use the limited broadcast.
pub struct Simple;

impl NetInfo for Simple {
    fn link_for_peer(&self, _peer: IpAddr) -> u8 {
        crate::LINK_WIFI
    }
    fn broadcasts(&self, port: u16) -> Vec<SocketAddr> {
        vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), port)]
    }
}

/// An IPv4 network one adapter is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Network {
    pub address: Ipv4Addr,
    pub prefix: u8,
    pub tethering: bool,
}

impl Network {
    pub fn contains(&self, ip: Ipv4Addr) -> bool {
        let mask = if self.prefix == 0 {
            0
        } else {
            u32::MAX << (32 - self.prefix.min(32))
        };
        u32::from(ip) & mask == u32::from(self.address) & mask
    }

    pub fn broadcast(&self) -> Ipv4Addr {
        let mask = if self.prefix == 0 {
            0
        } else {
            u32::MAX << (32 - self.prefix.min(32))
        };
        Ipv4Addr::from(u32::from(self.address) | !mask)
    }
}

/// The link for `peer` given the PC's networks.
pub fn classify(networks: &[Network], peer: IpAddr) -> u8 {
    let IpAddr::V4(ip) = peer else {
        return crate::LINK_WIFI;
    };
    match networks.iter().find(|n| n.contains(ip)) {
        Some(n) if n.tethering => crate::LINK_USB_TETHERING,
        _ => crate::LINK_WIFI,
    }
}

/// Adapter descriptions that mean a phone's USB tethering.
pub fn is_tethering_adapter(description: &str) -> bool {
    let d = description.to_ascii_lowercase();
    d.contains("remote ndis") || d.contains("rndis") || d.contains("ncm")
}

#[cfg(windows)]
pub use win::Adapters;

#[cfg(not(windows))]
pub use nix::Adapters;

#[cfg(not(windows))]
mod nix {
    use super::*;

    pub struct Adapters;

    impl Adapters {
        pub fn networks() -> Vec<Network> {
            Vec::new()
        }
    }

    impl NetInfo for Adapters {
        fn link_for_peer(&self, peer: IpAddr) -> u8 {
            classify(&Self::networks(), peer)
        }
        fn broadcasts(&self, port: u16) -> Vec<SocketAddr> {
            vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), port)]
        }
    }
}

#[cfg(windows)]
mod win {
    use super::*;
    use windows::Win32::NetworkManagement::IpHelper::{
        GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST,
        GetAdaptersAddresses, IP_ADAPTER_ADDRESSES_LH,
    };
    use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};

    /// Reads the adapters on every question; they change as cables and networks come and go.
    pub struct Adapters;

    impl Adapters {
        pub fn networks() -> Vec<Network> {
            let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
            let mut size = 16 * 1024u32;
            let mut buf: Vec<u64> = Vec::new();
            for _ in 0..3 {
                buf = vec![0; (size as usize).div_ceil(8)];
                let r = unsafe {
                    GetAdaptersAddresses(
                        AF_INET.0 as u32,
                        flags,
                        None,
                        Some(buf.as_mut_ptr().cast()),
                        &mut size,
                    )
                };
                if r == 0 {
                    break;
                }
                if r != 111 {
                    return Vec::new();
                }
            }
            let mut out = Vec::new();
            let mut p = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
            while !p.is_null() {
                let a = unsafe { &*p };
                if a.OperStatus == IfOperStatusUp {
                    let description = unsafe { a.Description.to_string() }.unwrap_or_default();
                    let tethering = is_tethering_adapter(&description);
                    let mut u = a.FirstUnicastAddress;
                    while !u.is_null() {
                        let ua = unsafe { &*u };
                        let sa = ua.Address.lpSockaddr;
                        if !sa.is_null() && unsafe { (*sa).sa_family } == AF_INET {
                            let sin = unsafe { &*(sa as *const SOCKADDR_IN) };
                            let address =
                                Ipv4Addr::from(u32::from_be(unsafe { sin.sin_addr.S_un.S_addr }));
                            if !address.is_loopback() {
                                out.push(Network {
                                    address,
                                    prefix: ua.OnLinkPrefixLength,
                                    tethering,
                                });
                            }
                        }
                        u = ua.Next;
                    }
                }
                p = a.Next;
            }
            out
        }
    }

    impl NetInfo for Adapters {
        fn link_for_peer(&self, peer: IpAddr) -> u8 {
            classify(&Self::networks(), peer)
        }
        fn broadcasts(&self, port: u16) -> Vec<SocketAddr> {
            let mut out: Vec<SocketAddr> = Self::networks()
                .iter()
                .map(|n| SocketAddr::new(IpAddr::V4(n.broadcast()), port))
                .collect();
            out.push(SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), port));
            out.dedup();
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peers_on_a_tethering_network_are_usb_tethering() {
        let nets = [
            Network {
                address: "192.168.42.10".parse().unwrap(),
                prefix: 24,
                tethering: true,
            },
            Network {
                address: "192.168.1.20".parse().unwrap(),
                prefix: 24,
                tethering: false,
            },
        ];
        assert_eq!(
            classify(&nets, "192.168.42.129".parse().unwrap()),
            crate::LINK_USB_TETHERING
        );
        assert_eq!(
            classify(&nets, "192.168.1.7".parse().unwrap()),
            crate::LINK_WIFI
        );
        assert_eq!(
            classify(&nets, "10.0.0.2".parse().unwrap()),
            crate::LINK_WIFI
        );
        assert_eq!(
            nets[0].broadcast(),
            "192.168.42.255".parse::<Ipv4Addr>().unwrap()
        );
        assert!(is_tethering_adapter(
            "Remote NDIS based Internet Sharing Device"
        ));
        assert!(!is_tethering_adapter("Intel(R) Wi-Fi 6 AX201"));
    }
}
