//! Keys, proofs and packet sealing (protocol/README.md, sections 5 and 6).

use aes_gcm::aead::{Aead, AeadInPlace, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce, Tag};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use p256::elliptic_curve::sec1::ToEncodedPoint;
use p256::{PublicKey, SecretKey};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

pub type Key = [u8; 32];

/// A P-256 key pair: a device's identity, or one side's ephemeral key for a session.
pub struct KeyPair {
    secret: SecretKey,
}

impl KeyPair {
    pub fn generate() -> Self {
        Self {
            secret: SecretKey::random(&mut OsRng),
        }
    }

    pub fn from_private(bytes: &[u8; 32]) -> Option<Self> {
        SecretKey::from_slice(bytes)
            .ok()
            .map(|secret| Self { secret })
    }

    pub fn private_bytes(&self) -> [u8; 32] {
        self.secret.to_bytes().into()
    }

    /// The 65-byte uncompressed point.
    pub fn public(&self) -> [u8; 65] {
        self.secret
            .public_key()
            .to_encoded_point(false)
            .as_bytes()
            .try_into()
            .unwrap()
    }

    /// ECDH: the x-coordinate of the shared point, or `None` for a key that isn't on the curve.
    pub fn agree(&self, peer_public: &[u8]) -> Option<Key> {
        let peer = PublicKey::from_sec1_bytes(peer_public).ok()?;
        let shared = p256::ecdh::diffie_hellman(self.secret.to_nonzero_scalar(), peer.as_affine());
        Some((*shared.raw_secret_bytes()).into())
    }
}

pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut out = [0; N];
    OsRng.fill_bytes(&mut out);
    out
}

pub fn key_hint(public: &[u8]) -> [u8; 8] {
    Sha256::digest(public)[..8].try_into().unwrap()
}

pub fn pairing_key(static_shared: &Key, phone_id: &[u8; 16], pc_id: &[u8; 16]) -> Key {
    let mut info = [0; 32];
    info[..16].copy_from_slice(phone_id);
    info[16..].copy_from_slice(pc_id);
    hkdf(b"owlmic pair v3", static_shared, &info)
}

pub fn transcript(hello_payload: &[u8], hello_ack_payload: &[u8]) -> Key {
    let mut h = Sha256::new();
    h.update(hello_payload);
    h.update(hello_ack_payload);
    h.finalize().into()
}

pub struct SessionKeys {
    pub phone_to_pc: Key,
    pub pc_to_phone: Key,
    pub auth: Key,
}

pub fn session_master(
    pairing_key: &Key,
    ephemeral_shared: &Key,
    phone_nonce: &[u8; 32],
    pc_nonce: &[u8; 32],
) -> Key {
    let salt = [phone_nonce.as_slice(), pc_nonce].concat();
    let ikm = [pairing_key.as_slice(), ephemeral_shared].concat();
    hkdf(&salt, &ikm, b"owlmic session v3")
}

