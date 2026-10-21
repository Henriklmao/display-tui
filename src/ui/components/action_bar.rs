//! Clickable action bar in the display list's footer.
//!
//! The bar replaces the old keybind text with buttons that dispatch exactly the
//! key they name. The renderer and the mouse hit-test share [`resolve`], so a
//! click always lands on the button that was drawn. Range hints (`0-9`, `k/j`,
//! …) stay pure documentation and are deliberately not clickable.

use crate::App;
use crate::app::state::HoverTarget;
use crate::ui::styles;
use crate::utils::TUIMode;
use crossterm::event::KeyCode;
#[cfg(test)]
use ratatui::widgets::Block;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

// Terminal width below which the list shows its compact footer set.
pub const MINI_MODE_WIDTH: u16 = 115;

// An action a footer button performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionId {
    Move,
    Resolution,
    Scale,
    Rotate,
    ToggleEnabled,
    Presets,
    Save,
    Quit,
    Help,
    Apply,
    ApplyPending,
    Cancel,
    CancelAll,
    Done,
}

impl ActionId {
    // The key the button stands for. Dispatching it through the normal key
    // handler keeps mouse and keyboard behaviour identical.
    pub fn key(self, enabled: bool) -> KeyCode {
        match self {
            ActionId::Move => KeyCode::Char('m'),
            ActionId::Resolution => KeyCode::Char('r'),
            ActionId::Scale => KeyCode::Char('s'),
            ActionId::Rotate => KeyCode::Char('o'),
            ActionId::ToggleEnabled if enabled => KeyCode::Char('d'),
            ActionId::ToggleEnabled => KeyCode::Char('e'),
            ActionId::Presets => KeyCode::Char('p'),
            ActionId::Save => KeyCode::Char('w'),
            ActionId::Quit => KeyCode::Char('q'),
            ActionId::Help => KeyCode::Char('K'),
            ActionId::Apply | ActionId::ApplyPending => KeyCode::Enter,
            ActionId::Cancel | ActionId::CancelAll | ActionId::Done => KeyCode::Esc,
        }
    }

    // The verb shown inside the button.
    pub fn label(self, enabled: bool) -> &'static str {
        match self {
            ActionId::Move => "Move",
            ActionId::Resolution => "Resolution",
            ActionId::Scale => "Scale",
            ActionId::Rotate => "Rotate",
            ActionId::ToggleEnabled if enabled => "Disable",
            ActionId::ToggleEnabled => "Enable",
            ActionId::Presets => "Presets",
            ActionId::Save => "Save",
            ActionId::Quit => "Quit",
            ActionId::Help => "Help",
            ActionId::Apply | ActionId::ApplyPending => "Apply",
            ActionId::Cancel => "Cancel",
            ActionId::CancelAll => "Cancel all",
            ActionId::Done => "Done",
        }
    }

    // The key label drawn on the right of the verb.
    pub fn key_label(self, enabled: bool) -> &'static str {
        match self {
            ActionId::Move => "m",
            ActionId::Resolution => "r",
            ActionId::Scale => "s",
            ActionId::Rotate => "o",
            ActionId::ToggleEnabled if enabled => "d",
            ActionId::ToggleEnabled => "e",
            ActionId::Presets => "p",
            ActionId::Save => "w",
            ActionId::Quit => "q",
            ActionId::Help => "K",
            ActionId::Apply | ActionId::ApplyPending => "Enter",
            ActionId::Cancel | ActionId::CancelAll | ActionId::Done => "Esc",
        }
    }

    // The key shown when the bar has to shrink a button to its key alone.
    fn compact_key(self, enabled: bool) -> &'static str {
        match self {
            ActionId::Apply | ActionId::ApplyPending => "\u{21b5}", // ↵
            other => other.key_label(enabled),
        }
    }

    // The one-line tooltip for the button.
    pub fn description(self) -> &'static str {
        match self {
            ActionId::Move => "Switch to move mode",
            ActionId::Resolution => "Choose a resolution",
            ActionId::Scale => "Choose a scale",
            ActionId::Rotate => "Rotate the selected display",
            ActionId::ToggleEnabled => "Enable or disable the selected display",
            ActionId::Presets => "Open the preset menu",
            ActionId::Save => "Validate and save the configuration",
            ActionId::Quit => "Quit and save the monitor state",
            ActionId::Help => "Show the keybindings help",
            ActionId::Apply => "Apply the highlighted value",
            ActionId::ApplyPending => "Apply every pending move; right-click too",
            ActionId::Cancel => "Cancel and return to view mode",
            ActionId::CancelAll => "Revert every pending move",
            ActionId::Done => "Leave move mode",
        }
    }
}

