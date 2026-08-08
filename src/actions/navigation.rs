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
}
