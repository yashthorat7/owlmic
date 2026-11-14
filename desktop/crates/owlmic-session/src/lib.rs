//! The Session Hub: the authority on who is connected (SYSTEM_DESIGN sections 11.4 and 14.5).

pub mod hub;
pub mod identity;
pub mod sessions;

pub use hub::{SessionEvent, SessionHub, SessionMsg};
pub use identity::Identity;
pub use sessions::{Decision, Phase, Sessions};

/// 16 bytes as 32 lowercase hex characters.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 32 hex characters as 16 bytes.
pub fn id_from_hex(s: &str) -> Option<[u8; 16]> {
    if s.len() != 32 {
        return None;
    }
    let mut out = [0; 16];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn ids_round_trip_through_hex() {
        let id = [0xAB; 16];
        assert_eq!(super::id_from_hex(&super::hex(&id)), Some(id));
        assert_eq!(super::id_from_hex("zz"), None);
    }
}
