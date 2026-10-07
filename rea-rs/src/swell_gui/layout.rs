//! Pure geometry and layout primitives for native HWND-backed controls.
//!
//! This module deliberately does not know about HWNDs. Native containers use
//! these results to position themselves and their child windows.
//! Dimensions and flow-layout coordinates are non-negative logical pixels;
//! converting native signed coordinates to these types clamps negative
//! origins to zero. Use signed native rectangles directly when negative
//! screen-space coordinates (such as monitors left of the primary display)
//! must be preserved.

use log::trace;
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
    /// Keep the preferred size on this axis; do not distribute fill space.
    Fixed,
    Fill,
    FillPortion(u32),
}

impl WidgetFills {
    const fn participates_in_fill(self) -> bool {
        !matches!(self, Self::Fixed)
    }
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

    pub const fn new(x: u32, y: u32) -> Self {
        Self::fixed(x, y)
    }

    pub const fn new_flex(
        preferred_x: u32,
        preferred_y: u32,
        fill_x: WidgetFills,
        fill_y: WidgetFills,
    ) -> Self {
        Self::Flex {
            preferred: (preferred_x, preferred_y),
            min_x: None,
            min_y: None,
            max_x: None,
            max_y: None,
            fill_x,
            fill_y,
        }
    }

    pub const fn new_fill_both(x: u32, y: u32) -> Self {
        Self::new_flex(x, y, WidgetFills::Fill, WidgetFills::Fill)
    }

    pub const fn new_fill_x(x: u32, y: u32) -> Self {
        Self::new_flex(x, y, WidgetFills::Fill, WidgetFills::Fixed)
    }

    pub const fn new_fill_y(x: u32, y: u32) -> Self {
        Self::new_flex(x, y, WidgetFills::Fixed, WidgetFills::Fill)
    }

    pub const fn set_min_x(self, value: u32) -> Self {
        self.with_bounds(Some(value), None, None, None)
    }

    pub const fn set_min_y(self, value: u32) -> Self {
        self.with_bounds(None, Some(value), None, None)
    }

    pub const fn set_max_x(self, value: u32) -> Self {
        self.with_bounds(None, None, Some(value), None)
    }

    pub const fn set_max_y(self, value: u32) -> Self {
        self.with_bounds(None, None, None, Some(value))
    }

    const fn with_bounds(
        self,
        requested_min_x: Option<u32>,
        requested_min_y: Option<u32>,
        requested_max_x: Option<u32>,
        requested_max_y: Option<u32>,
    ) -> Self {
        match self {
            Self::Fixed { x, y } => Self::Flex {
                preferred: (x, y),
                min_x: requested_min_x,
                min_y: requested_min_y,
                max_x: requested_max_x,
                max_y: requested_max_y,
                fill_x: WidgetFills::Fill,
                fill_y: WidgetFills::Fill,
            },
            Self::Flex {
                preferred,
                min_x,
                min_y,
                max_x,
                max_y,
                fill_x,
                fill_y,
            } => Self::Flex {
                preferred,
                min_x: match requested_min_x {
                    Some(value) => Some(value),
                    None => min_x,
                },
                min_y: match requested_min_y {
                    Some(value) => Some(value),
                    None => min_y,
                },
                max_x: match requested_max_x {
                    Some(value) => Some(value),
                    None => max_x,
                },
                max_y: match requested_max_y {
                    Some(value) => Some(value),
                    None => max_y,
                },
                fill_x,
                fill_y,
            },
        }
    }

    pub const fn set_fill_x(self, fill: WidgetFills) -> Self {
        match self {
            Self::Fixed { x, y } => {
                Self::new_flex(x, y, fill, WidgetFills::Fill)
            }
            Self::Flex {
                preferred,
                min_x,
                min_y,
                max_x,
                max_y,
                fill_y,
                ..
            } => Self::Flex {
                preferred,
                min_x,
                min_y,
                max_x,
                max_y,
                fill_x: fill,
                fill_y,
            },
        }
    }