pub fn session_keys(master: &Key) -> SessionKeys {
    let prk = Hkdf::<Sha256>::from_prk(master).expect("a 32-byte PRK is long enough");
    let expand = |info: &[u8]| {
        let mut out = [0; 32];
        prk.expand(info, &mut out)
            .expect("32 bytes is a valid HKDF length");
        out
    };
    SessionKeys {
        phone_to_pc: expand(b"phone->pc"),
        pc_to_phone: expand(b"pc->phone"),
        auth: expand(b"auth"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Phone,
    Pc,
}

pub fn proof(auth: &Key, role: Role, transcript: &Key) -> Key {
    let label: &[u8] = match role {
        Role::Phone => b"phone",
        Role::Pc => b"pc",
    };
    hmac(auth, &[label, transcript])
}

/// Constant-time check of a peer's proof.
pub fn proof_matches(auth: &Key, role: Role, transcript: &Key, mac: &[u8]) -> bool {
    let mut m = <Hmac<Sha256> as Mac>::new_from_slice(auth).unwrap();
    m.update(match role {
        Role::Phone => b"phone".as_slice(),
        Role::Pc => b"pc",
    });
    m.update(transcript);
    m.verify_slice(mac).is_ok()
}

pub fn approval_code(pairing_key: &Key, transcript: &Key) -> String {
    let mac = hmac(pairing_key, &[b"code", transcript]);
    format!(
        "{:04}",
        u32::from_be_bytes(mac[..4].try_into().unwrap()) % 10_000
    )
}

pub fn carrier_mac(auth: &Key, session_id: &[u8; 16]) -> [u8; 16] {
    hmac(auth, &[b"carrier", session_id])[..16]
        .try_into()
        .unwrap()
}

fn hkdf(salt: &[u8], ikm: &[u8], info: &[u8]) -> Key {
    let mut out = [0; 32];
    Hkdf::<Sha256>::new(Some(salt), ikm)
        .expand(info, &mut out)
        .expect("32 bytes is a valid HKDF length");
    out
}

fn hmac(key: &[u8], parts: &[&[u8]]) -> Key {
    let mut m = <Hmac<Sha256> as Mac>::new_from_slice(key).unwrap();
    for p in parts {
        m.update(p);
    }
    m.finalize().into_bytes().into()
}

/// Stream byte used in the nonce of encrypted control frames.
pub const CONTROL_STREAM: u8 = 0xFF;

fn nonce(stream: u8, counter: u64) -> [u8; 12] {
    let mut n = [0; 12];
    n[0] = stream;
    n[4..].copy_from_slice(&counter.to_be_bytes());
    n
}

/// AES-256-GCM for one direction of a session.
pub struct Cipher {
    aead: Aes256Gcm,
}

impl Cipher {
    pub fn new(key: &Key) -> Self {
        Self {
            aead: Aes256Gcm::new(key.into()),
        }
    }

    /// `plaintext` sealed with its 16-byte tag appended. `counter` is the packet's seq for media,
    /// or the direction's control counter for control frames.
    pub fn seal(&self, stream: u8, counter: u64, aad: &[u8], plaintext: &[u8]) -> Vec<u8> {
        let nonce = nonce(stream, counter);
        self.aead
            .encrypt(
                &Nonce::from(nonce),
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .expect("AES-GCM sealing can't fail for these sizes")
    }

    pub fn open(&self, stream: u8, counter: u64, aad: &[u8], sealed: &[u8]) -> Option<Vec<u8>> {
        let nonce = nonce(stream, counter);
        self.aead
            .decrypt(&Nonce::from(nonce), Payload { msg: sealed, aad })
            .ok()
    }

    /// Seals `packet[aad_len..]` where it is, with `packet[..aad_len]` as additional data, and
    /// appends the tag: the media path reuses one buffer instead of allocating per packet.
    pub fn seal_in_place(&self, stream: u8, counter: u64, packet: &mut Vec<u8>, aad_len: usize) {
        let (aad, body) = packet.split_at_mut(aad_len);
        let tag = self
            .aead
            .encrypt_in_place_detached(&Nonce::from(nonce(stream, counter)), aad, body)
            .expect("AES-GCM sealing can't fail for these sizes");
        packet.extend_from_slice(&tag);
    }

    /// Opens `packet[aad_len..]` (sealed bytes, then the tag) where it is. Returns the length of
    /// the plaintext, which then starts at `packet[aad_len]`.
    pub fn open_in_place(
        &self,
        stream: u8,
        counter: u64,
        packet: &mut [u8],
        aad_len: usize,
    ) -> Option<usize> {
        let len = packet.len().checked_sub(aad_len + 16)?;
        let (aad, rest) = packet.split_at_mut(aad_len);
        let (body, tag) = rest.split_at_mut(len);
        let tag: [u8; 16] = (&*tag).try_into().ok()?;
        self.aead
            .decrypt_in_place_detached(
                &Nonce::from(nonce(stream, counter)),
                aad,
                body,
                &Tag::from(tag),
            )
            .ok()?;
        Some(len)
    }
}

/// Accepts each `seq` once and nothing older than the last 64.
#[derive(Default)]
pub struct ReplayWindow {
    highest: Option<u32>,
    seen: u64,
}

impl ReplayWindow {
    /// True the first time a `seq` inside the window is offered; it is then remembered.
    pub fn accept(&mut self, seq: u32) -> bool {
        let Some(highest) = self.highest else {
            self.highest = Some(seq);
            self.seen = 1;
            return true;
        };
        let ahead = seq.wrapping_sub(highest);
        if ahead != 0 && ahead < 1 << 31 {
            self.seen = if ahead >= 64 { 0 } else { self.seen << ahead };
            self.seen |= 1;
            self.highest = Some(seq);
            return true;
        }
        let behind = highest.wrapping_sub(seq);
        if behind >= 64 || self.seen & (1 << behind) != 0 {
            return false;
        }
        self.seen |= 1 << behind;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vectors::{hex, load};
    use serde_json::Value;

    fn bytes<const N: usize>(v: &Value) -> [u8; N] {
        hex(v).try_into().unwrap()
    }

    #[test]
    fn every_derived_value_matches_the_independent_vectors() {
        let v = load("crypto.json");
        let e = &v["expected"];
        let phone = KeyPair::from_private(&bytes(&v["phoneStatic"]["private"])).unwrap();
        let pc = KeyPair::from_private(&bytes(&v["pcStatic"]["private"])).unwrap();
        let phone_eph = KeyPair::from_private(&bytes(&v["phoneEphemeral"]["private"])).unwrap();
        let pc_eph = KeyPair::from_private(&bytes(&v["pcEphemeral"]["private"])).unwrap();
        assert_eq!(phone.public().to_vec(), hex(&v["phoneStatic"]["public"]));
        assert_eq!(pc.public().to_vec(), hex(&v["pcStatic"]["public"]));
        assert_eq!(key_hint(&pc.public()).to_vec(), hex(&e["pcKeyHint"]));

        let static_shared = phone.agree(&pc.public()).unwrap();
        assert_eq!(pc.agree(&phone.public()), Some(static_shared));
        assert_eq!(static_shared.to_vec(), hex(&e["staticShared"]));
        let k = pairing_key(&static_shared, &bytes(&v["phoneId"]), &bytes(&v["pcId"]));
        assert_eq!(k.to_vec(), hex(&e["pairingKey"]));

        let t = transcript(
            v["helloPayload"].as_str().unwrap().as_bytes(),
            v["helloAckPayload"].as_str().unwrap().as_bytes(),
        );
        assert_eq!(t.to_vec(), hex(&e["transcript"]));
        let eph = phone_eph.agree(&pc_eph.public()).unwrap();
        assert_eq!(eph.to_vec(), hex(&e["ephemeralShared"]));
        let master = session_master(&k, &eph, &bytes(&v["phoneNonce"]), &bytes(&v["pcNonce"]));
        assert_eq!(master.to_vec(), hex(&e["sessionMaster"]));
        let keys = session_keys(&master);
        assert_eq!(keys.phone_to_pc.to_vec(), hex(&e["phoneToPc"]));
        assert_eq!(keys.pc_to_phone.to_vec(), hex(&e["pcToPhone"]));
        assert_eq!(keys.auth.to_vec(), hex(&e["auth"]));

        assert_eq!(
            proof(&keys.auth, Role::Phone, &t).to_vec(),
            hex(&e["proofPhone"])
        );
        assert_eq!(proof(&keys.auth, Role::Pc, &t).to_vec(), hex(&e["proofPc"]));
        assert!(proof_matches(&keys.auth, Role::Pc, &t, &hex(&e["proofPc"])));
        assert!(!proof_matches(
            &keys.auth,
            Role::Phone,
            &t,
            &hex(&e["proofPc"])
        ));
        assert_eq!(approval_code(&k, &t), e["approvalCode"].as_str().unwrap());
        assert_eq!(
            carrier_mac(&keys.auth, &bytes(&e["sessionId"])).to_vec(),
            hex(&e["carrierMac"])
        );
    }

    #[test]
    fn sealing_matches_the_independent_vectors() {
        let v = load("crypto.json");
        for c in v["aead"].as_array().unwrap() {
            let key: Key = bytes(&v["expected"][c["key"].as_str().unwrap()]);
            let cipher = Cipher::new(&key);
            let (stream, counter) = (
                c["stream"].as_u64().unwrap() as u8,
                c["counter"].as_u64().unwrap(),
            );
            let (aad, plain) = (hex(&c["headerHex"]), hex(&c["plaintextHex"]));
            let sealed = cipher.seal(stream, counter, &aad, &plain);
            assert_eq!(sealed, hex(&c["sealedHex"]), "{}", c["name"]);
            assert_eq!(
                cipher.open(stream, counter, &aad, &sealed).as_ref(),
                Some(&plain),
                "{}",
                c["name"]
            );
            assert_eq!(
                cipher.open(stream, counter + 1, &aad, &sealed),
                None,
                "{}: wrong counter",
                c["name"]
            );
            let mut packet = [aad.as_slice(), &plain].concat();
            cipher.seal_in_place(stream, counter, &mut packet, aad.len());
            assert_eq!(packet[aad.len()..], sealed, "{}: in place", c["name"]);
            let len = cipher.open_in_place(stream, counter, &mut packet, aad.len());
            assert_eq!(len, Some(plain.len()));
            assert_eq!(packet[aad.len()..aad.len() + plain.len()], plain);
        }
    }

    #[test]
    fn generated_keys_agree_with_each_other() {
        let (a, b) = (KeyPair::generate(), KeyPair::generate());
        assert_eq!(a.agree(&b.public()), b.agree(&a.public()));
        let restored = KeyPair::from_private(&a.private_bytes()).unwrap();
        assert_eq!(restored.public(), a.public());
        assert_eq!(a.agree(&[4; 65]), None);
    }

    #[test]
    fn replay_window_accepts_each_seq_once_and_nothing_too_old() {
        let mut w = ReplayWindow::default();
        assert!(w.accept(100));
        assert!(!w.accept(100));
        assert!(w.accept(103));
        assert!(w.accept(101));
        assert!(!w.accept(101));
        assert!(w.accept(102));
        assert!(w.accept(200));
        assert!(!w.accept(136), "64 or more behind");
        assert!(w.accept(137));
        assert!(!w.accept(u32::MAX - 1), "far behind after wrapping is old");
        let mut wrap = ReplayWindow::default();
        assert!(wrap.accept(u32::MAX));
        assert!(wrap.accept(0), "seq wraps forward");
        assert!(!wrap.accept(u32::MAX));
    }
}
