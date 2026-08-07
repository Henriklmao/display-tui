#[derive(Default,Debug, Clone, Copy, PartialEq)]
pub enum TUIMode {
    #[default]
    View,
    Move,
    Resolution,
    Scale
}

pub struct ScaleValue {
    pub name: &'static str,
    pub value: f32,
}

impl ScaleValue {
    pub fn new(name: &'static str, value: f32) -> Self {
        ScaleValue { name, value }
    }
    pub fn table() -> Vec<Self> {
        vec![
            ScaleValue::new("50%", 0.5),
            ScaleValue::new("66%", 0.6),
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

pub fn find_best_delta(sources: &[f64], targets: &[f64], direction: i32) -> Option<f64> {
    let mut best_delta: Option<f64> = None;

    for s in sources {
        for t in targets {
            let diff = t - s;
            if (direction < 0 && diff < -0.1) || (direction > 0 && diff > 0.1) {
                match best_delta {
                    None => best_delta = Some(diff),
                    Some(current) => {
                        if diff.abs() < current.abs() {
                            best_delta = Some(diff);
                        }
                    }
                }
            }
        }
    }
    best_delta
}

pub fn change_mode(app: &mut crate::App, mode: TUIMode) {
    if app.mode == TUIMode::Move || app.mode == TUIMode::Scale {
        let _ = crate::configuration::Configuration::save_monitor_state(&app.monitors);
    }
    app.mode = mode;
}
