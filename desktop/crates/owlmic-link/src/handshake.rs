//! The PC's half of the handshake (protocol/README.md, sections 4 and 5). Pure: bytes in,
//! messages out, so it is tested against a simulated phone.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use owlmic_proto::crypto::{self, Key, KeyPair, Role, SessionKeys};
use owlmic_proto::messages::{AckStatus, Hello, HelloAck, Message, Welcome};
use owlmic_session::{Decision, sessions::Phone};
use owlmic_settings::store::Versioned;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandshakeError {
    /// Not protocol version 3: answer REJECT version.
    Version,
    Malformed,
    /// The PROOF didn't match: close without a word.
    BadProof,
}

/// What the PC's identity contributes.
pub struct Ours<'a> {
    pub keys: &'a KeyPair,
    pub pc_id: [u8; 16],
    pub name: &'a str,
    pub bt_addr: Option<String>,
}

pub struct ServerHandshake {
    hello_payload: Vec<u8>,
    phone_id: [u8; 16],
    phone_static: Vec<u8>,
    phone_eph: Vec<u8>,
    phone_nonce: [u8; 32],
    keys: Option<SessionKeys>,
    pairing: Key,
    transcript: Key,
}

impl ServerHandshake {
    /// Reads the phone's HELLO. Returns the handshake and who the phone is.
    pub fn hello(payload: &[u8]) -> Result<(Self, Hello, Phone), HandshakeError> {
        let Ok(Some(Message::Hello(h))) =
            Message::from_payload(owlmic_proto::frame::kind::HELLO, payload)
        else {
            return Err(HandshakeError::Malformed);
        };
        if h.proto != owlmic_proto::VERSION {
            return Err(HandshakeError::Version);
        }
        let phone_id = owlmic_session::id_from_hex(&h.phone_id).ok_or(HandshakeError::Malformed)?;
        let decode = |s: &str| B64.decode(s).map_err(|_| HandshakeError::Malformed);
        let phone_static = decode(&h.static_pub)?;
        let phone_eph = decode(&h.eph_pub)?;
        let phone_nonce: [u8; 32] = decode(&h.nonce)?
            .try_into()
            .map_err(|_| HandshakeError::Malformed)?;
        if phone_static.len() != 65 || phone_eph.len() != 65 {
            return Err(HandshakeError::Malformed);
        }
        let phone = Phone {
            id: phone_id,
            name: h.name.clone(),
            model: h.model.clone(),
            static_pub: h.static_pub.clone(),
        };
        let hs = Self {
            hello_payload: payload.to_vec(),
            phone_id,
            phone_static,
            phone_eph,
            phone_nonce,
            keys: None,
            pairing: [0; 32],
            transcript: [0; 32],
        };
        Ok((hs, h, phone))
    }

    /// The HELLO_ACK for `decision`, as the exact bytes to send. For a phone that may continue,
    /// this also derives the session keys.
    pub fn ack(&mut self, ours: &Ours, decision: &Decision) -> Result<Vec<u8>, HandshakeError> {
        let status = match decision {
            Decision::Known | Decision::Resume => AckStatus::Known,
            Decision::New => AckStatus::New,
            Decision::Busy { .. } => AckStatus::Busy,
            Decision::Blocked => AckStatus::Blocked,
        };
        let eph = KeyPair::generate();
        let pc_nonce = crypto::random_bytes::<32>();
        let ack = Message::HelloAck(HelloAck {
            proto: owlmic_proto::VERSION,
            pc_id: owlmic_session::hex(&ours.pc_id),
            name: ours.name.to_owned(),
            static_pub: B64.encode(ours.keys.public()),
            eph_pub: B64.encode(eph.public()),
            nonce: B64.encode(pc_nonce),
            status,
            bt_addr: ours.bt_addr.clone(),
        });
        let payload = ack.to_payload();
        if matches!(status, AckStatus::Known | AckStatus::New) {
            let static_shared = ours
                .keys
                .agree(&self.phone_static)
                .ok_or(HandshakeError::Malformed)?;
            let eph_shared = eph
                .agree(&self.phone_eph)
                .ok_or(HandshakeError::Malformed)?;
            self.pairing = crypto::pairing_key(&static_shared, &self.phone_id, &ours.pc_id);
            self.transcript = crypto::transcript(&self.hello_payload, &payload);
            let master =
                crypto::session_master(&self.pairing, &eph_shared, &self.phone_nonce, &pc_nonce);
            self.keys = Some(crypto::session_keys(&master));
        }
        Ok(payload)
    }

