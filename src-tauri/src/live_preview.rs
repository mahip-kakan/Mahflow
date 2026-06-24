//! Live transcription preview ("rolling re-transcription").
//!
//! While the user is recording, this periodically snapshots the speech
//! captured so far, runs a normal (fast) batch transcription on it, and emits
//! the partial text to the recording overlay. The text is **preview-only**:
//! the authoritative paste still happens once at the end in `actions.rs`.
//!
//! This is the pragmatic alternative to true incremental streaming: the
//! underlying `transcribe-rs` engines don't expose a partial-result API, but
//! they're fast enough that re-transcribing the growing buffer ~once per
//! second looks like live captioning to the user.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::managers::audio::AudioRecordingManager;
use crate::managers::transcription::TranscriptionManager;
use crate::settings::get_settings;

/// Monotonic session id. Each `start` bumps it; a running loop exits as soon as
/// it no longer matches, guaranteeing at most one preview loop is ever active.
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// How often to re-transcribe the audio-so-far. Kept brisk so the scrolling
/// caption feels live; on long dictations the decode time itself becomes the
/// real floor (we cap the window at MAX_PREVIEW_SAMPLES to keep that bounded).
const PREVIEW_INTERVAL: Duration = Duration::from_millis(600);

/// Minimum speech (in 16 kHz samples) before the first preview. Avoids emitting
/// garbage from a fraction of a second of audio. 16_000 * 0.4s.
const MIN_SAMPLES: usize = 6_400;

/// Cap the preview window (in 16 kHz samples) so per-tick cost stays bounded on
/// long dictations. The *final* transcription still uses the full buffer.
/// 16_000 * 30s.
const MAX_PREVIEW_SAMPLES: usize = 480_000;

/// Event name consumed by the recording overlay (`RecordingOverlay.tsx`).
const PARTIAL_EVENT: &str = "partial-transcription";
const OVERLAY_WINDOW: &str = "recording_overlay";

/// Start a live-preview loop for the current recording session.
///
/// No-op unless the `live_preview_enabled` setting is on. Safe to call on every
/// recording start; the generation counter ensures only the latest loop runs.
pub fn start(app: &AppHandle) {
    if !get_settings(app).live_preview_enabled {
        return;
    }

    let my_gen = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();

    std::thread::spawn(move || {
        let Some(rm) = app
            .try_state::<Arc<AudioRecordingManager>>()
            .map(|s| s.inner().clone())
        else {
            return;
        };
        let Some(tm) = app
            .try_state::<Arc<TranscriptionManager>>()
            .map(|s| s.inner().clone())
        else {
            return;
        };

        loop {
            std::thread::sleep(PREVIEW_INTERVAL);

            // Superseded by a newer session, or recording ended.
            if GENERATION.load(Ordering::SeqCst) != my_gen || !rm.is_recording() {
                break;
            }

            let mut samples = match rm.get_partial_samples() {
                Some(s) if s.len() >= MIN_SAMPLES => s,
                _ => continue,
            };

            // Keep only the tail to bound transcription cost.
            if samples.len() > MAX_PREVIEW_SAMPLES {
                samples = samples.split_off(samples.len() - MAX_PREVIEW_SAMPLES);
            }

            // Re-check right before the (serialized) decode so we don't grab the
            // transcription lock after the user has already stopped.
            if GENERATION.load(Ordering::SeqCst) != my_gen || !rm.is_recording() {
                break;
            }

            match tm.transcribe(samples) {
                Ok(text) => {
                    let text = text.trim().to_string();
                    if text.is_empty() {
                        continue;
                    }
                    // Final check: don't paint a partial over the "transcribing"
                    // overlay state once recording has ended.
                    if GENERATION.load(Ordering::SeqCst) != my_gen || !rm.is_recording() {
                        break;
                    }
                    let _ = app.emit_to(OVERLAY_WINDOW, PARTIAL_EVENT, text);
                }
                Err(e) => {
                    log::debug!("Live preview transcription skipped: {e}");
                }
            }
        }
    });
}

/// Invalidate any running preview loop. Called when recording stops or is
/// cancelled so the loop terminates promptly instead of after a full interval.
pub fn stop() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
}
