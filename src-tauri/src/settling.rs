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
}
#[derive(Debug, PartialEq)]
pub enum Status {
    Waiting,
    Ready,
    TimedOut,
}
impl Settler {
    pub fn new(now: u64) -> Self {
        Self {
            started_ms: now,
            changed_ms: now,
        }
    }
    pub fn observe(&mut self, now: u64, changed: bool) -> Status {
        if changed {
            self.changed_ms = now;
        }
        if now.saturating_sub(self.started_ms) >= 5000 {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delayed_rendering_resets_the_quiet_period_and_has_a_deadline() {
        let mut settling = Settler::new(0);
        assert_eq!(settling.observe(450, false), Status::Waiting);
        assert_eq!(settling.observe(690, true), Status::Waiting);
        assert_eq!(settling.observe(1000, false), Status::Waiting);
        assert_eq!(settling.observe(1140, false), Status::Ready);
        assert_eq!(Settler::new(0).observe(5000, true), Status::TimedOut);
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
