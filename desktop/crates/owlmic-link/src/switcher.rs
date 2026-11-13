//! Which link carries the session (SYSTEM_DESIGN section 14.7). The PC decides: a better link
//! must be healthy for 2 seconds before media moves to it; a link that failed sits out 10
//! seconds; when the active link dies, media moves at once to the best healthy one.

use std::collections::HashMap;
use std::time::{Duration, Instant};

pub const STABLE: Duration = Duration::from_secs(2);
pub const PROBATION: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Candidate {
    link: u8,
    healthy_since: Instant,
}

#[derive(Debug, Default)]
pub struct Switcher {
    active: Option<u64>,
    candidates: HashMap<u64, Candidate>,
    failed: HashMap<u8, Instant>,
}

impl Switcher {
    /// A connection finished its handshake for the session.
    pub fn add(&mut self, conn: u64, link: u8, now: Instant) {
        self.candidates.insert(
            conn,
            Candidate {
                link,
                healthy_since: now,
            },
        );
        if self.active.is_none() {
            self.active = Some(conn);
        }
    }

    /// A connection missed heartbeats or closed. Returns the connection media should move to
    /// now, if it was the active one.
    pub fn remove(&mut self, conn: u64, now: Instant) -> Option<u64> {
        let gone = self.candidates.remove(&conn)?;
        self.failed.insert(gone.link, now);
        if self.active != Some(conn) {
            return None;
        }
        // Any live link beats none: probation only holds back upgrades.
        self.active = self.best(now, false, false);
        self.active
    }

    /// An upgrade that is due: a better link, healthy for [`STABLE`] and out of probation.
    pub fn upgrade(&mut self, now: Instant) -> Option<u64> {
        let best = self.best(now, true, true)?;
        let active_link = self
            .active
            .and_then(|a| self.candidates.get(&a))
            .map_or(u8::MAX, |c| c.link);
        if self.candidates[&best].link < active_link {
            self.active = Some(best);
            return Some(best);
        }
        None
    }

    pub fn active(&self) -> Option<u64> {
        self.active
    }

    /// The best other connection, kept warm as a standby.
    pub fn standby(&self) -> Option<u64> {
        self.candidates
            .iter()
            .filter(|(c, _)| Some(**c) != self.active)
            .min_by_key(|(_, c)| c.link)
            .map(|(c, _)| *c)
    }

    pub fn link_of(&self, conn: u64) -> Option<u8> {
        self.candidates.get(&conn).map(|c| c.link)
    }

    pub fn clear(&mut self) {
        self.active = None;
        self.candidates.clear();
    }

    fn best(&self, now: Instant, require_stable: bool, probation: bool) -> Option<u64> {
        self.candidates
            .iter()
            .filter(|(_, c)| !require_stable || now.duration_since(c.healthy_since) >= STABLE)
            .filter(|(_, c)| {
                !probation
                    || self
                        .failed
                        .get(&c.link)
                        .is_none_or(|t| now.duration_since(*t) >= PROBATION)
            })
            .min_by_key(|(_, c)| c.link)
            .map(|(conn, _)| *conn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LINK_USB_DEBUGGING as USB, LINK_WIFI as WIFI};

    #[test]
    fn a_cable_takes_over_after_2_stable_seconds() {
        let t = Instant::now();
        let mut s = Switcher::default();
        s.add(1, WIFI, t);
        assert_eq!(s.active(), Some(1));
        s.add(2, USB, t + Duration::from_secs(5));
        assert_eq!(s.upgrade(t + Duration::from_secs(6)), None);
        assert_eq!(s.upgrade(t + Duration::from_secs(7)), Some(2));
        assert_eq!(s.standby(), Some(1), "Wi-Fi stays warm");
    }

    #[test]
    fn unplugging_moves_media_to_the_standby_at_once() {
        let t = Instant::now();
        let mut s = Switcher::default();
        s.add(1, USB, t);
        s.add(2, WIFI, t);
        assert_eq!(s.remove(1, t + Duration::from_secs(1)), Some(2));
        assert_eq!(s.active(), Some(2));
    }

    #[test]
    fn a_dead_link_fails_over_even_to_a_link_of_the_same_kind() {
        let t = Instant::now();
        let mut s = Switcher::default();
        s.add(1, USB, t);
        s.add(2, USB, t);
        assert_eq!(s.remove(1, t), Some(2));
    }

    #[test]
    fn a_failed_link_sits_out_its_probation() {
        let t = Instant::now();
        let mut s = Switcher::default();
        s.add(1, WIFI, t);
        s.add(2, USB, t);
        s.remove(2, t);
        s.add(3, USB, t + Duration::from_secs(1));
        assert_eq!(
            s.upgrade(t + Duration::from_secs(5)),
            None,
            "still on probation"
        );
        assert_eq!(s.upgrade(t + Duration::from_secs(10)), Some(3));
    }
}
