//! Control message payloads (protocol/README.md, section 4): JSON objects with camelCase keys.

use crate::frame::kind;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hello {
    pub proto: u8,
    /// 32 hex characters.
    pub phone_id: String,
    pub name: String,
    pub model: String,
    /// Base64 of a 65-byte uncompressed P-256 point.
    pub static_pub: String,
    pub eph_pub: String,
    /// Base64 of 32 random bytes.
    pub nonce: String,
    pub link: u8,
    /// The current session's id (hex), when this connection is a second link for a running session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resume: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AckStatus {
    Known,
    New,
    Busy,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HelloAck {
    pub proto: u8,
    pub pc_id: String,
    pub name: String,
    pub static_pub: String,
    pub eph_pub: String,
    pub nonce: String,
    pub status: AckStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bt_addr: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Proof {
    /// Base64 HMAC.
    pub mac: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pending {
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Welcome {
    pub session_id: String,
    pub mac: String,
    #[serde(default)]
    pub settings: BTreeMap<String, String>,
    #[serde(default)]
    pub caps: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RejectReason {
    Busy,
    Denied,
    Blocked,
    Version,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reject {
    pub reason: RejectReason,
    /// The phone that has the PC, when `reason` is busy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// The PC's own protocol version, when `reason` is version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proto: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ping {
    /// The sender's clock in microseconds; a PONG echoes it.
    pub t: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub loss_pct: f64,
    pub jitter_ms: u32,
    pub rtt_ms: u32,
    /// Received rate per stream, keyed by stream number.
    #[serde(default)]
    pub kbps: BTreeMap<String, u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thermal: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FeatureState {
    #[default]
    Off,
    On,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct State {
    pub mic: FeatureState,
    pub camera: FeatureState,
    pub speaker: FeatureState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingChange {
    pub id: String,
    pub value: String,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub changes: Vec<SettingChange>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamStart {
    pub stream: u8,
    pub codec: String,
    /// Codec parameters, such as sample rate or bitrate.
    #[serde(flatten)]
    pub params: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamRef {
    pub stream: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Switch {
    pub link: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Bye {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
struct Empty {}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Hello(Hello),
    HelloAck(HelloAck),
    Proof(Proof),
    Pending(Pending),
    Welcome(Welcome),
    Reject(Reject),
    Ping(Ping),
    Pong(Ping),
    Report(Report),
    State(State),
    Settings(Settings),
    StreamStart(StreamStart),
    StreamStop(StreamRef),
    KeyframeRequest,
    RestartStream(StreamRef),
    Switch(Switch),
    Bye(Bye),
}

impl Message {
    pub fn kind(&self) -> u8 {
        match self {
            Message::Hello(_) => kind::HELLO,
            Message::HelloAck(_) => kind::HELLO_ACK,
            Message::Proof(_) => kind::PROOF,
            Message::Pending(_) => kind::PENDING,
            Message::Welcome(_) => kind::WELCOME,
            Message::Reject(_) => kind::REJECT,
            Message::Ping(_) => kind::PING,
            Message::Pong(_) => kind::PONG,
            Message::Report(_) => kind::REPORT,
            Message::State(_) => kind::STATE,
            Message::Settings(_) => kind::SETTINGS,
            Message::StreamStart(_) => kind::STREAM_START,
            Message::StreamStop(_) => kind::STREAM_STOP,
            Message::KeyframeRequest => kind::KEYFRAME_REQUEST,
            Message::RestartStream(_) => kind::RESTART_STREAM,
            Message::Switch(_) => kind::SWITCH,
            Message::Bye(_) => kind::BYE,
        }
    }

    pub fn to_payload(&self) -> Vec<u8> {
        let json = match self {
            Message::Hello(m) => serde_json::to_vec(m),
            Message::HelloAck(m) => serde_json::to_vec(m),
            Message::Proof(m) => serde_json::to_vec(m),
            Message::Pending(m) => serde_json::to_vec(m),
            Message::Welcome(m) => serde_json::to_vec(m),
            Message::Reject(m) => serde_json::to_vec(m),
            Message::Ping(m) | Message::Pong(m) => serde_json::to_vec(m),
            Message::Report(m) => serde_json::to_vec(m),
            Message::State(m) => serde_json::to_vec(m),
            Message::Settings(m) => serde_json::to_vec(m),
            Message::StreamStart(m) => serde_json::to_vec(m),
            Message::StreamStop(m) | Message::RestartStream(m) => serde_json::to_vec(m),
            Message::KeyframeRequest => serde_json::to_vec(&Empty {}),
            Message::Switch(m) => serde_json::to_vec(m),
            Message::Bye(m) => serde_json::to_vec(m),
        };
        json.expect("control messages always serialize")
    }

    /// `Ok(None)` for a type this version doesn't know, so newer peers can add messages.
    pub fn from_payload(kind: u8, payload: &[u8]) -> Result<Option<Self>, serde_json::Error> {
        let p = payload;
        Ok(Some(match kind {
            kind::HELLO => Message::Hello(serde_json::from_slice(p)?),
            kind::HELLO_ACK => Message::HelloAck(serde_json::from_slice(p)?),
            kind::PROOF => Message::Proof(serde_json::from_slice(p)?),
            kind::PENDING => Message::Pending(serde_json::from_slice(p)?),
            kind::WELCOME => Message::Welcome(serde_json::from_slice(p)?),
            kind::REJECT => Message::Reject(serde_json::from_slice(p)?),
            kind::PING => Message::Ping(serde_json::from_slice(p)?),
            kind::PONG => Message::Pong(serde_json::from_slice(p)?),
            kind::REPORT => Message::Report(serde_json::from_slice(p)?),
            kind::STATE => Message::State(serde_json::from_slice(p)?),
            kind::SETTINGS => Message::Settings(serde_json::from_slice(p)?),
            kind::STREAM_START => Message::StreamStart(serde_json::from_slice(p)?),
            kind::STREAM_STOP => Message::StreamStop(serde_json::from_slice(p)?),
            kind::KEYFRAME_REQUEST => {
                serde_json::from_slice::<Empty>(p)?;
                Message::KeyframeRequest
            }
            kind::RESTART_STREAM => Message::RestartStream(serde_json::from_slice(p)?),
            kind::SWITCH => Message::Switch(serde_json::from_slice(p)?),
            kind::BYE => Message::Bye(serde_json::from_slice(p)?),
            _ => return Ok(None),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectors::load;

    #[test]
    fn every_vector_message_decodes_and_encodes_back_to_the_same_json() {
        for m in load("messages.json")["messages"].as_array().unwrap() {
            let kind = m["type"].as_u64().unwrap() as u8;
            let payload = serde_json::to_vec(&m["payload"]).unwrap();
            let message = Message::from_payload(kind, &payload)
                .unwrap()
                .unwrap_or_else(|| panic!("{}", m["name"]));
            assert_eq!(message.kind(), kind, "{}", m["name"]);
            let back: serde_json::Value = serde_json::from_slice(&message.to_payload()).unwrap();
            assert_eq!(back, m["payload"], "{}", m["name"]);
        }
    }

    #[test]
    fn a_version_reject_says_the_pcs_version() {
        let m = Message::from_payload(kind::REJECT, br#"{"reason":"version","proto":4}"#)
            .unwrap()
            .unwrap();
        let expected = Reject {
            reason: RejectReason::Version,
            owner: None,
            proto: Some(4),
        };
        assert_eq!(m, Message::Reject(expected));
    }

    #[test]
    fn unknown_types_and_unknown_keys_are_ignored() {
        assert_eq!(Message::from_payload(0x7E, b"{}").unwrap(), None);
        let ping = Message::from_payload(kind::PING, br#"{"t":5,"future":true}"#).unwrap();
        assert_eq!(ping, Some(Message::Ping(Ping { t: 5 })));
    }
}
