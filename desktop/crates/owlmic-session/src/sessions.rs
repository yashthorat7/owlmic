//! The session state machine (SYSTEM_DESIGN section 11.4). One phone per PC: while a phone is
//! approved and connected, waiting for approval, or held after a drop, every other phone is busy.

use owlmic_settings::store::{PhoneRecord, StoreData};
use std::time::{Duration, Instant};

pub const HOLD: Duration = Duration::from_secs(30);
/// A phone stops waiting for approval after this long.
pub const APPROVAL_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Known,
    New,
    Busy {
        owner: String,
    },
    Blocked,
    /// A second link for the running session.
    Resume,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phone {
    pub id: [u8; 16],
    pub name: String,
    pub model: String,
    /// Base64 of the phone's public key.
    pub static_pub: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    Idle,
    /// A new phone waits for Allow or Deny on connection `conn`.
    Pending {
        conn: u64,
        phone: Phone,
        code: String,
        link: u8,
        since: Instant,
    },
    Active {
        session_id: [u8; 16],
        phone: Phone,
        link: u8,
    },
    /// The link dropped; the session waits [`HOLD`] for the phone to come back.
    Held {
        session_id: [u8; 16],
        phone: Phone,
        since: Instant,
    },
}

#[derive(Debug, Default)]
pub struct Sessions {
    phase: Option<Phase>,
}

/// What a timer ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expired {
    Session([u8; 16]),
    Approval { conn: u64 },
}

impl Sessions {
    pub fn phase(&self) -> &Phase {
        self.phase.as_ref().unwrap_or(&Phase::Idle)
    }

    pub fn decide(&self, store: &StoreData, phone: &Phone, resume: Option<[u8; 16]>) -> Decision {
        let record = store.phones.get(&crate::hex(&phone.id));
        if record.is_some_and(|r| r.blocked) {
            return Decision::Blocked;
        }
        match self.phase() {
            Phase::Active {
                session_id,
                phone: p,
                ..
            }
            | Phase::Held {
                session_id,
                phone: p,
                ..
            } => {
                if p.id == phone.id
                    && p.static_pub == phone.static_pub
                    && resume == Some(*session_id)
                {
                    return Decision::Resume;
                }
                if p.id != phone.id {
                    return Decision::Busy {
                        owner: p.name.clone(),
                    };
                }
            }
            Phase::Pending { phone: p, .. } if p.id != phone.id => {
                return Decision::Busy {
                    owner: p.name.clone(),
                };
            }
            _ => {}
        }
        match record {
            Some(r) if r.static_pub == phone.static_pub => Decision::Known,
            _ => Decision::New,
        }
    }

    /// A new phone proved itself and now waits for the user.
    pub fn begin_approval(
        &mut self,
        conn: u64,
        phone: Phone,
        code: String,
        link: u8,
        now: Instant,
    ) {
        self.phase = Some(Phase::Pending {
            conn,
            phone,
            code,
            link,
            since: now,
        });
    }

    /// The user allowed the waiting phone: it is recorded and its session starts.
    pub fn approve(
        &mut self,
        store: &mut StoreData,
        session_id: [u8; 16],
        now_secs: u64,
    ) -> Option<(u64, Phone)> {
        let Some(Phase::Pending {
            conn, phone, link, ..
        }) = self.phase.take()
        else {
            return None;
        };
        remember(store, &phone, now_secs);
        self.phase = Some(Phase::Active {
            session_id,
            phone: phone.clone(),
            link,
        });
        Some((conn, phone))
    }

    /// The user denied the waiting phone. Returns its connection.
    pub fn deny(&mut self) -> Option<u64> {
        match self.phase.take() {
            Some(Phase::Pending { conn, .. }) => Some(conn),
            other => {
                self.phase = other;
                None
            }
        }
    }

    /// A known phone proved itself.
    pub fn activate(
        &mut self,
        store: &mut StoreData,
        phone: Phone,
        session_id: [u8; 16],
        link: u8,
        now_secs: u64,
    ) {
        remember(store, &phone, now_secs);
        self.phase = Some(Phase::Active {
            session_id,
            phone,
            link,
        });
    }

    pub fn link_changed(&mut self, session: [u8; 16], new_link: u8) {
        if let Some(Phase::Active {
            session_id, link, ..
        }) = self.phase.as_mut()
            && *session_id == session
        {
            *link = new_link;
        }
    }

    /// The session's last link dropped.
    pub fn link_lost(&mut self, session: [u8; 16], now: Instant) {
        if let Some(Phase::Active {
            session_id, phone, ..
        }) = self.phase.clone()
            && session_id == session
        {
            self.phase = Some(Phase::Held {
                session_id,
                phone,
                since: now,
            });
        }
    }

    /// The held session got a link again.
    pub fn resumed(&mut self, session: [u8; 16], link: u8) {
        if let Some(Phase::Held {
            session_id, phone, ..
        }) = self.phase.clone()
            && session_id == session
        {
            self.phase = Some(Phase::Active {
                session_id,
                phone,
                link,
            });
        }
    }

    /// The connection carrying a pending approval closed: the phone gave up.
    pub fn connection_closed(&mut self, closed: u64) {
        if matches!(self.phase, Some(Phase::Pending { conn, .. }) if conn == closed) {
            self.phase = None;
        }
    }

