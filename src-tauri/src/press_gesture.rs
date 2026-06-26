//! Shared double-press / double-click timing for UI open and transcribe activation.

use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const DOUBLE_PRESS_WINDOW: Duration = Duration::from_millis(450);

static LAST_WINDOW_OPEN: Mutex<Option<Instant>> = Mutex::new(None);
static LAST_TRANSCRIBE_ACTIVATION: Mutex<Option<Instant>> = Mutex::new(None);

/// Pure detector: returns `(is_double_press, updated_last)`.
pub fn evaluate_double_press(last: Option<Instant>, now: Instant) -> (bool, Option<Instant>) {
    if let Some(previous) = last {
        if now.duration_since(previous) <= DOUBLE_PRESS_WINDOW {
            return (true, None);
        }
    }
    (false, Some(now))
}

fn register_double_press(slot: &Mutex<Option<Instant>>) -> bool {
    let mut guard = slot
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let now = Instant::now();
    let (is_double, updated) = evaluate_double_press(*guard, now);
    *guard = updated;
    is_double
}

/// Second press within the window returns `true` (open main window).
pub fn register_window_open_press() -> bool {
    register_double_press(&LAST_WINDOW_OPEN)
}

/// Second press within the window returns `true` (toggle transcribe).
pub fn register_transcribe_activation_press() -> bool {
    register_double_press(&LAST_TRANSCRIBE_ACTIVATION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_press_does_not_activate() {
        let t0 = Instant::now();
        let (is_double, last) = evaluate_double_press(None, t0);
        assert!(!is_double);
        assert_eq!(last, Some(t0));
    }

    #[test]
    fn two_presses_within_window_activate() {
        let t0 = Instant::now();
        let (_, last) = evaluate_double_press(None, t0);
        let (is_double, cleared) = evaluate_double_press(last, t0 + Duration::from_millis(200));
        assert!(is_double);
        assert_eq!(cleared, None);
    }

    #[test]
    fn slow_second_press_starts_over() {
        let t0 = Instant::now();
        let (_, last) = evaluate_double_press(None, t0);
        let late = t0 + DOUBLE_PRESS_WINDOW + Duration::from_millis(1);
        let (is_double, restarted) = evaluate_double_press(last, late);
        assert!(!is_double);
        assert_eq!(restarted, Some(late));
    }
}
