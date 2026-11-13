//! Link health (SYSTEM_DESIGN section 14.6): a heartbeat every second in both directions, a
//! quality report every 10 seconds, and media statistics. Kept in memory only.

use owlmic_proto::messages::Report;
use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub const HEARTBEAT: Duration = Duration::from_secs(1);
pub const REPORT_EVERY: Duration = Duration::from_secs(10);
/// One heartbeat a second over 60 seconds.
const HISTORY: usize = 60;

/// Missed heartbeats before a link counts as dead: 3 on wireless links, 2 on cables.
pub fn missed_limit(link: u8) -> u32 {
    if matches!(link, crate::LINK_USB_DEBUGGING | crate::LINK_USB_TETHERING) {
        2
    } else {
        3
    }
}

pub struct Monitor {
    link: u8,
    last_heard: Instant,
    rtt_ms: Option<f64>,
    history: VecDeque<u32>,
}

impl Monitor {
    pub fn new(link: u8, now: Instant) -> Self {
        Self {
            link,
            last_heard: now,
            rtt_ms: None,
            history: VecDeque::with_capacity(HISTORY),
        }
    }

    /// Any control traffic proves the link is alive.
    pub fn heard(&mut self, now: Instant) {
        self.last_heard = now;
    }

    /// A PONG for a PING sent `sent_us` microseconds into `clock`.
    pub fn pong(&mut self, sent_us: u64, now_us: u64, now: Instant) {
        self.heard(now);
        let rtt = now_us.saturating_sub(sent_us) as f64 / 1000.0;
        self.rtt_ms = Some(self.rtt_ms.map_or(rtt, |r| r * 0.8 + rtt * 0.2));
        if self.history.len() == HISTORY {
            self.history.pop_front();
        }
        self.history.push_back(rtt.round() as u32);
    }

    pub fn is_dead(&self, now: Instant) -> bool {
        // Half a heartbeat of slack, so a reply that is merely late isn't a miss.
        now.duration_since(self.last_heard) > HEARTBEAT * missed_limit(self.link) + HEARTBEAT / 2
    }

    pub fn rtt_ms(&self) -> u32 {
        self.rtt_ms.map_or(0, |r| r.round() as u32)
    }

    pub fn link(&self) -> u8 {
        self.link
    }
}

/// Counters a media receive path updates without locks; the Link Hub turns them into reports.
#[derive(Default)]
pub struct StreamStats {
    pub packets: AtomicU64,
    pub bytes: AtomicU64,
    pub lost: AtomicU64,
    /// Interarrival jitter in microseconds, smoothed as in RTP.
    pub jitter_us: AtomicU32,
    /// When the last packet arrived, in microseconds of the process clock.
    pub last_packet_us: AtomicU64,
    highest_seq: AtomicU64,
    last_transit_us: AtomicU64,
}

impl StreamStats {
    /// One packet arrived: `seq` from its header, `sent_us` its timestamp, `arrival_us` now.
    pub fn packet(&self, seq: u32, sent_us: u32, arrival_us: u64, len: usize) {
        self.packets.fetch_add(1, Ordering::Relaxed);
        self.bytes.fetch_add(len as u64, Ordering::Relaxed);
        self.last_packet_us.store(arrival_us, Ordering::Relaxed);
        let prev = self.highest_seq.load(Ordering::Relaxed);
        let next = seq as u64 + 1;
        if prev != 0 {
            let gap = (seq.wrapping_sub(prev as u32 - 1)) as u64;
            if (2..1 << 31).contains(&gap) {
                self.lost.fetch_add(gap - 1, Ordering::Relaxed);
            }
            if gap == 0 || gap >= 1 << 31 {
                return;
            }
        }
        self.highest_seq.store(next, Ordering::Relaxed);
        let transit = arrival_us.wrapping_sub(sent_us as u64);
        let last = self.last_transit_us.swap(transit, Ordering::Relaxed);
        if last != 0 {
            let d = transit.abs_diff(last) as i64;
            let j = self.jitter_us.load(Ordering::Relaxed) as i64;
            self.jitter_us
                .store((j + (d - j) / 16).max(0) as u32, Ordering::Relaxed);
        }
    }

