//! Double-click / double-tap detection before showing the main window.

use tauri::AppHandle;

use crate::press_gesture;
use crate::show_main_window;

/// Show the main window only after a double-click/double-tap open gesture.
pub fn try_show_main_window_on_double_click(app: &AppHandle) {
    if press_gesture::register_window_open_press() {
        show_main_window(app);
    }
}
