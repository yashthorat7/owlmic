pub mod book;
pub mod hub;
pub mod store;

pub use store::{default_dir, default_path};

/// Which side a setting lives on. Shared settings sync both ways; the others stay on one side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Shared,
    Phone,
    Pc,
}

/// One setting from design/settings.json.
#[derive(Debug)]
pub struct SettingDef {
    pub id: &'static str,
    pub values: &'static [&'static str],
    pub default: &'static str,
    pub scope: Scope,
    pub on_phone: bool,
    pub on_pc: bool,
}

include!(concat!(env!("OUT_DIR"), "/settings.rs"));

pub fn find(id: &str) -> Option<&'static SettingDef> {
    SETTINGS.iter().find(|s| s.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_default_is_one_of_its_values() {
        for s in SETTINGS {
            assert!(
                s.values.contains(&s.default),
                "{} defaults to {}",
                s.id,
                s.default
            );
        }
    }

    #[test]
    fn ids_are_unique_and_every_setting_is_shown_somewhere() {
        let mut ids = HashSet::new();
        for s in SETTINGS {
            assert!(ids.insert(s.id), "{} appears twice", s.id);
            assert!(s.on_phone || s.on_pc, "{} is never shown", s.id);
        }
    }

    #[test]
    fn finds_settings_by_id() {
        let lens = find("camera.lens").unwrap();
        assert_eq!(lens.default, "front");
        assert_eq!(lens.scope, Scope::Shared);
        assert_eq!(find("speaker.quietPc").unwrap().scope, Scope::Pc);
        assert!(find("nothing").is_none());
    }
}
