use gpui::{FocusHandle, Global};

#[derive(Clone, Debug)]
pub struct ThemeColors {
    pub canvas_bg:       u32,
    pub sidebar_bg:      u32,
    pub surface_default: u32,
    pub surface_hover:   u32,
    pub surface_active:  u32,
    pub text_default:    u32,
    pub text_muted:      u32,
    /// Text colour to use when rendered ON a `surface_active` background.
    /// Light mode needs white here; dark mode matches `text_default`.
    pub text_on_active:  u32,
    pub status_green:    u32,
    pub status_amber:    u32,
    pub status_red:      u32,
    pub status_grey:     u32,
    pub font_family:     String,
}

/// Holds the root window's focus handle so any nested form can restore focus
/// after dismissal without threading the handle through constructors.
pub struct RootFocusHandle(pub FocusHandle);
impl Global for RootFocusHandle {}

#[derive(Clone, Debug)]
pub struct AppSettings {
    pub logged_in_user_id: i64,
    pub username:          String,
    pub is_admin:          bool,
    pub can_inventory:     bool,
    pub can_pricebook:     bool,
    pub can_quotations:    bool,
    pub allow_delete:      bool,
    pub allow_price_edit:  bool,
}

impl gpui::Global for AppSettings {}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            logged_in_user_id: 0,
            username:          String::new(),
            is_admin:          false,
            can_inventory:     false,
            can_pricebook:     false,
            can_quotations:    false,
            allow_delete:      false,
            allow_price_edit:  false,
        }
    }
}

impl ThemeColors {
    pub fn dark() -> Self {
        Self {
            canvas_bg:       0x1e1e2e,
            sidebar_bg:      0x181825,
            surface_default: 0x313244,
            surface_hover:   0x3d3f52,
            surface_active:  0x1a3c5e,
            text_default:    0xcdd6f4,
            text_muted:      0x6c7086,
            text_on_active:  0xcdd6f4, // light text on dark-blue active bg
            status_green:    0xa6e3a1,
            status_amber:    0xf9e2af,
            status_red:      0xf38ba8,
            status_grey:     0x585b70,
            font_family:     "system-ui".into(),
        }
    }

    pub fn light() -> Self {
        Self {
            canvas_bg:       0xeff1f5,
            sidebar_bg:      0xe6e9ef,
            surface_default: 0xccd0da,
            surface_hover:   0xbec2cc,
            surface_active:  0x1e66f5,
            // Darkened for WCAG AA compliance on all light surfaces (~13:1 on canvas)
            text_default:    0x232634,
            // Darkened: 0x8c8fa1 gave ~2:1 on surfaces; 0x5c5f77 gives ~5:1 on canvas
            text_muted:      0x5c5f77,
            text_on_active:  0xffffff, // white on vivid-blue active bg
            status_green:    0x40a02b,
            status_amber:    0xdf8e1d,
            status_red:      0xd20f39,
            status_grey:     0x9ca0b0,
            font_family:     "system-ui".into(),
        }
    }

    pub fn with_font(mut self, family: impl Into<String>) -> Self {
        self.font_family = family.into();
        self
    }
}

pub struct ThemeHandle(pub ThemeColors);
impl Global for ThemeHandle {}

/// Data for the TextInput right-click context menu (stored as an observed Entity).
pub struct TextContextMenuState {
    pub position:      Option<gpui::Point<gpui::Pixels>>,
    pub input:         Option<gpui::Entity<crate::text_input::TextInput>>,
    pub has_selection: bool,
}

impl Default for TextContextMenuState {
    fn default() -> Self {
        Self { position: None, input: None, has_selection: false }
    }
}

/// Global handle — wraps the entity so VasslRoot can observe it for re-render.
pub struct TextContextMenuHandle(pub gpui::Entity<TextContextMenuState>);
impl gpui::Global for TextContextMenuHandle {}
