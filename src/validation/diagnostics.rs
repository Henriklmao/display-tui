//! Layout diagnostics and move-only auto-fix planning.
//!
//! This module turns the boolean verdicts of [`super::rules`] into the geometry
//! the UI needs: which edge portions of which monitors overlap, where a
//! detached monitor should be drawn/connected to, and a single-step move that
//! resolves one reported problem without introducing a new one.
//!
//! Everything here is pure (no ratatui, no I/O) and operates on
//! [`Monitor::get_geometry`], which already applies scale and 90/270 rotation.
//! Enabled monitors without a usable rectangle (no position or no resolution)
//! are skipped: they have nothing on the map to highlight.

use crate::monitor::Monitor;
use std::collections::{HashMap, HashSet};

// Runtime tolerance mirroring [`super::rules::validate_contiguous`].
const EPS: f64 = 2.0;

// A rectangle in world pixels: `(x, y, width, height)`, y growing downwards.
pub type Rect = (f64, f64, f64, f64);

// Which side of a detached monitor faces its anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

// A straight, axis-aligned piece of a monitor border or a connector.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeSegment {
    // True for a horizontal segment (its `fixed` coordinate is a row),
    // false for a vertical one (`fixed` is a column).
    pub horizontal: bool,
    // The y (horizontal) or x (vertical) coordinate the segment lies on.
    pub fixed: f64,
    // The segment's span along its axis; `start <= end`.
    pub start: f64,
    pub end: f64,
}

impl EdgeSegment {
    fn horizontal(y: f64, start: f64, end: f64) -> Self {
        EdgeSegment {
            horizontal: true,
            fixed: y,
            start: start.min(end),
            end: start.max(end),
        }
    }

    fn vertical(x: f64, start: f64, end: f64) -> Self {
        EdgeSegment {
            horizontal: false,
            fixed: x,
            start: start.min(end),
            end: start.max(end),
        }
    }

    // The segment's two endpoints as points.
    pub fn endpoints(&self) -> ((f64, f64), (f64, f64)) {
        if self.horizontal {
            ((self.start, self.fixed), (self.end, self.fixed))
        } else {
            ((self.fixed, self.start), (self.fixed, self.end))
        }
    }
}

// A pair of enabled monitors whose rectangles overlap, plus the intersection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlapRegion {
    pub a: usize,
    pub b: usize,
    // The intersection rectangle `(x, y, width, height)`.
    pub rect: Rect,
}

impl OverlapRegion {
    // The portions of each monitor's border that lie strictly inside the other
    // rectangle, as `(monitor index, segment)`. A monitor fully contained in
    // the other yields all four of its edges.
    pub fn edge_segments(&self, monitors: &[Monitor]) -> Vec<(usize, EdgeSegment)> {
        let mut out = Vec::new();
        let ra = monitors[self.a].get_geometry();
        let rb = monitors[self.b].get_geometry();
        collect_edges_within(self.a, ra, rb, &mut out);
        collect_edges_within(self.b, rb, ra, &mut out);
        out
    }
}

// A detached monitor and the nearest monitor it should be re-attached to.
#[derive(Debug, Clone, PartialEq)]
pub struct DetachInfo {
    // The disconnected monitor.
    pub monitor: usize,
    // The nearest monitor in a different component.
    pub anchor: usize,
    // The direction from `monitor` towards its anchor.
    pub direction: Direction,
    // The overlap of the two rectangles on the perpendicular axis, if any.
    pub contact_span: Option<(f64, f64)>,
    // The gap between the two rectangles along `direction` (>= 0).
    pub gap: f64,
}

impl DetachInfo {
    // The two axis-aligned segments of the L-shaped connector, drawn between
    // the centre of the out-of-bounds monitor's nearest edge and the centre of
    // the anchor's nearest (facing) edge.
    pub fn connector(&self, monitors: &[Monitor]) -> Vec<EdgeSegment> {
        let m = monitors[self.monitor].get_geometry();
        let a = monitors[self.anchor].get_geometry();
        let source = match self.direction {
            Direction::Right => (m.0 + m.2, m.1 + m.3 / 2.0),
            Direction::Left => (m.0, m.1 + m.3 / 2.0),
            Direction::Down => (m.0 + m.2 / 2.0, m.1 + m.3),
            Direction::Up => (m.0 + m.2 / 2.0, m.1),
        };
        let target = match self.direction {
            Direction::Right => (a.0, a.1 + a.3 / 2.0),
            Direction::Left => (a.0 + a.2, a.1 + a.3 / 2.0),
            Direction::Down => (a.0 + a.2 / 2.0, a.1),
            Direction::Up => (a.0 + a.2 / 2.0, a.1 + a.3),
        };

        let mut segments = Vec::new();
        if (source.0 - target.0).abs() > f64::EPSILON {
            segments.push(EdgeSegment::horizontal(source.1, source.0, target.0));
        }
        if (source.1 - target.1).abs() > f64::EPSILON {
            segments.push(EdgeSegment::vertical(target.0, source.1, target.1));
        }
        segments
    }
}

// A single move that resolves one reported problem.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AutoFix {
    // Push `monitor` apart from an overlapping neighbour.
    Separate { monitor: usize, dx: i32, dy: i32 },
    // Translate a detached monitor flush onto its anchor edge.
    Abut { monitor: usize, dx: i32, dy: i32 },
    // Translate every monitor so `monitor` starts at the origin.
    ToOrigin { monitor: usize, dx: i32, dy: i32 },
}

// Monitors that should be flagged with a caution background in the list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CautionFlags {
    // Monitors taking part in an overlap (both sides of every pair).
    pub overlap: HashSet<usize>,
    // Monitors that are not contiguous with the largest component.
    pub detached: HashSet<usize>,
    // Monitors sharing a duplicated workspace id.
    pub dup_ws: HashSet<usize>,
}

impl CautionFlags {
    // Whether the monitor at `index` carries any caution.
    pub fn flags(&self, index: usize) -> bool {
        self.overlap.contains(&index)
            || self.detached.contains(&index)
            || self.dup_ws.contains(&index)
    }
}

// ---------------------------------------------------------------------------
// rectangle primitives
// ---------------------------------------------------------------------------

