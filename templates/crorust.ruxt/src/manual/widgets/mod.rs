pub mod diagram_viewer;

use egui::{Color32, Pos2};

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ManualPlaygroundState {
    // --- Chapter 1 & 2: Platform Foundation & Bridge Labs ---
    /// System clipboard diagnostic payload tested in Ch 2.
    pub test_clipboard_input: String,
    /// Currently hovered cursor name in cursor tester.
    pub active_cursor_name: String,

    // --- Chapter 3: IMGUI Paradigm & ID Hashing Labs ---
    /// Whether unique ID scoping (ui.push_id / salt) is enabled in collision lab.
    pub collision_scoping_enabled: bool,
    /// Counter A for collision testing.
    pub collision_counter_a: i32,
    /// Counter B for collision testing.
    pub collision_counter_b: i32,
    /// Individual expansion state for Item 0.
    pub item_0_expanded: bool,
    /// Individual expansion state for Item 1.
    pub item_1_expanded: bool,
    /// Shared expansion state when collision scoping is disabled.
    pub shared_expanded: bool,

    // --- Interactive Diagram Viewer States ---
    #[cfg_attr(feature = "serde", serde(skip))]
    pub diagram_states: std::collections::HashMap<&'static str, diagram_viewer::DiagramViewerState>,

    // --- Chapter 4: Context Internals & Theming Sandbox ---
    /// Corner rounding radius for live theme customization.
    pub style_corner_radius: f32,
    /// Item spacing for live layout customization.
    pub style_item_spacing: f32,
    /// Dark mode override toggle.
    pub dark_mode_override: Option<bool>,

    // --- Chapter 5: Layout Geometry & Custom Widget Workshop ---
    /// Current rotary knob dial value (0.0 ..= 100.0).
    pub knob_value: f32,

    // --- Chapter 6: GPU Pipeline & Tessellation Wireframe Viewer ---
    /// Whether wireframe rendering is enabled in tessellation lab.
    pub wireframe_enabled: bool,
    /// Number of polygon subdivisions for circle/curve tessellation.
    pub wireframe_tessellation_subdivisions: usize,

    // --- Chapter 7: Multi-Platform Invariants & Ergonomics ---
    /// Whether 48dp mobile touch target ergonomics are active in live tester.
    pub touch_target_ergonomics_active: bool,

    // --- Retained / Interactive Sampler Compatibility Fields ---
    pub user_name: String,
    pub click_count: usize,
    pub toggle_enabled: bool,
    pub slider_value: f32,
    pub color: Color32,
    pub selected_radio: usize,
    pub split_ratio: f32,
    pub canvas_points: Vec<Option<Pos2>>,
}

impl Default for ManualPlaygroundState {
    fn default() -> Self {
        Self {
            test_clipboard_input: "Hello from Ruxt Platform Bridge!".to_owned(),
            active_cursor_name: "Default".to_owned(),
            collision_scoping_enabled: false,
            collision_counter_a: 0,
            collision_counter_b: 0,
            item_0_expanded: false,
            item_1_expanded: false,
            shared_expanded: false,
            diagram_states: std::collections::HashMap::new(),
            style_corner_radius: 6.0,
            style_item_spacing: 8.0,
            dark_mode_override: None,
            knob_value: 45.0,
            wireframe_enabled: false,
            wireframe_tessellation_subdivisions: 16,
            touch_target_ergonomics_active: true,
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

impl ManualPlaygroundState {
    pub fn diagram_state_mut(
        &mut self,
        key: &'static str,
    ) -> &mut diagram_viewer::DiagramViewerState {
        self.diagram_states.entry(key).or_default()
    }
}
