//! Layout computation for skim's TUI.
//!
//! The layout logic is split into two phases so that the expensive option-
//! inspection work is done only once (at startup and on option changes) while
//! the per-frame cost is reduced to a small number of cheap rect splits.
//!
//! * [`LayoutTemplate`] — built from [`SkimOptions`] and the static header
//!   height.  Stores pre-computed constraints and flags; no terminal-size
//!   dependency.  Stored on [`App`](super::App) and rebuilt only when options
//!   that affect layout change (e.g. [`TogglePreview`]).
//!
//! * [`AppLayout`] — produced by [`LayoutTemplate::apply`] from a concrete
//!   terminal [`Rect`].  Contains the final widget areas for one render frame.
//!   Computed in `render()` and cached on `App` so that code between renders
//!   (e.g. mouse hit-testing) can read it.

use ratatui::layout::{Constraint, Direction as RatatuiDirection, Layout, Rect};

use crate::SkimOptions;
use crate::tui::options::TuiLayout;
use crate::tui::statusline::InfoDisplay;
use crate::tui::{Direction, Size};

// ---------------------------------------------------------------------------
// LayoutTemplate
// ---------------------------------------------------------------------------

/// Orientation of the preview pane relative to the work area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewPlacement {
    /// Preview splits the full area horizontally (left / right of everything).
    Left,
    Right,
    /// Preview splits the full area vertically (above / below everything).
    Up,
    Down,
    /// No preview.
    None,
}

/// Pre-computed layout descriptor built from [`SkimOptions`].
///
/// Contains everything needed to split a concrete [`Rect`] into widget areas,
/// but contains no coordinates itself — those only appear in [`AppLayout`].
/// Build with [`LayoutTemplate::from_options`] and store on
/// [`App`](super::App); call [`LayoutTemplate::apply`] in every render.
#[derive(Debug, Clone)]
pub struct LayoutTemplate {
    /// Whether the header widget should be rendered at all.
    show_header: bool,
    /// Where the preview pane is placed relative to the rest of the UI.
    preview_placement: PreviewPlacement,
    /// Whether the work layout emits slots in reverse order `[input, header,
    /// list]` instead of the default `[list, header, input]`.
    work_layout_reversed: bool,
    /// Pre-built [`Layout`] for carving the preview out of the full area
    /// (step 1).  `None` when no preview is visible.
    preview_layout: Option<Layout>,
    /// Pre-built [`Layout`] for splitting the work area into three slots.
    ///
    /// When `work_layout_reversed` is `false` the slots map to
    /// `[list, header, input]`; when `true` they map to
    /// `[input, header, list]`.  Applied in step 2 of [`Self::apply`].
    work_layout: Layout,
}

impl LayoutTemplate {
    /// Build a [`LayoutTemplate`] from [`SkimOptions`] and the static header
    /// height (content rows only, excluding any border rows).
    ///
    /// `header_height` should come from
    /// [`Header::height()`](super::header::Header::height), which returns a
    /// value fixed at construction time from `options.header_lines` and the
    /// line-count of `options.header`.
    #[must_use]
    pub fn from_options(options: &SkimOptions, header_height: u16) -> Self {
        let has_border = options.border.is_some();

        // Rows consumed by the input widget.
        let input_rows: u16 = if has_border {
            3 // 1 content + 2 border rows
        } else {
            1 + u16::from(options.info.display == InfoDisplay::Default)
        };

        // Rows consumed by the header widget.
        let show_header = options.header.is_some() || options.header_lines > 0;
        let header_rows: u16 = if show_header {
            if has_border { header_height + 2 } else { header_height }
        } else {
            0
        };

        // Preview placement and layout.
        let preview_visible = (options.preview.is_some() || options.preview_fn.is_some())
            && !options.preview_window.hidden
            && !matches!(options.preview_window.size, Size::Fixed(0));

        let (preview_placement, preview_layout) = if preview_visible {
            let (preview_c, rest_c) = size_to_constraint(options.preview_window.size);
            let placement = match options.preview_window.direction {
                Direction::Left => PreviewPlacement::Left,
                Direction::Right => PreviewPlacement::Right,
                Direction::Up => PreviewPlacement::Up,
                Direction::Down => PreviewPlacement::Down,
            };
            let layout = match placement {
                PreviewPlacement::Left => Layout::new(RatatuiDirection::Horizontal, [preview_c, rest_c]),
                PreviewPlacement::Right => Layout::new(RatatuiDirection::Horizontal, [rest_c, preview_c]),
                PreviewPlacement::Up => Layout::new(RatatuiDirection::Vertical, [preview_c, rest_c]),
                PreviewPlacement::Down => Layout::new(RatatuiDirection::Vertical, [rest_c, preview_c]),
                PreviewPlacement::None => unreachable!(),
            };
            (placement, Some(layout))
        } else {
            (PreviewPlacement::None, None)
        };

        // Work-area layout: single 3-way vertical split into [list, header, input].
        //
        // For Default / ReverseList: slots are [list, header, input] top-to-bottom.
        // For Reverse:               slots are [input, header, list] top-to-bottom.
        let non_list_rows = input_rows + header_rows;
        let work_layout_reversed = options.layout == TuiLayout::Reverse;
        let work_layout = if show_header {
            match options.layout {
                TuiLayout::Default | TuiLayout::ReverseList => Layout::vertical([
                    Constraint::Fill(1),
                    Constraint::Length(header_rows),
                    Constraint::Length(input_rows),
                ]),
                TuiLayout::Reverse => Layout::vertical([
                    Constraint::Length(input_rows),
                    Constraint::Length(header_rows),
                    Constraint::Fill(1),
                ]),
            }
        } else {
            match options.layout {
                TuiLayout::Default | TuiLayout::ReverseList => Layout::vertical([
                    Constraint::Fill(1),
                    Constraint::Length(0),
                    Constraint::Length(non_list_rows),
                ]),
                TuiLayout::Reverse => Layout::vertical([
                    Constraint::Length(non_list_rows),
                    Constraint::Length(0),
                    Constraint::Fill(1),
                ]),
            }
        };

        Self {
            show_header,
            preview_placement,
            work_layout_reversed,
            preview_layout,
            work_layout,
        }
    }