// A monitor participates in the geometry when it is enabled and placed.
fn is_placed(monitor: &Monitor) -> bool {
    if !monitor.enabled {
        return false;
    }
    let (_, _, w, h) = monitor.get_geometry();
    w > 0.0 && h > 0.0
}

// Indices of every placed (enabled + usable rectangle) monitor.
fn placed_indices(monitors: &[Monitor]) -> Vec<usize> {
    monitors
        .iter()
        .enumerate()
        .filter(|(_, m)| is_placed(m))
        .map(|(i, _)| i)
        .collect()
}

// Strict overlap, matching [`super::rules::validate_no_overlap`].
fn overlaps(a: Rect, b: Rect) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

// Touching-or-overlapping, matching the contiguity epsilon.
fn touches(a: Rect, b: Rect) -> bool {
    let touches_x = a.0 <= b.0 + b.2 + EPS && b.0 <= a.0 + a.2 + EPS;
    let touches_y = a.1 <= b.1 + b.3 + EPS && b.1 <= a.1 + a.3 + EPS;
    touches_x && touches_y
}

fn intersection(a: Rect, b: Rect) -> Rect {
    let x0 = a.0.max(b.0);
    let y0 = a.1.max(b.1);
    let x1 = (a.0 + a.2).min(b.0 + b.2);
    let y1 = (a.1 + a.3).min(b.1 + b.3);
    (x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0))
}

// Gap between two intervals: 0 when they overlap or touch.
fn axis_gap(a0: f64, a1: f64, b0: f64, b1: f64) -> f64 {
    if a1 < b0 {
        b0 - a1
    } else if b1 < a0 {
        a0 - b1
    } else {
        0.0
    }
}

// Overlap between two intervals, if any.
fn overlap_span(a0: f64, a1: f64, b0: f64, b1: f64) -> Option<(f64, f64)> {
    let start = a0.max(b0);
    let end = a1.min(b1);
    if end > start {
        Some((start, end))
    } else {
        None
    }
}

// The distance between the nearest points of two rectangles.
fn rect_distance(a: Rect, b: Rect) -> f64 {
    let dx = axis_gap(a.0, a.0 + a.2, b.0, b.0 + b.2);
    let dy = axis_gap(a.1, a.1 + a.3, b.1, b.1 + b.3);
    (dx * dx + dy * dy).sqrt()
}

// Rounds away from zero, so a separating move never leaves a sliver of overlap.
fn round_away(value: f64) -> i32 {
    if value >= 0.0 {
        value.ceil() as i32
    } else {
        value.floor() as i32
    }
}

// Rounds towards zero, so an abutting move never overshoots into an overlap.
fn round_toward_zero(value: f64) -> i32 {
    value.trunc() as i32
}

// ---------------------------------------------------------------------------
// overlap + contiguity diagnostics
// ---------------------------------------------------------------------------

// Every overlapping pair of placed monitors, with its intersection rectangle.
pub fn overlap_regions(monitors: &[Monitor]) -> Vec<OverlapRegion> {
    let placed = placed_indices(monitors);
    let mut out = Vec::new();
    for i in 0..placed.len() {
        for j in (i + 1)..placed.len() {
            let a = placed[i];
            let b = placed[j];
            let ra = monitors[a].get_geometry();
            let rb = monitors[b].get_geometry();
            if overlaps(ra, rb) {
                out.push(OverlapRegion {
                    a,
                    b,
                    rect: intersection(ra, rb),
                });
            }
        }
    }
    out
}

// Connected components of the touching graph, sorted largest first.
//
// Mirrors [`super::rules::validate_contiguous`] over placed monitors and
// returns monitor indices rather than names.
pub fn components(monitors: &[Monitor]) -> Vec<Vec<usize>> {
    let placed = placed_indices(monitors);
    if placed.is_empty() {
        return Vec::new();
    }

    let mut adjacency = vec![Vec::new(); placed.len()];
    for i in 0..placed.len() {
        for j in (i + 1)..placed.len() {
            let ra = monitors[placed[i]].get_geometry();
            let rb = monitors[placed[j]].get_geometry();
            if touches(ra, rb) {
                adjacency[i].push(j);
                adjacency[j].push(i);
            }
        }
    }

    let mut visited = vec![false; placed.len()];
    let mut components = Vec::new();
    for start in 0..placed.len() {
        if visited[start] {
            continue;
        }
        let mut component = Vec::new();
        let mut stack = vec![start];
        visited[start] = true;
        while let Some(node) = stack.pop() {
            component.push(placed[node]);
            for &neighbour in &adjacency[node] {
                if !visited[neighbour] {
                    visited[neighbour] = true;
                    stack.push(neighbour);
                }
            }
        }
        component.sort_unstable();
        components.push(component);
    }

    components.sort_by_key(|component| std::cmp::Reverse(component.len()));
    components
}

// The disconnected offenders: every monitor outside the largest component.
pub fn disconnected_monitors(monitors: &[Monitor]) -> Vec<usize> {
    let components = components(monitors);
    if components.len() <= 1 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for component in components.iter().skip(1) {
        out.extend(component.iter().copied());
    }
    out
}

// Flags for the display list's live caution background. O(n^2) but n is tiny.
pub fn caution_flags(monitors: &[Monitor]) -> CautionFlags {
    let mut overlap = HashSet::new();
    for region in overlap_regions(monitors) {
        overlap.insert(region.a);
        overlap.insert(region.b);
    }

    let detached: HashSet<usize> = disconnected_monitors(monitors).into_iter().collect();

    let mut counts: HashMap<u8, usize> = HashMap::new();
    for monitor in monitors {
        if let Some(ws) = monitor.workspace {
            *counts.entry(ws).or_insert(0) += 1;
        }
    }
    let mut dup_ws = HashSet::new();
    for (index, monitor) in monitors.iter().enumerate() {
        if monitor
            .workspace
            .is_some_and(|ws| counts.get(&ws).copied().unwrap_or(0) > 1)
        {
            dup_ws.insert(index);
        }
    }

    CautionFlags {
        overlap,
        detached,
        dup_ws,
    }
}

// ---------------------------------------------------------------------------
// detachment anchors
// ---------------------------------------------------------------------------

