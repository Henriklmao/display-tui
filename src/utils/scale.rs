//! Scale value definitions.

// Available display scale values.
pub struct ScaleValue {
    pub name: &'static str,
    pub value: f32,
}

impl ScaleValue {
    // Creates a new scale value.
    pub fn new(name: &'static str, value: f32) -> Self {
        ScaleValue { name, value }
    }

    // Returns the standard table of available scale values.
    pub fn table() -> Vec<Self> {
        vec![
            ScaleValue::new("50%", 0.5),
            ScaleValue::new("66%", 0.66),
            ScaleValue::new("75%", 0.75),
            ScaleValue::new("80%", 0.8),
            ScaleValue::new("100%", 1.0),
            ScaleValue::new("125%", 1.25),
            ScaleValue::new("160%", 1.6),
            ScaleValue::new("175%", 1.75),
            ScaleValue::new("200%", 2.0),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::ScaleValue;

    #[test]
    fn new_stores_name_and_value() {
        let scale = ScaleValue::new("150%", 1.5);
        assert_eq!(scale.name, "150%");
        assert_eq!(scale.value, 1.5);
    }

    #[test]
    fn table_contains_expected_entries() {
        let table = ScaleValue::table();
        assert_eq!(table.len(), 9);
        assert_eq!(table.first().unwrap().name, "50%");
        assert_eq!(table.last().unwrap().name, "200%");
    }

    #[test]
    fn table_contents_are_pinned() {
        // Snapshot of the whole table so accidental edits to labels or values
        // are caught. Every label must match its stored value, so "66%"
        // maps to 0.66.
        let table: Vec<(&str, f32)> = ScaleValue::table()
            .iter()
            .map(|s| (s.name, s.value))
            .collect();
        assert_eq!(
            table,
            vec![
                ("50%", 0.5),
                ("66%", 0.66),
                ("75%", 0.75),
                ("80%", 0.8),
                ("100%", 1.0),
                ("125%", 1.25),
                ("160%", 1.6),
                ("175%", 1.75),
                ("200%", 2.0),
            ]
        );
    }

    #[test]
    fn table_labels_are_percentages() {
        for scale in ScaleValue::table() {
            assert!(
                scale.name.ends_with('%') && scale.name.len() > 1,
                "unexpected label {:?}",
                scale.name
            );
        }
    }

    #[test]
    fn table_is_sorted_ascending_and_duplicate_free() {
        let table = ScaleValue::table();
        for pair in table.windows(2) {
            assert!(
                pair[1].value > pair[0].value,
                "values must strictly increase: {} then {}",
                pair[0].value,
                pair[1].value
            );
            assert_ne!(pair[0].name, pair[1].name);
        }
    }

    #[test]
    fn table_includes_the_default_scale() {
        assert!(
            ScaleValue::table().iter().any(|s| s.value == 1.0),
            "100% must be selectable"
        );
    }
}