// The pending-move footer's movement hints. Arrow keys mirror h/j/k/l, so a
// mouse drag can be finished on the keyboard while Enter/Esc still apply or
// revert the move. They stay plain text (hints, not buttons).
const SNAP_MOVE_HINT: &str = "<h/j/k/l/\u{2191}\u{2193}\u{2190}\u{2192}>";
const FAST_MOVE_HINT: &str = "<H/J/K/L>";

// Which footer set the current view shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionBarContext {
    pub mode: TUIMode,
    pub pending: bool,
    pub has_monitors: bool,
    pub selected_enabled: bool,
}

impl ActionBarContext {
    // Reads the same state the renderer draws from.
    pub fn from_app(app: &App) -> Self {
        ActionBarContext {
            mode: app.mode,
            pending: app.has_pending_move(),
            has_monitors: !app.monitors.is_empty(),
            selected_enabled: app
                .monitors
                .get(app.selected_monitor)
                .is_some_and(|monitor| monitor.enabled),
        }
    }
}

// A clickable button placed in the bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionButton {
    pub action: ActionId,
    pub rect: Rect,
}

// A laid-out item, including the text still to be drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LaidOutItem {
    rect: Rect,
    action: Option<ActionId>,
    spans: Vec<Span<'static>>,
    // The full chip text, used to repaint the whole button on hover.
    hover_label: Option<String>,
}

// The footer row and the clickable buttons inside it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ActionBar {
    pub area: Rect,
    pub buttons: Vec<ActionButton>,
    items: Vec<LaidOutItem>,
}

impl ActionBar {
    // The action under `point`, if a button was drawn there.
    pub fn button_at(&self, point: (u16, u16)) -> Option<ActionId> {
        self.buttons
            .iter()
            .find(|button| inside(button.rect, point))
            .map(|button| button.action)
    }

    // Whether `point` is inside the footer row at all. Footer clicks must not
    // fall through to the display rows behind it.
    pub fn contains(&self, point: (u16, u16)) -> bool {
        inside(self.area, point)
    }
}

// Half-open containment so adjacent rects never both claim a cell.
fn inside(rect: Rect, point: (u16, u16)) -> bool {
    point.0 >= rect.x && point.0 < rect.right() && point.1 >= rect.y && point.1 < rect.bottom()
}

// One footer item before layout: static text plus the precomputed widths of
// its full and compact forms. Nothing is allocated here, so a mouse move can
// run the hit-test without formatting a single chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ItemSpec {
    action: Option<ActionId>,
    priority: u8,
    label: &'static str,
    key: &'static str,
    compact_key: &'static str,
    full_width: usize,
    compact_width: Option<usize>,
}

// Cell width of static text, matching what ratatui draws.
fn text_width(text: &str) -> usize {
    Span::raw(text).width()
}

impl ItemSpec {
    const EMPTY: ItemSpec = ItemSpec {
        action: None,
        priority: 0,
        label: "",
        key: "",
        compact_key: "",
        full_width: 0,
        compact_width: None,
    };

    fn width(self, compact: bool) -> usize {
        if compact {
            self.compact_width.unwrap_or(self.full_width)
        } else {
            self.full_width
        }
    }
}

const MAX_ITEMS: usize = 16;
const MAX_PLACEMENTS: usize = MAX_ITEMS;

// Fixed-capacity staging buffer for the footer set, so building it does not
// allocate either.
struct ItemList {
    items: [ItemSpec; MAX_ITEMS],
    len: usize,
}