    /// The phone numbers each link's packets from zero: after a switch, loss and jitter count
    /// from the new link's first packet.
    pub fn new_sequence(&self) {
        self.highest_seq.store(0, Ordering::Relaxed);
        self.last_transit_us.store(0, Ordering::Relaxed);
    }

    pub fn reset(&self) {
        for a in [
            &self.packets,
            &self.bytes,
            &self.lost,
            &self.last_packet_us,
            &self.highest_seq,
            &self.last_transit_us,
        ] {
            a.store(0, Ordering::Relaxed);
        }
        self.jitter_us.store(0, Ordering::Relaxed);
    }
}

/// Snapshot of a stream's counters at the last report, to turn totals into rates.
#[derive(Default, Clone, Copy)]
pub struct StatsMark {
    packets: u64,
    bytes: u64,
    lost: u64,
}

/// The REPORT for the period since `marks`, updating them.
pub fn report(
    streams: &[(u8, &StreamStats)],
    marks: &mut BTreeMap<u8, StatsMark>,
    period: Duration,
    rtt_ms: u32,
) -> Report {
    let (mut got, mut lost, mut jitter) = (0u64, 0u64, 0u32);
    let mut kbps = BTreeMap::new();
    for (stream, s) in streams {
        let now = StatsMark {
            packets: s.packets.load(Ordering::Relaxed),
            bytes: s.bytes.load(Ordering::Relaxed),
            lost: s.lost.load(Ordering::Relaxed),
        };
        // Counters start over with a new session, so a mark can be ahead of them.
        let then = marks.insert(*stream, now).unwrap_or_default();
        got += now.packets.saturating_sub(then.packets);
        lost += now.lost.saturating_sub(then.lost);
        jitter = jitter.max(s.jitter_us.load(Ordering::Relaxed) / 1000);
        let bits = now.bytes.saturating_sub(then.bytes) * 8;
        kbps.insert(
            stream.to_string(),
            (bits as f64 / period.as_secs_f64().max(0.001) / 1000.0).round() as u32,
        );
    }
    let loss_pct = if got + lost == 0 {
        0.0
    } else {
        (lost as f64 * 1000.0 / (got + lost) as f64).round() / 10.0
    };
    Report {
        loss_pct,
        jitter_ms: jitter,
        rtt_ms,
        kbps,
        thermal: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_dies_after_its_missed_heartbeats() {
        let t = Instant::now();
        let wifi = Monitor::new(crate::LINK_WIFI, t);
        assert!(!wifi.is_dead(t + Duration::from_millis(3400)));
        assert!(wifi.is_dead(t + Duration::from_millis(3600)));
        let usb = Monitor::new(crate::LINK_USB_DEBUGGING, t);
        assert!(usb.is_dead(t + Duration::from_millis(2600)));
    }

    #[test]
    fn round_trip_time_is_smoothed() {
        let t = Instant::now();
        let mut m = Monitor::new(crate::LINK_WIFI, t);
        m.pong(1_000, 11_000, t);
        assert_eq!(m.rtt_ms(), 10);
        m.pong(2_000, 22_000, t);
        assert_eq!(m.rtt_ms(), 12);
    }

    #[test]
    fn stats_count_losses_and_reports_turn_them_into_rates() {
        let s = StreamStats::default();
        for seq in [1u32, 2, 3, 6, 7] {
            s.packet(seq, seq * 10_000, seq as u64 * 10_000 + 5_000, 100);
        }
        s.packet(2, 20_000, 99_000, 100);
        assert_eq!(s.lost.load(Ordering::Relaxed), 2);
        let mut marks = BTreeMap::new();
        let r = report(&[(1, &s)], &mut marks, Duration::from_secs(10), 15);
        assert_eq!(r.loss_pct, 25.0);
        assert_eq!(r.rtt_ms, 15);
        assert_eq!(r.kbps["1"], 0);
        let again = report(&[(1, &s)], &mut marks, Duration::from_secs(10), 15);
        assert_eq!(again.loss_pct, 0.0, "only the new period counts");
    }
}