    /// Checks the phone's PROOF.
    pub fn proof(&self, payload: &[u8]) -> Result<(), HandshakeError> {
        let Ok(Some(Message::Proof(p))) =
            Message::from_payload(owlmic_proto::frame::kind::PROOF, payload)
        else {
            return Err(HandshakeError::Malformed);
        };
        let keys = self.keys.as_ref().ok_or(HandshakeError::BadProof)?;
        let mac = B64.decode(&p.mac).map_err(|_| HandshakeError::Malformed)?;
        if crypto::proof_matches(&keys.auth, Role::Phone, &self.transcript, &mac) {
            Ok(())
        } else {
            Err(HandshakeError::BadProof)
        }
    }

    /// The 4-digit code both screens show for a new phone.
    pub fn code(&self) -> String {
        crypto::approval_code(&self.pairing, &self.transcript)
    }

    pub fn welcome(&self, session_id: [u8; 16], settings: &BTreeMap<String, Versioned>) -> Message {
        let keys = self
            .keys
            .as_ref()
            .expect("welcome follows a proven handshake");
        Message::Welcome(Welcome {
            session_id: owlmic_session::hex(&session_id),
            mac: B64.encode(crypto::proof(&keys.auth, Role::Pc, &self.transcript)),
            settings: settings
                .iter()
                .map(|(k, v)| (k.clone(), v.value.clone()))
                .collect(),
            caps: vec!["mic".into(), "camera".into(), "speaker".into()],
        })
    }

    /// The session keys, once the ack is out.
    pub fn into_keys(self) -> Option<SessionKeys> {
        self.keys
    }
}

#[cfg(test)]
pub(crate) mod phone {
    //! A simulated phone, built only from the protocol crate, for testing the PC side.

    use super::*;
    use owlmic_proto::messages::Proof;

    pub struct SimPhone {
        pub id: [u8; 16],
        pub keys: KeyPair,
        eph: KeyPair,
        nonce: [u8; 32],
        hello: Vec<u8>,
        pub session: Option<SessionKeys>,
        pairing: Key,
        transcript: Key,
    }

    impl SimPhone {
        pub fn new(id: u8) -> Self {
            Self {
                id: [id; 16],
                keys: KeyPair::generate(),
                eph: KeyPair::generate(),
                nonce: crypto::random_bytes(),
                hello: Vec::new(),
                session: None,
                pairing: [0; 32],
                transcript: [0; 32],
            }
        }

        pub fn hello(&mut self, link: u8, resume: Option<[u8; 16]>) -> Vec<u8> {
            self.eph = KeyPair::generate();
            self.nonce = crypto::random_bytes();
            self.hello = Message::Hello(Hello {
                proto: owlmic_proto::VERSION,
                phone_id: owlmic_session::hex(&self.id),
                name: "Pixel 8".into(),
                model: "Pixel 8".into(),
                static_pub: B64.encode(self.keys.public()),
                eph_pub: B64.encode(self.eph.public()),
                nonce: B64.encode(self.nonce),
                link,
                resume: resume.map(|r| owlmic_session::hex(&r)),
            })
            .to_payload();
            self.hello.clone()
        }