impl ItemList {
    fn new() -> Self {
        ItemList {
            items: [ItemSpec::EMPTY; MAX_ITEMS],
            len: 0,
        }
    }

    fn push(&mut self, item: ItemSpec) {
        assert!(self.len < MAX_ITEMS, "the footer outgrew MAX_ITEMS");
        if self.len < MAX_ITEMS {
            self.items[self.len] = item;
            self.len += 1;
        }
    }
}

// An item while the bar is being fitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedItem {
    spec: ItemSpec,
    compact: bool,
    removed: bool,
}

impl ResolvedItem {
    const EMPTY: ResolvedItem = ResolvedItem {
        spec: ItemSpec::EMPTY,
        compact: false,
        removed: false,
    };

    fn width(self) -> usize {
        if self.removed {
            0
        } else {
            self.spec.width(self.compact)
        }
    }
}

// One visible item after placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Placement {
    rect: Rect,
    spec: ItemSpec,
    compact: bool,
}

impl Placement {
    const EMPTY: Placement = Placement {
        rect: Rect::new(0, 0, 0, 0),
        spec: ItemSpec::EMPTY,
        compact: false,
    };
}

// Builds a button item. `label` can override the action's default verb (the
// compact view calls the help button "Keybinds").
fn button(action: ActionId, priority: u8, enabled: bool) -> ItemSpec {
    button_with_label(action, action.label(enabled), priority, enabled)
}

fn button_with_label(
    action: ActionId,
    label: &'static str,
    priority: u8,
    enabled: bool,
) -> ItemSpec {
    let key = action.key_label(enabled);
    let compact_key = action.compact_key(enabled);
    ItemSpec {
        action: Some(action),
        priority,
        label,
        key,
        compact_key,
        full_width: text_width(label) + text_width(key) + 6,
        compact_width: Some(text_width(compact_key) + 2),
    }
}

// Builds a range hint that stays text and cannot be clicked.
fn hint(priority: u8, label: &'static str, key: &'static str) -> ItemSpec {
    ItemSpec {
        action: None,
        priority,
        label,
        key,
        compact_key: "",
        full_width: text_width(label) + text_width(key) + 1,
        compact_width: None,
    }
}

// The footer set for the current mode and state.
fn items(context: ActionBarContext, mini: bool) -> ItemList {
    let enabled = context.selected_enabled;
    let mut list = ItemList::new();

    if context.pending && context.mode == TUIMode::View {
        list.push(hint(0, "Fastmove", FAST_MOVE_HINT));
        list.push(hint(0, "Snapmove", SNAP_MOVE_HINT));
        list.push(button(ActionId::ApplyPending, 4, enabled));
        list.push(button(ActionId::CancelAll, 4, enabled));
        return list;
    }

    match context.mode {
        TUIMode::View if mini => {
            list.push(button(ActionId::Save, 4, enabled));
            list.push(button(ActionId::Presets, 2, enabled));
            list.push(button(ActionId::Quit, 4, enabled));
            list.push(button_with_label(ActionId::Help, "Keybinds", 4, enabled));
        }
        TUIMode::View => {
            list.push(hint(0, "Scroll \u{f431}\u{f433}", "<k/j>"));
            list.push(button(ActionId::Move, 3, enabled));
            list.push(button(ActionId::Resolution, 3, enabled));
            list.push(button(ActionId::Scale, 3, enabled));
            list.push(button(ActionId::Rotate, 1, enabled));
            list.push(hint(0, "Workspace", "<0-9>"));
            if context.has_monitors {
                list.push(button(ActionId::ToggleEnabled, 3, enabled));
            }
            list.push(button(ActionId::Presets, 2, enabled));
            list.push(button(ActionId::Save, 4, enabled));
            list.push(button(ActionId::Quit, 4, enabled));
            list.push(button(ActionId::Help, 4, enabled));
        }
        TUIMode::Move => {
            list.push(hint(0, "Fastmove", FAST_MOVE_HINT));
            list.push(hint(0, "Snapmove", "<h/j/k/l>"));
            list.push(button(ActionId::Save, 4, enabled));
            list.push(button(ActionId::Done, 4, enabled));
        }
        TUIMode::Resolution | TUIMode::Scale => {
            list.push(hint(0, "Navigate", "<k/j>"));
            list.push(button(ActionId::Apply, 4, enabled));
            list.push(button(ActionId::Save, 4, enabled));
            list.push(button(ActionId::Cancel, 4, enabled));
        }
    }

    list
}

