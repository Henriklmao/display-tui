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

#[cfg(test)]
mod tests {
    use super::find_best_delta;

    #[test]
    fn picks_smallest_positive_delta() {
        let delta = find_best_delta(&[0.0, 100.0], &[50.0, 200.0], 1);
        assert_eq!(delta, Some(50.0));
    }

    #[test]
    fn picks_negative_delta_closest_to_zero() {
        // Sources 0/100, targets 90/300: the -10 delta from 100 -> 90 wins.
        let delta = find_best_delta(&[0.0, 100.0], &[90.0, 300.0], -1);
        assert_eq!(delta, Some(-10.0));
    }

    #[test]
    fn direction_selects_which_side_is_searched() {
        let sources = [10.0];
        let targets = [0.0, 20.0];

        assert_eq!(find_best_delta(&sources, &targets, 1), Some(10.0));
        assert_eq!(find_best_delta(&sources, &targets, -1), Some(-10.0));
    }

    #[test]
    fn zero_delta_is_never_selected() {
        // Already aligned: there is no strictly positive/negative gap to take.
        assert_eq!(find_best_delta(&[10.0], &[10.0], 1), None);
        assert_eq!(find_best_delta(&[10.0], &[10.0], -1), None);
    }

    #[test]
    fn returns_none_when_no_match_in_direction() {
        assert_eq!(find_best_delta(&[100.0], &[0.0], 1), None);
        assert_eq!(find_best_delta(&[0.0], &[100.0], -1), None);
    }

    #[test]
    fn returns_none_for_zero_direction() {
        assert_eq!(find_best_delta(&[0.0], &[100.0], 0), None);
    }

    #[test]
    fn returns_none_for_empty_input() {
        assert_eq!(find_best_delta(&[], &[100.0], 1), None);
        assert_eq!(find_best_delta(&[100.0], &[], 1), None);
        assert_eq!(find_best_delta(&[], &[], 1), None);
    }

    #[test]
    fn searches_every_source_target_pair() {
        // The best pair is source 1 -> target 1, not the first pair visited.
        let delta = find_best_delta(&[0.0, 300.0], &[120.0, 400.0], 1);
        assert_eq!(delta, Some(100.0));
    }

    #[test]
    fn handles_negative_positions() {
        // The winner comes from the second source even though the first one
        // also has a valid (but larger) positive delta.
        let delta = find_best_delta(&[-100.0, -50.0], &[-60.0, -40.0], 1);
        assert_eq!(delta, Some(10.0));
    }

    #[test]
    fn handles_fractional_deltas() {
        // Snap targets are often midpoints, which need not be integral.
        let delta = find_best_delta(&[0.0], &[540.5], 1);
        assert_eq!(delta, Some(540.5));
    }
}
