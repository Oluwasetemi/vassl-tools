use gpui::Global;

#[derive(Clone, Debug)]
pub struct ThemeColors {
    pub canvas_bg:       u32,
    pub sidebar_bg:      u32,
    pub surface_default: u32,
    pub surface_hover:   u32,
    pub surface_active:  u32,
    pub text_default:    u32,
    pub text_muted:      u32,
    pub status_green:    u32,
    pub status_amber:    u32,
    pub status_red:      u32,
    pub status_grey:     u32,
    pub font_family:     String,
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
            text_default:    0x4c4f69,
            text_muted:      0x8c8fa1,
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
