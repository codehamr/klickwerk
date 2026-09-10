use image::RgbImage;

// Ignore tiny changes such as a blinking caret, but catch local text and controls.
// A global percentage would miss a small editor inside a large desktop.
pub fn changed(before: &RgbImage, after: &RgbImage) -> bool {
    if before.dimensions() != after.dimensions() {
        return true;
    }
    let (width, height) = before.dimensions();
    for top in (0..height).step_by(32) {
        for left in (0..width).step_by(32) {
            let mut differences = 0;
            for y in top..(top + 32).min(height) {
                for x in left..(left + 32).min(width) {
                    if before
                        .get_pixel(x, y)
                        .0
                        .iter()
                        .zip(after.get_pixel(x, y).0)
                        .any(|(&a, b)| a.abs_diff(b) > 18)
                    {
                        differences += 1;
                        if differences > 48 {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

pub struct Settler {
    started_ms: u64,
    changed_ms: u64,
    effect_wait_ms: u64,
    effect_seen: bool,
}
#[derive(Debug, PartialEq)]
pub enum Status {
    Waiting,
    Ready,
    NoEffect,
    TimedOut,
}
impl Settler {
    pub fn new(now: u64) -> Self {
        Self {
            started_ms: now,
            changed_ms: now,
            effect_wait_ms: 0,
            effect_seen: false,
        }
    }
    pub fn awaiting_effect(now: u64, wait_ms: u64) -> Self {
        Self {
            effect_wait_ms: wait_ms,
            ..Self::new(now)
        }
    }
    pub fn observe(&mut self, now: u64, changed: bool, effect: bool) -> Status {
        self.effect_seen |= effect;
        if changed {
            self.changed_ms = now;
        }
        let elapsed = now.saturating_sub(self.started_ms);
        // A quiet desktop is not evidence that a delayed launch has completed.
        if self.effect_wait_ms > 0 && !self.effect_seen {
            return if elapsed >= self.effect_wait_ms {
                Status::NoEffect
            } else {
                Status::Waiting
            };
        }
        if elapsed >= self.effect_wait_ms.max(5000) {
            return Status::TimedOut;
        }
        if now.saturating_sub(self.started_ms) >= 700 && now.saturating_sub(self.changed_ms) >= 450
        {
            Status::Ready
        } else {
            Status::Waiting
        }
    }
}

// Compare only an independently identified text field, in physical desktop pixels.
// Unknown or partially off-screen fields must use the full-screen check instead.
pub fn region_changed(
    before: &RgbImage,
    after: &RgbImage,
    origin: (i32, i32),
    bounds: [i32; 4],
) -> bool {
    if before.dimensions() != after.dimensions() {
        return true;
    }
    let [left, top, right, bottom] = bounds;
    let x = left as i64 - origin.0 as i64;
    let y = top as i64 - origin.1 as i64;
    let width = right as i64 - left as i64;
    let height = bottom as i64 - top as i64;
    if x < 0
        || y < 0
        || width <= 0
        || height <= 0
        || x + width > before.width() as i64
        || y + height > before.height() as i64
    {
        return true;
    }
    changed(
        &image::imageops::crop_imm(before, x as u32, y as u32, width as u32, height as u32)
            .to_image(),
        &image::imageops::crop_imm(after, x as u32, y as u32, width as u32, height as u32)
            .to_image(),
    )
}

#[derive(Debug, PartialEq)]
pub enum RepeatDecision {
    Allow,
    ObserveAgain,
    Pause,
}

#[derive(Default)]
pub struct RepeatGuard {
    reobserved: bool,
}
impl RepeatGuard {
    pub fn evaluate(
        &mut self,
        previous: &crate::action::Action,
        proposed: &crate::action::Action,
        effect: bool,
    ) -> RepeatDecision {
        use crate::action::Action;
        if effect
            || !matches!(
                proposed,
                Action::Click { .. }
                    | Action::DoubleClick { .. }
                    | Action::Text { .. }
                    | Action::Key { .. }
            )
            || !previous.same_input(proposed)
        {
            return RepeatDecision::Allow;
        }
        if self.reobserved {
            RepeatDecision::Pause
        } else {
            self.reobserved = true;
            RepeatDecision::ObserveAgain
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delayed_rendering_resets_the_quiet_period_and_has_a_deadline() {
        let mut settling = Settler::new(0);
        assert_eq!(settling.observe(450, false, false), Status::Waiting);
        assert_eq!(settling.observe(690, true, false), Status::Waiting);
        assert_eq!(settling.observe(1000, false, false), Status::Waiting);
        assert_eq!(settling.observe(1140, false, false), Status::Ready);
        assert_eq!(Settler::new(0).observe(5000, true, false), Status::TimedOut);
    }
    #[test]
    fn delayed_launch_waits_for_effect_and_live_tables_have_a_deadline() {
        let mut settling = Settler::awaiting_effect(0, 8000);
        assert_eq!(settling.observe(900, false, false), Status::Waiting);
        assert_eq!(settling.observe(3500, true, false), Status::Waiting);
        assert_eq!(settling.observe(4200, true, true), Status::Waiting);
        assert_eq!(settling.observe(4700, false, true), Status::Ready);
        assert_eq!(
            Settler::awaiting_effect(0, 8000).observe(8000, true, false),
            Status::NoEffect
        );
        for now in (5000..8000).step_by(200) {
            settling.observe(now, true, true);
        }
        assert_eq!(settling.observe(8000, true, true), Status::TimedOut);
    }
    #[test]
    fn unchanged_launch_is_reobserved_before_pausing_and_never_replayed() {
        use crate::action::{Action, Modifier};
        let launch = Action::Key {
            key: "ESC".into(),
            modifiers: vec![Modifier::Ctrl, Modifier::Shift],
        };
        let retry = Action::Key {
            key: "escape".into(),
            modifiers: vec![Modifier::Shift, Modifier::Ctrl],
        };
        let mut guard = RepeatGuard::default();
        assert_eq!(
            guard.evaluate(&launch, &retry, false),
            RepeatDecision::ObserveAgain
        );
        assert_eq!(
            guard.evaluate(&launch, &Action::Observe, false),
            RepeatDecision::Allow
        );
        assert_eq!(
            guard.evaluate(&launch, &retry, false),
            RepeatDecision::Pause
        );
        assert_eq!(guard.evaluate(&launch, &retry, true), RepeatDecision::Allow);
        assert_eq!(
            guard.evaluate(&launch, &Action::Text { text: "chr".into() }, true),
            RepeatDecision::Allow
        );
    }
    #[test]
    fn live_table_changes_do_not_invalidate_an_unchanged_search_field() {
        let before = RgbImage::new(320, 240);
        let mut after = before.clone();
        for y in 100..200 {
            for x in 100..200 {
                after.put_pixel(x, y, image::Rgb([255; 3]));
            }
        }
        let bounds = [-100, -40, 100, -10];
        assert!(changed(&before, &after));
        assert!(!region_changed(&before, &after, (-120, -60), bounds));
        for y in 20..40 {
            for x in 40..80 {
                after.put_pixel(x, y, image::Rgb([255; 3]));
            }
        }
        assert!(region_changed(&before, &after, (-120, -60), bounds));
        assert!(region_changed(&before, &after, (0, 0), bounds));
    }
    #[test]
    fn small_local_text_changes_are_detected_but_caret_blinks_are_ignored() {
        let before = RgbImage::new(1920, 1080);
        let mut after = before.clone();
        for y in 64..80 {
            after.put_pixel(64, y, image::Rgb([255; 3]));
        }
        assert!(!changed(&before, &after));
        for y in 64..80 {
            for x in 64..80 {
                after.put_pixel(x, y, image::Rgb([255; 3]));
            }
        }
        assert!(changed(&before, &after));
        assert!(changed(&before, &RgbImage::new(1280, 720)));
    }
}
