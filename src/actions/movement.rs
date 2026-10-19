//! Monitor movement and snapping actions.

use crate::monitor::Monitor;
use crate::utils::find_best_delta;

// Moves a monitor vertically.
pub fn move_vertical(monitor: &mut Monitor, delta: i32) {
    if let Some(ref mut pos) = monitor.position {
        pos.y += delta;
    }
}

// Moves a monitor horizontally.
pub fn move_horizontal(monitor: &mut Monitor, delta: i32) {
    if let Some(ref mut pos) = monitor.position {
        pos.x += delta;
    }
}

// Snaps a monitor vertically to align with other monitors.
pub fn snap_vertical(monitors: &mut [Monitor], selected: usize, direction: i32) {
    let mut targets = vec![0.0];
    for (i, m) in monitors.iter().enumerate() {
        if i == selected || !m.enabled {
            continue;
        }
        let (_, y, _, h) = m.get_geometry();
        targets.push(y);
        targets.push(y + h);
        targets.push(y + h / 2.0);
    }

    let (_, sy, _, sh) = monitors[selected].get_geometry();
    let sources = vec![sy, sy + sh, sy + sh / 2.0];

    if let Some(delta) = find_best_delta(&sources, &targets, direction) {
        move_vertical(&mut monitors[selected], delta.round() as i32);
    }
}

// Snaps a monitor horizontally.
pub fn snap_horizontal(monitors: &mut [Monitor], selected: usize, direction: i32) {
    let mut targets = vec![0.0];
    for (i, m) in monitors.iter().enumerate() {
        if i == selected || !m.enabled {
            continue;
        }
        let (x, _, w, _) = m.get_geometry();
        targets.push(x);
        targets.push(x + w);
        targets.push(x + w / 2.0);
    }

    let (sx, _, sw, _) = monitors[selected].get_geometry();
    let sources = vec![sx, sx + sw, sx + sw / 2.0];

    if let Some(delta) = find_best_delta(&sources, &targets, direction) {
        move_horizontal(&mut monitors[selected], delta.round() as i32);
    }
}

// World-space alignment guides an active snap locked onto.
//
// `x`/`y` hold the world coordinate of the aligned vertical/horizontal line
// when a snap happened, so the map can draw a guide. `center_x`/`center_y` are
// set when the snap aligned two monitor *centres*: the guide then spans both
// monitors so the centre-to-centre relationship is visible at a glance.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SnapGuides {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub center_x: Option<CenterGuide>,
    pub center_y: Option<CenterGuide>,
}

// A centre-to-centre alignment: the shared world coordinate and the two
// monitor extents that should be spanned by the guide.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CenterGuide {
    // The aligned centre coordinate on the guide's axis.
    pub at: f64,
    // The span of the moved monitor along the other axis.
    pub moved: (f64, f64),
    // The span of the neighbour it aligned with along the other axis.
    pub other: (f64, f64),
}

// Snaps the selected monitor to the nearest neighbouring edge or centre on
// each axis independently, but only when it is within `threshold` world
// pixels. Used for live snapping while dragging with the mouse.
//
// Centre-to-centre snaps are reported separately in `center_x`/`center_y` so
// the map can draw a guide that spans both monitors.
pub fn snap_to_alignment(monitors: &mut [Monitor], selected: usize, threshold: f64) -> SnapGuides {
    if selected >= monitors.len() || monitors[selected].position.is_none() {
        return SnapGuides::default();
    }

    let mut x_targets = Vec::with_capacity(1 + 3 * monitors.len());
    x_targets.push(0.0);
    let mut y_targets = Vec::with_capacity(1 + 3 * monitors.len());
    y_targets.push(0.0);
    for (i, m) in monitors.iter().enumerate() {
        if i == selected || !m.enabled {
            continue;
        }
        let (x, y, w, h) = m.get_geometry();
        // A monitor without a usable geometry is not an alignment target.
        if w == 0.0 && h == 0.0 {
            continue;
        }
        x_targets.push(x);
        x_targets.push(x + w);
        x_targets.push(x + w / 2.0);
        y_targets.push(y);
        y_targets.push(y + h);
        y_targets.push(y + h / 2.0);
    }

    let (sx, sy, sw, sh) = monitors[selected].get_geometry();
    if sw == 0.0 && sh == 0.0 {
        return SnapGuides::default();
    }

    let mut guides = SnapGuides::default();

    let x_sources = [sx, sx + sw, sx + sw / 2.0];
    if let Some((delta, guide, source)) = nearest_alignment(&x_sources, &x_targets, threshold) {
        move_horizontal(&mut monitors[selected], delta.round() as i32);
        guides.x = Some(guide);
        // Source 2 is the monitor's centre; a centre target there means the
        // two monitors are now centre-aligned on x.
        if source == 2
            && let Some(other) = centre_span(monitors, selected, guide, Axis::X)
        {
            let (_, my, _, mh) = monitors[selected].get_geometry();
            guides.center_x = Some(CenterGuide {
                at: guide,
                moved: (my, my + mh),
                other,
            });
        }
    }

    let y_sources = [sy, sy + sh, sy + sh / 2.0];
    if let Some((delta, guide, source)) = nearest_alignment(&y_sources, &y_targets, threshold) {
        move_vertical(&mut monitors[selected], delta.round() as i32);
        guides.y = Some(guide);
        if source == 2
            && let Some(other) = centre_span(monitors, selected, guide, Axis::Y)
        {
            let (mx, _, mw, _) = monitors[selected].get_geometry();
            guides.center_y = Some(CenterGuide {
                at: guide,
                moved: (mx, mx + mw),
                other,
            });
        }
    }

    guides
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    X,
    Y,
}

