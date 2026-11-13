//! The PC's identity: one P-256 key pair, a random id and the computer's name, created on first
//! run and kept in the store with the private key protected.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use owlmic_proto::crypto::{KeyPair, key_hint, random_bytes};
use owlmic_settings::store::{IdentityRecord, Store};

pub struct Identity {
    pub keys: KeyPair,
    pub pc_id: [u8; 16],
    pub name: String,
}

impl Identity {
    pub fn load_or_create(store: &Store) -> Self {
        let restored = store.read(|d| {
            let rec = d.identity.as_ref()?;
            let private = store.unprotect(&B64.decode(&rec.private_protected).ok()?)?;
            let keys = KeyPair::from_private(&private.try_into().ok()?)?;
            let pc_id = crate::id_from_hex(d.pc_id.as_deref()?)?;
            Some((keys, pc_id))
        });
        let (keys, pc_id) = restored.unwrap_or_else(|| {
            let keys = KeyPair::generate();
            let pc_id = random_bytes::<16>();
            store.update(|d| {
                d.identity = Some(IdentityRecord {
                    private_protected: B64.encode(store.protect(&keys.private_bytes())),
                    public: B64.encode(keys.public()),
                });
                d.pc_id = Some(crate::hex(&pc_id));
            });
            (keys, pc_id)
        });
        Self {
            keys,
            pc_id,
            name: computer_name(),
        }
    }

    pub fn key_hint(&self) -> [u8; 8] {
        key_hint(&self.keys.public())
    }
}

fn computer_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "PC".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use owlmic_settings::store::PlainProtector;

    #[test]
    fn the_identity_is_created_once_and_restored_after() {
        let dir = std::env::temp_dir().join(format!("owlmic-identity-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("owlmic.json");
        let first = Identity::load_or_create(&Store::open(&path, Box::new(PlainProtector)));
        let again = Identity::load_or_create(&Store::open(&path, Box::new(PlainProtector)));
        assert_eq!(first.pc_id, again.pc_id);
        assert_eq!(first.keys.public(), again.keys.public());
        assert_eq!(first.key_hint(), again.key_hint());
    }
}
