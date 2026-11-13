//! Abuse limits (SYSTEM_DESIGN section 15.5): at most 20 probe answers a second, and at most 4
//! handshakes a minute from one address.

use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::time::{Duration, Instant};

pub const PROBES_PER_SECOND: u32 = 20;
pub const HANDSHAKES_PER_MINUTE: usize = 4;

/// Refills `rate` tokens a second, up to `rate`.
pub struct TokenBucket {
    rate: f64,
    tokens: f64,
    last: Instant,
}

impl TokenBucket {
    pub fn new(rate: u32, now: Instant) -> Self {
        Self {
            rate: rate as f64,
            tokens: rate as f64,
            last: now,
        }
    }

    pub fn allow(&mut self, now: Instant) -> bool {
        let refill = now.duration_since(self.last).as_secs_f64() * self.rate;
        self.tokens = (self.tokens + refill).min(self.rate);
        self.last = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[derive(Default)]
pub struct HandshakeLimit {
    recent: HashMap<IpAddr, VecDeque<Instant>>,
}

impl HandshakeLimit {
    pub fn allow(&mut self, from: IpAddr, now: Instant) -> bool {
        // The phone over USB debugging always comes from this PC itself; never lock it out.
        if from.is_loopback() {
            return true;
        }
        let times = self.recent.entry(from).or_default();
        while times
            .front()
            .is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(60))
        {
            times.pop_front();
        }
        if times.len() >= HANDSHAKES_PER_MINUTE {
            return false;
        }
        times.push_back(now);
        self.recent.retain(|_, t| !t.is_empty());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_probe_bucket_allows_20_a_second() {
        let t = Instant::now();
        let mut b = TokenBucket::new(PROBES_PER_SECOND, t);
        assert_eq!((0..30).filter(|_| b.allow(t)).count(), 20);
        assert!(b.allow(t + Duration::from_millis(50)));
        assert!(!b.allow(t + Duration::from_millis(50)));
    }

    #[test]
    fn four_handshakes_a_minute_per_address_and_loopback_is_exempt() {
        let t = Instant::now();
        let mut l = HandshakeLimit::default();
        let a: IpAddr = "192.168.1.9".parse().unwrap();
        assert!((0..4).all(|_| l.allow(a, t)));
        assert!(!l.allow(a, t));
        assert!(l.allow("192.168.1.10".parse().unwrap(), t));
        assert!(l.allow(a, t + Duration::from_secs(60)));
        assert!((0..10).all(|_| l.allow("127.0.0.1".parse().unwrap(), t)));
    }
}
