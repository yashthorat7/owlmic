//! Owlmic wire formats, protocol version 3 (protocol/README.md). No I/O: just bytes in and out.

pub mod crypto;
pub mod discovery;
pub mod frame;
pub mod messages;

pub const VERSION: u8 = 3;
pub const PORT_CONTROL: u16 = 7653;
pub const PORT_DISCOVERY: u16 = 7654;
pub const PORT_MEDIA: u16 = 7655;

/// Names longer than this are cut, at a character boundary.
pub const MAX_NAME_BYTES: usize = 255;

pub(crate) fn cut_name(name: &str) -> &[u8] {
    let mut end = name.len().min(MAX_NAME_BYTES);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    &name.as_bytes()[..end]
}

#[cfg(test)]
pub(crate) mod vectors {
    use serde_json::Value;

    pub fn load(name: &str) -> Value {
        let path = format!(
            "{}/../../../shared/protocol/vectors/{name}",
            env!("CARGO_MANIFEST_DIR")
        );
        serde_json::from_str(&std::fs::read_to_string(&path).expect(&path)).unwrap()
    }

    pub fn hex(v: &Value) -> Vec<u8> {
        let s = v.as_str().unwrap();
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }
}
