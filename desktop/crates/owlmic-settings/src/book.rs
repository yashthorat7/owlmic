//! Versioned settings and their sync (SYSTEM_DESIGN section 18.1): every change bumps its
//! setting's version; when both sides changed a setting, the higher version wins, and the PC
//! wins a tie.

use crate::store::Versioned;
use crate::{Scope, find};
use owlmic_proto::messages::SettingChange;
use std::collections::BTreeMap;

pub type Book = BTreeMap<String, Versioned>;

/// The current value, or the design's default.
pub fn value<'a>(book: &'a Book, id: &str) -> &'a str {
    book.get(id)
        .map(|v| v.value.as_str())
        .or_else(|| find(id).map(|d| d.default))
        .unwrap_or("")
}

/// A local change. `None` for an unknown setting or a value the setting doesn't have.
pub fn set_local(book: &mut Book, id: &str, value: &str) -> Option<SettingChange> {
    let def = find(id)?;
    if !def.values.contains(&value) {
        return None;
    }
    let version = book.get(id).map_or(0, |v| v.version) + 1;
    book.insert(
        id.to_owned(),
        Versioned {
            value: value.to_owned(),
            version,
        },
    );
    Some(SettingChange {
        id: id.to_owned(),
        value: value.to_owned(),
        version,
    })
}

/// Applies the other side's changes, as the PC. Returns the ones that took effect. Only shared
/// settings sync; anything else, unknown or invalid is ignored.
pub fn merge_remote(book: &mut Book, changes: &[SettingChange]) -> Vec<SettingChange> {
    let mut applied = Vec::new();
    for c in changes {
        let Some(def) = find(&c.id) else { continue };
        if def.scope != Scope::Shared || !def.values.contains(&c.value.as_str()) {
            continue;
        }
        let local = book.get(&c.id).map_or(0, |v| v.version);
        // A tie goes to the PC: only a strictly newer version replaces ours.
        if c.version > local {
            book.insert(
                c.id.clone(),
                Versioned {
                    value: c.value.clone(),
                    version: c.version,
                },
            );
            applied.push(c.clone());
        }
    }
    applied
}

/// Every shared setting, for the WELCOME and for a phone that reconnects.
pub fn shared_snapshot(book: &Book) -> Vec<SettingChange> {
    crate::SETTINGS
        .iter()
        .filter(|d| d.scope == Scope::Shared)
        .map(|d| {
            let v = book.get(d.id);
            SettingChange {
                id: d.id.to_owned(),
                value: v.map_or(d.default, |v| v.value.as_str()).to_owned(),
                version: v.map_or(0, |v| v.version),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(id: &str, value: &str, version: u64) -> SettingChange {
        SettingChange {
            id: id.into(),
            value: value.into(),
            version,
        }
    }

    #[test]
    fn missing_settings_read_as_their_default() {
        assert_eq!(value(&Book::new(), "camera.lens"), "front");
        assert_eq!(value(&Book::new(), "nothing"), "");
    }

    #[test]
    fn local_changes_bump_the_version_and_reject_bad_values() {
        let mut b = Book::new();
        assert_eq!(
            set_local(&mut b, "camera.lens", "front"),
            Some(change("camera.lens", "front", 1))
        );
        assert_eq!(
            set_local(&mut b, "camera.lens", "back"),
            Some(change("camera.lens", "back", 2))
        );
        assert_eq!(set_local(&mut b, "camera.lens", "sideways"), None);
        assert_eq!(set_local(&mut b, "nothing", "on"), None);
    }

    #[test]
    fn newer_remote_versions_win_and_the_pc_wins_a_tie() {
        let mut b = Book::new();
        set_local(&mut b, "camera.lens", "front");
        assert!(
            merge_remote(&mut b, &[change("camera.lens", "back", 1)]).is_empty(),
            "tie keeps the PC's"
        );
        assert_eq!(value(&b, "camera.lens"), "front");
        let applied = merge_remote(&mut b, &[change("camera.lens", "back", 2)]);
        assert_eq!(applied, vec![change("camera.lens", "back", 2)]);
        assert_eq!(value(&b, "camera.lens"), "back");
    }

    #[test]
    fn only_valid_shared_settings_sync() {
        let mut b = Book::new();
        let applied = merge_remote(
            &mut b,
            &[
                change("camera.framing", "fit", 9),
                change("camera.quality", "8k", 9),
                change("x.y", "on", 9),
            ],
        );
        assert!(applied.is_empty());
    }

    #[test]
    fn the_snapshot_has_every_shared_setting_and_no_pc_ones() {
        let snap = shared_snapshot(&Book::new());
        assert!(
            snap.iter()
                .any(|c| c.id == "mic.noiseReduction" && c.value == "5" && c.version == 0)
        );
        assert!(
            !snap
                .iter()
                .any(|c| c.id == "camera.framing" || c.id == "speaker.quietPc")
        );
    }
}