// The perpendicular span of the first enabled monitor whose centre lies on
// `coordinate` along `axis`.
fn centre_span(
    monitors: &[Monitor],
    selected: usize,
    coordinate: f64,
    axis: Axis,
) -> Option<(f64, f64)> {
    monitors.iter().enumerate().find_map(|(i, m)| {
        if i == selected || !m.enabled {
            return None;
        }
        let (x, y, w, h) = m.get_geometry();
        if w == 0.0 && h == 0.0 {
            return None;
        }
        let (centre, span) = match axis {
            Axis::X => (x + w / 2.0, (y, y + h)),
            Axis::Y => (y + h / 2.0, (x, x + w)),
        };
        ((centre - coordinate).abs() < f64::EPSILON).then_some(span)
    })
}

// Finds the smallest alignment move between any source edge and any target
// edge that is within `threshold`, returning `(delta, target, source_index)`.
fn nearest_alignment(
    sources: &[f64],
    targets: &[f64],
    threshold: f64,
) -> Option<(f64, f64, usize)> {
    let mut best: Option<(f64, f64, f64, usize)> = None; // (distance, delta, guide, source)
    for (index, &source) in sources.iter().enumerate() {
        for &target in targets {
            let delta = target - source;
            let distance = delta.abs();
            if distance <= threshold && best.is_none_or(|(b, _, _, _)| distance < b) {
                best = Some((distance, delta, target, index));
            }
        }
    }
    best.map(|(_, delta, guide, index)| (delta, guide, index))
}

#[cfg(test)]
mod tests {
    use super::{move_horizontal, move_vertical, snap_horizontal, snap_vertical};
    use crate::monitor::{Monitor, Position, Resolution};

    // A monitor of 1920x1080 logical pixels placed at (x, y).
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

    fn x_of(m: &Monitor) -> i32 {
        m.position.clone().unwrap().x
    }

    fn y_of(m: &Monitor) -> i32 {
        m.position.clone().unwrap().y
    }

    #[test]
    fn move_actions_shift_the_position() {
        let mut m = monitor("A", 100, 100);

        move_horizontal(&mut m, 500);
        move_vertical(&mut m, -250);

        assert_eq!((x_of(&m), y_of(&m)), (600, -150));
    }

    #[test]
    fn move_actions_ignore_monitors_without_position() {
        let mut m = monitor("A", 0, 0);
        m.position = None;

        move_horizontal(&mut m, 10);
        move_vertical(&mut m, 10);

        assert_eq!(m.position, None);
    }

    #[test]
    fn snap_horizontal_aligns_the_left_edge_with_the_neighbour() {
        // B sits 1080px to the right of A's right edge; snapping left puts
        // B's left edge exactly on A's right edge.
        let mut monitors = vec![monitor("A", 0, 0), monitor("B", 3000, 0)];

        snap_horizontal(&mut monitors, 1, -1);

        assert_eq!(x_of(&monitors[1]), 1920);
        assert_eq!(x_of(&monitors[0]), 0, "other monitors must not move");
    }

    #[test]
    fn snap_horizontal_never_moves_against_the_direction() {
        // Everything is to the left of B, so snapping right finds no target.
        let mut monitors = vec![monitor("A", 0, 0), monitor("B", 3000, 0)];

        snap_horizontal(&mut monitors, 1, 1);

        assert_eq!(x_of(&monitors[1]), 3000);
    }

