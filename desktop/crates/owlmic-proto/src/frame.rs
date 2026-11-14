//! Control frames, media packets and video fragments (protocol/README.md, sections 3, 4 and 6).

/// The first byte of every TCP connection to the control port.
pub const CHANNEL_CONTROL: u8 = 0x01;
pub const CHANNEL_MEDIA: u8 = 0x02;

pub const MAX_CONTROL_PAYLOAD: u32 = 1 << 20;
/// Starts a media packet on a stream carrier; control types are always below it.
pub const MEDIA_MARKER: u8 = 0x80;
pub const MAX_FRAGMENT_PAYLOAD: usize = 1200;
/// The most fragments one picture may have; more is treated as a bad packet.
pub const MAX_FRAGMENTS: u16 = 4096;

/// Control message types.
pub mod kind {
    pub const HELLO: u8 = 0x01;
    pub const HELLO_ACK: u8 = 0x02;
    pub const PROOF: u8 = 0x03;
    pub const PENDING: u8 = 0x04;
    pub const WELCOME: u8 = 0x05;
    pub const REJECT: u8 = 0x06;
    pub const PING: u8 = 0x10;
    pub const PONG: u8 = 0x11;
    pub const REPORT: u8 = 0x12;
    pub const STATE: u8 = 0x20;
    pub const SETTINGS: u8 = 0x21;
    pub const STREAM_START: u8 = 0x22;
    pub const STREAM_STOP: u8 = 0x23;
    pub const KEYFRAME_REQUEST: u8 = 0x24;
    pub const RESTART_STREAM: u8 = 0x25;
    pub const SWITCH: u8 = 0x30;
    pub const BYE: u8 = 0x3F;
}

/// Media streams.
pub mod stream {
    pub const CARRIER_HELLO: u8 = 0;
    pub const MIC: u8 = 1;
    pub const CAMERA: u8 = 2;
    pub const SPEAKER: u8 = 3;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlHeader {
    pub kind: u8,
    pub len: u32,
}

impl ControlHeader {
    pub const LEN: usize = 4;

    pub fn encode(&self) -> [u8; Self::LEN] {
        let l = self.len.to_be_bytes();
        [self.kind, l[1], l[2], l[3]]
    }

