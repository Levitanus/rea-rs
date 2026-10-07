//! Pure scroll state and command handling for native ScrollView containers.
//!
//! This module deliberately does not own HWNDs. A native viewport can use
//! [`ScrollState`] to calculate scrollbar metrics and the content offset, then
//! apply that offset to its content window.
//! Offsets and extents are measured in pixels from the content's top-left
//! corner. A state value is pure data and does not itself move a native
//! window.

use super::layout::{Axis, Size};

/// The current position of scrollable content in content coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScrollOffset {
    pub x: u32,
    pub y: u32,
}

impl ScrollOffset {
    pub const fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }

    pub const fn axis(self, axis: Axis) -> u32 {
        match axis {
            Axis::X => self.x,
            Axis::Y => self.y,
        }
    }

    pub const fn with_axis(self, axis: Axis, value: u32) -> Self {
        match axis {
            Axis::X => Self { x: value, ..self },
            Axis::Y => Self { y: value, ..self },
        }
    }
}

/// The range and page information exposed to a native scrollbar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScrollMetrics {
    pub content: Size,
    pub viewport: Size,
    pub offset: ScrollOffset,
    pub max_offset: ScrollOffset,
}

/// The native scrollbar range is expressed in content coordinates. With a
/// page size of `viewport`, the largest legal thumb position is
/// `content - viewport`; therefore `nMax` must be `content - 1`.
pub(crate) const fn scrollbar_range_max(content_extent: u32) -> u32 {
    content_extent.saturating_sub(1)
}

/// The scrollbar backend requested by a native ScrollView.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScrollbarRenderer {
    /// Use the platform's standard scrollbar implementation. This is only
    /// supported on Windows and returns an error on other SWELL backends.
    Native,
    /// Use REAPER's CoolSB implementation. This returns an error when the
    /// running REAPER backend does not provide CoolSB.
    CoolSb,
    /// Use REAPER CoolSB when available, otherwise use the platform-native
    /// backend when supported. The selected backend is exposed by the handle.
    Auto,
}

impl Default for ScrollbarRenderer {
    fn default() -> Self {
        Self::Auto
    }
}

/// Visibility of the two viewport-owned scrollbars.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScrollbarVisibility {
    pub horizontal: bool,
    pub vertical: bool,
}

impl ScrollbarVisibility {
    /// Computes stable visibility when a visible bar reduces the other axis.
    pub const fn resolve(
        content: Size,
        viewport: Size,
        thickness: u32,
    ) -> Self {
        let mut horizontal = content.x > viewport.x;
        let mut vertical = content.y > viewport.y;
        let mut iteration = 0;
        while iteration < 2 {
            let effective_width = viewport.x.saturating_sub(if vertical {
                thickness
            } else {
                0
            });
            let effective_height = viewport.y.saturating_sub(if horizontal {
                thickness
            } else {
                0
            });
            let next_horizontal = content.x > effective_width;
            let next_vertical = content.y > effective_height;
            if next_horizontal == horizontal && next_vertical == vertical {
                break;
            }
            horizontal = next_horizontal;
            vertical = next_vertical;
            iteration += 1;
        }
        Self {
            horizontal,
            vertical,
        }
    }
}

impl ScrollMetrics {
    pub const fn with_visibility(
        content: Size,
        viewport: Size,
        thickness: u32,
    ) -> (Self, ScrollbarVisibility) {
        let visibility =
            ScrollbarVisibility::resolve(content, viewport, thickness);
        let effective_viewport = Size {
            x: viewport.x.saturating_sub(if visibility.vertical {
                thickness
            } else {
                0
            }),
            y: viewport.y.saturating_sub(if visibility.horizontal {
                thickness
            } else {
                0
            }),
        };
        let offset = ScrollOffset::new(0, 0);
        (
            Self {
                content,
                viewport: effective_viewport,
                offset,
                max_offset: Self::max_for(content, effective_viewport),
            },
            visibility,
        )
    }
}

impl ScrollMetrics {
    pub const fn max_for(content: Size, viewport: Size) -> ScrollOffset {
        ScrollOffset::new(
            content.x.saturating_sub(viewport.x),
            content.y.saturating_sub(viewport.y),
        )
    }

    pub const fn max_offset(self) -> ScrollOffset {
        self.max_offset
    }

    pub const fn can_scroll(self, axis: Axis) -> bool {
        self.max_offset.axis(axis) > 0
    }
}

/// A native scrollbar command after decoding its low-word notification code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollCommand {
    LineBackward,
    LineForward,
    PageBackward,
    PageForward,
    ThumbTrack(u32),
    ThumbPosition(u32),
    Start,
    End,
    EndScroll,
}