// The nearest cross-component anchor for every disconnected monitor.
pub fn detach_infos(monitors: &[Monitor]) -> Vec<DetachInfo> {
    let components = components(monitors);
    if components.len() <= 1 {
        return Vec::new();
    }

    // Component membership and size per monitor index.
    let mut member_of = vec![usize::MAX; monitors.len()];
    let mut component_size = vec![0usize; monitors.len()];
    for (id, component) in components.iter().enumerate() {
        for &index in component {
            member_of[index] = id;
            component_size[index] = component.len();
        }
    }

    let placed = placed_indices(monitors);
    let mut out = Vec::new();
    for component in components.iter().skip(1) {
        for &monitor in component {
            let source = monitors[monitor].get_geometry();
            let mut best: Option<(f64, usize, usize)> = None; // (distance, size, index)
            for &candidate in &placed {
                if member_of[candidate] == member_of[monitor] {
                    continue;
                }
                let distance = rect_distance(source, monitors[candidate].get_geometry());
                let entry = (distance, component_size[candidate], candidate);
                let better = match best {
                    None => true,
                    Some(current) => {
                        if (entry.0 - current.0).abs() > 1e-9 {
                            entry.0 < current.0
                        } else if entry.1 != current.1 {
                            entry.1 > current.1
                        } else {
                            entry.2 < current.2
                        }
                    }
                };
                if better {
                    best = Some(entry);
                }
            }
            if let Some((gap, _, anchor)) = best {
                out.push(build_detach(monitors, monitor, anchor, gap));
            }
        }
    }
    out
}

fn build_detach(monitors: &[Monitor], monitor: usize, anchor: usize, gap: f64) -> DetachInfo {
    let m = monitors[monitor].get_geometry();
    let a = monitors[anchor].get_geometry();
    let dx = axis_gap(m.0, m.0 + m.2, a.0, a.0 + a.2);
    let dy = axis_gap(m.1, m.1 + m.3, a.1, a.1 + a.3);

    // Attach along the nearest edge: the axis the rectangles are separated on,
    // and when they are separated on both, the one with the smaller gap.
    let horizontal = match (dx > 0.0, dy > 0.0) {
        (true, false) => true,
        (false, true) => false,
        (true, true) => dx <= dy,
        (false, false) => dx >= dy, // touching/overlapping: not detached
    };

    let (direction, contact_span) = if horizontal {
        let direction = if a.0 >= m.0 {
            Direction::Right
        } else {
            Direction::Left
        };
        (direction, overlap_span(m.1, m.1 + m.3, a.1, a.1 + a.3))
    } else {
        let direction = if a.1 >= m.1 {
            Direction::Down
        } else {
            Direction::Up
        };
        (direction, overlap_span(m.0, m.0 + m.2, a.0, a.0 + a.2))
    };

    DetachInfo {
        monitor,
        anchor,
        direction,
        contact_span,
        gap,
    }
}

// ---------------------------------------------------------------------------
// auto-fix planning
// ---------------------------------------------------------------------------

// The pending monitor wins; otherwise the smaller area, then the later index.
fn choose_overlap_monitor(monitors: &[Monitor], a: usize, b: usize, moved: Option<usize>) -> usize {
    if moved == Some(a) {
        return a;
    }
    if moved == Some(b) {
        return b;
    }
    let area = |index: usize| {
        let (_, _, w, h) = monitors[index].get_geometry();
        w * h
    };
    match area(a).partial_cmp(&area(b)) {
        Some(std::cmp::Ordering::Less) => a,
        Some(std::cmp::Ordering::Greater) => b,
        _ => a.max(b),
    }
}

// Minimum-translation move pushing `chosen` off `other`.
fn separation_delta(monitors: &[Monitor], chosen: usize, other: usize) -> (i32, i32) {
    let c = monitors[chosen].get_geometry();
    let o = monitors[other].get_geometry();
    let overlap_x = (c.0 + c.2).min(o.0 + o.2) - c.0.max(o.0);
    let overlap_y = (c.1 + c.3).min(o.1 + o.3) - c.1.max(o.1);

    if overlap_x < overlap_y {
        let delta = if c.0 >= o.0 {
            (o.0 + o.2) - c.0
        } else {
            (o.0 - c.2) - c.0
        };
        (round_away(delta), 0)
    } else {
        let delta = if c.1 >= o.1 {
            (o.1 + o.3) - c.1
        } else {
            (o.1 - c.3) - c.1
        };
        (0, round_away(delta))
    }
}

// Move that makes a detached monitor flush onto its anchor's facing edge and
// centred on it: the middle of the monitor's facing edge meets the middle of
// the anchor's facing edge (matching the arrow/connector endpoint).
fn abut_delta(monitors: &[Monitor], info: &DetachInfo) -> (i32, i32) {
    let m = monitors[info.monitor].get_geometry();
    let a = monitors[info.anchor].get_geometry();
    match info.direction {
        Direction::Right => {
            // Flush against the anchor's left edge, centres aligned on y.
            let dx = a.0 - (m.0 + m.2);
            let dy = (a.1 + a.3 / 2.0) - (m.1 + m.3 / 2.0);
            (round_toward_zero(dx), dy.round() as i32)
        }
        Direction::Left => {
            let dx = (a.0 + a.2) - m.0;
            let dy = (a.1 + a.3 / 2.0) - (m.1 + m.3 / 2.0);
            (round_toward_zero(dx), dy.round() as i32)
        }
        Direction::Down => {
            // Flush against the anchor's top edge, centres aligned on x.
            let dy = a.1 - (m.1 + m.3);
            let dx = (a.0 + a.2 / 2.0) - (m.0 + m.2 / 2.0);
            (dx.round() as i32, round_toward_zero(dy))
        }
        Direction::Up => {
            let dy = (a.1 + a.3) - m.1;
            let dx = (a.0 + a.2 / 2.0) - (m.0 + m.2 / 2.0);
            (dx.round() as i32, round_toward_zero(dy))
        }
    }
}

fn shifted(monitors: &[Monitor], index: usize, dx: i32, dy: i32) -> Vec<Monitor> {
    let mut after = monitors.to_vec();
    if let Some(position) = after[index].position.as_mut() {
        position.x += dx;
        position.y += dy;
    }
    after
}

// Normalised overlapping pairs, used to compare layouts before/after a fix.
fn problem_pairs(monitors: &[Monitor]) -> Vec<(usize, usize)> {
    overlap_regions(monitors)
        .into_iter()
        .map(|region| (region.a.min(region.b), region.a.max(region.b)))
        .collect()
}