        /// Reads HELLO_ACK and returns the PROOF payload.
        pub fn proof(&mut self, ack_payload: &[u8]) -> Vec<u8> {
            let Some(Message::HelloAck(ack)) =
                Message::from_payload(owlmic_proto::frame::kind::HELLO_ACK, ack_payload).unwrap()
            else {
                panic!("not an ack")
            };
            let pc_static = B64.decode(&ack.static_pub).unwrap();
            let pc_eph = B64.decode(&ack.eph_pub).unwrap();
            let pc_nonce: [u8; 32] = B64.decode(&ack.nonce).unwrap().try_into().unwrap();
            let pc_id = owlmic_session::id_from_hex(&ack.pc_id).unwrap();
            self.pairing =
                crypto::pairing_key(&self.keys.agree(&pc_static).unwrap(), &self.id, &pc_id);
            self.transcript = crypto::transcript(&self.hello, ack_payload);
            let master = crypto::session_master(
                &self.pairing,
                &self.eph.agree(&pc_eph).unwrap(),
                &self.nonce,
                &pc_nonce,
            );
            let keys = crypto::session_keys(&master);
            let mac = crypto::proof(&keys.auth, Role::Phone, &self.transcript);
            self.session = Some(keys);
            Message::Proof(Proof {
                mac: B64.encode(mac),
            })
            .to_payload()
        }

        pub fn code(&self) -> String {
            crypto::approval_code(&self.pairing, &self.transcript)
        }

        pub fn welcome_is_genuine(&self, w: &Welcome) -> bool {
            let keys = self.session.as_ref().unwrap();
            crypto::proof_matches(
                &keys.auth,
                Role::Pc,
                &self.transcript,
                &B64.decode(&w.mac).unwrap(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::phone::SimPhone;
    use super::*;

    fn pc() -> (KeyPair, [u8; 16]) {
        (KeyPair::generate(), [9; 16])
    }

    #[test]
    fn a_full_handshake_agrees_on_keys_and_codes() {
        let (keys, pc_id) = pc();
        let ours = Ours {
            keys: &keys,
            pc_id,
            name: "DESKTOP-A",
            bt_addr: None,
        };
        let mut phone = SimPhone::new(1);
        let (mut hs, hello, who) = ServerHandshake::hello(&phone.hello(3, None)).unwrap();
        assert_eq!(hello.link, 3);
        assert_eq!(who.id, [1; 16]);
        let ack = hs.ack(&ours, &Decision::New).unwrap();
        let proof = phone.proof(&ack);
        hs.proof(&proof).unwrap();
        assert_eq!(hs.code(), phone.code());
        let Message::Welcome(w) = hs.welcome([3; 16], &BTreeMap::new()) else {
            panic!()
        };
        assert!(phone.welcome_is_genuine(&w));
        let pc_keys = hs.into_keys().unwrap();
        let phone_keys = phone.session.unwrap();
        assert_eq!(pc_keys.phone_to_pc, phone_keys.phone_to_pc);
        assert_eq!(pc_keys.pc_to_phone, phone_keys.pc_to_phone);
    }

    #[test]
    fn a_forged_proof_and_a_wrong_version_are_refused() {
        let (keys, pc_id) = pc();
        let ours = Ours {
            keys: &keys,
            pc_id,
            name: "PC",
            bt_addr: None,
        };
        let mut phone = SimPhone::new(1);
        let (mut hs, _, _) = ServerHandshake::hello(&phone.hello(3, None)).unwrap();
        let ack = hs.ack(&ours, &Decision::Known).unwrap();
        let mut other = SimPhone::new(1);
        other.hello(3, None);
        let forged = other.proof(&ack);
        assert_eq!(hs.proof(&forged), Err(HandshakeError::BadProof));
        let _ = phone.proof(&ack);

        let mut h: serde_json::Value = serde_json::from_slice(&phone.hello(3, None)).unwrap();
        h["proto"] = 2.into();
        assert_eq!(
            ServerHandshake::hello(&serde_json::to_vec(&h).unwrap()).err(),
            Some(HandshakeError::Version)
        );
    }

    #[test]
    fn a_busy_pc_derives_no_keys() {
        let (keys, pc_id) = pc();
        let ours = Ours {
            keys: &keys,
            pc_id,
            name: "PC",
            bt_addr: None,
        };
        let mut phone = SimPhone::new(2);
        let (mut hs, _, _) = ServerHandshake::hello(&phone.hello(3, None)).unwrap();
        hs.ack(
            &ours,
            &Decision::Busy {
                owner: "Pixel 8".into(),
            },
        )
        .unwrap();
        assert!(hs.into_keys().is_none());
    }
}
