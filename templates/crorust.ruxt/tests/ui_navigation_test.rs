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

    nav.apply_hash("#/docs/ruxt/gpu-pipeline");
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
            assert!(
                !windows.sidebar_open(),
                "Sidebar must be closed on mobile entry"
            );

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
fn test_website_view_mobile_fluid_fits_viewport() {
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
        width <= 360.0,
        "Mobile content width must fluidly fit within 360px viewport without overflow, got {width}"
    );
}

#[test]
fn test_multi_subscriber_file_delivery() {
    let file = ruxt::platform::dialog::PickedFile::new("test.png", vec![1, 2, 3, 4]);
    ruxt::platform::dialog::deliver_picked_file(file);

    let mut gen1 = 0;
    let mut gen2 = 0;

    let sub1 = ruxt::platform::dialog::poll_picked_file_subscriber(&mut gen1);
    let sub2 = ruxt::platform::dialog::poll_picked_file_subscriber(&mut gen2);

    assert!(
        sub1.is_some(),
        "Subscriber 1 must receive the delivered file"
    );
    assert!(
        sub2.is_some(),
        "Subscriber 2 must also receive the delivered file"
    );
    assert_eq!(sub1.unwrap().name, "test.png");
    assert_eq!(sub2.unwrap().name, "test.png");

    // Second poll without new file delivered must return None
    assert!(ruxt::platform::dialog::poll_picked_file_subscriber(&mut gen1).is_none());
    assert!(ruxt::platform::dialog::poll_picked_file_subscriber(&mut gen2).is_none());

    // Delivering a new file updates generation and is delivered to both subscribers
    let file2 = ruxt::platform::dialog::PickedFile::new("test2.png", vec![5, 6]);
    ruxt::platform::dialog::deliver_picked_file(file2);

    let sub1_next = ruxt::platform::dialog::poll_picked_file_subscriber(&mut gen1);
    let sub2_next = ruxt::platform::dialog::poll_picked_file_subscriber(&mut gen2);
    assert_eq!(sub1_next.unwrap().name, "test2.png");
    assert_eq!(sub2_next.unwrap().name, "test2.png");

    // Retained delivery across 3 distinct subscriber windows (Showcase, MediaViewer, Editor)
    let file3 = ruxt::platform::dialog::PickedFile::new("shared_doc.md", b"# Title".to_vec());
    ruxt::platform::dialog::deliver_picked_file(file3);

    let mut showcase_gen = 0;
    let mut media_viewer_gen = 0;
    let mut editor_gen = 0;

    let s1 = ruxt::platform::dialog::poll_picked_file_subscriber(&mut showcase_gen);
    assert!(s1.is_some());
    assert_eq!(s1.unwrap().name, "shared_doc.md");

    // Other subscribers receive the SAME file in the same or subsequent frame without starvation:
    let s2 = ruxt::platform::dialog::poll_picked_file_subscriber(&mut media_viewer_gen);
    let s3 = ruxt::platform::dialog::poll_picked_file_subscriber(&mut editor_gen);
    assert!(s2.is_some());
    assert!(s3.is_some());
    assert_eq!(s2.unwrap().name, "shared_doc.md");
    assert_eq!(s3.unwrap().name, "shared_doc.md");

    // All subscribers are now up-to-date; subsequent poll returns None:
    assert!(ruxt::platform::dialog::poll_picked_file_subscriber(&mut showcase_gen).is_none());
    assert!(ruxt::platform::dialog::poll_picked_file_subscriber(&mut media_viewer_gen).is_none());
    assert!(ruxt::platform::dialog::poll_picked_file_subscriber(&mut editor_gen).is_none());

    // Calling clear_picked_file() drops the file from static memory
    ruxt::platform::dialog::clear_picked_file();
    let mut gen_zero = 0;
    assert!(ruxt::platform::dialog::poll_picked_file_subscriber(&mut gen_zero).is_none());
}

