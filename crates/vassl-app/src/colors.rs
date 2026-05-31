// Named color constants for the VASSL UI — Catppuccin Mocha-inspired dark theme.
// All values are u32 RGB hex for use with gpui::rgb(CONSTANT).

pub const CANVAS_BG: u32       = 0x1e1e2e; // main pane area background
pub const SIDEBAR_BG: u32      = 0x181825; // sidebar and status bar background
pub const SURFACE_DEFAULT: u32 = 0x313244; // inactive buttons, borders
pub const SURFACE_ACTIVE: u32  = 0x1a3c5e; // VASSL navy — active module highlight
pub const TEXT_DEFAULT: u32    = 0xcdd6f4; // active button text
pub const TEXT_MUTED: u32      = 0x6c7086; // inactive button text, status bar text

pub const STATUS_GREEN: u32  = 0xa6e3a1; // Catppuccin Mocha green — healthy stock
pub const STATUS_AMBER: u32  = 0xf9e2af; // Catppuccin Mocha yellow — low stock
pub const STATUS_RED: u32    = 0xf38ba8; // Catppuccin Mocha red — critical stock
pub const STATUS_GREY: u32   = 0x585b70; // no-alert / disabled state
