use egui::{Color32, Pos2};

#[derive(Debug, Clone)]
pub struct ManualPlaygroundState {
    // Chapter 1: Counter & Greeter
    pub user_name: String,
    pub click_count: usize,

    // Chapter 2: Widgets Sampler
    pub toggle_enabled: bool,
    pub slider_value: f32,
    pub color: Color32,
    pub selected_radio: usize,

    // Chapter 3: Layout Splitter
    pub split_ratio: f32,

    // Chapter 5: Mini Painting Canvas
    pub canvas_points: Vec<Option<Pos2>>,
}

impl Default for ManualPlaygroundState {
    fn default() -> Self {
        Self {
            user_name: "Ferris".to_owned(),
            click_count: 0,
            toggle_enabled: true,
            slider_value: 42.0,
            color: Color32::from_rgb(31, 111, 235),
            selected_radio: 0,
            split_ratio: 0.5,
            canvas_points: Vec::new(),
        }
    }
}