// Whether `after` introduces no problem that `before` did not already have,
// while resolving at least one. A move that trades one problem for another
// (e.g. abutting onto a third monitor) is rejected.
fn dials_back(before: &[Monitor], after: &[Monitor]) -> bool {
    let pairs_before = problem_pairs(before);
    let pairs_after = problem_pairs(after);
    let detached_before = disconnected_monitors(before);
    let detached_after = disconnected_monitors(after);

    let no_new_pairs = pairs_after.iter().all(|pair| pairs_before.contains(pair));
    let no_new_detached = detached_after
        .iter()
        .all(|index| detached_before.contains(index));
    let improved =
        pairs_after.len() < pairs_before.len() || detached_after.len() < detached_before.len();

    no_new_pairs && no_new_detached && improved
}

// Plans one move that resolves the first reported problem.
//
// One fix per call; the caller applies it, re-validates and calls again. When
// no single move resolves a problem without creating a new one, this returns
// `None` ("no auto fix possible").
pub fn plan_auto_fix(monitors: &[Monitor], moved: Option<usize>) -> Option<AutoFix> {
    let regions = overlap_regions(monitors);
    if let Some(region) = regions.first() {
        let chosen = choose_overlap_monitor(monitors, region.a, region.b, moved);
        let other = if chosen == region.a {
            region.b
        } else {
            region.a
        };

        let (dx, dy) = separation_delta(monitors, chosen, other);
        if dials_back(monitors, &shifted(monitors, chosen, dx, dy)) {
            return Some(AutoFix::Separate {
                monitor: chosen,
                dx,
                dy,
            });
        }

        // The other side of the pair may separate cleanly instead.
        let (dx, dy) = separation_delta(monitors, other, chosen);
        if dials_back(monitors, &shifted(monitors, other, dx, dy)) {
            return Some(AutoFix::Separate {
                monitor: other,
                dx,
                dy,
            });
        }
        return None;
    }

    if let Some(info) = detach_infos(monitors).into_iter().next() {
        let (dx, dy) = abut_delta(monitors, &info);
        if dials_back(monitors, &shifted(monitors, info.monitor, dx, dy)) {
            return Some(AutoFix::Abut {
                monitor: info.monitor,
                dx,
                dy,
            });
        }
        // No single abut resolves the detachment, but anchoring the layout may
        // still be a valid one-step fix.
        return origin_fix(monitors);
    }

    origin_fix(monitors)
}

// Whether the monitor has a usable rectangle and its top-left at the origin.
fn starts_at_origin(monitors: &[Monitor], index: usize) -> bool {
    if !is_placed(&monitors[index]) {
        return false;
    }
    monitors[index]
        .position
        .as_ref()
        .is_some_and(|position| position.x == 0 && position.y == 0)
}

// The monitor whose top-left becomes the origin when the whole layout shifts.
//
// With two placed monitors the leftmost wins; with more, the monitor closest to
// the centroid of all monitor centres, so the layout stays visually centred.
fn choose_origin_monitor(monitors: &[Monitor], placed: &[usize]) -> usize {
    if placed.len() == 2 {
        // A placed monitor always has a position (zero geometry otherwise).
        let key = |index: usize| {
            let position = monitors[index]
                .position
                .as_ref()
                .expect("placed monitors have a position");
            (position.x, position.y, index)
        };
        return if key(placed[0]) <= key(placed[1]) {
            placed[0]
        } else {
            placed[1]
        };
    }

    let centre = |index: usize| {
        let (x, y, w, h) = monitors[index].get_geometry();
        (x + w / 2.0, y + h / 2.0)
    };
    let count = placed.len() as f64;
    let (sum_x, sum_y) = placed.iter().fold((0.0, 0.0), |(sx, sy), &index| {
        let (x, y) = centre(index);
        (sx + x, sy + y)
    });
    let (cx, cy) = (sum_x / count, sum_y / count);

    let mut best = placed[0];
    let mut best_distance = f64::MAX;
    for &index in placed {
        let (x, y) = centre(index);
        let distance = (x - cx).powi(2) + (y - cy).powi(2);
        if distance < best_distance {
            best_distance = distance;
            best = index;
        }
    }
    best
}

// The global translation that anchors the layout at the origin, if needed.
fn origin_fix(monitors: &[Monitor]) -> Option<AutoFix> {
    let placed = placed_indices(monitors);
    if placed.is_empty()
        || placed
            .iter()
            .any(|&index| starts_at_origin(monitors, index))
    {
        return None;
    }

    let monitor = choose_origin_monitor(monitors, &placed);
    let position = monitors[monitor].position.as_ref()?;
    Some(AutoFix::ToOrigin {
        monitor,
        dx: -position.x,
        dy: -position.y,
    })
}

// Border segments and connectors to overlay on the map for the current
// problems: overlap edge portions, the full border of every detached monitor,
// the facing edge of each anchor, and the L-connectors between them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayoutHighlights {
    // Border portions drawn hot (red).
    pub segments: Vec<EdgeSegment>,
    // Axis-aligned connector paths for detached monitors.
    pub connectors: Vec<EdgeSegment>,
}

impl LayoutHighlights {
    // Whether there is anything to draw.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty() && self.connectors.is_empty()
    }
}

// Computes the map overlay for the current layout.
pub fn layout_highlights(monitors: &[Monitor]) -> LayoutHighlights {
    let mut segments = Vec::new();
    for region in overlap_regions(monitors) {
        for (_, segment) in region.edge_segments(monitors) {
            segments.push(segment);
        }
    }

    let mut connectors = Vec::new();
    for info in detach_infos(monitors) {
        segments.extend(full_border(monitors, info.monitor));
        segments.push(facing_edge(monitors, info.anchor, info.direction));
        connectors.extend(info.connector(monitors));
    }

    LayoutHighlights {
        segments,
        connectors,
    }
}

// The four edges of a monitor's rectangle, full length.
fn full_border(monitors: &[Monitor], index: usize) -> Vec<EdgeSegment> {
    let (x, y, w, h) = monitors[index].get_geometry();
    vec![
        EdgeSegment::horizontal(y, x, x + w),
        EdgeSegment::horizontal(y + h, x, x + w),
        EdgeSegment::vertical(x, y, y + h),
        EdgeSegment::vertical(x + w, y, y + h),
    ]
}

