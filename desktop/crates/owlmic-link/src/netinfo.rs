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
    d.contains("remote ndis")
        || d.contains("rndis")
        || d.contains("ncm")
        || d.contains("usb gadget")
        || d.contains("android")
        || d.contains("tethering")
}

#[cfg(windows)]
pub use win::Adapters;

#[cfg(not(windows))]
pub use nix::Adapters;

#[cfg(not(windows))]
mod nix {
    use super::*;

    fn is_linux_tethering(iface: &str) -> bool {
        let d = iface.to_ascii_lowercase();
        d.starts_with("rndis")
            || d.starts_with("usb")
            || d.starts_with("ncm")
            || d.starts_with("ecm")
            || d.contains("tether")
    }

    fn linux_rank(iface: &str, tethering: bool) -> u8 {
        if tethering {
            return 2;
        }
        let d = iface.to_ascii_lowercase();
        if d.starts_with("wl") {
            return 0;
        }
        if d.starts_with("docker")
            || d.starts_with("br-")
            || d.starts_with("virbr")
            || d.starts_with("veth")
            || d.starts_with("tun")
            || d.starts_with("tap")
            || d.starts_with("wg")
        {
            return 3;
        }
        1
    }

    pub struct Adapters;

    impl Adapters {
        pub fn networks() -> Vec<Network> {
            let Ok(output) = std::process::Command::new("ip")
                .args(["-o", "-4", "addr", "show", "up"])
                .output()
            else {
                return Vec::new();
            };
            if !output.status.success() {
                return Vec::new();
            }
            let text = String::from_utf8_lossy(&output.stdout);
            let mut ranked = Vec::new();
            for line in text.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                let Some(iface) = parts.get(1).copied() else {
                    continue;
                };
                let Some(pos) = parts.iter().position(|&p| p == "inet") else {
                    continue;
                };
                let Some(cidr) = parts.get(pos + 1).copied() else {
                    continue;
                };
                let Some((ip_str, prefix_str)) = cidr.split_once('/') else {
                    continue;
                };
                let Ok(address) = ip_str.parse::<Ipv4Addr>() else {
                    continue;
                };
                let Ok(prefix) = prefix_str.parse::<u8>() else {
                    continue;
                };
                if address.is_loopback() || address.is_link_local() {
                    continue;
                }
                let tethering = is_linux_tethering(iface);
                ranked.push((
                    linux_rank(iface, tethering),
                    Network {
                        address,
                        prefix,
                        tethering,
                    },
                ));
            }
            ranked.sort_by_key(|(rank, _)| *rank);
            ranked.into_iter().map(|(_, n)| n).collect()
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

#[cfg(windows)]
mod win {
    use super::*;
    use windows::Win32::NetworkManagement::IpHelper::{
        GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST,
        GetAdaptersAddresses, IP_ADAPTER_ADDRESSES_LH,
    };
    use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};

    fn is_virtual_adapter(description: &str) -> bool {
        let d = description.to_ascii_lowercase();
        d.contains("hyper-v")
            || d.contains("vethernet")
            || d.contains("wsl")
            || d.contains("vmware")
            || d.contains("virtualbox")
            || d.contains("virtual")
            || d.contains("tailscale")
            || d.contains("zerotier")
            || d.contains("wireguard")
            || d.contains("wintun")
            || d.contains("tap-")
            || d.contains("loopback")
    }

    fn adapter_rank(if_type: u32, description: &str, tethering: bool) -> u8 {
        if is_virtual_adapter(description) {
            return 3;
        }
        if tethering {
            return 2;
        }
        // IF_TYPE_IEEE80211 (Wi-Fi) first so host IP matches phone's LAN even with virtual NICs.
        if if_type == 71 { 0 } else { 1 }
    }

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
            let mut ranked = Vec::new();
            let mut p = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
            while !p.is_null() {
                let a = unsafe { &*p };
                if a.OperStatus == IfOperStatusUp {
                    let description = unsafe { a.Description.to_string() }.unwrap_or_default();
                    let tethering = is_tethering_adapter(&description);
                    let rank = adapter_rank(a.IfType, &description, tethering);
                    let mut u = a.FirstUnicastAddress;
                    while !u.is_null() {
                        let ua = unsafe { &*u };
                        let sa = ua.Address.lpSockaddr;
                        if !sa.is_null() && unsafe { (*sa).sa_family } == AF_INET {
                            let sin = unsafe { &*(sa as *const SOCKADDR_IN) };
                            let address =
                                Ipv4Addr::from(u32::from_be(unsafe { sin.sin_addr.S_un.S_addr }));
                            if !address.is_loopback() && !address.is_link_local() {
                                ranked.push((
                                    rank,
                                    Network {
                                        address,
                                        prefix: ua.OnLinkPrefixLength,
                                        tethering,
                                    },
                                ));
                            }
                        }
                        u = ua.Next;
                    }
                }
                p = a.Next;
            }
            ranked.sort_by_key(|(rank, _)| *rank);
            ranked.into_iter().map(|(_, n)| n).collect()
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