// Total drawn width, including one space between adjacent items.
fn total_width(items: &[ResolvedItem]) -> usize {
    let mut count = 0usize;
    let mut width = 0usize;
    for item in items.iter().filter(|item| !item.removed) {
        count += 1;
        width += item.width();
    }
    width + count.saturating_sub(1)
}

// Shrinks the bar until it fits: the lowest-priority item is compacted first,
// then removed. Whole items only, so no label is ever drawn half a chip.
fn fit(items: &mut [ResolvedItem], available: usize) {
    while total_width(items) > available {
        let Some(index) = items
            .iter()
            .enumerate()
            .filter(|(_, item)| !item.removed)
            .min_by_key(|(_, item)| (item.spec.priority, item.compact))
            .map(|(index, _)| index)
        else {
            break;
        };

        if !items[index].compact && items[index].spec.compact_width.is_some() {
            items[index].compact = true;
            if total_width(items) <= available {
                break;
            }
        }

        items[index].removed = true;
    }
}

// Fits and places the footer set. The lightweight hit-test and the renderer
// both use this, so their geometry cannot diverge.
fn resolve(
    footer: Rect,
    context: ActionBarContext,
    list_width: u16,
) -> ([Placement; MAX_PLACEMENTS], usize) {
    if footer.width == 0 || footer.height == 0 {
        return ([Placement::EMPTY; MAX_PLACEMENTS], 0);
    }

    let mini = list_width < MINI_MODE_WIDTH;
    let list = items(context, mini);
    let mut resolved = [ResolvedItem::EMPTY; MAX_ITEMS];
    for (index, spec) in list.items[..list.len].iter().enumerate() {
        resolved[index] = ResolvedItem {
            spec: *spec,
            compact: false,
            removed: false,
        };
    }
    let resolved = &mut resolved[..list.len];

    fit(resolved, footer.width as usize);

    // The pending-move confirm state is a pair: offering Apply without its
    // Cancel (or vice versa) is worse than showing no button at all.
    if context.pending && context.mode == TUIMode::View {
        let remains = |action: ActionId| {
            resolved
                .iter()
                .any(|item| !item.removed && item.spec.action == Some(action))
        };
        if remains(ActionId::ApplyPending) != remains(ActionId::CancelAll) {
            for item in resolved.iter_mut() {
                if matches!(
                    item.spec.action,
                    Some(ActionId::ApplyPending | ActionId::CancelAll)
                ) {
                    item.removed = true;
                }
            }
        }
    }

    let total = total_width(resolved).min(footer.width as usize) as u16;
    let mut x = footer.x + footer.width.saturating_sub(total) / 2;
    let mut placements = [Placement::EMPTY; MAX_PLACEMENTS];
    let mut len = 0;
    let mut first = true;

    for item in resolved.iter().filter(|item| !item.removed) {
        if !first {
            x = x.saturating_add(1);
        }
        first = false;

        let width = (item.width() as u16).min(footer.right().saturating_sub(x));
        if width == 0 {
            break;
        }
        placements[len] = Placement {
            rect: Rect {
                x,
                y: footer.y,
                width,
                height: 1,
            },
            spec: item.spec,
            compact: item.compact,
        };
        len += 1;
        x = x.saturating_add(width);
    }

    (placements, len)
}

// The full chip text, used to repaint a button on hover.
fn button_label(spec: ItemSpec, compact: bool) -> String {
    if compact {
        format!("[{}]", spec.compact_key)
    } else {
        format!("[ {}  {} ]", spec.label, spec.key)
    }
}

