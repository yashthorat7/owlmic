//! The phone's probe and the PC's answer (protocol/README.md, section 2).

use crate::cut_name;

pub const PROBE_MAGIC: &[u8; 8] = b"OWLMIC?3";
pub const ANSWER_MAGIC: &[u8; 8] = b"OWLMIC!3";

const PROBE_FIXED: usize = 8 + 16 + 1;
const ANSWER_FIXED: usize = 8 + 16 + 8 + 2 + 2 + 1 + 1 + 1 + 1;

const FLAG_BUSY: u8 = 1;
const FLAG_APPROVAL_REQUIRED: u8 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Probe {
    pub phone_id: [u8; 16],
    pub name: String,
}

impl Probe {
    pub fn encode(&self) -> Vec<u8> {
        let name = cut_name(&self.name);
        let mut out = Vec::with_capacity(PROBE_FIXED + name.len());
        out.extend_from_slice(PROBE_MAGIC);
        out.extend_from_slice(&self.phone_id);
        out.push(name.len() as u8);
        out.extend_from_slice(name);
        out
    }

    /// `None` for anything that isn't a whole v3 probe.
    pub fn decode(buf: &[u8]) -> Option<Self> {
        if buf.len() < PROBE_FIXED || &buf[..8] != PROBE_MAGIC {
            return None;
        }
        let name_len = buf[PROBE_FIXED - 1] as usize;
        let name = buf.get(PROBE_FIXED..PROBE_FIXED + name_len)?;
        Some(Self {
            phone_id: buf[8..24].try_into().unwrap(),
            name: String::from_utf8_lossy(name).into_owned(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub pc_id: [u8; 16],
    /// The first 8 bytes of the SHA-256 of the PC's public key.
    pub key_hint: [u8; 8],
    pub tcp_port: u16,
    pub media_port: u16,
    pub proto: u8,
    pub busy: bool,
    pub approval_required: bool,
    /// The link the probe arrived over: 2 USB tethering, 3 Wi-Fi.
    pub link: u8,
    pub name: String,
}

impl Answer {
    pub fn encode(&self) -> Vec<u8> {
        let name = cut_name(&self.name);
        let flags = if self.busy { FLAG_BUSY } else { 0 }
            | if self.approval_required {
                FLAG_APPROVAL_REQUIRED
            } else {
                0
            };
        let mut out = Vec::with_capacity(ANSWER_FIXED + name.len());
        out.extend_from_slice(ANSWER_MAGIC);
        out.extend_from_slice(&self.pc_id);
        out.extend_from_slice(&self.key_hint);
        out.extend_from_slice(&self.tcp_port.to_be_bytes());
        out.extend_from_slice(&self.media_port.to_be_bytes());
        out.extend_from_slice(&[self.proto, flags, self.link, name.len() as u8]);
        out.extend_from_slice(name);
        out
    }

    /// `None` for anything that isn't a whole v3 answer.
    pub fn decode(buf: &[u8]) -> Option<Self> {
        if buf.len() < ANSWER_FIXED || &buf[..8] != ANSWER_MAGIC {
            return None;
        }
        let name_len = buf[ANSWER_FIXED - 1] as usize;
        let name = buf.get(ANSWER_FIXED..ANSWER_FIXED + name_len)?;
        let flags = buf[37];
        Some(Self {
            pc_id: buf[8..24].try_into().unwrap(),
            key_hint: buf[24..32].try_into().unwrap(),
            tcp_port: u16::from_be_bytes([buf[32], buf[33]]),
            media_port: u16::from_be_bytes([buf[34], buf[35]]),
            proto: buf[36],
            busy: flags & FLAG_BUSY != 0,
            approval_required: flags & FLAG_APPROVAL_REQUIRED != 0,
            link: buf[38],
            name: String::from_utf8_lossy(name).into_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectors::{hex, load};

    fn id<const N: usize>(v: &serde_json::Value) -> [u8; N] {
        hex(v).try_into().unwrap()
    }

    #[test]
    fn probes_match_the_vectors() {
        for v in load("discovery.json")["probes"].as_array().unwrap() {
            let probe = Probe {
                phone_id: id(&v["phoneId"]),
                name: v["phoneName"].as_str().unwrap().into(),
            };
            assert_eq!(probe.encode(), hex(&v["hex"]), "{}", v["name"]);
            assert_eq!(Probe::decode(&hex(&v["hex"])), Some(probe), "{}", v["name"]);
        }
    }

    #[test]
    fn answers_match_the_vectors() {
        for v in load("discovery.json")["answers"].as_array().unwrap() {
            let answer = Answer {
                pc_id: id(&v["pcId"]),
                key_hint: id(&v["keyHint"]),
                tcp_port: v["tcpPort"].as_u64().unwrap() as u16,
                media_port: v["mediaPort"].as_u64().unwrap() as u16,
                proto: v["proto"].as_u64().unwrap() as u8,
                busy: v["busy"].as_bool().unwrap(),
                approval_required: v["approvalRequired"].as_bool().unwrap(),
                link: v["link"].as_u64().unwrap() as u8,
                name: v["pcName"].as_str().unwrap().into(),
            };
            assert_eq!(answer.encode(), hex(&v["hex"]), "{}", v["name"]);
            assert_eq!(
                Answer::decode(&hex(&v["hex"])),
                Some(answer),
                "{}",
                v["name"]
            );
        }
    }

    #[test]
    fn ignores_what_is_not_a_whole_v3_packet() {
        for v in load("discovery.json")["ignored"].as_array().unwrap() {
            let bytes = hex(&v["hex"]);
            match v["parser"].as_str().unwrap() {
                "probe" => assert_eq!(Probe::decode(&bytes), None, "{}", v["name"]),
                _ => assert_eq!(Answer::decode(&bytes), None, "{}", v["name"]),
            }
        }
    }

    #[test]
    fn long_names_are_cut_at_a_character_boundary() {
        let probe = Probe {
            phone_id: [0; 16],
            name: "é".repeat(200),
        };
        let decoded = Probe::decode(&probe.encode()).unwrap();
        assert_eq!(decoded.name, "é".repeat(127));
    }
}