// The anchor's edge facing back towards `direction` (the direction the
// detached monitor lies in, so the anchor's facing edge is the opposite one).
fn facing_edge(monitors: &[Monitor], index: usize, direction: Direction) -> EdgeSegment {
    let (x, y, w, h) = monitors[index].get_geometry();
    match direction {
        // Detached monitor is to the right: the anchor faces it with its left.
        Direction::Right => EdgeSegment::vertical(x, y, y + h),
        Direction::Left => EdgeSegment::vertical(x + w, y, y + h),
        Direction::Down => EdgeSegment::horizontal(y, x, x + w),
        Direction::Up => EdgeSegment::horizontal(y + h, x, x + w),
    }
}

// ---------------------------------------------------------------------------
// edge collection
// ---------------------------------------------------------------------------

fn collect_edges_within(owner: usize, m: Rect, other: Rect, out: &mut Vec<(usize, EdgeSegment)>) {
    push_horizontal(owner, m.1, m.0, m.0 + m.2, other, out);
    push_horizontal(owner, m.1 + m.3, m.0, m.0 + m.2, other, out);
    push_vertical(owner, m.0, m.1, m.1 + m.3, other, out);
    push_vertical(owner, m.0 + m.2, m.1, m.1 + m.3, other, out);
}

fn push_horizontal(
    owner: usize,
    y: f64,
    xs: f64,
    xe: f64,
    other: Rect,
    out: &mut Vec<(usize, EdgeSegment)>,
) {
    if y > other.1 && y < other.1 + other.3 {
        let start = xs.max(other.0);
        let end = xe.min(other.0 + other.2);
        if start < end {
            out.push((owner, EdgeSegment::horizontal(y, start, end)));
        }
    }
}

