use gpui::UniformListScrollHandle;

const MIN_THUMB_H: f32 = 25.0;

/// Computed geometry for positioning and sizing the scrollbar thumb.
pub struct ScrollbarGeometry {
    /// Distance from the track top to the thumb top, in pixels.
    pub thumb_top: f32,
    /// Thumb height in pixels.
    pub thumb_h: f32,
    /// Visible container (viewport) height in pixels.
    pub viewport_h: f32,
    /// Maximum scroll extent (always positive), from `ScrollHandle::max_offset().y`.
    pub max_scroll: f32,
}

/// State captured at the moment the user starts dragging the scrollbar thumb.
pub struct ScrollDragState {
    /// Position within the thumb (thumb-local Y) where the drag began.
    pub drag_offset: f32,
    /// Thumb height at drag start.
    pub thumb_h: f32,
    /// Viewport height at drag start.
    pub viewport_h: f32,
    /// Maximum scroll extent at drag start (positive).
    pub max_scroll: f32,
}

impl ScrollDragState {
    /// Given the mouse's current Y position in track/overlay space, return the
    /// new scroll offset to pass to `ScrollHandle::set_offset` (negative value).
    pub fn compute_offset(&self, track_y: f32) -> f32 {
        let travel = (self.viewport_h - self.thumb_h).max(1.0);
        let thumb_top = (track_y - self.drag_offset).clamp(0.0, travel);
        -(thumb_top / travel) * self.max_scroll
    }
}

/// Compute scrollbar thumb geometry from a `UniformListScrollHandle`.
///
/// Returns `None` when content fits within the viewport (nothing to scroll)
/// or the list has not been laid out yet.
///
/// # Sign convention (mirrors GPUI's `ScrollHandle`)
/// - `max_offset().y` → **positive** pixels (how much content exceeds the viewport)
/// - `offset().y`     → **negative** pixels (0 at top, `-max_scroll` at bottom)
pub fn scrollbar_geometry(handle: &UniformListScrollHandle) -> Option<ScrollbarGeometry> {
    let state = handle.0.borrow();
    let base = &state.base_handle;
    let max_scroll = base.max_offset().y.as_f32(); // always >= 0
    if max_scroll <= 0.0 {
        return None;
    }

    let item_size = state.last_item_size?;
    let viewport_h = item_size.item.height.as_f32();
    if viewport_h <= 0.0 {
        return None;
    }

    let content_h = viewport_h + max_scroll;
    let thumb_h = ((viewport_h * viewport_h) / content_h).max(MIN_THUMB_H);
    if thumb_h >= viewport_h {
        return None;
    }

    let travel = viewport_h - thumb_h;
    let current_scroll = base.offset().y.as_f32().abs(); // how far we've scrolled (positive)
    let thumb_top = (current_scroll / max_scroll) * travel;

    Some(ScrollbarGeometry {
        thumb_top,
        thumb_h,
        viewport_h,
        max_scroll,
    })
}
