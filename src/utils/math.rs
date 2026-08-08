//! Math utilities for snapping and alignment.

// Finds the smallest positive delta from sources to targets in a given direction.
//
// Returns the best delta (closest source-target pair) or None if no match.
pub fn find_best_delta(sources: &[f64], targets: &[f64], direction: i32) -> Option<f64> {
    let mut best_delta = None;
    let mut best_diff = f64::MAX;

    for &src in sources {
        for &tgt in targets {
            let delta = tgt - src;
            if direction > 0 && delta > 0.0 && delta < best_diff {
                best_diff = delta;
                best_delta = Some(delta);
            } else if direction < 0 && delta < 0.0 && -delta < best_diff {
                best_diff = -delta;
                best_delta = Some(delta);
            }
        }
    }
    best_delta
}