    #[test]
    fn snap_vertical_aligns_with_the_neighbour() {
        // A spans y 0..1080; the nearest edge above B (y=2000) is 1080.
        let mut monitors = vec![monitor("A", 0, 0), monitor("B", 0, 2000)];

        snap_vertical(&mut monitors, 1, -1);

        assert_eq!(y_of(&monitors[1]), 1080);
    }

    #[test]
    fn snap_vertical_can_move_downwards_too() {
        // B sits above A; snapping down stacks it directly on top.
        let mut monitors = vec![monitor("A", 0, 0), monitor("B", 0, -3000)];

        snap_vertical(&mut monitors, 1, 1);

        assert!(y_of(&monitors[1]) > -3000, "B must move down");
        assert_eq!(y_of(&monitors[1]), -1080, "B ends up flush on top of A");
    }

    #[test]
    fn origin_is_always_a_snap_target() {
        // Nothing else to snap against, so the single monitor lands on 0,0.
        let mut monitors = vec![monitor("A", 137, 42)];

        snap_horizontal(&mut monitors, 0, -1);
        snap_vertical(&mut monitors, 0, -1);

        assert_eq!((x_of(&monitors[0]), y_of(&monitors[0])), (0, 0));
    }

    #[test]
    fn disabled_monitors_are_not_snap_targets() {
        // A is disabled, so B may only snap to the origin.
        let mut monitors = vec![
            {
                let mut m = monitor("A", 0, 0);
                m.enabled = false;
                m
            },
            monitor("B", 3000, 0),
        ];

        snap_horizontal(&mut monitors, 1, -1);

        assert_eq!(x_of(&monitors[1]), 0);
    }

    #[test]
    fn a_disabled_selected_monitor_still_snaps() {
        // Only the *targets* exclude disabled monitors; the selected one is
        // still allowed to move.
        let mut monitors = vec![monitor("A", 0, 0), {
            let mut m = monitor("B", 3000, 0);
            m.enabled = false;
            m
        }];

        snap_horizontal(&mut monitors, 1, -1);

        assert_eq!(x_of(&monitors[1]), 1920);
    }

    #[test]
    fn snap_targets_use_logical_size_of_the_neighbour() {
        // A is scaled to 2x, so its logical width is 960 and B lands on 960
        // instead of the 1920 it would snap to at scale 1.
        let mut scaled = vec![monitor("A", 0, 0), monitor("B", 3000, 0)];
        scaled[0].scale = Some(2.0);
        snap_horizontal(&mut scaled, 1, -1);
        assert_eq!(x_of(&scaled[1]), 960);

        let mut unscaled = vec![monitor("A", 0, 0), monitor("B", 3000, 0)];
        snap_horizontal(&mut unscaled, 1, -1);
        assert_eq!(x_of(&unscaled[1]), 1920);
    }

    #[test]
    fn snap_rounds_fractional_deltas_to_whole_pixels() {
        // A at 175% scale is 1097.14... logical pixels wide, so the snap gap
        // is fractional; the result must still be a whole pixel.
        let mut monitors = vec![monitor("A", 0, 0), monitor("B", 1000, 0)];
        monitors[0].scale = Some(1.75);

        snap_horizontal(&mut monitors, 1, 1);

        assert_eq!(x_of(&monitors[1]), 1097);
    }

    // --- live drag snapping ---------------------------------------------

    use super::{CenterGuide, SnapGuides, snap_to_alignment};

    #[test]
    fn alignment_snaps_to_a_nearby_edge_on_each_axis() {
        // B is 8px short of A's right edge horizontally and level vertically,
        // well within a 20px threshold.
        let mut monitors = vec![monitor("A", 0, 0), monitor("B", 1912, 5)];

        let guides = snap_to_alignment(&mut monitors, 1, 20.0);

        assert_eq!((x_of(&monitors[1]), y_of(&monitors[1])), (1920, 0));
        assert_eq!(
            guides,
            SnapGuides {
                x: Some(1920.0),
                y: Some(0.0),
                ..Default::default()
            }
        );
    }

    // Centre-to-centre snapping must be reported so the map can draw a guide
    // that spans both monitors.
    #[test]
    fn alignment_reports_a_centre_to_centre_x_guide() {
        // A is 100 wide at x=0 (centre 50). B is 40 wide at x=28, so its centre
        // is 48: 2px short of A's centre, within the threshold.
        let mut a = monitor("A", 0, 0);
        a.modes[0].width = 100;
        a.modes[0].height = 100;
        let mut b = monitor("B", 28, 400);
        b.modes[0].width = 40;
        b.modes[0].height = 100;

        let mut monitors = vec![a, b];
        let guides = snap_to_alignment(&mut monitors, 1, 20.0);

        // B's centre moved onto A's centre (50), and no edge won the snap.
        assert_eq!(x_of(&monitors[1]), 30);
        assert_eq!(
            guides.center_x,
            Some(CenterGuide {
                at: 50.0,
                moved: (400.0, 500.0),
                other: (0.0, 100.0),
            })
        );
        assert_eq!(guides.center_y, None);
    }

