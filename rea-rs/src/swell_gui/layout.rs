//! Pure geometry and layout primitives for native HWND-backed controls.
//!
//! This module deliberately does not know about HWNDs. Native containers use
//! these results to position themselves and their child windows.

use rea_rs_low::raw;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Size {
    pub x: u32,
    pub y: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Point {
    pub x: u32,
    pub y: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

fn native_coordinate(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

impl From<raw::POINT> for Point {
    fn from(point: raw::POINT) -> Self {
        Self {
            x: point.x.max(0) as u32,
            y: point.y.max(0) as u32,
        }
    }
}

impl From<Point> for raw::POINT {
    fn from(point: Point) -> Self {
        Self {
            x: native_coordinate(point.x),
            y: native_coordinate(point.y),
        }
    }
}

impl From<raw::RECT> for Rect {
    fn from(rect: raw::RECT) -> Self {
        let width =
            (i64::from(rect.right) - i64::from(rect.left)).max(0) as u32;
        let height =
            (i64::from(rect.bottom) - i64::from(rect.top)).max(0) as u32;
        Self {
            x: rect.left.max(0) as u32,
            y: rect.top.max(0) as u32,
            width,
            height,
        }
    }
}

impl From<Rect> for raw::RECT {
    fn from(rect: Rect) -> Self {
        let left = native_coordinate(rect.x);
        let top = native_coordinate(rect.y);
        Self {
            left,
            top,
            right: left.saturating_add(native_coordinate(rect.width)),
            bottom: top.saturating_add(native_coordinate(rect.height)),
        }
    }
}

impl Rect {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn size(self) -> Size {
        Size {
            x: self.width,
            y: self.height,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Insets {
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
}

impl Insets {
    pub const fn horizontal(self) -> u32 {
        self.left.saturating_add(self.right)
    }
    pub const fn vertical(self) -> u32 {
        self.top.saturating_add(self.bottom)
    }

    pub fn apply(self, rect: Rect) -> Rect {
        Rect {
            x: rect.x.saturating_add(self.left),
            y: rect.y.saturating_add(self.top),
            width: rect.width.saturating_sub(self.horizontal()),
            height: rect.height.saturating_sub(self.vertical()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetFills {
    Fill,
    FillPortion(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WidgetSize {
    Fixed {
        x: u32,
        y: u32,
    },
    Flex {
        preferred: (u32, u32),
        min_x: Option<u32>,
        min_y: Option<u32>,
        max_x: Option<u32>,
        max_y: Option<u32>,
        fill_x: WidgetFills,
        fill_y: WidgetFills,
    },
}

impl WidgetSize {
    pub const fn fixed(x: u32, y: u32) -> Self {
        Self::Fixed { x, y }
    }

    pub const fn preferred(self) -> Size {
        match self {
            Self::Fixed { x, y } => Size { x, y },
            Self::Flex {
                preferred: (x, y), ..
            } => Size { x, y },
        }
    }

    pub const fn minimum(self) -> Size {
        match self {
            Self::Fixed { x, y } => Size { x, y },
            Self::Flex {
                preferred: (_, _),
                min_x,
                min_y,
                ..
            } => Size {
                x: match min_x {
                    Some(value) => value,
                    None => 0,
                },
                y: match min_y {
                    Some(value) => value,
                    None => 0,
                },
            },
        }
    }

    pub const fn maximum(self) -> Size {
        match self {
            Self::Fixed { x, y } => Size { x, y },
            Self::Flex {
                preferred: (_, _),
                max_x,
                max_y,
                ..
            } => Size {
                x: match max_x {
                    Some(value) => value,
                    None => u32::MAX,
                },
                y: match max_y {
                    Some(value) => value,
                    None => u32::MAX,
                },
            },
        }
    }

    pub const fn fill_x(self) -> Option<WidgetFills> {
        match self {
            Self::Fixed { .. } => None,
            Self::Flex { fill_x, .. } => Some(fill_x),
        }
    }

    pub const fn fill_y(self) -> Option<WidgetFills> {
        match self {
            Self::Fixed { .. } => None,
            Self::Flex { fill_y, .. } => Some(fill_y),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverflowPolicy {
    WrapScroll,
    WrapClip,
    Scroll,
    Clip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

impl Axis {
    pub const fn cross(self) -> Self {
        match self {
            Self::X => Self::Y,
            Self::Y => Self::X,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Panel {
    Central,
    Top,
    Bottom,
    Left,
    Right,
}

impl Panel {
    pub const fn axis(self) -> Axis {
        match self {
            Self::Top | Self::Bottom => Axis::X,
            Self::Central | Self::Left | Self::Right => Axis::Y,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelSizes {
    pub top: u32,
    pub bottom: u32,
    pub left: u32,
    pub right: u32,
}

impl Default for PanelSizes {
    fn default() -> Self {
        Self {
            top: 0,
            bottom: 0,
            left: 0,
            right: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PanelRects {
    pub central: Rect,
    pub top: Option<Rect>,
    pub bottom: Option<Rect>,
    pub left: Option<Rect>,
    pub right: Option<Rect>,
}

impl PanelRects {
    pub const fn get(self, panel: Panel) -> Option<Rect> {
        match panel {
            Panel::Central => Some(self.central),
            Panel::Top => self.top,
            Panel::Bottom => self.bottom,
            Panel::Left => self.left,
            Panel::Right => self.right,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PanelLayout {
    pub central: Vec<LayoutItem>,
    pub top: Vec<LayoutItem>,
    pub bottom: Vec<LayoutItem>,
    pub left: Vec<LayoutItem>,
    pub right: Vec<LayoutItem>,
    pub sizes: PanelSizes,
}

impl PanelLayout {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, panel: Panel, item: LayoutItem) {
        self.items_mut(panel).push(item);
    }

    pub fn with_widget(mut self, panel: Panel, item: LayoutItem) -> Self {
        self.push(panel, item);
        self
    }

    pub fn items(&self, panel: Panel) -> &[LayoutItem] {
        match panel {
            Panel::Central => &self.central,
            Panel::Top => &self.top,
            Panel::Bottom => &self.bottom,
            Panel::Left => &self.left,
            Panel::Right => &self.right,
        }
    }

    fn items_mut(&mut self, panel: Panel) -> &mut Vec<LayoutItem> {
        match panel {
            Panel::Central => &mut self.central,
            Panel::Top => &mut self.top,
            Panel::Bottom => &mut self.bottom,
            Panel::Left => &mut self.left,
            Panel::Right => &mut self.right,
        }
    }

    pub fn allocate(&self, bounds: Rect) -> PanelRects {
        allocate_panels_for(self, bounds)
    }

    /// Lays out the children of one allocated panel inside its content area.
    ///
    /// A top or bottom panel flows horizontally; the central and side panels
    /// flow vertically. Missing optional panels produce an empty result.
    pub fn layout_panel(
        &self,
        bounds: Rect,
        panel: Panel,
        insets: Insets,
        spacing: u32,
        policy: OverflowPolicy,
    ) -> LayoutOutput {
        let Some(panel_bounds) = self.allocate(bounds).get(panel) else {
            return LayoutOutput::empty();
        };
        layout_flow(
            insets.apply(panel_bounds),
            panel.axis(),
            self.items(panel),
            spacing,
            policy,
        )
    }
}

pub fn allocate_panels(bounds: Rect, sizes: PanelSizes) -> PanelRects {
    let mut layout = PanelLayout::new();
    layout.sizes = sizes;
    layout.central.push(LayoutItem {
        size: WidgetSize::fixed(0, 0),
    });
    layout.top.push(LayoutItem {
        size: WidgetSize::fixed(0, 0),
    });
    layout.bottom.push(LayoutItem {
        size: WidgetSize::fixed(0, 0),
    });
    layout.left.push(LayoutItem {
        size: WidgetSize::fixed(0, 0),
    });
    layout.right.push(LayoutItem {
        size: WidgetSize::fixed(0, 0),
    });
    allocate_panels_for(&layout, bounds)
}

pub fn allocate_panels_for(layout: &PanelLayout, bounds: Rect) -> PanelRects {
    let top_height = if layout.top.is_empty() {
        0
    } else {
        layout.sizes.top.min(bounds.height)
    };
    let bottom_height = if layout.bottom.is_empty() {
        0
    } else {
        layout
            .sizes
            .bottom
            .min(bounds.height.saturating_sub(top_height))
    };
    let middle_height = bounds
        .height
        .saturating_sub(top_height)
        .saturating_sub(bottom_height);
    let left_width = if layout.left.is_empty() {
        0
    } else {
        layout.sizes.left.min(bounds.width)
    };
    let right_width = if layout.right.is_empty() {
        0
    } else {
        layout
            .sizes
            .right
            .min(bounds.width.saturating_sub(left_width))
    };
    let middle_width = bounds
        .width
        .saturating_sub(left_width)
        .saturating_sub(right_width);
    let middle_y = bounds.y.saturating_add(top_height);

    PanelRects {
        top: (!layout.top.is_empty())
            .then(|| Rect::new(bounds.x, bounds.y, bounds.width, top_height)),
        bottom: (!layout.bottom.is_empty()).then(|| {
            Rect::new(
                bounds.x,
                middle_y.saturating_add(middle_height),
                bounds.width,
                bottom_height,
            )
        }),
        left: (!layout.left.is_empty())
            .then(|| Rect::new(bounds.x, middle_y, left_width, middle_height)),
        right: (!layout.right.is_empty()).then(|| {
            Rect::new(
                bounds
                    .x
                    .saturating_add(left_width)
                    .saturating_add(middle_width),
                middle_y,
                right_width,
                middle_height,
            )
        }),
        central: Rect::new(
            bounds.x.saturating_add(left_width),
            middle_y,
            middle_width,
            middle_height,
        ),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Spread,
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutItem {
    pub size: WidgetSize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub index: usize,
    pub rect: Rect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutOutput {
    pub placements: Vec<Placement>,
    pub content_extent: Size,
    pub overflow_x: bool,
    pub overflow_y: bool,
    pub wrapped_lanes: u32,
    pub clipped_items: Vec<usize>,
}

impl LayoutOutput {
    fn empty() -> Self {
        Self {
            placements: Vec::new(),
            content_extent: Size::default(),
            overflow_x: false,
            overflow_y: false,
            wrapped_lanes: 0,
            clipped_items: Vec::new(),
        }
    }
}

fn axis_value(size: Size, axis: Axis) -> u32 {
    if axis == Axis::X {
        size.x
    } else {
        size.y
    }
}
fn distribute_primary(
    items: &[LayoutItem],
    available: u32,
    axis: Axis,
    spacing: u32,
) -> Vec<u32> {
    let gaps = spacing.saturating_mul(items.len().saturating_sub(1) as u32);
    let preferred: Vec<u32> = items
        .iter()
        .map(|item| axis_value(item.size.preferred(), axis))
        .collect();
    let minimum: Vec<u32> = items
        .iter()
        .map(|item| axis_value(item.size.minimum(), axis))
        .collect();
    let maximum: Vec<u32> = items
        .iter()
        .map(|item| axis_value(item.size.maximum(), axis))
        .collect();
    let base = preferred
        .iter()
        .fold(0u32, |sum, value| sum.saturating_add(*value))
        .saturating_add(gaps);
    let mut result = preferred;
    let grow = available.saturating_sub(base);
    let fills: Vec<(usize, u32)> = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            let fill = if axis == Axis::X {
                item.size.fill_x()
            } else {
                item.size.fill_y()
            }?;
            Some((
                index,
                match fill {
                    WidgetFills::Fill => 1,
                    WidgetFills::FillPortion(portion) => portion.max(1),
                },
            ))
        })
        .collect();
    let total_portion = fills
        .iter()
        .fold(0u32, |sum, (_, portion)| sum.saturating_add(*portion));
    if total_portion > 0 && grow > 0 {
        let mut assigned = 0;
        for (position, (index, portion)) in fills.iter().enumerate() {
            let amount = if position + 1 == fills.len() {
                grow.saturating_sub(assigned)
            } else {
                grow.saturating_mul(*portion) / total_portion
            };
            result[*index] = result[*index]
                .saturating_add(amount)
                .clamp(minimum[*index], maximum[*index]);
            assigned = assigned.saturating_add(amount);
        }
    }
    result
}

pub fn layout_flow(
    bounds: Rect,
    axis: Axis,
    items: &[LayoutItem],
    spacing: u32,
    policy: OverflowPolicy,
) -> LayoutOutput {
    let mut output = LayoutOutput::empty();
    if items.is_empty() {
        return output;
    }
    let primary_limit = axis_value(bounds.size(), axis);
    let cross_limit = axis_value(bounds.size(), axis.cross());
    let sizes = distribute_primary(items, primary_limit, axis, spacing);
    let mut primary = 0u32;
    let mut cross = 0u32;
    let mut lane_cross = 0u32;
    let mut lane = 0u32;
    let mut used_primary = 0u32;
    let mut used_cross = 0u32;

    for (index, item) in items.iter().enumerate() {
        let preferred = item.size.preferred();
        let item_cross = axis_value(preferred, axis.cross());
        let item_primary = sizes[index];
        if primary > 0
            && primary.saturating_add(spacing).saturating_add(item_primary)
                > primary_limit
        {
            cross = cross.saturating_add(lane_cross).saturating_add(spacing);
            lane_cross = 0;
            primary = 0;
            lane = lane.saturating_add(1);
        }
        let cross_end = cross.saturating_add(item_cross);
        let primary_end = primary.saturating_add(item_primary);
        let fits_cross = cross_end <= cross_limit;
        let wraps = lane > 0;
        let allow = fits_cross || matches!(policy, OverflowPolicy::Scroll);
        if allow {
            let mut rect = Rect::new(0, 0, 0, 0);
            if axis == Axis::X {
                rect.x = bounds.x.saturating_add(primary);
                rect.y = bounds.y.saturating_add(cross);
                rect.width = item_primary;
                rect.height = item_cross;
            } else {
                rect.x = bounds.x.saturating_add(cross);
                rect.y = bounds.y.saturating_add(primary);
                rect.width = item_cross;
                rect.height = item_primary;
            }
            output.placements.push(Placement { index, rect });
        } else {
            output.clipped_items.push(index);
        }
        if wraps
            && matches!(
                policy,
                OverflowPolicy::WrapClip | OverflowPolicy::Clip
            )
        {
            output.overflow_y |= axis == Axis::Y;
            output.overflow_x |= axis == Axis::X;
        }
        primary = primary_end.saturating_add(spacing);
        lane_cross = lane_cross.max(item_cross);
        used_primary = used_primary.max(primary_end);
        used_cross = used_cross.max(cross_end);
    }

    output.wrapped_lanes = lane;
    output.content_extent = if axis == Axis::X {
        Size {
            x: used_primary,
            y: used_cross,
        }
    } else {
        Size {
            x: used_cross,
            y: used_primary,
        }
    };
    let overflow_primary = output.content_extent != bounds.size()
        && axis_value(output.content_extent, axis) > primary_limit;
    if axis == Axis::X {
        output.overflow_x |= overflow_primary;
        output.overflow_y |= used_cross > cross_limit;
    } else {
        output.overflow_y |= overflow_primary;
        output.overflow_x |= used_cross > cross_limit;
    }
    output
}

pub fn layout_row(
    bounds: Rect,
    items: &[LayoutItem],
    spacing: u32,
    align_x: Align,
    align_y: Align,
) -> LayoutOutput {
    let mut output =
        layout_flow(bounds, Axis::X, items, spacing, OverflowPolicy::Clip);
    let total_width: u32 =
        output
            .placements
            .iter()
            .map(|placement| placement.rect.width)
            .sum::<u32>()
            .saturating_add(spacing.saturating_mul(
                output.placements.len().saturating_sub(1) as u32,
            ));
    let extra = bounds.width.saturating_sub(total_width);
    let leading = match align_x {
        Align::Start | Align::Spread => 0,
        Align::Center => extra / 2,
        Align::End => extra,
    };
    let gap = if align_x == Align::Spread && output.placements.len() > 1 {
        extra / (output.placements.len() as u32 - 1)
    } else {
        spacing
    };
    let mut x = bounds.x.saturating_add(leading);
    for placement in &mut output.placements {
        placement.rect.x = x;
        placement.rect.y = match align_y {
            Align::Start | Align::Spread => bounds.y,
            Align::Center => bounds.y.saturating_add(
                bounds.height.saturating_sub(placement.rect.height) / 2,
            ),
            Align::End => bounds.y.saturating_add(
                bounds.height.saturating_sub(placement.rect.height),
            ),
        };
        x = x.saturating_add(placement.rect.width).saturating_add(gap);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_converts_to_and_from_native_geometry() {
        let point = Point { x: 24, y: 48 };
        let native: raw::POINT = point.into();

        assert_eq!(native.x, 24);
        assert_eq!(native.y, 48);
        assert_eq!(Point::from(native), point);
    }

    #[test]
    fn native_rect_converts_endpoints_to_layout_size() {
        let rect = Rect::from(raw::RECT {
            left: 10,
            top: 20,
            right: 110,
            bottom: 80,
        });

        assert_eq!(rect, Rect::new(10, 20, 100, 60));
    }

    #[test]
    fn layout_rect_converts_size_to_native_endpoints() {
        let native: raw::RECT = Rect::new(10, 20, 100, 60).into();

        assert_eq!(native.left, 10);
        assert_eq!(native.top, 20);
        assert_eq!(native.right, 110);
        assert_eq!(native.bottom, 80);
    }

    #[test]
    fn native_negative_coordinates_are_clamped_for_layout_geometry() {
        let point = Point::from(raw::POINT { x: -10, y: -20 });
        let rect = Rect::from(raw::RECT {
            left: -10,
            top: -20,
            right: 40,
            bottom: 60,
        });

        assert_eq!(point, Point { x: 0, y: 0 });
        assert_eq!(rect, Rect::new(0, 0, 50, 80));
    }

    #[test]
    fn edge_panels_leave_central_remainder() {
        let panels = allocate_panels(
            Rect::new(0, 0, 800, 600),
            PanelSizes {
                top: 40,
                bottom: 30,
                left: 100,
                right: 120,
            },
        );
        assert_eq!(panels.top, Some(Rect::new(0, 0, 800, 40)));
        assert_eq!(panels.left, Some(Rect::new(0, 40, 100, 530)));
        assert_eq!(panels.central, Rect::new(100, 40, 580, 530));
        assert_eq!(panels.right, Some(Rect::new(680, 40, 120, 530)));
        assert_eq!(panels.bottom, Some(Rect::new(0, 570, 800, 30)));
    }

    #[test]
    fn empty_panels_are_not_allocated() {
        let mut layout = PanelLayout::new();
        layout.sizes = PanelSizes {
            top: 40,
            bottom: 30,
            left: 100,
            right: 120,
        };
        layout.push(
            Panel::Central,
            LayoutItem {
                size: WidgetSize::fixed(20, 20),
            },
        );
        let panels = layout.allocate(Rect::new(0, 0, 800, 600));
        assert_eq!(panels.central, Rect::new(0, 0, 800, 600));
        assert_eq!(panels.top, None);
        assert_eq!(panels.bottom, None);
        assert_eq!(panels.left, None);
        assert_eq!(panels.right, None);
    }

    #[test]
    fn panel_layout_applies_insets_and_panel_orientation() {
        let layout = PanelLayout::new()
            .with_widget(
                Panel::Central,
                LayoutItem {
                    size: WidgetSize::fixed(40, 15),
                },
            )
            .with_widget(
                Panel::Top,
                LayoutItem {
                    size: WidgetSize::fixed(20, 10),
                },
            )
            .with_widget(
                Panel::Top,
                LayoutItem {
                    size: WidgetSize::fixed(30, 10),
                },
            );

        let layout = PanelLayout {
            sizes: PanelSizes {
                top: 20,
                ..PanelSizes::default()
            },
            ..layout
        };

        let top = layout.layout_panel(
            Rect::new(10, 20, 100, 80),
            Panel::Top,
            Insets {
                left: 5,
                top: 2,
                right: 5,
                bottom: 3,
            },
            4,
            OverflowPolicy::Clip,
        );
        assert_eq!(top.placements[0].rect, Rect::new(15, 22, 20, 10));
        assert_eq!(top.placements[1].rect, Rect::new(39, 22, 30, 10));

        let central = layout.layout_panel(
            Rect::new(10, 20, 100, 80),
            Panel::Central,
            Insets::default(),
            0,
            OverflowPolicy::Clip,
        );
        assert_eq!(central.placements[0].rect, Rect::new(10, 40, 40, 15));
    }

    #[test]
    fn vertical_flow_wraps_into_columns() {
        let item = LayoutItem {
            size: WidgetSize::fixed(50, 40),
        };
        let output = layout_flow(
            Rect::new(0, 0, 100, 80),
            Axis::Y,
            &[item, item, item],
            0,
            OverflowPolicy::WrapClip,
        );
        assert_eq!(output.placements[0].rect, Rect::new(0, 0, 50, 40));
        assert_eq!(output.placements[1].rect, Rect::new(0, 40, 50, 40));
        assert_eq!(output.placements[2].rect, Rect::new(50, 0, 50, 40));
        assert_eq!(output.wrapped_lanes, 1);
    }

    #[test]
    fn row_distribution_centers_items() {
        let item = LayoutItem {
            size: WidgetSize::fixed(20, 10),
        };
        let output = layout_row(
            Rect::new(0, 0, 100, 20),
            &[item, item],
            0,
            Align::Center,
            Align::Center,
        );
        assert_eq!(output.placements[0].rect, Rect::new(30, 5, 20, 10));
        assert_eq!(output.placements[1].rect, Rect::new(50, 5, 20, 10));
    }

    #[test]
    fn flex_size_consumes_remaining_primary_space() {
        let output = layout_row(
            Rect::new(0, 0, 100, 20),
            &[
                LayoutItem {
                    size: WidgetSize::Flex {
                        preferred: (10, 8),
                        min_x: Some(20),
                        min_y: Some(8),
                        max_x: Some(70),
                        max_y: Some(8),
                        fill_x: WidgetFills::Fill,
                        fill_y: WidgetFills::Fill,
                    },
                },
                LayoutItem {
                    size: WidgetSize::fixed(20, 8),
                },
            ],
            5,
            Align::Start,
            Align::Start,
        );

        assert_eq!(output.placements[0].rect, Rect::new(0, 0, 70, 8));
        assert_eq!(output.placements[1].rect, Rect::new(75, 0, 20, 8));
    }

    #[test]
    fn flex_size_distributes_remaining_space_by_portion() {
        let first = LayoutItem {
            size: WidgetSize::Flex {
                preferred: (10, 10),
                min_x: None,
                min_y: None,
                max_x: None,
                max_y: None,
                fill_x: WidgetFills::FillPortion(1),
                fill_y: WidgetFills::Fill,
            },
        };
        let second = LayoutItem {
            size: WidgetSize::Flex {
                preferred: (10, 10),
                min_x: None,
                min_y: None,
                max_x: None,
                max_y: None,
                fill_x: WidgetFills::FillPortion(2),
                fill_y: WidgetFills::Fill,
            },
        };

        let output = layout_row(
            Rect::new(0, 0, 90, 20),
            &[first, second],
            0,
            Align::Start,
            Align::Start,
        );

        assert_eq!(output.placements[0].rect.width, 33);
        assert_eq!(output.placements[1].rect.width, 57);
        assert_eq!(output.placements[1].rect.x, 33);
    }

    #[test]
    fn flex_size_respects_maximum_when_growing() {
        let item = LayoutItem {
            size: WidgetSize::Flex {
                preferred: (10, 10),
                min_x: Some(5),
                min_y: None,
                max_x: Some(30),
                max_y: None,
                fill_x: WidgetFills::Fill,
                fill_y: WidgetFills::Fill,
            },
        };

        let output = layout_row(
            Rect::new(0, 0, 100, 20),
            &[item],
            0,
            Align::Start,
            Align::Start,
        );

        assert_eq!(output.placements[0].rect, Rect::new(0, 0, 30, 10));
        assert!(!output.overflow_x);
    }

    #[test]
    fn non_zero_spacing_is_preserved_between_row_items() {
        let item = LayoutItem {
            size: WidgetSize::fixed(20, 10),
        };
        let output = layout_row(
            Rect::new(10, 5, 100, 20),
            &[item, item, item],
            7,
            Align::Start,
            Align::Start,
        );

        assert_eq!(output.placements[0].rect, Rect::new(10, 5, 20, 10));
        assert_eq!(output.placements[1].rect, Rect::new(37, 5, 20, 10));
        assert_eq!(output.placements[2].rect, Rect::new(64, 5, 20, 10));
    }

    #[test]
    fn non_zero_spacing_is_used_when_flow_wraps() {
        let item = LayoutItem {
            size: WidgetSize::fixed(40, 15),
        };
        let output = layout_flow(
            Rect::new(10, 20, 90, 50),
            Axis::X,
            &[item, item, item],
            5,
            OverflowPolicy::WrapClip,
        );

        assert_eq!(output.placements[0].rect, Rect::new(10, 20, 40, 15));
        assert_eq!(output.placements[1].rect, Rect::new(55, 20, 40, 15));
        assert_eq!(output.placements[2].rect, Rect::new(10, 40, 40, 15));
        assert_eq!(output.wrapped_lanes, 1);
    }
}