/// Decodes a low-word native scrollbar notification.
pub const fn decode_scroll_command(
    code: i32,
    position: u32,
) -> Option<ScrollCommand> {
    match code {
        0 => Some(ScrollCommand::LineBackward),
        1 => Some(ScrollCommand::LineForward),
        2 => Some(ScrollCommand::PageBackward),
        3 => Some(ScrollCommand::PageForward),
        4 => Some(ScrollCommand::ThumbPosition(position)),
        5 => Some(ScrollCommand::ThumbTrack(position)),
        6 => Some(ScrollCommand::Start),
        7 => Some(ScrollCommand::End),
        8 => Some(ScrollCommand::EndScroll),
        _ => None,
    }
}

/// A small, platform-independent scroll model used by a viewport HWND.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScrollState {
    content: Size,
    viewport: Size,
    offset: ScrollOffset,
    line_step: u32,
    wheel_remainder: i32,
}

impl Default for ScrollState {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollState {
    pub const DEFAULT_LINE_STEP: u32 = 16;

    pub const fn new() -> Self {
        Self {
            content: Size { x: 0, y: 0 },
            viewport: Size { x: 0, y: 0 },
            offset: ScrollOffset::new(0, 0),
            line_step: Self::DEFAULT_LINE_STEP,
            wheel_remainder: 0,
        }
    }

    pub const fn with_line_step(line_step: u32) -> Self {
        Self {
            line_step,
            ..Self::new()
        }
    }

    pub const fn content(self) -> Size {
        self.content
    }

    pub const fn viewport(self) -> Size {
        self.viewport
    }

    /// Replaces the current viewport while retaining content and offset
    /// clamping semantics. Used after visibility reserves scrollbar space.
    pub fn set_effective_viewport(mut self, viewport: Size) -> Self {
        self.viewport = viewport;
        self.offset = self.clamp(self.offset);
        self
    }

    pub const fn offset(self) -> ScrollOffset {
        self.offset
    }

    pub const fn metrics(self) -> ScrollMetrics {
        ScrollMetrics {
            content: self.content,
            viewport: self.viewport,
            offset: self.offset,
            max_offset: ScrollMetrics::max_for(self.content, self.viewport),
        }
    }

    pub fn set_content(mut self, content: Size) -> Self {
        self.content = content;
        self.offset = self.clamp(self.offset);
        self
    }

    pub fn set_viewport(mut self, viewport: Size) -> Self {
        self.viewport = viewport;
        self.offset = self.clamp(self.offset);
        self
    }

    pub fn set_line_step(mut self, line_step: u32) -> Self {
        self.line_step = line_step;
        self
    }

    pub fn set_offset(mut self, offset: ScrollOffset) -> Self {
        self.offset = self.clamp(offset);
        self
    }

    pub fn scroll_by(mut self, axis: Axis, delta: i32) -> Self {
        let current = self.offset.axis(axis);
        let target = if delta.is_negative() {
            current.saturating_sub(delta.unsigned_abs())
        } else {
            current.saturating_add(delta as u32)
        };
        self.offset = self.clamp(self.offset.with_axis(axis, target));
        self
    }

    pub fn scroll_wheel(mut self, axis: Axis, wheel_delta: i32) -> Self {
        // Windows reports wheel movement in multiples of WHEEL_DELTA, but
        // precision touchpads and some platforms can report partial deltas.
        // Retain the remainder so several small inputs eventually produce a
        // complete line instead of silently losing movement.
        let accumulated = self.wheel_remainder.saturating_add(wheel_delta);
        let notches = accumulated / 120;
        self.wheel_remainder = accumulated % 120;
        self.scroll_by(axis, notches.saturating_mul(self.line_step as i32))
    }

    pub fn apply(self, axis: Axis, command: ScrollCommand) -> Self {
        let page = match axis {
            Axis::X => self.viewport.x,
            Axis::Y => self.viewport.y,
        };
        match command {
            ScrollCommand::LineBackward => {
                self.scroll_by(axis, -(self.line_step as i32))
            }
            ScrollCommand::LineForward => {
                self.scroll_by(axis, self.line_step as i32)
            }
            ScrollCommand::PageBackward => {
                self.scroll_by(axis, -(page as i32))
            }
            ScrollCommand::PageForward => self.scroll_by(axis, page as i32),
            ScrollCommand::ThumbTrack(position)
            | ScrollCommand::ThumbPosition(position) => {
                self.set_offset(self.offset.with_axis(axis, position))
            }
            ScrollCommand::Start => {
                self.set_offset(self.offset.with_axis(axis, 0))
            }
            ScrollCommand::End => {
                let end = self.metrics().max_offset.axis(axis);
                self.set_offset(self.offset.with_axis(axis, end))
            }
            ScrollCommand::EndScroll => self,
        }
    }