    /// `None` for a media marker or a payload over 1 MiB.
    pub fn decode(b: [u8; Self::LEN]) -> Option<Self> {
        let len = u32::from_be_bytes([0, b[1], b[2], b[3]]);
        (b[0] < MEDIA_MARKER && len <= MAX_CONTROL_PAYLOAD).then_some(Self { kind: b[0], len })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaHeader {
    pub stream: u8,
    pub keyframe: bool,
    pub seq: u32,
    /// Microseconds on the sender's clock, wrapping.
    pub timestamp_us: u32,
}

impl MediaHeader {
    pub const LEN: usize = 10;

    pub fn encode(&self) -> [u8; Self::LEN] {
        let mut out = [0; Self::LEN];
        out[0] = self.stream;
        out[1] = self.keyframe as u8;
        out[2..6].copy_from_slice(&self.seq.to_be_bytes());
        out[6..10].copy_from_slice(&self.timestamp_us.to_be_bytes());
        out
    }

    pub fn decode(buf: &[u8]) -> Option<Self> {
        let b = buf.get(..Self::LEN)?;
        Some(Self {
            stream: b[0],
            keyframe: b[1] & 1 != 0,
            seq: u32::from_be_bytes(b[2..6].try_into().unwrap()),
            timestamp_us: u32::from_be_bytes(b[6..10].try_into().unwrap()),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FragmentHeader {
    pub frame: u16,
    pub index: u16,
    pub count: u16,
}

impl FragmentHeader {
    pub const LEN: usize = 6;

    pub fn encode(&self) -> [u8; Self::LEN] {
        let (f, i, c) = (
            self.frame.to_be_bytes(),
            self.index.to_be_bytes(),
            self.count.to_be_bytes(),
        );
        [f[0], f[1], i[0], i[1], c[0], c[1]]
    }

    /// `None` when the index is outside the count or the count is over [`MAX_FRAGMENTS`].
    pub fn decode(buf: &[u8]) -> Option<Self> {
        let b = buf.get(..Self::LEN)?;
        let h = Self {
            frame: u16::from_be_bytes([b[0], b[1]]),
            index: u16::from_be_bytes([b[2], b[3]]),
            count: u16::from_be_bytes([b[4], b[5]]),
        };
        (h.index < h.count && h.count <= MAX_FRAGMENTS).then_some(h)
    }
}

/// Appends `packet` the way stream carriers send it: the media marker and a 2-byte length.
pub fn wrap_for_stream(packet: &[u8], out: &mut Vec<u8>) {
    let len = u16::try_from(packet.len()).expect("media packets fit in 64 KiB");
    out.push(MEDIA_MARKER);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(packet);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectors::{hex, load};

    #[test]
    fn constants_match_the_vectors() {
        let v = load("frames.json");
        assert_eq!(v["channelBytes"]["control"], CHANNEL_CONTROL as u64);
        assert_eq!(v["channelBytes"]["media"], CHANNEL_MEDIA as u64);
        assert_eq!(v["maxControlPayload"], MAX_CONTROL_PAYLOAD as u64);
        assert_eq!(v["maxFragmentPayload"], MAX_FRAGMENT_PAYLOAD as u64);
        assert_eq!(v["maxFragments"], MAX_FRAGMENTS as u64);
    }

    #[test]
    fn control_headers_match_the_vectors() {
        let v = load("frames.json");
        for c in v["controlHeaders"].as_array().unwrap() {
            let h = ControlHeader {
                kind: c["type"].as_u64().unwrap() as u8,
                len: c["length"].as_u64().unwrap() as u32,
            };
            let bytes: [u8; 4] = hex(&c["hex"]).try_into().unwrap();
            assert_eq!(h.encode(), bytes);
            assert_eq!(ControlHeader::decode(bytes), Some(h));
        }
        for c in v["controlHeadersRejected"].as_array().unwrap() {
            assert_eq!(
                ControlHeader::decode(hex(&c["hex"]).try_into().unwrap()),
                None,
                "{}",
                c["name"]
            );
        }
    }

    #[test]
    fn media_and_fragment_headers_match_the_vectors() {
        let v = load("frames.json");
        for m in v["mediaHeaders"].as_array().unwrap() {
            let h = MediaHeader {
                stream: m["stream"].as_u64().unwrap() as u8,
                keyframe: m["keyframe"].as_bool().unwrap(),
                seq: m["seq"].as_u64().unwrap() as u32,
                timestamp_us: m["timestampUs"].as_u64().unwrap() as u32,
            };
            assert_eq!(h.encode().to_vec(), hex(&m["hex"]));
            assert_eq!(MediaHeader::decode(&hex(&m["hex"])), Some(h));
        }
        for f in v["fragmentHeaders"].as_array().unwrap() {
            let h = FragmentHeader {
                frame: f["frame"].as_u64().unwrap() as u16,
                index: f["index"].as_u64().unwrap() as u16,
                count: f["count"].as_u64().unwrap() as u16,
            };
            assert_eq!(h.encode().to_vec(), hex(&f["hex"]));
            assert_eq!(FragmentHeader::decode(&hex(&f["hex"])), Some(h));
        }
        assert_eq!(FragmentHeader::decode(&[0, 0, 0, 2, 0, 2]), None);
        assert_eq!(
            FragmentHeader::decode(&[0, 0, 0, 0, 0x10, 0x01]),
            None,
            "over MAX_FRAGMENTS"
        );
    }

    #[test]
    fn stream_wrapping_matches_the_vectors() {
        for w in load("frames.json")["streamWrap"].as_array().unwrap() {
            let mut out = Vec::new();
            wrap_for_stream(&hex(&w["packetHex"]), &mut out);
            assert_eq!(out, hex(&w["hex"]));
        }
    }

    #[test]
    fn every_message_in_the_vectors_has_a_known_type() {
        let known = [
            kind::HELLO,
            kind::HELLO_ACK,
            kind::PROOF,
            kind::PENDING,
            kind::WELCOME,
            kind::REJECT,
            kind::PING,
            kind::PONG,
            kind::REPORT,
            kind::STATE,
            kind::SETTINGS,
            kind::STREAM_START,
            kind::STREAM_STOP,
            kind::KEYFRAME_REQUEST,
            kind::RESTART_STREAM,
            kind::SWITCH,
            kind::BYE,
        ];
        let messages = load("messages.json");
        let types: Vec<u8> = messages["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["type"].as_u64().unwrap() as u8)
            .collect();
        assert_eq!(types, known);
    }
}