#[test]
fn test_chapter_slug_deep_linking() {
    let mut nav = NavigationState::default();
    nav.apply_hash("#/docs/crorust/crates");
    assert_eq!(nav.mode, AppMode::Home);
    assert_eq!(nav.crorust_chapter, CrorustChapter::Crates);
    assert!(
        nav.pending_hash_sync,
        "Navigating to crorust chapter must trigger pending_hash_sync"
    );
    assert_eq!(nav.current_hash(), "#/docs/crorust/crates");

    // Headless render of WebsiteView should consume pending_hash_sync and apply vertical offset
    let mut website = ruxt::site::WebsiteView::default();
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1080.0, 800.0))
        .build_ui(|ui| {
            website.ui(ui, &mut nav);
        });
    harness.run_steps(1);
    drop(harness);
    assert!(
        !nav.pending_hash_sync,
        "WebsiteView::ui must consume pending_hash_sync"
    );

    nav.apply_hash("#/docs/ruxt/gpu-pipeline");
    assert_eq!(nav.mode, AppMode::Workspace);
    assert_eq!(
        nav.ruxt_chapter,
        ruxt::manual::chapter::RuxtTutorialChapter::GpuPipeline
    );
    assert_eq!(nav.current_hash(), "#/docs/ruxt/gpu-pipeline");
}

#[test]
fn test_workbench_adaptive_renders_headlessly() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1080.0, 800.0))
        .build_ui(|ui| {
            let mut workbench = ruxt::site::terminal::TerminalWorkbench::default();
            workbench.ui(ui);

            // Switch host and target
            workbench.host = ruxt::site::terminal::HostOS::Windows;
            workbench.target = ruxt::site::terminal::TargetPlatform::WindowsMsix;
            assert_eq!(workbench.target_triple(), "x86_64-pc-windows-msvc");

            // Switch tabs
            workbench.active_tab = ruxt::site::terminal::WorkbenchTab::CiWorkflow;
            workbench.ui(ui);
            workbench.active_tab = ruxt::site::terminal::WorkbenchTab::ManifestToml;
            workbench.ui(ui);
            workbench.active_tab = ruxt::site::terminal::WorkbenchTab::PackagingSpec;
            workbench.ui(ui);
        });

    harness.run_steps(2);
}

#[test]
fn test_workbench_mobile_touch_renders_headlessly() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(360.0, 640.0))
        .build_ui(|ui| {
            let mut workbench = ruxt::site::terminal::TerminalWorkbench::default();
            workbench.ui(ui);
            assert_eq!(workbench.host, ruxt::site::terminal::HostOS::Linux);
        });

    harness.run_steps(2);
}

#[test]
fn test_website_view_mobile_responsiveness_360px() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(360.0, 740.0))
        .build_ui(|ui| {
            let mut website = ruxt::site::WebsiteView::default();
            let mut nav = NavigationState::default();
            website.ui(ui, &mut nav);
        });

    harness.run();
    assert!(harness.ctx.globally_used_rect().height() > 300.0);
}

#[test]
fn test_website_view_extra_narrow_320px() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(320.0, 568.0))
        .build_ui(|ui| {
            let mut website = ruxt::site::WebsiteView::default();
            let mut nav = NavigationState::default();
            website.ui(ui, &mut nav);
        });

    harness.run();
    assert!(harness.ctx.globally_used_rect().height() > 300.0);
}

#[test]
fn test_crates_explorer_mobile_touch_renders_headlessly() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(360.0, 640.0))
        .build_ui(|ui| {
            let mut crates_view = ruxt::site::views::crates::CratesExplorerView::default();
            let mut nav = NavigationState::default();
            crates_view.ui(ui, &mut nav);
        });

    harness.run();
    assert!(harness.ctx.globally_used_rect().height() > 100.0);
}

#[test]
fn test_responsive_scroll_strip_primitive() {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(320.0, 400.0))
        .build_ui(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);

            let result = ruxt::ui::responsive_scroll_strip(ui, "test_strip", |ui| {
                assert_eq!(ui.style().wrap_mode, Some(egui::TextWrapMode::Extend));
                for i in 0..10 {
                    let _ = ui.button(format!("Very Long Horizontal Button Label #{i}"));
                }
                42
            });

            assert_eq!(result, 42);
        });

    harness.run();
}
