//! Takes Return and Escape away from the focused text field while recording.
//!
//! Tauri's Carbon hotkeys never receive a plain Return while a text box is
//! focused — the box keeps the key, so the recitation does not end. This grab
//! uses a blocking event tap for those keys only, then hands them to the same
//! shortcut handler. The tap exists only while a recording is in progress.

use std::collections::HashMap;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use handy_keys::{Hotkey, HotkeyId, HotkeyManager, HotkeyState, Key, Modifiers};
use log::{error, info};
use tauri::{AppHandle, Manager};

use crate::settings::{get_settings, ShortcutBinding};

use super::handler::handle_shortcut_event;
use super::tauri_impl;

struct Grab {
    shutdown: mpsc::Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Grab {
    fn drop(&mut self) {
        let _ = self.shutdown.send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub struct RecordingKeyGrab {
    grab: std::sync::Mutex<Option<Grab>>,
}

impl RecordingKeyGrab {
    fn new() -> Self {
        Self {
            grab: std::sync::Mutex::new(None),
        }
    }
}

/// A plain Return from the keyboard often arrives with Fn still down, or with
/// Option left over from the listen shortcut. Grab every combination so that
/// key still ends the recitation. A shortcut the user set themselves, such as
/// Command+Return, stays exactly that combination.
pub fn hotkeys_for_binding(raw: &str) -> Result<Vec<Hotkey>, String> {
    let hotkey: Hotkey = raw
        .parse()
        .map_err(|e| format!("Invalid recording shortcut '{raw}': {e}"))?;
    if hotkey.modifiers.is_empty() {
        if hotkey.key == Some(Key::Return) {
            let mut hotkeys = variants_for_key(Key::Return)?;
            hotkeys.extend(variants_for_key(Key::KeypadEnter)?);
            return Ok(hotkeys);
        }
        if hotkey.key == Some(Key::Escape) {
            return variants_for_key(Key::Escape);
        }
    }
    Ok(vec![hotkey])
}

fn variants_for_key(key: Key) -> Result<Vec<Hotkey>, String> {
    modifier_combinations()
        .into_iter()
        .map(|modifiers| {
            Hotkey::new(modifiers, key).map_err(|e| format!("Could not grab {key:?}: {e}"))
        })
        .collect()
}

fn modifier_combinations() -> Vec<Modifiers> {
    let groups = [
        Modifiers::SHIFT,
        Modifiers::OPT,
        Modifiers::CTRL,
        Modifiers::CMD,
        Modifiers::FN,
    ];
    let mut masks = vec![Modifiers::empty()];
    for group in groups {
        let snapshot = masks.clone();
        for mask in snapshot {
            masks.push(mask | group);
        }
    }
    masks
}

/// Start swallowing the recording-only keys. Falls back to a Carbon hotkey if
/// the event tap cannot start.
pub fn install(app: &AppHandle, binding_ids: &[String]) {
    uninstall(app);

    let settings = get_settings(app);
    let bindings: Vec<ShortcutBinding> = binding_ids
        .iter()
        .filter_map(|id| settings.bindings.get(id).cloned())
        .collect();

    if bindings.is_empty() {
        return;
    }

    match start_grab(app.clone(), bindings) {
        Ok(grab) => {
            let state = recording_state(app);
            if let Ok(mut slot) = state.grab.lock() {
                *slot = Some(grab);
            };
        }
        Err(e) => {
            error!("Recording key grab failed ({e}). Falling back to a Carbon hotkey for Return.");
            tauri_impl::register_dynamic_shortcuts(app, binding_ids);
        }
    }
}

/// Stop swallowing recording-only keys. Safe to call if none were grabbed.
pub fn uninstall(app: &AppHandle) {
    let Some(state) = app.try_state::<RecordingKeyGrab>() else {
        return;
    };
    if let Ok(mut slot) = state.grab.lock() {
        // Drop joins the tap thread, so Return is a normal key again
        // before the transcript is pasted.
        slot.take();
    };
}

fn recording_state(app: &AppHandle) -> tauri::State<'_, RecordingKeyGrab> {
    if app.try_state::<RecordingKeyGrab>().is_none() {
        app.manage(RecordingKeyGrab::new());
    }
    app.state::<RecordingKeyGrab>()
}

fn start_grab(app: AppHandle, bindings: Vec<ShortcutBinding>) -> Result<Grab, String> {
    let (ready_tx, ready_rx) = mpsc::channel();
    let (shutdown_tx, shutdown_rx) = mpsc::channel();

    let thread = thread::spawn(move || {
        let manager = match HotkeyManager::new_with_blocking() {
            Ok(manager) => manager,
            Err(e) => {
                let _ = ready_tx.send(Err(e.to_string()));
                return;
            }
        };

        let mut id_to_binding: HashMap<HotkeyId, (String, String)> = HashMap::new();
        for binding in &bindings {
            let hotkeys = match hotkeys_for_binding(&binding.current_binding) {
                Ok(hotkeys) => hotkeys,
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            for hotkey in hotkeys {
                match manager.register(hotkey) {
                    Ok(id) => {
                        id_to_binding
                            .insert(id, (binding.id.clone(), binding.current_binding.clone()));
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(format!(
                            "Could not grab '{}' ({hotkey}): {e}",
                            binding.current_binding
                        )));
                        return;
                    }
                }
            }
        }

        let names: Vec<&str> = bindings
            .iter()
            .map(|binding| binding.current_binding.as_str())
            .collect();
        info!(
            "Recording keys grabbed (Return ends the recitation and pastes): {}",
            names.join(", ")
        );
        let _ = ready_tx.send(Ok(()));

        loop {
            while let Some(event) = manager.try_recv() {
                let Some((binding_id, hotkey_string)) = id_to_binding.get(&event.id) else {
                    continue;
                };
                let is_pressed = event.state == HotkeyState::Pressed;
                if is_pressed {
                    info!("Recording key pressed: {hotkey_string} ({binding_id})");
                }
                let app = app.clone();
                let binding_id = binding_id.clone();
                let hotkey_string = hotkey_string.clone();
                // Handle off this thread. Escape cancels synchronously and
                // tears this grab down; joining from here would deadlock.
                thread::spawn(move || {
                    handle_shortcut_event(&app, &binding_id, &hotkey_string, is_pressed);
                });
            }

            match shutdown_rx.recv_timeout(Duration::from_millis(10)) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }

        drop(manager);
    });

    match ready_rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Ok(())) => Ok(Grab {
            shutdown: shutdown_tx,
            thread: Some(thread),
        }),
        Ok(Err(e)) => {
            let _ = shutdown_tx.send(());
            let _ = thread.join();
            Err(e)
        }
        Err(_) => {
            let _ = shutdown_tx.send(());
            let _ = thread.join();
            Err("Timed out starting the recording key grab".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::hotkeys_for_binding;
    use handy_keys::{Key, Modifiers};

    #[test]
    fn plain_return_also_grabs_the_keypad_enter_key() {
        let hotkeys = hotkeys_for_binding("enter").expect("enter parses");
        // 5 modifier groups, every on/off combination, for Return and keypad Enter.
        assert_eq!(hotkeys.len(), 64);
        assert!(hotkeys
            .iter()
            .any(|hotkey| hotkey.key == Some(Key::Return) && hotkey.modifiers.is_empty()));
        assert!(hotkeys
            .iter()
            .any(|hotkey| hotkey.key == Some(Key::Return)
                && hotkey.modifiers.contains(Modifiers::FN)));
        assert!(hotkeys
            .iter()
            .any(|hotkey| hotkey.key == Some(Key::KeypadEnter)));
    }

    #[test]
    fn escape_is_grabbed_with_any_modifier() {
        let hotkeys = hotkeys_for_binding("escape").expect("escape parses");
        assert_eq!(hotkeys.len(), 32);
        assert!(hotkeys
            .iter()
            .any(|hotkey| hotkey.key == Some(Key::Escape) && hotkey.modifiers.is_empty()));
    }

    #[test]
    fn a_modified_return_does_not_add_the_keypad() {
        let hotkeys = hotkeys_for_binding("command+enter").expect("command+enter parses");
        assert_eq!(hotkeys.len(), 1);
        assert_eq!(hotkeys[0].key, Some(Key::Return));
        assert!(!hotkeys[0].modifiers.is_empty());
    }
}