    pub const fn set_fill_y(self, fill: WidgetFills) -> Self {
        match self {
            Self::Fixed { x, y } => {
                Self::new_flex(x, y, WidgetFills::Fill, fill)
            }
            Self::Flex {
                preferred,
                min_x,
                min_y,
                max_x,
                max_y,
                fill_x,
                ..
            } => Self::Flex {
                preferred,
                min_x,
                min_y,
                max_x,
                max_y,
                fill_x,
                fill_y: fill,
            },
        }
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
    Wrap,
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

    let result = PanelRects {
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
    };
    result
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

fn cross_size(item: WidgetSize, axis: Axis, available: u32) -> u32 {
    let cross_axis = axis.cross();
    let preferred = axis_value(item.preferred(), cross_axis);
    let minimum = axis_value(item.minimum(), cross_axis);
    let maximum = axis_value(item.maximum(), cross_axis);
    let fills = if cross_axis == Axis::X {
        item.fill_x()
    } else {
        item.fill_y()
    };
    match fills {
        Some(WidgetFills::Fixed) | None => preferred,
        Some(_) if maximum != u32::MAX => available.max(minimum).min(maximum),
        // Horizontal cross-axis fill tracks the parent width, which is
        // important for fill-sized containers in vertical flows. Vertical
        // cross-axis fill keeps its preferred size unless constrained, so
        // row controls do not unexpectedly stretch taller.
        Some(_) if cross_axis == Axis::X => available.max(minimum),
        Some(_) => preferred.min(available).max(minimum),
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
    let mut result = preferred.clone();
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
            if !fill.participates_in_fill() {
                return None;
            }
            Some((
                index,
                match fill {
                    WidgetFills::Fixed => unreachable!(),
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

fn assign_lanes(
    items: &[LayoutItem],
    sizes: &[u32],
    primary_limit: u32,
    axis: Axis,
    spacing: u32,
    policy: OverflowPolicy,
) -> Vec<u32> {
    let may_wrap = matches!(
        policy,
        OverflowPolicy::WrapScroll
            | OverflowPolicy::WrapClip
            | OverflowPolicy::Wrap
    );
    let mut lanes = Vec::with_capacity(items.len());
    let mut lane = 0u32;
    let mut primary = 0u32;

    for (index, item) in items.iter().enumerate() {
        let next_fill_item = items.get(index + 1).map_or(false, |next| {
            axis == Axis::Y
                && next
                    .size
                    .fill_y()
                    .is_some_and(WidgetFills::participates_in_fill)
                && !item
                    .size
                    .fill_y()
                    .is_some_and(WidgetFills::participates_in_fill)
        });
        let next_primary = sizes.get(index + 1).copied().unwrap_or(0);
        let would_leave_fill_item_out = next_fill_item
            && primary
                .saturating_add(sizes[index])
                .saturating_add(spacing)
                .saturating_add(next_primary)
                > primary_limit;

        if may_wrap
            && primary > 0
            && (primary.saturating_add(sizes[index]) > primary_limit
                || would_leave_fill_item_out)
        {
            lane = lane.saturating_add(1);
            primary = 0;
        }
        lanes.push(lane);
        primary = primary.saturating_add(sizes[index]).saturating_add(spacing);
    }
    lanes
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
    let mut sizes = distribute_primary(items, primary_limit, axis, spacing);
    let lanes =
        assign_lanes(items, &sizes, primary_limit, axis, spacing, policy);
    let lane_count = lanes.last().copied().unwrap_or(0).saturating_add(1);
    // A bounded-height container (e.g. a GroupBox) still needs its declared
    // preferred height after a preceding lane wraps. Re-run the primary-axis
    // distribution per lane; the global pre-wrap distribution can otherwise
    // starve a later fill group to a thin remainder.
    if axis == Axis::Y && lane_count > 1 {
        for lane_index in 0..lane_count {
            let lane_items: Vec<_> = items
                .iter()
                .zip(&lanes)
                .filter_map(|(item, lane)| {
                    (*lane == lane_index).then_some(*item)
                })
                .collect();
            let lane_sizes =
                distribute_primary(&lane_items, primary_limit, axis, spacing);
            let mut lane_size_index = 0;
            for (index, item_lane) in lanes.iter().enumerate() {
                if *item_lane == lane_index {
                    sizes[index] = lane_sizes[lane_size_index];
                    lane_size_index += 1;
                }
            }
        }
    }
    // A wrapped lane's cross-axis size is driven by its children, not
    // automatically by the viewport. Bounded fill controls use their maximum
    // as their intrinsic lane width; unbounded fill lanes share whatever
    // cross-axis space remains after bounded/fixed lanes.
    let mut lane_cross_sizes = vec![0; lane_count as usize];
    let mut flexible_lanes = vec![false; lane_count as usize];
    for (item, lane) in items.iter().zip(&lanes) {
        let lane_index = *lane as usize;
        let cross_axis = axis.cross();
        let fill = if cross_axis == Axis::X {
            item.size.fill_x()
        } else {
            item.size.fill_y()
        };
        let maximum = axis_value(item.size.maximum(), cross_axis);
        let unbounded_fill = fill
            .is_some_and(WidgetFills::participates_in_fill)
            && maximum == u32::MAX;
        flexible_lanes[lane_index] |= unbounded_fill;
        let required = if unbounded_fill {
            axis_value(item.size.preferred(), cross_axis)
        } else {
            cross_size(item.size, axis, cross_limit)
        };
        lane_cross_sizes[lane_index] =
            lane_cross_sizes[lane_index].max(required);
    }
    if lane_count == 1 {
        // Preserve ordinary fill behavior when no wrapping occurs: the sole
        // lane gets the viewport extent, with each child's own bounds applied
        // later by `cross_size`.
        lane_cross_sizes[0] = cross_limit;
    }
    let gaps = spacing.saturating_mul(lane_count.saturating_sub(1));
    let used_cross = lane_cross_sizes
        .iter()
        .fold(0u32, |sum, size| sum.saturating_add(*size))
        .saturating_add(gaps);
    let flexible_count =
        flexible_lanes.iter().filter(|flexible| **flexible).count();
    if flexible_count > 0 && lane_count > 1 {
        let share =
            cross_limit.saturating_sub(used_cross) / flexible_count as u32;
        let mut remainder =
            cross_limit.saturating_sub(used_cross) % flexible_count as u32;
        for (index, flexible) in flexible_lanes.iter().enumerate() {
            if *flexible {
                let extra = share + u32::from(remainder > 0);
                remainder = remainder.saturating_sub(1);
                lane_cross_sizes[index] =
                    lane_cross_sizes[index].saturating_add(extra);
            }
        }
    }
    let mut lane_cross_offsets = vec![0u32; lane_count as usize];
    for lane_index in 1..lane_count as usize {
        lane_cross_offsets[lane_index] = lane_cross_offsets[lane_index - 1]
            .saturating_add(lane_cross_sizes[lane_index - 1])
            .saturating_add(spacing);
    }
    let mut primary = 0u32;
    let mut cross = 0u32;
    let mut lane = 0u32;
    let mut used_primary = 0u32;
    let mut used_cross = 0u32;

    for (index, item) in items.iter().enumerate() {
        let target_lane = lanes[index];
        if target_lane > lane {
            lane = target_lane;
            cross = lane_cross_offsets[lane as usize];
            primary = 0;
        }
        let item_cross =
            cross_size(item.size, axis, lane_cross_sizes[lane as usize]);
        let mut item_primary = sizes[index];
        // `primary` already includes the gap after the previous item. Do not
        // add `spacing` a second time here: doing so falsely wraps an item
        // that exactly fits after the previous gap (most visible with a
        // fixed button followed by a filling edit field).
        if axis == Axis::Y
            && item
                .size
                .fill_y()
                .is_some_and(WidgetFills::participates_in_fill)
        {
            item_primary =
                item_primary.max(axis_value(item.size.minimum(), axis));
            if policy != OverflowPolicy::Scroll {
                item_primary =
                    item_primary.min(primary_limit.saturating_sub(primary));
            }
        }
        let cross_end = cross.saturating_add(item_cross);
        let primary_end = primary.saturating_add(item_primary);
        let fits_cross = cross_end <= cross_limit;
        let wraps = lane > 0;
        let allow = fits_cross
            || matches!(
                policy,
                OverflowPolicy::Scroll
                    | OverflowPolicy::WrapScroll
                    | OverflowPolicy::Wrap
            );
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
    trace!(
        target: "rea_rs::layout",
        "layout_flow: bounds={bounds:?} axis={axis:?} spacing={spacing} policy={policy:?} items={} output={output:?}",
        items.len()
    );
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
            OverflowPolicy::WrapScroll,
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
    fn fill_width_items_keep_space_in_wrapped_columns() {
        let item = LayoutItem {
            size: WidgetSize::new_fill_x(90, 40),
        };
        let output = layout_flow(
            Rect::new(0, 0, 200, 80),
            Axis::Y,
            &[item, item, item],
            0,
            OverflowPolicy::Wrap,
        );

        assert_eq!(output.placements.len(), 3);
        assert_eq!(output.placements[0].rect, Rect::new(0, 0, 100, 40));
        assert_eq!(output.placements[1].rect, Rect::new(0, 40, 100, 40));
        assert_eq!(output.placements[2].rect, Rect::new(100, 0, 100, 40));
        assert_eq!(output.content_extent.x, 200);
        assert_eq!(output.wrapped_lanes, 1);
    }

    #[test]
    fn bounded_fill_item_places_next_column_after_its_max_width() {
        let tracklist = LayoutItem {
            size: WidgetSize::new_fill_both(900, 70).set_max_x(400),
        };
        let next = LayoutItem {
            size: WidgetSize::new_fill_both(300, 100),
        };
        let output = layout_flow(
            Rect::new(0, 0, 900, 160),
            Axis::Y,
            &[tracklist, tracklist, next],
            8,
            OverflowPolicy::WrapScroll,
        );

        assert_eq!(output.placements[0].rect.width, 400);
        assert_eq!(output.placements[1].rect.width, 400);
        assert_eq!(output.placements[2].rect.x, 408);
        assert_eq!(output.placements[2].rect.width, 492);
        assert_eq!(output.wrapped_lanes, 1);
    }

    #[test]
    fn wrapped_fill_container_gets_vertical_space_in_its_lane() {
        let output = layout_flow(
            Rect::new(0, 0, 600, 180),
            Axis::Y,
            &[
                LayoutItem {
                    size: WidgetSize::fixed(300, 60),
                },
                LayoutItem {
                    size: WidgetSize::new_fill_both(300, 100),
                },
                LayoutItem {
                    size: WidgetSize::new_fill_both(300, 100),
                },
            ],
            8,
            OverflowPolicy::WrapScroll,
        );

        assert_eq!(output.wrapped_lanes, 1);
        assert_eq!(output.placements[2].rect.x, 308);
        assert_eq!(output.placements[2].rect.y, 0);
        assert_eq!(output.placements[2].rect.height, 180);
    }

    #[test]
    fn wrap_overflow_keeps_wrapped_content_extent() {
        let item = LayoutItem {
            size: WidgetSize::fixed(50, 40),
        };
        let output = layout_flow(
            Rect::new(0, 0, 100, 80),
            Axis::Y,
            &[item, item, item],
            0,
            OverflowPolicy::Wrap,
        );

        assert_eq!(output.wrapped_lanes, 1);
        assert_eq!(output.placements.len(), 3);
        assert_eq!(output.content_extent, Size { x: 100, y: 80 });
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
    fn unconstrained_cross_axis_fill_tracks_available_space() {
        let output = layout_flow(
            Rect::new(0, 0, 912, 200),
            Axis::Y,
            &[LayoutItem {
                size: WidgetSize::new_fill_both(640, 180),
            }],
            0,
            OverflowPolicy::WrapScroll,
        );

        assert_eq!(output.placements[0].rect, Rect::new(0, 0, 912, 200));
    }

    #[test]
    fn unconstrained_cross_axis_fill_shrinks_to_available_space() {
        let output = layout_flow(
            Rect::new(0, 0, 600, 200),
            Axis::Y,
            &[LayoutItem {
                size: WidgetSize::new_fill_both(640, 180),
            }],
            0,
            OverflowPolicy::WrapScroll,
        );

        assert_eq!(output.placements[0].rect, Rect::new(0, 0, 600, 200));
    }

    #[test]
    fn scroll_flow_preserves_minimum_for_fill_item_past_viewport() {
        let fixed = |width, height| LayoutItem {
            size: WidgetSize::fixed(width, height),
        };
        let list = LayoutItem {
            size: WidgetSize::new_fill_both(350, 55)
                .set_min_x(180)
                .set_min_y(40),
        };
        let output = layout_flow(
            Rect::new(0, 0, 140, 142),
            Axis::Y,
            &[
                fixed(120, 28),
                fixed(210, 28),
                fixed(180, 24),
                fixed(160, 28),
                list,
            ],
            8,
            OverflowPolicy::Scroll,
        );

        let list = output.placements.last().unwrap().rect;
        assert!(list.height >= 40);
        assert!(output.content_extent.y >= list.y + list.height);
        assert!(output.content_extent.x > 140);
    }

    #[test]
    fn fixed_fill_axis_does_not_consume_list_height() {
        let output = layout_flow(
            Rect::new(0, 0, 300, 180),
            Axis::Y,
            &[
                LayoutItem {
                    size: WidgetSize::new_flex(
                        210,
                        28,
                        WidgetFills::Fill,
                        WidgetFills::Fixed,
                    ),
                },
                LayoutItem {
                    size: WidgetSize::new_fill_both(350, 55)
                        .set_min_x(180)
                        .set_min_y(40),
                },
            ],
            8,
            OverflowPolicy::Scroll,
        );

        assert_eq!(output.placements[0].rect, Rect::new(0, 0, 300, 28));
        assert!(output.placements[1].rect.height >= 40);
        assert_eq!(output.placements[1].rect.y, 36);
    }

    #[test]
    fn widget_size_builders_update_one_flex_parameter_at_a_time() {
        let size = WidgetSize::new_flex(
            20,
            10,
            WidgetFills::FillPortion(1),
            WidgetFills::Fill,
        )
        .set_min_x(12)
        .set_min_y(8)
        .set_max_x(80)
        .set_max_y(40)
        .set_fill_y(WidgetFills::FillPortion(2));

        assert_eq!(
            size,
            WidgetSize::Flex {
                preferred: (20, 10),
                min_x: Some(12),
                min_y: Some(8),
                max_x: Some(80),
                max_y: Some(40),
                fill_x: WidgetFills::FillPortion(1),
                fill_y: WidgetFills::FillPortion(2),
            }
        );
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

    #[test]
    fn single_line_row_reports_horizontal_overflow_without_wrapping() {
        let item = LayoutItem {
            size: WidgetSize::fixed(40, 15),
        };
        let output = layout_flow(
            Rect::new(0, 0, 90, 24),
            Axis::X,
            &[item, item, item],
            8,
            OverflowPolicy::Clip,
        );

        assert_eq!(output.placements.len(), 3);
        assert_eq!(output.placements[1].rect, Rect::new(48, 0, 40, 15));
        assert_eq!(output.placements[2].rect, Rect::new(96, 0, 40, 15));
        assert_eq!(output.wrapped_lanes, 0);
        assert!(output.overflow_x);
        assert!(!output.overflow_y);
    }

    #[test]
    fn single_line_row_distributes_flex_width_and_respects_bounds() {
        let output = layout_flow(
            Rect::new(0, 0, 180, 24),
            Axis::X,
            &[
                LayoutItem {
                    size: WidgetSize::new_flex(
                        40,
                        20,
                        WidgetFills::Fill,
                        WidgetFills::Fixed,
                    )
                    .set_min_x(50)
                    .set_max_x(100),
                },
                LayoutItem {
                    size: WidgetSize::fixed(40, 20),
                },
            ],
            8,
            OverflowPolicy::Clip,
        );

        assert_eq!(output.placements[0].rect.width, 100);
        assert_eq!(output.placements[1].rect.x, 108);
        assert_eq!(output.placements[1].rect.width, 40);
    }
}