fn push_vertical(
    owner: usize,
    x: f64,
    ys: f64,
    ye: f64,
    other: Rect,
    out: &mut Vec<(usize, EdgeSegment)>,
) {
    if x > other.0 && x < other.0 + other.2 {
        let start = ys.max(other.1);
        let end = ye.min(other.1 + other.3);
        if start < end {
            out.push((owner, EdgeSegment::vertical(x, start, end)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::{Monitor, Position, Resolution};

    // An enabled 1920x1080 monitor at (x, y) unless overridden by the builder.
    fn monitor(name: &str, x: i32, y: i32) -> Monitor {
        Monitor {
            name: name.to_string(),
            enabled: true,
            position: Some(Position { x, y }),
            scale: Some(1.0),
            modes: vec![Resolution {
                width: 1920,
                height: 1080,
                refresh: 60.0,
                preferred: true,
                current: true,
            }],
            ..Default::default()
        }
    }

    fn sized(mut m: Monitor, w: i32, h: i32) -> Monitor {
        m.modes[0].width = w;
        m.modes[0].height = h;
        m
    }

    fn segments_for(monitors: &[Monitor], index: usize) -> Vec<EdgeSegment> {
        let regions = overlap_regions(monitors);
        regions
            .iter()
            .flat_map(|region| region.edge_segments(monitors))
            .filter(|(owner, _)| *owner == index)
            .map(|(_, segment)| segment)
            .collect()
    }

    // --- overlap regions ------------------------------------------------

    #[test]
    fn no_regions_for_a_contiguous_layout() {
        let monitors = vec![monitor("A", 0, 0), monitor("B", 1920, 0)];
        assert!(overlap_regions(&monitors).is_empty());
    }

    #[test]
    fn a_partial_overlap_reports_the_intersection() {
        let monitors = vec![
            sized(monitor("A", 0, 0), 100, 100),
            sized(monitor("B", 60, 0), 100, 100),
        ];

        let regions = overlap_regions(&monitors);

        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].a, 0);
        assert_eq!(regions[0].b, 1);
        assert_eq!(regions[0].rect, (60.0, 0.0, 40.0, 100.0));
    }

    #[test]
    fn a_partial_overlap_highlights_only_the_overlapping_edge_portion() {
        // A and B share the top edge but B pokes 40px into A horizontally.
        let monitors = vec![
            sized(monitor("A", 0, 0), 100, 100),
            sized(monitor("B", 60, 0), 100, 100),
        ];

        // A's right edge (x=100) is strictly inside B's x-range and A's y-range,
        // so the whole right edge is flagged; A's top edge (y=0) is not strictly
        // inside B vertically (same y), so it is not.
        let a = segments_for(&monitors, 0);
        assert_eq!(a.len(), 1, "got {a:?}");
        assert_eq!(a[0], EdgeSegment::vertical(100.0, 0.0, 100.0));

        // B's left edge (x=60) is inside A: the sub-span from y start..end.
        let b = segments_for(&monitors, 1);
        assert_eq!(b.len(), 1, "got {b:?}");
        assert_eq!(b[0], EdgeSegment::vertical(60.0, 0.0, 100.0));
    }

    #[test]
    fn a_fully_contained_monitor_flags_all_four_edges() {
        let monitors = vec![
            sized(monitor("Big", 0, 0), 1000, 1000),
            sized(monitor("Small", 100, 100), 100, 100),
        ];

        let small = segments_for(&monitors, 1);

        assert_eq!(small.len(), 4, "got {small:?}");
        assert!(
            small
                .iter()
                .any(|s| s == &EdgeSegment::horizontal(100.0, 100.0, 200.0))
        );
        assert!(
            small
                .iter()
                .any(|s| s == &EdgeSegment::horizontal(200.0, 100.0, 200.0))
        );
        assert!(
            small
                .iter()
                .any(|s| s == &EdgeSegment::vertical(100.0, 100.0, 200.0))
        );
        assert!(
            small
                .iter()
                .any(|s| s == &EdgeSegment::vertical(200.0, 100.0, 200.0))
        );
    }

    #[test]
    fn a_one_pixel_corner_touch_is_not_an_overlap_but_a_penetration_is() {
        let touching = vec![
            sized(monitor("A", 0, 0), 1920, 1080),
            sized(monitor("B", 1920, 1080), 1920, 1080),
        ];
        assert!(overlap_regions(&touching).is_empty());

        let nudged = vec![
            sized(monitor("A", 0, 0), 1920, 1080),
            sized(monitor("B", 1919, 1079), 1920, 1080),
        ];
        assert_eq!(overlap_regions(&nudged).len(), 1);
    }

    #[test]
    fn disabled_and_unplaced_monitors_are_ignored() {
        let mut disabled = monitor("B", 0, 0);
        disabled.enabled = false;
        let mut unplaced = monitor("C", 0, 0);
        unplaced.position = None;
        let monitors = vec![monitor("A", 0, 0), disabled, unplaced];

        assert!(overlap_regions(&monitors).is_empty());
        assert!(caution_flags(&monitors).overlap.is_empty());
    }

    // --- components / detached -----------------------------------------

    #[test]
    fn components_group_touching_monitors_by_index() {
        let monitors = vec![
            monitor("A", 0, 0),
            monitor("B", 1920, 0),
            monitor("C", 50_000, 0),
        ];

        let components = components(&monitors);

        assert_eq!(components.len(), 2);
        assert_eq!(components[0], vec![0, 1]);
        assert_eq!(components[1], vec![2]);
        assert_eq!(disconnected_monitors(&monitors), vec![2]);
    }

    #[test]
    fn detach_info_names_the_nearest_anchor() {
        // A+B touch; C is far to the right of B and nearer to B than to A.
        let monitors = vec![
            monitor("A", 0, 0),
            monitor("B", 1920, 0),
            monitor("C", 5000, 0),
        ];

        let infos = detach_infos(&monitors);

        assert_eq!(infos.len(), 1);
        let info = &infos[0];
        assert_eq!(info.monitor, 2);
        assert_eq!(info.anchor, 1, "B is the nearest anchor");
        assert_eq!(info.direction, Direction::Left, "anchor is to C's left");
        assert_eq!(info.gap, 5000.0 - 3840.0);
        assert_eq!(info.contact_span, Some((0.0, 1080.0)));
    }

    #[test]
    fn detach_anchor_breaks_distance_ties_by_component_then_index() {
        // One big component A+B and two singleton components C and D, both at
        // the same distance from each other and from A/B. Ties prefer the
        // larger component, i.e. A or B, then the lower index -> A.
        let monitors = vec![
            monitor("A", 0, 0),
            monitor("B", 1920, 0),
            monitor("C", -5000, 0),
            monitor("D", -5000, 0),
        ];

        let infos = detach_infos(&monitors);
        let c = infos.iter().find(|info| info.monitor == 2).unwrap();

        assert_eq!(c.anchor, 0, "the largest component's lowest index wins");
    }

    #[test]
    fn connector_steps_are_axis_aligned() {
        // Diagonal-ish: C is to the right and slightly above its anchor.
        let monitors = vec![monitor("A", 0, 0), monitor("C", 5000, -300)];
        let info = detach_infos(&monitors).into_iter().next().unwrap();

        let connector = info.connector(&monitors);
        assert_eq!(connector.len(), 2, "an L needs two segments");
        for segment in &connector {
            let (p, q) = segment.endpoints();
            assert!(
                (p.0 - q.0).abs() < 1e-9 || (p.1 - q.1).abs() < 1e-9,
                "segment is not axis-aligned: {segment:?}"
            );
        }
        // Starts at the centre of C's left (nearest) edge, ends at the middle
        // of A's facing (right) edge.
        let first = connector.first().unwrap();
        assert_eq!(
            first.fixed,
            -300.0 + 1080.0 / 2.0,
            "starts at C's vertical centre"
        );
        let (_, q) = connector.last().unwrap().endpoints();
        assert_eq!(q.0, 1920.0, "ends on A's right edge");
        assert_eq!(q.1, 0.0 + 1080.0 / 2.0, "at A's vertical centre");
    }

    // The connector (the "arrow" pointing a detached monitor at its anchor)
    // runs from the centre of the out-of-bounds monitor's nearest edge to the
    // centre of the anchor's nearest (facing) edge, in every direction.
    #[test]
    fn the_connector_joins_the_two_nearest_edge_midpoints() {
        // (detached position, expected direction)
        let cases = [
            ((5000, 0), Direction::Left),   // anchor left of the monitor
            ((-5000, 0), Direction::Right), // anchor right of the monitor
            ((0, 5000), Direction::Up),     // anchor above the monitor
            ((0, -5000), Direction::Down),  // anchor below the monitor
            // Diagonal: the vertical gap (2000-1080=920) is smaller than the
            // horizontal one (5000-1920=3080), so the top edge is nearest.
            ((5000, 2000), Direction::Up),
        ];

        for ((x, y), expected) in cases {
            let monitors = vec![monitor("A", 0, 0), monitor("C", x, y)];
            let info = detach_infos(&monitors).into_iter().next().unwrap();
            assert_eq!(info.direction, expected, "case ({x}, {y})");

            let connector = info.connector(&monitors);
            // Segments are normalised (start <= end), so the path direction is
            // lost; the two degree-1 endpoints are the path ends.
            let mut counts: Vec<((f64, f64), usize)> = Vec::new();
            for segment in &connector {
                let (p, q) = segment.endpoints();
                for point in [p, q] {
                    match counts.iter_mut().find(|(existing, _)| *existing == point) {
                        Some((_, count)) => *count += 1,
                        None => counts.push((point, 1)),
                    }
                }
            }
            let mut ends: Vec<(f64, f64)> = counts
                .into_iter()
                .filter(|(_, count)| *count == 1)
                .map(|(point, _)| point)
                .collect();
            ends.sort_by(|a, b| a.partial_cmp(b).unwrap());

            let (ax, ay, aw, ah) = monitors[0].get_geometry();
            let (cx, cy, cw, ch) = monitors[1].get_geometry();

            let expected_start = match expected {
                Direction::Right => (cx + cw, cy + ch / 2.0),
                Direction::Left => (cx, cy + ch / 2.0),
                Direction::Down => (cx + cw / 2.0, cy + ch),
                Direction::Up => (cx + cw / 2.0, cy),
            };
            let expected_end = match expected {
                Direction::Right => (ax, ay + ah / 2.0),
                Direction::Left => (ax + aw, ay + ah / 2.0),
                Direction::Down => (ax + aw / 2.0, ay),
                Direction::Up => (ax + aw / 2.0, ay + ah),
            };
            let mut expected = vec![expected_start, expected_end];
            expected.sort_by(|a, b| a.partial_cmp(b).unwrap());

            assert_eq!(
                ends, expected,
                "{expected:?}: the connector must join the nearest edge midpoints"
            );
        }
    }

    // The nearest edge wins even when the rectangles are separated on both
    // axes: the smaller gap decides the attachment side.
    #[test]
    fn the_detach_direction_follows_the_smallest_gap() {
        let monitors = vec![monitor("A", 0, 0), monitor("C", 5000, 2000)];
        let info = detach_infos(&monitors).into_iter().next().unwrap();
        assert_eq!(
            info.direction,
            Direction::Up,
            "the top edge (gap 920) is nearer than the left edge (gap 3080)"
        );
    }

    // The reported example: monitor 1 at (0,0) 1920x1200, monitor 2 at
    // (2223,540) 1920x1200 runs from monitor 2's left edge centre to monitor
    // 1's right edge centre.
    #[test]
    fn the_connector_matches_the_reported_example() {
        let monitors = vec![
            sized(monitor("1", 0, 0), 1920, 1200),
            sized(monitor("2", 2223, 540), 1920, 1200),
        ];
        let info = detach_infos(&monitors).into_iter().next().unwrap();
        assert_eq!(info.direction, Direction::Left);

        assert_eq!(
            info.connector(&monitors),
            vec![
                EdgeSegment::horizontal(1140.0, 1920.0, 2223.0),
                EdgeSegment::vertical(1920.0, 600.0, 1140.0),
            ],
            "from (2223, 1140) to (1920, 600)"
        );
    }

    #[test]
    fn caution_flags_union_every_pair_of_a_three_way_overlap() {
        let monitors = vec![
            sized(monitor("A", 0, 0), 300, 100),
            sized(monitor("B", 100, 0), 300, 100),
            sized(monitor("C", 200, 0), 300, 100),
        ];

        let flags = caution_flags(&monitors);

        assert_eq!(flags.overlap, HashSet::from([0, 1, 2]));
    }

    #[test]
    fn caution_flags_mark_only_detached_offenders() {
        let monitors = vec![
            monitor("A", 0, 0),
            monitor("B", 1920, 0),
            monitor("C", 50_000, 0),
        ];

        let flags = caution_flags(&monitors);

        assert!(flags.overlap.is_empty());
        assert_eq!(flags.detached, HashSet::from([2]));
        assert!(flags.flags(2));
        assert!(!flags.flags(0), "the anchor component is not flagged");
        assert!(!flags.flags(1));
    }

    #[test]
    fn caution_flags_count_duplicate_workspaces_across_disabled_monitors() {
        let mut a = monitor("A", 0, 0);
        a.workspace = Some(4);
        let mut b = monitor("B", 50_000, 0);
        b.enabled = false;
        b.workspace = Some(4);
        let monitors = vec![a, b];

        let flags = caution_flags(&monitors);

        assert_eq!(flags.dup_ws, HashSet::from([0, 1]));
        assert!(flags.flags(1), "disabled monitors still count");
    }

    // --- auto-fix --------------------------------------------------------

    #[test]
    fn a_separating_fix_uses_the_minimum_penetration_axis() {
        // Horizontal penetration is 40px, vertical is 100px: push apart in x.
        let monitors = vec![
            sized(monitor("A", 0, 0), 100, 100),
            sized(monitor("B", 60, 0), 100, 100),
        ];

        let fix = plan_auto_fix(&monitors, None).expect("a fix must exist");

        // Equal areas: the later monitor is chosen and pushed right.
        assert_eq!(
            fix,
            AutoFix::Separate {
                monitor: 1,
                dx: 40,
                dy: 0
            }
        );
    }

    #[test]
    fn the_pending_monitor_is_the_one_separated() {
        let monitors = vec![
            sized(monitor("A", 0, 0), 100, 100),
            sized(monitor("B", 60, 0), 100, 100),
        ];

        let fix = plan_auto_fix(&monitors, Some(0)).expect("a fix must exist");

        assert_eq!(
            fix,
            AutoFix::Separate {
                monitor: 0,
                dx: -40,
                dy: 0
            },
            "the pending monitor moves left off B"
        );
    }

    #[test]
    fn the_smaller_monitor_is_separated_on_an_area_tie_break() {
        let monitors = vec![
            sized(monitor("Big", 0, 0), 1000, 1000),
            sized(monitor("Small", 900, 0), 100, 100),
        ];

        let fix = plan_auto_fix(&monitors, None).expect("a fix must exist");

        match fix {
            AutoFix::Separate { monitor, .. } => assert_eq!(monitor, 1, "the small one moves"),
            other => panic!("expected a separation, got {other:?}"),
        }
    }

    #[test]
    fn an_abut_fix_centres_the_detached_monitor_on_the_anchor_edge() {
        // C sits 5000px right of A's right edge but 400px lower: the fix must
        // both flush it and centre it vertically on A.
        let monitors = vec![monitor("A", 0, 0), monitor("C", 5000, 400)];

        let fix = plan_auto_fix(&monitors, None).expect("a fix must exist");

        assert_eq!(
            fix,
            AutoFix::Abut {
                monitor: 1,
                dx: 1920 - 5000,
                // A centre y = 540, C centre y = 940.
                dy: -400
            },
            "both axes move so the facing midpoints meet"
        );

        // The facing midpoints coincide after the move.
        let after = shifted(&monitors, 1, 1920 - 5000, -400);
        let (ax, ay, aw, ah) = after[0].get_geometry();
        let (cx, cy, _, ch) = after[1].get_geometry();
        assert_eq!(cx, ax + aw, "flush against A's right edge");
        assert_eq!(cy + ch / 2.0, ay + ah / 2.0, "the vertical midpoints match");
    }

    #[test]
    fn an_abut_fix_centres_a_vertically_detached_monitor_too() {
        // D is below A but shifted right by 300px.
        let monitors = vec![monitor("A", 0, 0), monitor("D", 300, 5000)];

        let fix = plan_auto_fix(&monitors, None).expect("a fix must exist");

        assert_eq!(
            fix,
            AutoFix::Abut {
                monitor: 1,
                // A centre x = 960, D centre x = 1260.
                dx: -300,
                dy: 1080 - 5000
            },
            "the horizontal centres are matched while abutting below"
        );
    }

    #[test]
    fn no_fix_when_every_separation_creates_a_new_overlap() {
        // A and B overlap. Pushing B right lands it on C; pushing A left lands
        // it on D. Neither single move resolves the overlap cleanly, so the
        // planner must report "no auto fix possible" instead of trading one
        // problem for another.
        let monitors = vec![
            sized(monitor("A", 80, 0), 100, 100),
            sized(monitor("B", 100, 0), 100, 100),
            sized(monitor("C", 250, 0), 100, 100),
            sized(monitor("D", -50, 0), 100, 100),
        ];

        assert_eq!(
            plan_auto_fix(&monitors, None),
            None,
            "both separation directions create a new overlap"
        );
    }

    #[test]
    fn no_fix_when_an_abut_leaves_the_monitor_detached() {
        // A+B form the main component; C and D are two isolated monitors. C's
        // nearest cross-component anchor is D, so abutting moves C onto D but
        // leaves C+D detached from A+B: no problem is actually resolved.
        let monitors = vec![
            sized(monitor("A", 0, 0), 100, 100),
            sized(monitor("B", 100, 0), 100, 100),
            sized(monitor("C", 5000, 0), 100, 100),
            sized(monitor("D", 7000, 0), 100, 100),
        ];

        assert_eq!(
            plan_auto_fix(&monitors, None),
            None,
            "abutting C onto D still leaves both detached from A+B"
        );
    }

    #[test]
    fn no_fix_when_nothing_is_wrong() {
        let monitors = vec![monitor("A", 0, 0), monitor("B", 1920, 0)];
        assert_eq!(plan_auto_fix(&monitors, None), None);
    }

    #[test]
    fn an_isolated_monitor_has_no_fix() {
        assert_eq!(plan_auto_fix(&[monitor("A", 0, 0)], None), None);
        assert_eq!(plan_auto_fix(&[], None), None);
    }

    // --- origin auto fix -------------------------------------------------

    #[test]
    fn a_contiguous_layout_off_the_origin_gets_anchored() {
        // Two touching monitors, neither at the origin: the leftmost is chosen.
        let monitors = vec![
            sized(monitor("A", 1000, 0), 1920, 1080),
            sized(monitor("B", 2920, 0), 1920, 1080),
        ];

        let fix = plan_auto_fix(&monitors, None).expect("an origin fix must exist");

        assert_eq!(
            fix,
            AutoFix::ToOrigin {
                monitor: 0,
                dx: -1000,
                dy: 0
            }
        );

        let mut after = monitors.clone();
        for monitor in &mut after {
            if let Some(position) = monitor.position.as_mut() {
                position.x -= 1000;
            }
        }
        assert!(
            after
                .iter()
                .any(|m| m.position == Some(crate::monitor::Position { x: 0, y: 0 })),
            "the chosen monitor now starts at the origin"
        );
    }

    #[test]
    fn three_monitors_anchor_the_centre_most_one() {
        // Three touching monitors: the middle one is closest to the centroid.
        let monitors = vec![
            sized(monitor("A", 1000, 0), 1920, 1080),
            sized(monitor("B", 2920, 0), 1920, 1080),
            sized(monitor("C", 4840, 0), 1920, 1080),
        ];

        assert_eq!(
            plan_auto_fix(&monitors, None),
            Some(AutoFix::ToOrigin {
                monitor: 1,
                dx: -2920,
                dy: 0
            })
        );
    }

    #[test]
    fn a_layout_already_at_the_origin_has_no_origin_fix() {
        let monitors = vec![
            sized(monitor("A", 0, 0), 1920, 1080),
            sized(monitor("B", 1920, 0), 1920, 1080),
        ];
        assert_eq!(plan_auto_fix(&monitors, None), None);
    }

    #[test]
    fn an_unresolvable_abut_still_offers_the_origin_fix() {
        // A+B touch, C+D touch only each other: abutting C onto D does not
        // join the components, so the planner falls back to the origin fix.
        let monitors = vec![
            sized(monitor("A", 1000, 0), 100, 100),
            sized(monitor("B", 1100, 0), 100, 100),
            sized(monitor("C", 5000, 0), 100, 100),
            sized(monitor("D", 7000, 0), 100, 100),
        ];

        assert!(matches!(
            plan_auto_fix(&monitors, None),
            Some(AutoFix::ToOrigin { .. })
        ));
    }

    // --- layout highlights ----------------------------------------------

    #[test]
    fn highlights_are_empty_for_a_contiguous_layout() {
        let monitors = vec![monitor("A", 0, 0), monitor("B", 1920, 0)];
        assert!(layout_highlights(&monitors).is_empty());
    }

    #[test]
    fn highlights_include_the_detached_border_anchor_edge_and_connector() {
        let monitors = vec![
            sized(monitor("A", 0, 0), 100, 100),
            sized(monitor("C", 5000, 0), 100, 100),
        ];

        let highlights = layout_highlights(&monitors);

        // C's detached border is flagged in full.
        assert!(
            highlights
                .segments
                .contains(&EdgeSegment::vertical(5000.0, 0.0, 100.0)),
            "C's left border must be flagged"
        );
        // A contributes only its facing (right) edge.
        assert!(
            highlights
                .segments
                .contains(&EdgeSegment::vertical(100.0, 0.0, 100.0)),
            "A's facing edge must be flagged"
        );
        assert!(
            !highlights
                .segments
                .contains(&EdgeSegment::vertical(0.0, 0.0, 100.0)),
            "A's far edge must not be flagged"
        );
        assert_eq!(
            highlights.connectors.len(),
            1,
            "a level connector collapses to a single straight segment"
        );
    }

    #[test]
    fn highlights_include_the_overlap_edge_portions() {
        let monitors = vec![
            sized(monitor("A", 0, 0), 100, 100),
            sized(monitor("B", 60, 0), 100, 100),
        ];

        let highlights = layout_highlights(&monitors);

        assert!(highlights.connectors.is_empty(), "no detachment here");
        assert!(
            highlights
                .segments
                .contains(&EdgeSegment::vertical(100.0, 0.0, 100.0)),
            "A's right edge overlaps B"
        );
    }
}