    /// Apply this template to a concrete terminal `area`, producing the
    /// absolute [`AppLayout`] for one render frame.
    #[must_use]
    pub fn apply(&self, area: Rect) -> AppLayout {
        // ── Step 1: carve out the preview from the full area ─────────────────
        let (work_area, preview_area): (Rect, Option<Rect>) = match &self.preview_layout {
            Some(layout) => {
                let [a, b]: [Rect; 2] = layout.areas(area);
                match self.preview_placement {
                    // preview is the first segment for Left / Up
                    PreviewPlacement::Left | PreviewPlacement::Up => (b, Some(a)),
                    // preview is the second segment for Right / Down
                    _ => (a, Some(b)),
                }
            }
            None => (area, None),
        };

        // ── Step 2: split work_area into list / header / input in one pass ───
        //
        // Slots are [list, header, input] when `work_layout_reversed` is false,
        // or [input, header, list] when true (Reverse layout).
        let [slot0, slot1, slot2]: [Rect; 3] = self.work_layout.areas(work_area);

        let (list_area, header_slot, input_area) = if self.work_layout_reversed {
            (slot2, slot1, slot0)
        } else {
            (slot0, slot1, slot2)
        };

        let header_area = if self.show_header { Some(header_slot) } else { None };

        AppLayout {
            list_area,
            input_area,
            header_area,
            preview_area,
        }
    }
}

// ---------------------------------------------------------------------------
// AppLayout
// ---------------------------------------------------------------------------

/// Concrete widget areas for one render frame, produced by
/// [`LayoutTemplate::apply`].
///
/// Cached on [`App`](super::App) after each render so that code between frames
/// (e.g. mouse hit-testing in `handle_mouse`) can read the last known areas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppLayout {
    /// Area for the item list widget.
    pub list_area: Rect,
    /// Area for the input / prompt widget.
    pub input_area: Rect,
    /// Area for the header widget (`None` when no header is shown).
    pub header_area: Option<Rect>,
    /// Area for the preview pane (`None` when preview is hidden or disabled).
    pub preview_area: Option<Rect>,
}

impl AppLayout {
    /// Convenience wrapper: build a [`LayoutTemplate`] from `options` and
    /// `header_height`, then immediately apply it to `area`.
    ///
    /// Prefer storing the [`LayoutTemplate`] and calling
    /// [`LayoutTemplate::apply`] directly when the template can be reused
    /// across frames.
    #[must_use]
    pub fn compute(area: Rect, options: &SkimOptions, header_height: u16) -> Self {
        LayoutTemplate::from_options(options, header_height).apply(area)
    }
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn size_to_constraint(size: Size) -> (Constraint, Constraint) {
    match size {
        Size::Fixed(n) => (Constraint::Length(n), Constraint::Fill(1)),
        Size::Percent(p) => (Constraint::Percentage(p), Constraint::Fill(1)),
        Size::Neg(n) => (Constraint::Fill(1), Constraint::Length(n)),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
