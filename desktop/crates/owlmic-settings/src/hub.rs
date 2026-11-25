//! The Settings Hub (SYSTEM_DESIGN section 18): the current pairing's settings, merged with the
//! phone's changes, saved, and applied live. `general.*` settings belong to the PC itself;
//! everything else belongs to the pairing and needs a connected phone.

use crate::book::{self, Book};
use crate::store::Store;
use owlmic_hub::Hub;
use owlmic_proto::messages::SettingChange;
use std::collections::BTreeMap;
use std::sync::Arc;

pub enum SettingsMsg {
    /// The phone (hex id) whose pairing is live, or none.
    Session(Option<String>),
    FromPhone(Vec<SettingChange>),
    /// A change on the PC panel.
    Set {
        id: String,
        value: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum SettingsEvent {
    /// Values to apply now: all of them when a pairing goes live, then each change.
    Apply(Vec<(String, String)>),
    /// Shared settings changed on the PC, for the phone.
    ToPhone(Vec<SettingChange>),
    View(SettingsView),
}

/// Every setting's current value, for the settings view. `paired` is false with no phone, when
/// only the PC's own settings can change.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SettingsView {
    pub values: BTreeMap<String, String>,
    pub paired: bool,
}

fn is_general(id: &str) -> bool {
    id.starts_with("general.")
}

fn is_link(id: &str) -> bool {
    id.starts_with("link.")
}

pub struct SettingsHub {
    store: Arc<Store>,
    phone: Option<String>,
    emit: Box<dyn Fn(SettingsEvent) + Send>,
}

impl SettingsHub {
    /// Applies the PC's own settings right away (autostart, for one).
    pub fn new(store: Arc<Store>, emit: impl Fn(SettingsEvent) + Send + 'static) -> Self {
        let hub = Self {
            store,
            phone: None,
            emit: Box::new(emit),
        };
        let general = hub.store.read(|d| d.general.clone());
        let values = crate::SETTINGS
            .iter()
            .filter(|d| is_general(d.id) || is_link(d.id))
            .map(|d| (d.id.to_owned(), book::value(&general, d.id).to_owned()));
        (hub.emit)(SettingsEvent::Apply(values.collect()));
        hub.publish();
        hub
    }

    fn pairing(&self) -> Book {
        self.phone
            .as_ref()
            .and_then(|p| self.store.read(|d| d.pairings.get(p).cloned()))
            .unwrap_or_default()
    }

    fn publish(&self) {
        let (general, pairing) = (self.store.read(|d| d.general.clone()), self.pairing());
        let values = crate::SETTINGS
            .iter()
            .map(|d| {
                let from = if is_general(d.id)
                    || (is_link(d.id) && (self.phone.is_none() || !pairing.contains_key(d.id)))
                {
                    &general
                } else {
                    &pairing
                };
                (d.id.to_owned(), book::value(from, d.id).to_owned())
            })
            .collect();
        (self.emit)(SettingsEvent::View(SettingsView {
            values,
            paired: self.phone.is_some(),
        }));
    }
}

impl Hub for SettingsHub {
    type Msg = SettingsMsg;

    fn handle(&mut self, msg: SettingsMsg) {
        match msg {
            SettingsMsg::Session(phone) => {
                self.phone = phone;
                if let Some(ref key) = self.phone {
                    let pushed = self.store.update(|d| {
                        let general = d.general.clone();
                        let pairing = d.pairings.entry(key.clone()).or_default();
                        let mut to_push = Vec::new();
                        for def in crate::SETTINGS.iter().filter(|s| is_link(s.id)) {
                            if let Some(gv) = general.get(def.id)
                                && pairing.get(def.id).is_none_or(|pv| gv.version > pv.version)
                            {
                                let v = pairing
                                    .values()
                                    .map(|x| x.version)
                                    .max()
                                    .unwrap_or(0)
                                    .max(gv.version);
                                pairing.insert(
                                    def.id.to_owned(),
                                    crate::store::Versioned {
                                        value: gv.value.clone(),
                                        version: v,
                                    },
                                );
                                to_push.push(SettingChange {
                                    id: def.id.to_owned(),
                                    value: gv.value.clone(),
                                    version: v,
                                });
                            }
                        }
                        to_push
                    });
                    let pairing = self.pairing();
                    let general = self.store.read(|d| d.general.clone());
                    let values = crate::SETTINGS
                        .iter()
                        .filter(|d| !is_general(d.id))
                        .map(|d| {
                            let from = if is_link(d.id) && !pairing.contains_key(d.id) {
                                &general
                            } else {
                                &pairing
                            };
                            (d.id.to_owned(), book::value(from, d.id).to_owned())
                        });
                    (self.emit)(SettingsEvent::Apply(values.collect()));
                    if !pushed.is_empty() {
                        (self.emit)(SettingsEvent::ToPhone(pushed));
                    }
                }
            }
            SettingsMsg::FromPhone(changes) => {
                let Some(phone) = self.phone.clone() else {
                    return;
                };
                let applied = self.store.update(|d| {
                    let out = book::merge_remote(d.pairings.entry(phone).or_default(), &changes);
                    for c in &out {
                        if is_link(&c.id) {
                            d.general.insert(
                                c.id.clone(),
                                crate::store::Versioned {
                                    value: c.value.clone(),
                                    version: c.version,
                                },
                            );
                        }
                    }
                    out
                });
                if !applied.is_empty() {
                    (self.emit)(SettingsEvent::Apply(
                        applied.into_iter().map(|c| (c.id, c.value)).collect(),
                    ));
                }
            }
            SettingsMsg::Set { id, value } => {
                let general = is_general(&id);
                let link = is_link(&id);
                let phone = self.phone.clone();
                if !general && !link && phone.is_none() {
                    return;
                }
                let change = self.store.update(|d| {
                    if link {
                        book::set_local(&mut d.general, &id, &value);
                    }
                    let target = if general || (link && phone.is_none()) {
                        &mut d.general
                    } else {
                        d.pairings.entry(phone.unwrap_or_default()).or_default()
                    };
                    book::set_local(target, &id, &value)
                });
                let Some(change) = change else { return };
                (self.emit)(SettingsEvent::Apply(vec![(id.clone(), value)]));
                if crate::find(&id).is_some_and(|d| d.scope == crate::Scope::Shared)
                    && self.phone.is_some()
                {
                    (self.emit)(SettingsEvent::ToPhone(vec![change]));
                }
            }
        }
        self.publish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::PlainProtector;
    use std::sync::Mutex;

    fn hub() -> (
        SettingsHub,
        Arc<Mutex<Vec<SettingsEvent>>>,
        std::path::PathBuf,
    ) {
        let dir = std::env::temp_dir().join(format!(
            "owlmic-settings-hub-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Arc::new(Store::open(
            dir.join("owlmic.json"),
            Box::new(PlainProtector),
        ));
        let events = Arc::new(Mutex::new(Vec::new()));
        let e = events.clone();
        (
            SettingsHub::new(store, move |ev| e.lock().unwrap().push(ev)),
            events,
            dir,
        )
    }

    fn applied(events: &Mutex<Vec<SettingsEvent>>) -> Vec<(String, String)> {
        events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|e| {
                if let SettingsEvent::Apply(v) = e {
                    Some(v.clone())
                } else {
                    None
                }
            })
            .flatten()
            .collect()
    }