// The styled spans for one item. Called only for visible items during a draw.
fn item_spans(spec: ItemSpec, compact: bool) -> Vec<Span<'static>> {
    if spec.action.is_some() {
        if compact {
            vec![Span::styled(
                button_label(spec, true),
                Style::default().fg(Color::Blue).bold(),
            )]
        } else {
            vec![
                Span::styled(
                    format!("[ {}  ", spec.label),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    spec.key.to_string(),
                    Style::default().fg(Color::Blue).bold(),
                ),
                Span::styled(" ]", Style::default().fg(Color::White)),
            ]
        }
    } else {
        vec![
            Span::styled(
                format!("{} ", spec.label),
                Style::default().fg(Color::White),
            ),
            Span::styled(
                spec.key.to_string(),
                Style::default().fg(Color::Blue).bold(),
            ),
        ]
    }
}

// The lightweight hit map used by the mouse. It contains the exact button
// rects but no text, so hovering does not format a single chip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionBarHits {
    pub area: Rect,
    buttons: [Option<ActionButton>; MAX_ITEMS],
    len: usize,
}

impl Default for ActionBarHits {
    fn default() -> Self {
        ActionBarHits {
            area: Rect::default(),
            buttons: [None; MAX_ITEMS],
            len: 0,
        }
    }
}

impl ActionBarHits {
    // The action under `point`, if a button was placed there.
    pub fn button_at(&self, point: (u16, u16)) -> Option<ActionId> {
        self.buttons[..self.len]
            .iter()
            .flatten()
            .find(|button| inside(button.rect, point))
            .map(|button| button.action)
    }

    // Whether `point` is inside the footer row at all.
    pub fn contains(&self, point: (u16, u16)) -> bool {
        inside(self.area, point)
    }
}

impl ActionBar {
    // Test helper: lays the bar out from a whole list area. Production uses the
    // exact footer row from `MonitorList::footer_rect` via `layout_in`.
    #[cfg(test)]
    pub fn layout(area: Rect, context: ActionBarContext) -> Self {
        let inner = Block::bordered().inner(area);
        if inner.width == 0 || inner.height == 0 {
            return ActionBar::default();
        }

        let footer = Rect {
            x: inner.x,
            y: inner.bottom() - 1,
            width: inner.width,
            height: 1,
        };
        ActionBar::layout_in(footer, context, area.width)
    }

    // Lays the bar out in an explicit footer row.
    pub fn layout_in(footer: Rect, context: ActionBarContext, list_width: u16) -> Self {
        if footer.width == 0 || footer.height == 0 {
            return ActionBar::default();
        }

        let (placements, count) = resolve(footer, context, list_width);
        let mut buttons = Vec::new();
        let mut items = Vec::new();

        for placement in &placements[..count] {
            if let Some(action) = placement.spec.action {
                buttons.push(ActionButton {
                    action,
                    rect: placement.rect,
                });
            }
            items.push(LaidOutItem {
                rect: placement.rect,
                action: placement.spec.action,
                spans: item_spans(placement.spec, placement.compact),
                hover_label: placement
                    .spec
                    .action
                    .map(|_| button_label(placement.spec, placement.compact)),
            });
        }

        ActionBar {
            area: footer,
            buttons,
            items,
        }
    }

    // Allocation-free hit map for the mouse. Geometry is shared with
    // `layout_in` through `resolve`.
    pub fn hit_test_in(footer: Rect, context: ActionBarContext, list_width: u16) -> ActionBarHits {
        if footer.width == 0 || footer.height == 0 {
            return ActionBarHits::default();
        }

        let (placements, count) = resolve(footer, context, list_width);
        let mut hits = ActionBarHits {
            area: footer,
            ..ActionBarHits::default()
        };
        for placement in &placements[..count] {
            if let Some(action) = placement.spec.action
                && hits.len < MAX_ITEMS
            {
                hits.buttons[hits.len] = Some(ActionButton {
                    action,
                    rect: placement.rect,
                });
                hits.len += 1;
            }
        }
        hits
    }

    // Test helper: draws the bar in the list area's last row.
    #[cfg(test)]
    pub fn render(
        area: Rect,
        buf: &mut Buffer,
        context: ActionBarContext,
        hover: Option<HoverTarget>,
    ) -> Self {
        let frame = ActionBar::layout(area, context);
        paint(&frame, buf, hover);
        frame
    }

