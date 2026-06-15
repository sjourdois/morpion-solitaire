pub mod board_view;
pub mod controls;
pub mod icons;

/// Prefix for the command modifier in shortcut hints. egui's `command` modifier
/// is ⌘ on macOS and Ctrl elsewhere; the ⌘ glyph isn't in the bundled fonts, so
/// spell it out to avoid tofu.
pub fn cmd_key() -> &'static str {
    if cfg!(target_os = "macos") {
        "Cmd+"
    } else {
        "Ctrl+"
    }
}
