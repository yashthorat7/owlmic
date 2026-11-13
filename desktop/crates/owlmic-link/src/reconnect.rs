//! The recovery ladder (SYSTEM_DESIGN section 14.8): when a stream that should be flowing stops
//! for a second, try one step at a time, each once, before telling the user anything.

use std::time::{Duration, Instant};

pub const STALL: Duration = Duration::from_secs(1);
/// After this long without recovery the user sees a short message.
pub const TELL_USER_AFTER: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Ask the phone to restart the stream at its source (and send a keyframe for video).
    RestartSource,
    /// Ask again, now also re-creating the media channel.
    RecreateChannel,
    /// Close the control link so the phone shakes hands again with `resume`.
    Rehandshake,
    /// Move to the standby link.
    SwitchLink,
    /// Hold the session and let the phone search.
    Hold,
}

const LADDER: [Step; 5] = [
    Step::RestartSource,
    Step::RecreateChannel,
    Step::Rehandshake,
    Step::SwitchLink,
    Step::Hold,
];

#[derive(Debug, Default)]
pub struct Reconnector {
    stalled_since: Option<Instant>,
    next: usize,
    last_step: Option<Instant>,
}

impl Reconnector {
    /// Media is flowing again: back to the first rung.
    pub fn flowing(&mut self) {
        *self = Self::default();
    }

    /// Checked once a second while the stream should be on. `last_packet` is when its last packet
    /// arrived. Returns the step to take now, if any.
    pub fn check(&mut self, last_packet: Instant, now: Instant) -> Option<Step> {
        if now.duration_since(last_packet) < STALL {
            self.flowing();
            return None;
        }
        self.stalled_since.get_or_insert(last_packet + STALL);
        let due = self
            .last_step
            .is_none_or(|t| now.duration_since(t) >= STALL);
        if !due || self.next >= LADDER.len() {
            return None;
        }
        let step = LADDER[self.next];
        self.next += 1;
        self.last_step = Some(now);
        Some(step)
    }

    pub fn should_tell_user(&self, now: Instant) -> bool {
        self.stalled_since
            .is_some_and(|t| now.duration_since(t) >= TELL_USER_AFTER)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_come_one_a_second_and_each_only_once() {
        let t = Instant::now();
        let mut r = Reconnector::default();
        assert_eq!(r.check(t, t + Duration::from_millis(500)), None);
        let mut taken = Vec::new();
        for s in 1..=8 {
            if let Some(step) = r.check(t, t + Duration::from_secs(s)) {
                taken.push(step);
            }
        }
        assert_eq!(taken, LADDER.to_vec());
    }

    #[test]
    fn recovery_resets_the_ladder_and_the_user_hears_after_5_seconds() {
        let t = Instant::now();
        let mut r = Reconnector::default();
        assert_eq!(
            r.check(t, t + Duration::from_secs(1)),
            Some(Step::RestartSource)
        );
        assert!(!r.should_tell_user(t + Duration::from_secs(3)));
        assert!(r.should_tell_user(t + Duration::from_secs(6)));
        let back = t + Duration::from_secs(6);
        assert_eq!(r.check(back, back), None);
        assert!(!r.should_tell_user(back));
        assert_eq!(
            r.check(back, back + Duration::from_secs(1)),
            Some(Step::RestartSource)
        );
    }
}
