use crate::action::Action;
use serde::{Deserialize, Serialize};

pub const HOTKEY: &str = "Ctrl + Alt + F8";
pub const HEARTBEAT_TIMEOUT_MS: u64 = 700;
pub const LEASE_MS: u64 = 250;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub id: u64,
    pub captured_ms: u64,
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub image_width: u32,
    pub image_height: u32,
    pub foreground: usize,
}

impl Frame {
    pub fn map(&self, x: i32, y: i32) -> Option<(i32, i32)> {
        if self.image_width == 0
            || self.image_height == 0
            || x < 0
            || y < 0
            || x as u32 >= self.image_width
            || y as u32 >= self.image_height
        {
            return None;
        }
        Some((
            self.left
                + (((2 * x as i64 + 1) * self.width as i64) / (2 * self.image_width as i64)) as i32,
            self.top
                + (((2 * y as i64 + 1) * self.height as i64) / (2 * self.image_height as i64))
                    as i32,
        ))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Heartbeat {
        sent_ms: u64,
        lease_sequence: Option<u64>,
    },
    Execute {
        sequence: u64,
        sent_ms: u64,
        frame: Frame,
        action: Action,
    },
    Stop,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Reply {
    Ready { bounds: [i32; 4] },
    Armed,
    Completed { sequence: u64 },
    Stopped { reason: String },
    Error { message: String },
}

#[derive(Debug)]
pub struct Gate {
    pub stopped: bool,
    pub armed: bool,
    last_heartbeat: u64,
    last_sequence: u64,
    active_sequence: Option<u64>,
    lease_until: u64,
}

impl Gate {
    pub fn new(now: u64) -> Self {
        Self {
            stopped: false,
            armed: false,
            last_heartbeat: now,
            last_sequence: 0,
            active_sequence: None,
            lease_until: 0,
        }
    }
    pub fn stop(&mut self) {
        self.stopped = true;
        self.armed = false;
        self.active_sequence = None;
        self.lease_until = 0;
    }
    pub fn heartbeat(&mut self, now: u64, sent_ms: u64, lease: Option<u64>) -> bool {
        if self.stopped
            || sent_ms > now
            || now - sent_ms > LEASE_MS
            || sent_ms < self.last_heartbeat
        {
            return false;
        }
        self.last_heartbeat = sent_ms;
        if lease.is_some() && lease == self.active_sequence {
            self.lease_until = sent_ms + LEASE_MS;
        }
        true
    }
    pub fn healthy(&self, now: u64) -> bool {
        !self.stopped
            && now >= self.last_heartbeat
            && now - self.last_heartbeat <= HEARTBEAT_TIMEOUT_MS
    }
    pub fn begin(&mut self, now: u64, sent_ms: u64, sequence: u64) -> bool {
        if !self.healthy(now)
            || !self.armed
            || self.active_sequence.is_some()
            || sequence <= self.last_sequence
            || sent_ms > now
            || now - sent_ms > LEASE_MS
        {
            return false;
        }
        self.last_sequence = sequence;
        self.active_sequence = Some(sequence);
        self.lease_until = sent_ms + LEASE_MS;
        true
    }
    pub fn can_input(&self, now: u64) -> bool {
        self.healthy(now) && self.armed && self.active_sequence.is_some() && now <= self.lease_until
    }
    pub fn complete(&mut self) {
        self.active_sequence = None;
        self.lease_until = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stop_is_irreversible_and_queued_actions_cannot_resume() {
        let mut gate = Gate::new(1000);
        gate.armed = true;
        assert!(gate.begin(1000, 1000, 1));
        gate.stop();
        assert!(!gate.heartbeat(1010, 1010, Some(1)));
        assert!(!gate.can_input(1010));
        assert!(!gate.begin(1010, 1010, 2));
    }
    #[test]
    fn lease_is_distinct_from_liveness() {
        let mut gate = Gate::new(1000);
        gate.armed = true;
        assert!(gate.begin(1000, 1000, 1));
        assert!(gate.heartbeat(1200, 1200, None));
        assert!(!gate.can_input(1251));
        assert!(gate.heartbeat(1300, 1300, Some(1)));
        assert!(gate.can_input(1300));
        assert!(!gate.healthy(2001));
    }
    #[test]
    fn replay_future_and_delayed_commands_are_rejected() {
        let mut gate = Gate::new(1000);
        gate.armed = true;
        assert!(!gate.begin(1000, 1001, 1));
        assert!(!gate.begin(1300, 1000, 1));
        assert!(gate.begin(1000, 1000, 1));
        assert!(!gate.begin(1000, 1000, 2));
        gate.complete();
        assert!(!gate.begin(1000, 1000, 1));
        assert!(gate.begin(1000, 1000, 2));
    }
    #[test]
    fn coordinate_mapping_handles_negative_monitors_and_boundaries() {
        let f = Frame {
            id: 1,
            captured_ms: 0,
            left: -1920,
            top: 0,
            width: 3840,
            height: 1080,
            image_width: 1280,
            image_height: 360,
            foreground: 0,
        };
        assert_eq!(f.map(0, 0), Some((-1919, 1)));
        assert_eq!(f.map(1279, 359), Some((1918, 1078)));
        assert_eq!(f.map(1280, 0), None);
        assert_eq!(f.map(-1, 0), None);
        for x in 0..1280 {
            let (px, _) = f.map(x, 0).unwrap();
            assert!((-1920..1920).contains(&px));
        }
    }
}
