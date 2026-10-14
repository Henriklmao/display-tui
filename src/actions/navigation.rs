//! Navigation utilities for cycling through items.

// Cycle forward through a list, wrapping around.
pub fn cycle_forward(current: usize, length: usize) -> usize {
    if length == 0 { return current; }
    if current >= length - 1 { 0 } else { current + 1 }
}

// Cycle backward through a list, wrapping around.
pub fn cycle_backward(current: usize, length: usize) -> usize {
    if length == 0 { return current; }
    if current == 0 { length - 1 } else { current - 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cycle_forward() {
        assert_eq!(cycle_forward(0, 5), 1);
        assert_eq!(cycle_forward(4, 5), 0);
    }

    #[test]
    fn test_cycle_backward() {
        assert_eq!(cycle_backward(1, 5), 0);
        assert_eq!(cycle_backward(0, 5), 4);
    }

    #[test]
    fn empty_list_leaves_the_index_untouched() {
        assert_eq!(cycle_forward(0, 0), 0);
        assert_eq!(cycle_backward(0, 0), 0);
        // Even a nonsensical index is passed through unchanged.
        assert_eq!(cycle_forward(7, 0), 7);
        assert_eq!(cycle_backward(7, 0), 7);
    }

    #[test]
    fn single_item_list_never_moves() {
        assert_eq!(cycle_forward(0, 1), 0);
        assert_eq!(cycle_backward(0, 1), 0);
    }

    #[test]
    fn forward_cycle_over_the_whole_list_returns_to_the_start() {
        let length = 4;
        let mut index = 0;
        for _ in 0..length {
            index = cycle_forward(index, length);
        }
        assert_eq!(index, 0);

        // One step further is the second element again.
        assert_eq!(cycle_forward(index, length), 1);
    }

    #[test]
    fn backward_cycle_over_the_whole_list_returns_to_the_start() {
        let length = 4;
        let mut index = 0;
        for _ in 0..length {
            index = cycle_backward(index, length);
        }
        assert_eq!(index, 0);

        assert_eq!(cycle_backward(index, length), length - 1);
    }

    #[test]
    fn forward_normalises_out_of_range_indices() {
        assert_eq!(cycle_forward(99, 5), 0);
        assert_eq!(cycle_forward(5, 5), 0);
    }

    #[test]
    fn forward_and_backward_are_inverse_moves() {
        for index in 0..6 {
            let there = cycle_forward(index, 6);
            assert_eq!(cycle_backward(there, 6), index);
        }
    }
}
