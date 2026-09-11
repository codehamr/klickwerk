use crate::action::Action;
use serde::{Deserialize, Serialize};

pub const HEARTBEAT_TIMEOUT_MS: u64 = 700;
pub const LEASE_MS: u64 = 250;
pub const COUNTDOWN_MS: u64 = 2000;
pub const MOUSE_TAKEOVER_TOLERANCE_PX: u32 = 100;

pub fn keyboard_can_interrupt(
    flags: u32,
    own_input: bool,
    monitoring: bool,
    running: bool,
) -> bool {
    // KBDLLHOOKSTRUCT: injected = 0x10, lower-integrity injected = 0x02,
    // key release = 0x80. Software-generated keys do not establish user takeover.
    monitoring && !own_input && flags & 0x12 == 0 && (running || flags & 0x80 == 0)
}

#[derive(Clone, Copy)]
pub struct PointerGuard {
    pub anchor: (i32, i32),
}
impl PointerGuard {
    pub const fn new(anchor: (i32, i32)) -> Self {
        Self { anchor }
    }

    pub fn observe(&mut self, position: (i32, i32), own_input: bool, monitoring: bool) -> bool {
        if own_input || !monitoring {
            self.anchor = position;
            return false;
        }
        // Keep the anchor through small movements so slow, deliberate movement
        // still interrupts. Agent movement and countdown input set a new baseline.
        let dx = self.anchor.0.abs_diff(position.0);
        let dy = self.anchor.1.abs_diff(position.1);
        dx > MOUSE_TAKEOVER_TOLERANCE_PX
            || dy > MOUSE_TAKEOVER_TOLERANCE_PX
            || dx * dx + dy * dy > MOUSE_TAKEOVER_TOLERANCE_PX * MOUSE_TAKEOVER_TOLERANCE_PX
    }
}

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
    pub fn absolute(&self, x: i32, y: i32) -> Option<(i32, i32)> {
        let (px, py) = self.map(x, y)?;
        if self.width == 0 || self.height == 0 {
            return None;
        }
        // Aim at the center of a physical pixel in the 16-bit virtual desktop grid.
        Some((
            (((px - self.left) as i64 * 65536 + 32768) / self.width as i64).clamp(0, 65535) as i32,
            (((py - self.top) as i64 * 65536 + 32768) / self.height as i64).clamp(0, 65535) as i32,
        ))
    }

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
    Ready {
        bounds: [i32; 4],
    },
    Armed,
    Completed {
        sequence: u64,
    },
    Stopped {
        reason: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        interruption: Option<crate::diagnostics::InputInterruption>,
    },
    Error {
        message: String,
    },
    Rejected {
        sequence: u64,
        diagnostic: crate::diagnostics::InputFailure,
    },
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
    fn injected_keyboard_events_never_establish_user_takeover() {
        // The reported search-field click was followed by an untagged keydown
        // with flags = 16. Its classification must not depend on a timing grace.
        for flags in [0x10, 0x12, 0x02, 0x90, 0x92, 0x30] {
            for own_input in [false, true] {
                for running in [false, true] {
                    assert!(!keyboard_can_interrupt(flags, own_input, true, running));
                }
            }
        }
    }

    #[test]
    fn physical_keyboard_takeover_starts_after_countdown_without_an_input_grace() {
        for flags in [0, 0x01, 0x20, 0x80, 0x81, 0xa0] {
            assert!(!keyboard_can_interrupt(flags, false, false, false));
            assert!(!keyboard_can_interrupt(flags, false, false, true));
            assert!(!keyboard_can_interrupt(flags, true, true, true));
            assert!(keyboard_can_interrupt(flags, false, true, true));
            assert_eq!(
                keyboard_can_interrupt(flags, false, true, false),
                flags & 0x80 == 0
            );
        }
    }

    #[test]
    fn mouse_jitter_is_tolerated_but_slow_movement_accumulates() {
        let mut pointer = PointerGuard::new((-100, 200));
        for offset in [1, -2, 3, -4, 0, 25, 50, 100] {
            assert!(!pointer.observe((-100 + offset, 200), false, true));
        }
        assert!(pointer.observe((1, 200), false, true));
        assert!(pointer.observe((-100, 99), false, true));
        assert!(pointer.observe((-28, 272), false, true));
    }

    #[test]
    fn agent_and_countdown_movements_reset_the_pointer_baseline() {
        let mut pointer = PointerGuard::new((0, 0));
        assert!(!pointer.observe((1000, 1000), false, false));
        assert!(!pointer.observe((1001, 1001), false, true));
        assert!(!pointer.observe((2242, 196), true, true));
        assert!(!pointer.observe((2242, 196), false, true));
        assert!(!pointer.observe((2243, 196), false, true));
        assert!(pointer.observe((2242, 297), false, true));
        assert!(!pointer.observe((i32::MIN, 0), true, true));
        assert!(pointer.observe((i32::MAX, 0), false, true));
    }

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
            let (absolute, _) = f.absolute(x, 0).unwrap();
            assert_eq!(
                absolute as i64 * f.width as i64 / 65536,
                (px - f.left) as i64
            );
        }
    }
    #[test]
    fn physical_pixel_targets_round_trip_at_common_display_scales() {
        for (width, height, image_width, image_height) in [
            (1920, 1080, 1280, 720),
            (2560, 1440, 1280, 720),
            (3840, 2160, 1920, 1080),
            (4480, 1440, 1280, 411),
        ] {
            let frame = Frame {
                id: 1,
                captured_ms: 0,
                left: -1920,
                top: -540,
                width,
                height,
                image_width,
                image_height,
                foreground: 0,
            };
            for x in 0..image_width as i32 {
                let y = (x as u32 % image_height) as i32;
                let (px, py) = frame.map(x, y).unwrap();
                let (ax, ay) = frame.absolute(x, y).unwrap();
                assert_eq!(
                    ax as i64 * width as i64 / 65536 + frame.left as i64,
                    px as i64
                );
                assert_eq!(
                    ay as i64 * height as i64 / 65536 + frame.top as i64,
                    py as i64
                );
            }
        }
    }
}