    #[test]
    fn pc_settings_apply_at_start_and_pairing_settings_need_a_phone() {
        let (mut h, events, dir) = hub();
        assert!(applied(&events).contains(&("general.startWithWindows".into(), "on".into())));
        events.lock().unwrap().clear();
        h.handle(SettingsMsg::Set {
            id: "camera.framing".into(),
            value: "fill".into(),
        });
        assert!(applied(&events).is_empty(), "no pairing yet");
        h.handle(SettingsMsg::Session(Some("01".into())));
        h.handle(SettingsMsg::Set {
            id: "camera.framing".into(),
            value: "fill".into(),
        });
        assert!(applied(&events).contains(&("camera.framing".into(), "fill".into())));
        assert!(
            !events
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e, SettingsEvent::ToPhone(_))),
            "PC-only settings stay on the PC"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn shared_changes_go_to_the_phone_and_newer_phone_changes_win() {
        let (mut h, events, dir) = hub();
        h.handle(SettingsMsg::Session(Some("02".into())));
        h.handle(SettingsMsg::Set {
            id: "camera.lens".into(),
            value: "front".into(),
        });
        assert!(
            events
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e, SettingsEvent::ToPhone(c) if c[0].version == 1))
        );
        events.lock().unwrap().clear();
        h.handle(SettingsMsg::Set {
            id: "camera.mirror".into(),
            value: "on".into(),
        });
        assert!(
            events
                .lock()
                .unwrap()
                .iter()
                .any(|e| matches!(e, SettingsEvent::ToPhone(c) if c[0].id == "camera.mirror" && c[0].value == "on"))
        );
        events.lock().unwrap().clear();
        let change = |value: &str, version| SettingChange {
            id: "camera.lens".into(),
            value: value.into(),
            version,
        };
        h.handle(SettingsMsg::FromPhone(vec![change("back", 1)]));
        assert!(applied(&events).is_empty(), "a tie goes to the PC");
        h.handle(SettingsMsg::FromPhone(vec![change("back", 3)]));
        assert_eq!(
            applied(&events),
            vec![("camera.lens".into(), "back".into())]
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn link_settings_toggle_without_phone() {
        let (mut h, events, dir) = hub();
        events.lock().unwrap().clear();
        h.handle(SettingsMsg::Set {
            id: "link.wifi".into(),
            value: "off".into(),
        });
        assert!(applied(&events).contains(&("link.wifi".into(), "off".into())));

        // Offline PC link change syncs forward to phone once paired and phone updates sync back to general.
        h.handle(SettingsMsg::Session(Some("03".into())));
        assert!(events.lock().unwrap().iter().any(|e| matches!(
            e,
            SettingsEvent::ToPhone(c) if c.iter().any(|x| x.id == "link.wifi" && x.value == "off")
        )));
        events.lock().unwrap().clear();
        h.handle(SettingsMsg::FromPhone(vec![SettingChange {
            id: "link.wifi".into(),
            value: "on".into(),
            version: 10,
        }]));
        assert!(applied(&events).contains(&("link.wifi".into(), "on".into())));
        assert_eq!(
            h.store
                .read(|d| book::value(&d.general, "link.wifi").to_owned()),
            "on"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
