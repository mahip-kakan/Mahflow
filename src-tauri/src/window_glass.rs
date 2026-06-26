//! Native window transparency + platform vibrancy for a true glass shell.
//! CSS `backdrop-filter` on panels blurs content inside the webview; these
//! APIs blur the desktop behind the window on macOS and Windows.

use tauri::WebviewWindow;
use tauri::window::{Color, Effect, EffectState, EffectsBuilder};

/// Make the main window transparent and apply platform-native frosted glass.
pub fn apply_main_window_glass(window: &WebviewWindow) {
    if let Err(e) = window.set_background_color(Some(Color(0, 0, 0, 0))) {
        log::warn!("Failed to clear window background color: {}", e);
    }

    #[cfg(target_os = "macos")]
    {
        let effects = EffectsBuilder::new()
            .effect(Effect::HudWindow)
            .state(EffectState::FollowsWindowActiveState)
            .radius(16.0)
            .build();
        if let Err(e) = window.set_effects(effects) {
            log::warn!("Failed to apply macOS HUD window glass: {}", e);
        }
    }

    #[cfg(target_os = "windows")]
    {
        let effects = EffectsBuilder::new()
            .effect(Effect::Acrylic)
            .color(Color(14, 16, 22, 165))
            .build();
        if let Err(e) = window.set_effects(effects) {
            log::warn!("Failed to apply Windows acrylic glass: {}", e);
        }
    }
}