    #[test]
    fn alignment_reports_a_centre_to_centre_y_guide() {
        // A is 100 tall at y=0 (centre 50). B is 40 tall at y=28 (centre 48).
        let mut a = monitor("A", 0, 0);
        a.modes[0].width = 100;
        a.modes[0].height = 100;
        let mut b = monitor("B", 400, 28);
        b.modes[0].width = 100;
        b.modes[0].height = 40;

        let mut monitors = vec![a, b];
        let guides = snap_to_alignment(&mut monitors, 1, 20.0);

        assert_eq!(y_of(&monitors[1]), 30);
        assert_eq!(
            guides.center_y,
            Some(CenterGuide {
                at: 50.0,
                moved: (400.0, 500.0),
                other: (0.0, 100.0),
            })
        );
    }

    #[test]
    fn an_edge_snap_does_not_report_a_centre_guide() {
        // B's left edge lands on A's right edge: an edge alignment, not centre.
        let mut monitors = vec![monitor("A", 0, 0), monitor("B", 1912, 5)];

        let guides = snap_to_alignment(&mut monitors, 1, 20.0);

        assert!(guides.x.is_some());
        assert_eq!(
            guides.center_x, None,
            "an edge snap must not claim a centre alignment"
        );
    }

    #[test]
    fn alignment_leaves_distant_monitors_alone() {
        // Everything is far outside the threshold: nothing moves, no guides.
        let mut monitors = vec![monitor("A", 0, 0), monitor("B", 5000, 5000)];

        let guides = snap_to_alignment(&mut monitors, 1, 20.0);

        assert_eq!((x_of(&monitors[1]), y_of(&monitors[1])), (5000, 5000));
        assert_eq!(guides, SnapGuides::default());
    }

    #[test]
    fn alignment_prefers_the_closest_edge() {
        // A narrow B sits with its left edge 4px from A's right edge while
        // its centre is 16px away: the nearer edge wins.
        let mut monitors = vec![monitor("A", 0, 0), monitor("B", 1916, 0)];
        monitors[1].modes[0].width = 40;

        let guides = snap_to_alignment(&mut monitors, 1, 20.0);

        assert_eq!(x_of(&monitors[1]), 1920);
        assert_eq!(guides.x, Some(1920.0));
    }

    #[test]
    fn alignment_can_snap_to_the_origin() {
        let mut monitors = vec![monitor("A", 3000, 3000)];

        let guides = snap_to_alignment(&mut monitors, 0, 20.0);

        // Only the origin is a target, and 3000 is far away — no snap.
        assert_eq!(guides, SnapGuides::default());
        assert_eq!((x_of(&monitors[0]), y_of(&monitors[0])), (3000, 3000));

        let mut near = vec![monitor("A", 7, 3)];
        let guides = snap_to_alignment(&mut near, 0, 20.0);
        assert_eq!((x_of(&near[0]), y_of(&near[0])), (0, 0));
        assert_eq!(
            guides,
            SnapGuides {
                x: Some(0.0),
                y: Some(0.0),
                ..Default::default()
            }
        );
    }

    #[test]
    fn alignment_ignores_disabled_monitors_as_targets() {
        // The only nearby edge belongs to a disabled monitor; the origin is
        // far away, so nothing may snap.
        let mut monitors = vec![
            {
                let mut m = monitor("A", 1000, 1000);
                m.enabled = false;
                m
            },
            monitor("B", 1008, 1008),
        ];

        let guides = snap_to_alignment(&mut monitors, 1, 20.0);

        assert_eq!(
            guides,
            SnapGuides::default(),
            "disabled target must not snap"
        );
        assert_eq!((x_of(&monitors[1]), y_of(&monitors[1])), (1008, 1008));
    }

    #[test]
    fn alignment_is_a_no_op_without_a_position() {
        let mut monitors = vec![monitor("A", 0, 0)];
        monitors[0].position = None;

        let guides = snap_to_alignment(&mut monitors, 0, 20.0);

        assert_eq!(guides, SnapGuides::default());
        assert_eq!(monitors[0].position, None);
    }

    #[test]
    fn alignment_out_of_range_selection_is_safe() {
        let mut monitors = vec![monitor("A", 0, 0)];
        assert_eq!(
            snap_to_alignment(&mut monitors, 9, 20.0),
            SnapGuides::default()
        );
    }
}
