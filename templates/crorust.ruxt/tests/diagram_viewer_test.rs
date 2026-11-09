#[cfg(test)]
mod tests {
    use egui::vec2;
    use egui_kittest::Harness;
    use ruxt::manual::widgets::diagram_viewer::{DiagramViewerState, InteractiveDiagramViewer};

    #[test]
    fn test_diagram_viewer_render_desktop() {
        let mut state = DiagramViewerState::default();
        let viewer = InteractiveDiagramViewer::new(
            "test_diagram",
            egui::include_image!("../assets/manual/svg/ch01_event_loop.svg"),
            "graph TD; A-->B;",
            vec2(1200.0, 900.0),
        );

        let mut harness = Harness::builder()
            .with_size(vec2(1024.0, 768.0))
            .build_ui(|ui| {
                viewer.show(ui, &mut state);
            });

        harness.run_steps(2);
        drop(harness);

        assert_eq!(state.zoom, 1.0);
        assert_eq!(state.pan, egui::Vec2::ZERO);
    }

    #[test]
    fn test_diagram_viewer_render_mobile_and_pan_toggle() {
        let mut state = DiagramViewerState::default();
        let viewer = InteractiveDiagramViewer::new(
            "test_diagram",
            egui::include_image!("../assets/manual/svg/ch01_event_loop.svg"),
            "graph TD; A-->B;",
            vec2(1200.0, 900.0),
        );

        // Mobile screen size simulation (360x640)
        let mut harness = Harness::builder()
            .with_size(vec2(360.0, 640.0))
            .build_ui(|ui| {
                viewer.show(ui, &mut state);
            });

        harness.run_steps(2);
        drop(harness);

        // State starts with pan_active false so touch scrolling passes through
        assert!(!state.pan_active);

        // Toggle pan_active and run in new harness pass
        state.pan_active = true;
        let mut harness = Harness::builder()
            .with_size(vec2(360.0, 640.0))
            .build_ui(|ui| {
                viewer.show(ui, &mut state);
            });

        harness.run_steps(1);
        drop(harness);
        assert!(state.pan_active);
    }

    #[test]
    fn test_diagram_viewer_zoom_clamping_invariant() {
        let mut state = DiagramViewerState {
            zoom: 12.0,
            ..Default::default()
        };
        state.clamp_zoom();
        assert_eq!(state.zoom, 4.0);

        state.zoom = 0.05;
        state.clamp_zoom();
        assert_eq!(state.zoom, 0.5);
    }

    #[test]
    fn test_diagram_viewer_continuous_zoom_and_pan() {
        let state = std::sync::Arc::new(std::sync::Mutex::new(DiagramViewerState::default()));
        let state_clone = state.clone();
        let viewer = InteractiveDiagramViewer::new(
            "test_diagram_multi_frame",
            egui::include_image!("../assets/manual/svg/ch01_event_loop.svg"),
            "graph TD; A-->B;",
            vec2(1200.0, 900.0),
        );

        let mut harness = Harness::builder()
            .with_size(vec2(800.0, 600.0))
            .build_ui(move |ui| {
                viewer.show(ui, &mut state_clone.lock().unwrap());
            });

        // Simulate 10 frames of continuous zoom changes with clamping
        for i in 1..=10 {
            {
                let mut s = state.lock().unwrap();
                s.zoom = 1.0 + (i as f32 * 0.5);
                s.clamp_zoom();
                s.pan += vec2(5.0 * i as f32, -3.0 * i as f32);
            }
            harness.run_steps(1);
        }
        drop(harness);

        let final_state = state.lock().unwrap();
        assert_eq!(final_state.zoom, 4.0);
        assert!(final_state.pan.x > 0.0);
    }

    #[test]
    fn test_diagram_viewer_state_lifecycle_defaults_and_reset() {
        let initial = DiagramViewerState::default();
        assert_eq!(initial.zoom, 1.0);
        assert_eq!(initial.pan, egui::Vec2::ZERO);
        assert!(!initial.is_fullscreen);
        assert!(!initial.show_source);
        assert!(!initial.pan_active);
        assert_eq!(initial.last_interaction_time, 0.0);
        assert_eq!(initial.active_lod, 1.5);

        let mut state = DiagramViewerState {
            zoom: 3.5,
            pan: vec2(250.0, -140.0),
            is_fullscreen: true,
            show_source: true,
            pan_active: true,
            last_interaction_time: 42.0,
            active_lod: 3.0,
        };

        state.reset();

        assert_eq!(state.zoom, initial.zoom);
        assert_eq!(state.pan, initial.pan);
        assert_eq!(state.is_fullscreen, initial.is_fullscreen);
        assert_eq!(state.show_source, initial.show_source);
        assert_eq!(state.pan_active, initial.pan_active);
        assert_eq!(state.last_interaction_time, initial.last_interaction_time);
        assert_eq!(state.active_lod, initial.active_lod);
    }

    #[test]
    fn test_svg_sanitization_runtime() {
        let mock_svg = br#"<svg><g class="label"><rect/><g><rect class="background" style="stroke: none"/><text>Hello</text></g></g></svg>"#;
        let cleaned = ruxt::platform::svg_loader::sanitize_svg_bytes(mock_svg);
        let cleaned_str = std::str::from_utf8(&cleaned).unwrap();

        assert!(!cleaned_str.contains("<rect/>"));
        assert!(!cleaned_str.contains("style=\"stroke: none\"/>"));
        assert!(cleaned_str.contains("<text>Hello</text>"));

        // Permutations with different whitespace, semicolon, and tag forms:
        let variations = br#"<svg><rect   /><rect style="stroke:none;" class="background"/><rect width="100" height="50" fill="red"/><text>Hi</text></svg>"#;
        let cleaned_vars = ruxt::platform::svg_loader::sanitize_svg_bytes(variations);
        let vars_str = std::str::from_utf8(&cleaned_vars).unwrap();

        assert!(!vars_str.contains("<rect   />"));
        assert!(!vars_str.contains("stroke:none"));
        // Legitimate dimensioned rect must be preserved:
        assert!(vars_str.contains("<rect width=\"100\" height=\"50\" fill=\"red\"/>"));
        assert!(vars_str.contains("<text>Hi</text>"));
    }
}