    /// The phone said BYE: the session ends at once, without a hold.
    pub fn end(&mut self, session: [u8; 16]) {
        if matches!(&self.phase, Some(Phase::Active { session_id, .. } | Phase::Held { session_id, .. }) if *session_id == session)
        {
            self.phase = None;
        }
    }

    pub fn next_deadline(&self) -> Option<Instant> {
        match self.phase() {
            Phase::Held { since, .. } => Some(*since + HOLD),
            Phase::Pending { since, .. } => Some(*since + APPROVAL_TIMEOUT),
            _ => None,
        }
    }

    pub fn tick(&mut self, now: Instant) -> Option<Expired> {
        let expired = match self.phase() {
            Phase::Held {
                session_id, since, ..
            } if now >= *since + HOLD => Expired::Session(*session_id),
            Phase::Pending { conn, since, .. } if now >= *since + APPROVAL_TIMEOUT => {
                Expired::Approval { conn: *conn }
            }
            _ => return None,
        };
        self.phase = None;
        Some(expired)
    }

    /// The active session's id and phone, for the link and the UI.
    pub fn current(&self) -> Option<([u8; 16], &Phone)> {
        match self.phase() {
            Phase::Active {
                session_id, phone, ..
            }
            | Phase::Held {
                session_id, phone, ..
            } => Some((*session_id, phone)),
            _ => None,
        }
    }
}

fn remember(store: &mut StoreData, phone: &Phone, now_secs: u64) {
    store.phones.insert(
        crate::hex(&phone.id),
        PhoneRecord {
            name: phone.name.clone(),
            model: phone.model.clone(),
            static_pub: phone.static_pub.clone(),
            blocked: false,
            last_seen: now_secs,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phone(n: u8, key: &str) -> Phone {
        Phone {
            id: [n; 16],
            name: format!("Phone {n}"),
            model: "Pixel".into(),
            static_pub: key.into(),
        }
    }

    #[test]
    fn a_new_phone_waits_for_approval_and_is_known_after_allow() {
        let mut store = StoreData::default();
        let mut s = Sessions::default();
        let p = phone(1, "k1");
        assert_eq!(s.decide(&store, &p, None), Decision::New);
        let t = Instant::now();
        s.begin_approval(9, p.clone(), "4821".into(), 3, t);
        assert_eq!(
            s.decide(&store, &phone(2, "k2"), None),
            Decision::Busy {
                owner: "Phone 1".into()
            }
        );
        assert_eq!(s.approve(&mut store, [7; 16], 100), Some((9, p.clone())));
        assert_eq!(s.current().map(|(id, _)| id), Some([7; 16]));
        let s2 = Sessions::default();
        assert_eq!(s2.decide(&store, &p, None), Decision::Known);
    }

    #[test]
    fn a_reinstalled_phone_with_a_new_key_is_new_again() {
        let mut store = StoreData::default();
        let mut s = Sessions::default();
        s.activate(&mut store, phone(1, "old"), [1; 16], 3, 0);
        let fresh = Sessions::default();
        assert_eq!(fresh.decide(&store, &phone(1, "new"), None), Decision::New);
    }

    #[test]
    fn denying_frees_the_pc_and_blocked_phones_are_refused() {
        let mut store = StoreData::default();
        let mut s = Sessions::default();
        s.begin_approval(4, phone(1, "k"), "0001".into(), 3, Instant::now());
        assert_eq!(s.deny(), Some(4));
        assert_eq!(*s.phase(), Phase::Idle);
        s.activate(&mut store, phone(1, "k"), [1; 16], 3, 0);
        store.phones.get_mut(&crate::hex(&[1; 16])).unwrap().blocked = true;
        assert_eq!(s.decide(&store, &phone(1, "k"), None), Decision::Blocked);
    }

    #[test]
    fn a_dropped_session_is_held_for_30_seconds_then_ends() {
        let mut store = StoreData::default();
        let mut s = Sessions::default();
        let p = phone(1, "k");
        s.activate(&mut store, p.clone(), [5; 16], 3, 0);
        let t = Instant::now();
        s.link_lost([5; 16], t);
        assert_eq!(s.decide(&store, &p, Some([5; 16])), Decision::Resume);
        assert_eq!(
            s.decide(&store, &phone(2, "x"), None),
            Decision::Busy {
                owner: "Phone 1".into()
            }
        );
        assert_eq!(s.tick(t + Duration::from_secs(29)), None);
        assert_eq!(s.tick(t + HOLD), Some(Expired::Session([5; 16])));
        assert_eq!(*s.phase(), Phase::Idle);
    }

    #[test]
    fn a_held_session_resumes_on_a_new_link() {
        let mut store = StoreData::default();
        let mut s = Sessions::default();
        s.activate(&mut store, phone(1, "k"), [5; 16], 3, 0);
        s.link_lost([5; 16], Instant::now());
        s.resumed([5; 16], 1);
        assert!(matches!(s.phase(), Phase::Active { link: 1, .. }));
    }

    #[test]
    fn an_unanswered_approval_expires_and_a_closed_one_clears() {
        let mut s = Sessions::default();
        let t = Instant::now();
        s.begin_approval(3, phone(1, "k"), "1111".into(), 3, t);
        assert_eq!(
            s.tick(t + APPROVAL_TIMEOUT),
            Some(Expired::Approval { conn: 3 })
        );
        s.begin_approval(4, phone(1, "k"), "1111".into(), 3, t);
        s.connection_closed(4);
        assert_eq!(*s.phase(), Phase::Idle);
    }
}
