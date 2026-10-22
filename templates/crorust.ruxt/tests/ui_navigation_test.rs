use egui_kittest::Harness;
use ruxt::navigation::{AppMode, CrorustChapter, NavigationState};

#[test]
fn test_two_pillar_navigation_transitions() {
    let mut nav = NavigationState::default();
    assert_eq!(nav.mode, AppMode::Home);
    assert_eq!(nav.current_hash(), "#/");

    // Transition to workspace
    nav.navigate_to_mode(AppMode::Workspace);
    assert_eq!(nav.mode, AppMode::Workspace);
    assert_eq!(nav.current_hash(), "#/workspace");

    // Transition back to home
    nav.navigate_to_mode(AppMode::Home);
    assert_eq!(nav.mode, AppMode::Home);
    assert_eq!(nav.current_hash(), "#/");

    // Hash routing aliases
    nav.apply_hash("#/workspace");
    assert_eq!(nav.mode, AppMode::Workspace);

    nav.apply_hash("#/showcase");
    assert_eq!(nav.mode, AppMode::Workspace);

    nav.apply_hash("#/docs/ruxt/graphics");
    assert_eq!(nav.mode, AppMode::Workspace);

    nav.apply_hash("#/docs/crorust/crates");
    assert_eq!(nav.mode, AppMode::Home);
    assert_eq!(nav.crorust_chapter, CrorustChapter::Crates);
}

#[test]
fn test_website_view_renders_headlessly() {
    let mut harness = Harness::new_ui(|ui| {
        let mut website = ruxt::site::WebsiteView::default();
        let mut nav = NavigationState::default();
        website.ui(ui, &mut nav);
    });

    harness.run();
    assert!(harness.ctx.globally_used_rect().height() > 300.0);
}

#[test]
fn test_workspace_manual_renders_headlessly() {
    let mut harness = Harness::new_ui(|ui| {
        let mut manual = ruxt::manual::RuxtManualWindow::default();
        manual.ui(ui);
    });

    harness.run();
    assert!(harness.ctx.globally_used_rect().height() > 100.0);
}

#[test]
fn test_workspace_window_manager_renders_headlessly() {
    let mut harness = Harness::new_ui(|ui| {
        let mut windows = ruxt::workspace::WindowManager::default();
        windows.ui(ui);
        assert!(windows.is_window_open("📖 Ruxt Framework Manual"));
        assert!(windows.is_window_open("About crorust.ruxt"));
    });

    harness.run();
}

#[test]
fn test_workspace_mobile_rules() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(360.0, 640.0))
        .build_ui(|ui| {
            let mut windows = ruxt::workspace::WindowManager::default();
            windows.ui(ui);

            // On mobile screen (< 550px):
            // 1. Sidebar is closed
            assert!(!windows.sidebar_open(), "Sidebar must be closed on mobile entry");

            // 2. ONLY the Ruxt Framework Manual is open
            assert!(
                windows.is_window_open("📖 Ruxt Framework Manual"),
                "Ruxt Framework Manual must be open"
            );
            assert!(
                !windows.is_window_open("About crorust.ruxt"),
                "About window must be closed on mobile"
            );
            assert!(
                !windows.is_window_open("📦 Widget Showcase"),
                "Widget Showcase must be closed on mobile"
            );
            assert!(
                !windows.is_window_open("📝 Markdown Editor"),
                "Markdown Editor must be closed on mobile"
            );
        });

    harness.run();
}

#[test]
fn test_website_view_desktop_no_horizontal_scroll() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1200.0, 800.0))
        .build_ui(|ui| {
            let mut website = ruxt::site::WebsiteView::default();
            let mut nav = NavigationState::default();
            let output = website.ui(ui, &mut nav);

            // On 1200px desktop, content fits well within viewport bounds (zero horizontal overflow)
            assert!(
                output.content_size.x <= 1200.0,
                "Desktop content width must not overflow viewport (1200px), got {}",
                output.content_size.x
            );
        });

    harness.run_steps(2);
}

#[test]
fn test_website_view_split_screen_desktop_no_horizontal_scroll() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(700.0, 800.0))
        .build_ui(|ui| {
            let mut website = ruxt::site::WebsiteView::default();
            let mut nav = NavigationState::default();
            let output = website.ui(ui, &mut nav);

            // On 700px split-screen, content must fit without horizontal overflow
            assert!(
                output.content_size.x <= 700.0,
                "Desktop split screen content width must not overflow 700px, got {}",
                output.content_size.x
            );
        });

    harness.run_steps(2);
}

#[test]
fn test_website_view_mobile_horizontal_scroll_no_squish() {
    let measured_width = std::sync::Arc::new(std::sync::Mutex::new(0.0f32));
    let measured_width_clone = measured_width.clone();

    let mut harness = Harness::builder()
        .with_size(egui::vec2(360.0, 640.0))
        .build_ui(move |ui| {
            let mut website = ruxt::site::WebsiteView::default();
            let mut nav = NavigationState::default();
            let output = website.ui(ui, &mut nav);
            if let Ok(mut w) = measured_width_clone.lock() {
                *w = output.content_size.x;
            }
        });

    harness.run_steps(2);
    let width = *measured_width.lock().unwrap();
    assert!(
        width >= 560.0,
        "Mobile content width must be >= 560.0px to prevent squishing, got {width}"
    );
}
