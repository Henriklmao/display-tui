use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
pub enum Rotation {
    #[default] Normal,
    Deg90,
    Deg180,
    Deg270,
}

impl Rotation {
    pub fn from_transform(transform: &Option<String>) -> Self {
        match transform.as_deref() {
            Some("90") => Rotation::Deg90,
            Some("180") => Rotation::Deg180,
            Some("270") => Rotation::Deg270,
            _ => Rotation::Normal,
        }
    }

    pub fn to_transform(&self) -> &str {
        match self {
            Rotation::Normal => "normal",
            Rotation::Deg90 => "90",
            Rotation::Deg180 => "180",
            Rotation::Deg270 => "270",
        }
    }

    pub fn to_hyprland_lua(&self) -> u8 {
        match self {
            Rotation::Normal => 0,
            Rotation::Deg90 => 1,
            Rotation::Deg180 => 2,
            Rotation::Deg270 => 3,
        }
    }

    pub fn to_hyprland(&self) -> String {
        match self {
            Rotation::Normal => "".to_string(),
            Rotation::Deg90 => "transform, 1".to_string(),
            Rotation::Deg180 => "transform, 2".to_string(),
            Rotation::Deg270 => "transform, 3".to_string(),
        }
    }

    pub fn cycle(&self) -> Self {
        match self {
            Rotation::Normal => Rotation::Deg90,
            Rotation::Deg90 => Rotation::Deg180,
            Rotation::Deg180 => Rotation::Deg270,
            Rotation::Deg270 => Rotation::Normal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Rotation;

    const ALL: [Rotation; 4] = [
        Rotation::Normal,
        Rotation::Deg90,
        Rotation::Deg180,
        Rotation::Deg270,
    ];

    #[test]
    fn default_is_normal() {
        assert_eq!(Rotation::default(), Rotation::Normal);
    }

    #[test]
    fn from_transform_recognizes_known_values() {
        assert_eq!(Rotation::from_transform(&Some("90".to_string())), Rotation::Deg90);
        assert_eq!(
            Rotation::from_transform(&Some("180".to_string())),
            Rotation::Deg180
        );
        assert_eq!(
            Rotation::from_transform(&Some("270".to_string())),
            Rotation::Deg270
        );
        assert_eq!(
            Rotation::from_transform(&Some("normal".to_string())),
            Rotation::Normal
        );
    }

    #[test]
    fn from_transform_defaults_to_normal() {
        assert_eq!(Rotation::from_transform(&None), Rotation::Normal);
        assert_eq!(Rotation::from_transform(&Some("".to_string())), Rotation::Normal);
        assert_eq!(Rotation::from_transform(&Some("45".to_string())), Rotation::Normal);
        assert_eq!(
            Rotation::from_transform(&Some("0".to_string())),
            Rotation::Normal
        );
        assert_eq!(
            Rotation::from_transform(&Some("90deg".to_string())),
            Rotation::Normal
        );
    }

    #[test]
    fn to_transform_round_trips_through_from_transform() {
        for rotation in ALL {
            let encoded = rotation.to_transform().to_string();
            assert_eq!(
                Rotation::from_transform(&Some(encoded)),
                rotation,
                "round trip failed for {:?}",
                rotation
            );
        }
    }

    #[test]
    fn to_hyprland_string_formatting() {
        assert_eq!(Rotation::Normal.to_hyprland(), "");
        assert_eq!(Rotation::Deg90.to_hyprland(), "transform, 1");
        assert_eq!(Rotation::Deg180.to_hyprland(), "transform, 2");
        assert_eq!(Rotation::Deg270.to_hyprland(), "transform, 3");
    }

    #[test]
    fn to_hyprland_lua_values_are_distinct_and_ordered() {
        let values: Vec<u8> = ALL.iter().map(|r| r.to_hyprland_lua()).collect();
        assert_eq!(values, vec![0, 1, 2, 3]);
    }

    #[test]
    fn cycle_visits_every_variant_then_wraps() {
        let mut rotation = Rotation::default();
        for expected in [
            Rotation::Deg90,
            Rotation::Deg180,
            Rotation::Deg270,
            Rotation::Normal,
        ] {
            rotation = rotation.cycle();
            assert_eq!(rotation, expected);
        }
    }

    #[test]
    fn cycle_is_reversible_with_four_steps() {
        // Four cycles must return any variant to itself.
        for rotation in ALL {
            let cycled = rotation.cycle().cycle().cycle().cycle();
            assert_eq!(cycled, rotation);
        }
    }
}
