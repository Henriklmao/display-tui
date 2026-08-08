//! Validation rules for monitor configurations.

use super::errors::ValidationError;
use crate::monitor::Monitor;

// Validates that workspaces are unique across monitors.
pub fn validate_workspaces_unique(monitors: &[Monitor]) -> Result<(), Vec<ValidationError>> {
    let mut ws_counts = std::collections::HashMap::new();
    for monitor in monitors {
        if let Some(ws) = monitor.workspace {
            ws_counts.entry(ws).or_insert_with(Vec::new).push(monitor.name.clone());
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
        Err(vec![ValidationError::NonContiguousMonitors { disconnected }])
    } else {
        Ok(())
    }
}