    // Draws the bar in an explicit footer row.
    pub fn render_in(
        footer: Rect,
        buf: &mut Buffer,
        context: ActionBarContext,
        hover: Option<HoverTarget>,
        list_width: u16,
    ) -> Self {
        let frame = ActionBar::layout_in(footer, context, list_width);
        paint(&frame, buf, hover);
        frame
    }
}

// Paints every laid-out item, inverting the hovered button.
fn paint(frame: &ActionBar, buf: &mut Buffer, hover: Option<HoverTarget>) {
    for item in &frame.items {
        let hovered = item
            .action
            .is_some_and(|action| hover == Some(HoverTarget::Action(action)));
        if hovered {
            let text = item.hover_label.clone().unwrap_or_default();
            Paragraph::new(Line::from(text).style(styles::hover_style().reversed()))
                .render(item.rect, buf);
        } else {
            Paragraph::new(Line::from(item.spans.clone())).render(item.rect, buf);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::tests::{find_text, text_of};

    fn context(mode: TUIMode, pending: bool) -> ActionBarContext {
        ActionBarContext {
            mode,
            pending,
            has_monitors: true,
            selected_enabled: true,
        }
    }

    fn area(width: u16) -> Rect {
        Rect::new(0, 0, width, 10)
    }

    fn labels(frame: &ActionBar, buf: &Buffer) -> String {
        let row = frame.area.y;
        (frame.area.x..frame.area.right())
            .map(|x| buf[(x, row)].symbol())
            .collect::<String>()
            .trim()
            .to_string()
    }

    #[test]
    fn every_button_has_a_drawn_label_and_a_clickable_rect() {
        for width in [120u16, 160, 240] {
            let screen = area(width);
            let mut buf = Buffer::empty(screen);
            let frame = ActionBar::render(screen, &mut buf, context(TUIMode::View, false), None);

            for button in &frame.buttons {
                assert!(
                    button.rect.width > 0,
                    "{} has no width at {width}",
                    button.action.label(true)
                );
                let drawn: String = (button.rect.x..button.rect.right())
                    .map(|x| buf[(x, button.rect.y)].symbol())
                    .collect();
                assert!(
                    drawn.starts_with('[') && drawn.ends_with(']'),
                    "{:?} is not a whole chip at {width}: {drawn:?}",
                    button.action
                );
                assert!(
                    drawn.contains(button.action.key_label(true))
                        || drawn.contains(button.action.compact_key(true)),
                    "{:?} does not show its key at {width}: {drawn:?}",
                    button.action
                );
            }
        }
    }

    #[test]
    fn buttons_never_overlap_and_stay_inside_the_footer() {
        for mode in [
            TUIMode::View,
            TUIMode::Move,
            TUIMode::Resolution,
            TUIMode::Scale,
        ] {
            for pending in [false, true] {
                for width in [20u16, 40, 60, 80, 100, 114, 115, 120, 130, 160, 240] {
                    let screen = area(width);
                    let frame = ActionBar::layout(screen, context(mode, pending));

                    for button in &frame.buttons {
                        assert!(
                            button.rect.x >= frame.area.x
                                && button.rect.right() <= frame.area.right()
                                && button.rect.y == frame.area.y
                                && button.rect.height == 1,
                            "{mode:?}/{pending} at {width}: {button:?} escapes {:?}",
                            frame.area
                        );
                    }
                    for pair in frame.buttons.windows(2) {
                        assert!(
                            pair[0].rect.right() <= pair[1].rect.x,
                            "{mode:?}/{pending} at {width}: {:?} overlaps {:?}",
                            pair[0],
                            pair[1]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn an_absent_footer_row_draws_nothing() {
        let frame = ActionBar::layout_in(Rect::new(0, 0, 80, 0), context(TUIMode::View, false), 80);

        assert!(frame.buttons.is_empty());
        assert!(!frame.contains((0, 0)));
    }

    #[test]
    fn pending_moves_offer_only_apply_and_cancel() {
        let screen = area(120);
        let mut buf = Buffer::empty(screen);
        let frame = ActionBar::render(screen, &mut buf, context(TUIMode::View, true), None);
        let actions: Vec<ActionId> = frame.buttons.iter().map(|button| button.action).collect();

        assert_eq!(actions, vec![ActionId::ApplyPending, ActionId::CancelAll]);
        let text = labels(&frame, &buf);
        assert!(text.contains("Apply"), "got: {text}");
        assert!(text.contains("Cancel all"), "got: {text}");
        assert!(
            text.contains(SNAP_MOVE_HINT) && text.contains(FAST_MOVE_HINT),
            "the pending state must still advertise keyboard movement: {text}"
        );
        // The hints are documentation, not actions: a click must not fall
        // through to Apply/Cancel.
        for hint in [SNAP_MOVE_HINT, FAST_MOVE_HINT] {
            let (x, y) = find_text(&buf, hint).expect("drawn movement hint");
            assert_eq!(frame.button_at((x, y)), None);
        }
    }

    #[test]
    fn the_pending_movement_hints_yield_to_the_confirm_buttons() {
        // The hints have the lowest priority: when the row is narrow, they are
        // dropped whole so Apply and Cancel all keep their labels.
        let screen = area(46);
        let mut buf = Buffer::empty(screen);
        let frame = ActionBar::render(screen, &mut buf, context(TUIMode::View, true), None);
        let text = labels(&frame, &buf);

        assert!(
            !text.contains(SNAP_MOVE_HINT) && !text.contains(FAST_MOVE_HINT),
            "got: {text}"
        );
        assert!(text.contains("[ Apply  Enter ]"), "got: {text}");
        assert!(text.contains("[ Cancel all  Esc ]"), "got: {text}");
    }

    #[test]
    fn the_compact_footer_keeps_save_quit_and_keybinds() {
        let screen = area(100);
        let mut buf = Buffer::empty(screen);
        let frame = ActionBar::render(screen, &mut buf, context(TUIMode::View, false), None);
        let text = labels(&frame, &buf);

        assert!(text.contains("Save"), "got: {text}");
        assert!(text.contains("Quit"), "got: {text}");
        assert!(text.contains("Keybinds"), "got: {text}");
        assert!(
            !text.contains("Move"),
            "compact mode drops the mode buttons: {text}"
        );
    }

    #[test]
    fn narrow_bars_compact_low_priority_buttons_instead_of_clipping() {
        let screen = area(50);
        let mut buf = Buffer::empty(screen);
        let frame = ActionBar::render(screen, &mut buf, context(TUIMode::View, false), None);
        let text = labels(&frame, &buf);

        assert!(!frame.buttons.is_empty(), "some buttons must survive");
        assert!(
            text.contains("[p]"),
            "the low-priority preset button should compact first: {text}"
        );
        for button in &frame.buttons {
            assert!(
                button.rect.width >= 3,
                "a rendered chip is at least [x]: {:?}",
                button
            );
        }
    }

    #[test]
    fn range_hints_are_drawn_but_cannot_be_clicked() {
        let screen = area(160);
        let mut buf = Buffer::empty(screen);
        let frame = ActionBar::render(screen, &mut buf, context(TUIMode::View, false), None);
        let text = labels(&frame, &buf);

        assert!(text.contains("<0-9>"), "workspace hint missing: {text}");
        assert!(text.contains("<k/j>"), "scroll hint missing: {text}");
        for hint in ["<0-9>", "<k/j>"] {
            let (x, y) = find_text(&buf, hint).expect("drawn range hint");
            assert_eq!(
                frame.button_at((x, y)),
                None,
                "{hint} must stay a non-clickable hint"
            );
        }
    }

    #[test]
    fn hovering_a_button_only_targets_that_action() {
        let screen = area(160);
        let mut buf = Buffer::empty(screen);
        let frame = ActionBar::render(
            screen,
            &mut buf,
            context(TUIMode::View, false),
            Some(HoverTarget::Action(ActionId::Save)),
        );
        let save = frame
            .buttons
            .iter()
            .find(|button| button.action == ActionId::Save)
            .expect("save button");

        assert_eq!(
            buf[(save.rect.x, save.rect.y)].style().add_modifier,
            ratatui::style::Modifier::REVERSED | ratatui::style::Modifier::BOLD,
            "the hovered chip must be inverted"
        );
    }

    #[test]
    fn each_mode_has_its_own_set() {
        let move_actions: Vec<ActionId> =
            ActionBar::layout(area(160), context(TUIMode::Move, false))
                .buttons
                .iter()
                .map(|button| button.action)
                .collect();
        assert_eq!(move_actions, vec![ActionId::Save, ActionId::Done]);

        for mode in [TUIMode::Resolution, TUIMode::Scale] {
            let actions: Vec<ActionId> = ActionBar::layout(area(160), context(mode, false))
                .buttons
                .iter()
                .map(|button| button.action)
                .collect();
            assert_eq!(
                actions,
                vec![ActionId::Apply, ActionId::Save, ActionId::Cancel]
            );
        }
    }

    #[test]
    fn the_button_key_dispatch_matches_the_advertised_key() {
        assert_eq!(ActionId::Save.key(true), KeyCode::Char('w'));
        assert_eq!(ActionId::Quit.key(true), KeyCode::Char('q'));
        assert_eq!(ActionId::Help.key(true), KeyCode::Char('K'));
        assert_eq!(ActionId::Apply.key(true), KeyCode::Enter);
        assert_eq!(ActionId::ApplyPending.key(true), KeyCode::Enter);
        assert_eq!(ActionId::Cancel.key(true), KeyCode::Esc);
        assert_eq!(ActionId::ToggleEnabled.key(true), KeyCode::Char('d'));
        assert_eq!(ActionId::ToggleEnabled.key(false), KeyCode::Char('e'));
    }

    #[test]
    fn the_click_map_returns_the_drawn_button() {
        let frame = ActionBar::layout(area(160), context(TUIMode::View, false));

        for button in &frame.buttons {
            assert_eq!(
                frame.button_at((button.rect.x, button.rect.y)),
                Some(button.action)
            );
            assert!(frame.contains((button.rect.x, button.rect.y)));
        }
        assert!(frame.button_at((0, 0)).is_none());
    }

    #[test]
    fn the_lightweight_hit_test_matches_the_drawn_bar_cell_for_cell() {
        for mode in [
            TUIMode::View,
            TUIMode::Move,
            TUIMode::Resolution,
            TUIMode::Scale,
        ] {
            for pending in [false, true] {
                for width in [20u16, 40, 60, 80, 100, 114, 115, 120, 130, 160, 240] {
                    let screen = area(width);
                    let context = context(mode, pending);
                    let frame = ActionBar::layout(screen, context);
                    let hits = ActionBar::hit_test_in(frame.area, context, width);

                    assert_eq!(hits.area, frame.area, "{mode:?}/{pending} at {width}");
                    for x in frame.area.x..frame.area.right() {
                        let point = (x, frame.area.y);
                        assert_eq!(
                            hits.button_at(point),
                            frame.button_at(point),
                            "{mode:?}/{pending} at {width}, cell {x}"
                        );
                        assert_eq!(
                            hits.contains(point),
                            frame.contains(point),
                            "{mode:?}/{pending} at {width}, cell {x}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_lightweight_hit_test_ignores_an_absent_footer() {
        let hits =
            ActionBar::hit_test_in(Rect::new(0, 0, 80, 0), context(TUIMode::View, false), 80);

        assert!(hits.button_at((0, 0)).is_none());
        assert!(!hits.contains((0, 0)));
    }

    #[test]
    fn find_text_still_sees_the_full_chip() {
        let screen = area(160);
        let mut buf = Buffer::empty(screen);
        ActionBar::render(screen, &mut buf, context(TUIMode::View, false), None);

        let (_, y) = find_text(&buf, "Save").expect("save label");
        assert!(
            text_of(&buf).contains("[ Save  w ]"),
            "got: {}",
            text_of(&buf)
        );
        assert_eq!(y, screen.height - 2);
    }
}
