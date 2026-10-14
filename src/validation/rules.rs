//! Validation rules for monitor configurations.

use super::errors::ValidationError;
use crate::monitor::Monitor;

// Validates that workspaces are unique across monitors.
pub fn validate_workspaces_unique(monitors: &[Monitor]) -> Result<(), Vec<ValidationError>> {
    let mut ws_counts = std::collections::HashMap::new();
    for monitor in monitors {
        if let Some(ws) = monitor.workspace {
            ws_counts
                .entry(ws)
                .or_insert_with(Vec::new)
                .push(monitor.name.clone());
        }
    }

    let mut errors = Vec::new();
    for (ws, names) in ws_counts {
        if names.len() > 1 {
            errors.push(ValidationError::DuplicateWorkspace {
                workspace: ws,
                monitors: names,
            });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

// Validates that monitors do not overlap.
pub fn validate_no_overlap(monitors: &[Monitor]) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();
    let enabled: Vec<&Monitor> = monitors.iter().filter(|m| m.enabled).collect();

    for i in 0..enabled.len() {
        for j in (i + 1)..enabled.len() {
            let (x1, y1, w1, h1) = enabled[i].get_geometry();
            let (x2, y2, w2, h2) = enabled[j].get_geometry();

            if x1 < x2 + w2 && x2 < x1 + w1 && y1 < y2 + h2 && y2 < y1 + h1 {
                errors.push(ValidationError::OverlappingMonitors {
                    monitor1: enabled[i].name.clone(),
                    monitor2: enabled[j].name.clone(),
                });
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

// Validates that enabled monitors form a contiguous area.
pub fn validate_contiguous(monitors: &[Monitor]) -> Result<(), Vec<ValidationError>> {
    let enabled_indices: Vec<usize> = monitors
        .iter()
        .enumerate()
        .filter(|(_, m)| m.enabled)
        .map(|(i, _)| i)
        .collect();

    if enabled_indices.len() <= 1 {
        return Ok(());
    }

    let mut adj = vec![vec![]; enabled_indices.len()];
    let mut geoms = Vec::new();
    for &idx in &enabled_indices {
        geoms.push(monitors[idx].get_geometry());
    }

    let eps = 2.0;
    for i in 0..geoms.len() {
        for j in (i + 1)..geoms.len() {
            let (x1, y1, w1, h1) = geoms[i];
            let (x2, y2, w2, h2) = geoms[j];

            let touches_x = x1 <= x2 + w2 + eps && x2 <= x1 + w1 + eps;
            let touches_y = y1 <= y2 + h2 + eps && y2 <= y1 + h1 + eps;

            if touches_x && touches_y {
                adj[i].push(j);
                adj[j].push(i);
            }
        }
    }

    let mut global_visited = vec![false; enabled_indices.len()];
    let mut components = Vec::new();

    for i in 0..enabled_indices.len() {
        if !global_visited[i] {
            let mut comp = Vec::new();
            let mut q = vec![i];
            global_visited[i] = true;

            while let Some(node) = q.pop() {
                comp.push(node);
                for &neighbor in &adj[node] {
                    if !global_visited[neighbor] {
                        global_visited[neighbor] = true;
                        q.push(neighbor);
                    }
                }
            }
            components.push(comp);
        }
    }

    if components.len() > 1 {
        components.sort_by_key(|a| std::cmp::Reverse(a.len()));
        let mut disconnected = Vec::new();
        for comp in components.iter().skip(1) {
            for &idx in comp {
                disconnected.push(monitors[enabled_indices[idx]].name.clone());
            }
        }
        Err(vec![ValidationError::NonContiguousMonitors {
            disconnected,
        }])
    } else {
        Ok(())
    }
}

// Validates that a preset name only contains ASCII alphanumeric characters and hyphens.
pub fn validate_preset_name(name: &str) -> Result<(), Vec<ValidationError>> {
    if !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        Ok(())
    } else {
        Err(vec![ValidationError::InvalidName])
    }
}

#[cfg(test)]
mod tests {
    use super::{
        validate_contiguous, validate_no_overlap, validate_preset_name,
        validate_workspaces_unique,
    };
    use crate::monitor::{Monitor, Position, Resolution};
    use crate::validation::errors::ValidationError;

    // A rectangular enabled monitor occupying (x, y) .. (x+w, y+h).
    fn box_at(name: &str, x: i32, y: i32, w: i32, h: i32) -> Monitor {
        Monitor {
            name: name.to_string(),
            enabled: true,
            position: Some(Position { x, y }),
            scale: Some(1.0),
            modes: vec![Resolution {
                width: w,
                height: h,
                refresh: 60.0,
                preferred: true,
                current: true,
            }],
            ..Default::default()
        }
    }

    fn disabled(mut m: Monitor) -> Monitor {
        m.enabled = false;
        m
    }

    #[test]
    fn accepts_valid_names() {
        assert!(validate_preset_name("default").is_ok());
        assert!(validate_preset_name("my-setup-1").is_ok());
        assert!(validate_preset_name("A").is_ok());
        assert!(validate_preset_name("123").is_ok());
        assert!(validate_preset_name("a-b-c-42").is_ok());
        assert!(validate_preset_name("-").is_ok()); // single hyphen is allowed
    }

    #[test]
    fn rejects_empty_name() {
        assert!(validate_preset_name("").is_err());
    }

    #[test]
    fn rejects_invalid_characters() {
        assert!(validate_preset_name("with space").is_err());
        assert!(validate_preset_name("under_score").is_err());
        assert!(validate_preset_name("dot.name").is_err());
        assert!(validate_preset_name("slash/name").is_err());
        assert!(validate_preset_name("../escape").is_err());
        assert!(validate_preset_name("unicode-é").is_err());
    }

    #[test]
    fn workspaces_may_be_unique_or_absent() {
        let monitors = vec![
            {
                let mut m = box_at("A", 0, 0, 1920, 1080);
                m.workspace = Some(1);
                m
            },
            {
                let mut m = box_at("B", 1920, 0, 1920, 1080);
                m.workspace = Some(2);
                m
            },
            box_at("C", 3840, 0, 1920, 1080), // no workspace at all
        ];

        assert_eq!(validate_workspaces_unique(&monitors), Ok(()));
    }

    #[test]
    fn duplicate_workspace_lists_every_offending_monitor() {
        let monitors = vec![
            {
                let mut m = box_at("A", 0, 0, 1920, 1080);
                m.workspace = Some(7);
                m
            },
            box_at("B", 1920, 0, 1920, 1080),
            {
                let mut m = box_at("C", 3840, 0, 1920, 1080);
                m.workspace = Some(7);
                m
            },
        ];

        let errors = validate_workspaces_unique(&monitors).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0],
            ValidationError::DuplicateWorkspace {
                workspace: 7,
                monitors: vec!["A".to_string(), "C".to_string()],
            }
        );
    }

    #[test]
    fn each_duplicated_workspace_produces_its_own_error() {
        let monitors = vec![
            {
                let mut m = box_at("A", 0, 0, 1920, 1080);
                m.workspace = Some(1);
                m
            },
            {
                let mut m = box_at("B", 1920, 0, 1920, 1080);
                m.workspace = Some(1);
                m
            },
            {
                let mut m = box_at("C", 3840, 0, 1920, 1080);
                m.workspace = Some(2);
                m
            },
            {
                let mut m = box_at("D", 5760, 0, 1920, 1080);
                m.workspace = Some(2);
                m
            },
        ];

        let errors = validate_workspaces_unique(&monitors).unwrap_err();
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn no_overlap_for_adjacent_monitors() {
        // Sharing an edge is fine: the comparison is strict.
        let monitors = vec![
            box_at("A", 0, 0, 1920, 1080),
            box_at("B", 1920, 0, 1920, 1080),
            box_at("C", 0, 1080, 1920, 1080),
        ];

        assert_eq!(validate_no_overlap(&monitors), Ok(()));
    }

    #[test]
    fn overlap_is_detected_in_both_orders() {
        let monitors = vec![box_at("A", 0, 0, 1920, 1080), box_at("B", 100, 0, 1920, 1080)];

        let errors = validate_no_overlap(&monitors).unwrap_err();
        assert_eq!(
            errors,
            vec![ValidationError::OverlappingMonitors {
                monitor1: "A".to_string(),
                monitor2: "B".to_string(),
            }]
        );

        // Same pair, reversed: still exactly one error.
        let reversed = vec![box_at("B", 100, 0, 1920, 1080), box_at("A", 0, 0, 1920, 1080)];
        let errors = validate_no_overlap(&reversed).unwrap_err();
        assert_eq!(
            errors,
            vec![ValidationError::OverlappingMonitors {
                monitor1: "B".to_string(),
                monitor2: "A".to_string(),
            }]
        );
    }

    #[test]
    fn overlap_at_a_single_corner_point_counts() {
        // Strict inequality means touching at exactly one point is not overlap,
        // but one pixel of penetration is.
        let touching = vec![box_at("A", 0, 0, 1920, 1080), box_at("B", 1920, 1080, 1920, 1080)];
        assert_eq!(validate_no_overlap(&touching), Ok(()));

        // One pixel of penetration on both axes.
        let nudged = vec![box_at("A", 0, 0, 1920, 1080), box_at("B", 1919, 1079, 1920, 1080)];
        assert!(validate_no_overlap(&nudged).is_err());
    }

    #[test]
    fn overlap_ignores_disabled_monitors() {
        let monitors = vec![
            box_at("A", 0, 0, 1920, 1080),
            disabled(box_at("B", 100, 100, 1920, 1080)),
        ];

        assert_eq!(validate_no_overlap(&monitors), Ok(()));
    }

    #[test]
    fn overlap_reports_every_conflicting_pair() {
        let monitors = vec![
            box_at("A", 0, 0, 1920, 1080),
            box_at("B", 100, 0, 1920, 1080),
            box_at("C", 200, 0, 1920, 1080),
        ];

        let errors = validate_no_overlap(&monitors).unwrap_err();
        assert_eq!(errors.len(), 3, "A-B, A-C and B-C all overlap");
    }

    #[test]
    fn contiguous_layout_with_two_or_fewer_monitors_is_always_valid() {
        assert_eq!(validate_contiguous(&[]), Ok(()));
        assert_eq!(validate_contiguous(&[box_at("A", 0, 0, 1920, 1080)]), Ok(()));
        assert_eq!(
            validate_contiguous(&[disabled(box_at("A", 0, 0, 1920, 1080))]),
            Ok(())
        );
    }

    #[test]
    fn touching_monitors_are_contiguous() {
        let monitors = vec![box_at("A", 0, 0, 1920, 1080), box_at("B", 1920, 0, 1920, 1080)];

        assert_eq!(validate_contiguous(&monitors), Ok(()));
    }

    #[test]
    fn sub_two_pixel_gaps_still_count_as_touching() {
        // The rule allows a 2.0px epsilon so that rounding never splits a layout.
        let monitors = vec![box_at("A", 0, 0, 1920, 1080), box_at("B", 1922, 0, 1920, 1080)];

        assert_eq!(validate_contiguous(&monitors), Ok(()));
    }

    #[test]
    fn separated_monitors_are_reported_as_disconnected() {
        let monitors = vec![
            box_at("A", 0, 0, 1920, 1080),
            box_at("B", 10_000, 0, 1920, 1080),
        ];

        let errors = validate_contiguous(&monitors).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0],
            ValidationError::NonContiguousMonitors {
                // The largest component is kept, only the other one is listed.
                disconnected: vec!["B".to_string()],
            }
        );
    }

    #[test]
    fn only_the_smallest_components_are_reported_as_disconnected() {
        // A and B touch, C is far away.
        let monitors = vec![
            box_at("A", 0, 0, 1920, 1080),
            box_at("B", 1920, 0, 1920, 1080),
            box_at("C", 20_000, 0, 1920, 1080),
        ];

        let errors = validate_contiguous(&monitors).unwrap_err();
        assert_eq!(
            errors,
            vec![ValidationError::NonContiguousMonitors {
                disconnected: vec!["C".to_string()],
            }]
        );
    }

    #[test]
    fn contiguity_follows_chains_of_touching_monitors() {
        // C touches neither A nor B directly, but the chain A-B-C keeps them
        // in a single component.
        let monitors = vec![
            box_at("A", 0, 0, 100, 100),
            box_at("B", 100, 0, 100, 100),
            box_at("C", 200, 0, 100, 100),
        ];

        assert_eq!(validate_contiguous(&monitors), Ok(()));
    }

    #[test]
    fn contiguity_ignores_disabled_monitors() {
        let monitors = vec![
            box_at("A", 0, 0, 1920, 1080),
            box_at("B", 1920, 0, 1920, 1080),
            disabled(box_at("C", 20_000, 0, 1920, 1080)),
        ];

        assert_eq!(validate_contiguous(&monitors), Ok(()));
    }
}