    fn clamp(self, offset: ScrollOffset) -> ScrollOffset {
        let max = ScrollMetrics::max_for(self.content, self.viewport);
        ScrollOffset::new(offset.x.min(max.x), offset.y.min(max.y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> ScrollState {
        ScrollState::with_line_step(10)
            .set_content(Size { x: 500, y: 1_000 })
            .set_viewport(Size { x: 100, y: 200 })
    }

    #[test]
    fn default_renderer_uses_automatic_selection() {
        assert_eq!(ScrollbarRenderer::default(), ScrollbarRenderer::Auto);
    }

    #[test]
    fn metrics_report_independent_ranges() {
        let metrics = state().metrics();
        assert_eq!(metrics.max_offset, ScrollOffset::new(400, 800));
        assert!(metrics.can_scroll(Axis::X));
        assert!(metrics.can_scroll(Axis::Y));
    }

    #[test]
    fn offsets_are_clamped_after_content_or_viewport_changes() {
        let state = state().set_offset(ScrollOffset::new(400, 800));
        assert_eq!(
            state.set_content(Size { x: 120, y: 250 }).offset(),
            ScrollOffset::new(20, 50)
        );
        assert_eq!(
            state.set_viewport(Size { x: 600, y: 2_000 }).offset(),
            ScrollOffset::default()
        );
    }

    #[test]
    fn line_and_page_commands_use_the_selected_axis() {
        let state = state()
            .apply(Axis::Y, ScrollCommand::LineForward)
            .apply(Axis::X, ScrollCommand::PageForward);
        assert_eq!(state.offset(), ScrollOffset::new(100, 10));
    }

    #[test]
    fn thumb_and_end_commands_are_clamped() {
        let state = state()
            .apply(Axis::Y, ScrollCommand::ThumbTrack(900))
            .apply(Axis::X, ScrollCommand::End);
        assert_eq!(state.offset(), ScrollOffset::new(400, 800));
    }

    #[test]
    fn wheel_delta_is_converted_to_line_steps() {
        let state = state().scroll_wheel(Axis::Y, 240);
        assert_eq!(state.offset().y, 20);
    }

    #[test]
    fn visibility_accounts_for_cross_axis_bar_space() {
        let visibility = ScrollbarVisibility::resolve(
            Size { x: 100, y: 100 },
            Size { x: 100, y: 100 },
            10,
        );
        assert!(!visibility.horizontal);
        assert!(!visibility.vertical);

        let visibility = ScrollbarVisibility::resolve(
            Size { x: 105, y: 100 },
            Size { x: 100, y: 100 },
            10,
        );
        assert!(visibility.horizontal);
        assert!(visibility.vertical);
    }

    #[test]
    fn effective_viewport_reserves_cross_axis_scrollbar_space() {
        let (metrics, visibility) = ScrollMetrics::with_visibility(
            Size { x: 110, y: 95 },
            Size { x: 100, y: 100 },
            10,
        );
        assert!(visibility.horizontal);
        assert!(visibility.vertical);
        assert_eq!(metrics.viewport, Size { x: 90, y: 90 });
        assert_eq!(metrics.max_offset, ScrollOffset::new(20, 5));
    }

    #[test]
    fn effective_viewport_keeps_content_and_clamps_offset() {
        let state = state()
            .set_offset(ScrollOffset::new(400, 800))
            .set_effective_viewport(Size { x: 150, y: 250 });
        assert_eq!(state.content(), Size { x: 500, y: 1_000 });
        assert_eq!(state.offset(), ScrollOffset::new(350, 750));
    }

    #[test]
    fn native_range_max_combines_with_page_to_match_scroll_extent() {
        let content = 138;
        let viewport = 128;
        let n_max = scrollbar_range_max(content);
        assert_eq!(n_max.saturating_add(1).saturating_sub(viewport), 10);
        assert_eq!(scrollbar_range_max(0), 0);
    }

    #[test]
    fn native_scroll_commands_decode_position() {
        assert_eq!(
            decode_scroll_command(5, 42),
            Some(ScrollCommand::ThumbTrack(42))
        );
        assert_eq!(decode_scroll_command(99, 0), None);
    }

    #[test]
    fn partial_wheel_deltas_are_preserved() {
        let state = state()
            .scroll_wheel(Axis::Y, 60)
            .scroll_wheel(Axis::Y, 60)
            .scroll_wheel(Axis::Y, 60);

        assert_eq!(state.offset().y, 10);
    }
}
