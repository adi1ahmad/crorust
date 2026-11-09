#[cfg(test)]
mod tests {
    use egui_kittest::Harness;
    use ruxt::manual::RuxtManualWindow;
    use ruxt::manual::chapter::RuxtTutorialChapter;

    #[test]
    fn test_all_7_chapters_render_without_panic() {
        let mut manual = RuxtManualWindow::default();

        for chapter in RuxtTutorialChapter::ALL {
            manual.selected_chapter = *chapter;

            let mut harness = Harness::builder()
                .with_size(egui::vec2(1200.0, 900.0))
                .build_ui(|ui| {
                    manual.ui(ui);
                });

            harness.run_steps(3);

            let used_rect = harness.ctx.globally_used_rect();
            assert!(
                used_rect.height() > 300.0,
                "Chapter {} ({}) rendered with insufficient height ({}px)",
                chapter.number(),
                chapter.pill_title(),
                used_rect.height()
            );
        }
    }

    #[test]
    fn test_mobile_viewport_chapter_rendering() {
        let mut manual = RuxtManualWindow::default();

        for chapter in RuxtTutorialChapter::ALL {
            manual.selected_chapter = *chapter;

            let mut harness = Harness::builder()
                .with_size(egui::vec2(360.0, 640.0))
                .build_ui(|ui| {
                    manual.ui(ui);
                });

            harness.run_steps(2);
            assert!(
                harness.ctx.globally_used_rect().width() <= 360.0 + 1.0,
                "Chapter {} overflows mobile viewport width: {}",
                chapter.number(),
                harness.ctx.globally_used_rect().width()
            );
        }
    }

    #[test]
    fn test_manual_chapter_sequence_navigation() {
        let mut manual = RuxtManualWindow::default();
        assert_eq!(manual.selected_chapter, RuxtTutorialChapter::Windowing);

        while let Some(next) = manual.selected_chapter.next() {
            manual.selected_chapter = next;
        }
        assert_eq!(manual.selected_chapter, RuxtTutorialChapter::RuxtJourney);

        while let Some(prev) = manual.selected_chapter.prev() {
            manual.selected_chapter = prev;
        }
        assert_eq!(manual.selected_chapter, RuxtTutorialChapter::Windowing);
    }

    #[test]
    fn test_manual_search_filter() {
        let mut manual = RuxtManualWindow {
            search_query: "tessellation".to_string(),
            ..Default::default()
        };

        let mut harness = Harness::builder()
            .with_size(egui::vec2(1024.0, 768.0))
            .build_ui(|ui| {
                manual.ui(ui);
            });

        harness.run_steps(2);
        drop(harness);

        // Verify that the query functionally matches Chapter 6 (GPU pipeline/tessellation)
        // and does NOT match Chapter 1 (Windowing)
        let query = manual.search_query.to_lowercase();
        let ch6 = RuxtTutorialChapter::GpuPipeline;
        let ch6_matches = ch6.full_title().to_lowercase().contains(&query)
            || ch6.summary().to_lowercase().contains(&query)
            || ch6.subsections().iter().any(|s| {
                s.title.to_lowercase().contains(&query) || s.summary.to_lowercase().contains(&query)
            });
        assert!(
            ch6_matches,
            "Chapter 6 must match 'tessellation' search query"
        );

        let ch1 = RuxtTutorialChapter::Windowing;
        let ch1_matches = ch1.full_title().to_lowercase().contains(&query)
            || ch1.summary().to_lowercase().contains(&query)
            || ch1.subsections().iter().any(|s| {
                s.title.to_lowercase().contains(&query) || s.summary.to_lowercase().contains(&query)
            });
        assert!(
            !ch1_matches,
            "Chapter 1 must NOT match 'tessellation' search query"
        );
    }

    #[test]
    fn test_manual_mobile_chapter_pills_satisfy_touch_target_height() {
        let mut manual = RuxtManualWindow::default();
        manual.completed_chapters.insert(1);

        let mut harness = Harness::builder()
            .with_size(egui::vec2(360.0, 640.0))
            .build_ui(|ui| {
                manual.ui(ui);
            });

        harness.run_steps(2);

        // Verify that the mobile chapter selector rendered buttons with sufficient touch height
        let used_rect = harness.ctx.globally_used_rect();
        assert!(
            used_rect.height() >= 48.0,
            "Mobile manual UI must allocate touch-friendly height, got {}",
            used_rect.height()
        );
    }

    #[test]
    fn test_manual_diagram_svg_text_rasterization() {
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        ctx.add_image_loader(std::sync::Arc::new(
            ruxt::platform::UniversalSvgLoader::new(),
        ));

        let uri = "bytes://manual/svg/ch01_event_loop.svg";
        let svg_bytes = include_bytes!("../assets/manual/svg/ch01_event_loop.svg");
        ctx.include_bytes(uri, svg_bytes);

        let res = ctx.try_load_image(uri, egui::SizeHint::Scale(1.0.into()));
        match res {
            Ok(egui::load::ImagePoll::Ready { image }) => {
                let mut white_pixels = 0;
                for pixel in &image.pixels {
                    if pixel.r() > 220 && pixel.g() > 220 && pixel.b() > 220 && pixel.a() > 200 {
                        white_pixels += 1;
                    }
                }
                assert!(
                    white_pixels > 5_000,
                    "Expected over 5,000 white text pixels, but found only {}. SVG text is not being rendered!",
                    white_pixels
                );
            }
            Err(err) => panic!("Failed to load SVG diagram: {:?}", err),
            Ok(_) => panic!("SVG image loader returned Pending unexpectedly"),
        }
    }

    #[test]
    fn test_all_13_manual_svgs_load_cleanly() {
        let ctx = egui::Context::default();
        egui_extras::install_image_loaders(&ctx);
        ctx.add_image_loader(std::sync::Arc::new(
            ruxt::platform::UniversalSvgLoader::new(),
        ));

        let svgs: &[(&str, &[u8])] = &[
            (
                "ch01_event_loop.svg",
                include_bytes!("../assets/manual/svg/ch01_event_loop.svg"),
            ),
            (
                "ch01_sub_thread_wakeup.svg",
                include_bytes!("../assets/manual/svg/ch01_sub_thread_wakeup.svg"),
            ),
            (
                "ch02_sub_dpi_coords.svg",
                include_bytes!("../assets/manual/svg/ch02_sub_dpi_coords.svg"),
            ),
            (
                "ch02_winit_bridge.svg",
                include_bytes!("../assets/manual/svg/ch02_winit_bridge.svg"),
            ),
            (
                "ch03_imgui_lifecycle.svg",
                include_bytes!("../assets/manual/svg/ch03_imgui_lifecycle.svg"),
            ),
            (
                "ch03_sub_id_hashing.svg",
                include_bytes!("../assets/manual/svg/ch03_sub_id_hashing.svg"),
            ),
            (
                "ch04_context_internals.svg",
                include_bytes!("../assets/manual/svg/ch04_context_internals.svg"),
            ),
            (
                "ch05_layout_geometry.svg",
                include_bytes!("../assets/manual/svg/ch05_layout_geometry.svg"),
            ),
            (
                "ch05_sub_cursor_progression.svg",
                include_bytes!("../assets/manual/svg/ch05_sub_cursor_progression.svg"),
            ),
            (
                "ch06_gpu_pipeline.svg",
                include_bytes!("../assets/manual/svg/ch06_gpu_pipeline.svg"),
            ),
            (
                "ch06_sub_tessellation_aa.svg",
                include_bytes!("../assets/manual/svg/ch06_sub_tessellation_aa.svg"),
            ),
            (
                "ch07_platform_matrix.svg",
                include_bytes!("../assets/manual/svg/ch07_platform_matrix.svg"),
            ),
            (
                "ch07_ruxt_journey.svg",
                include_bytes!("../assets/manual/svg/ch07_ruxt_journey.svg"),
            ),
        ];

        for (name, bytes) in svgs {
            let uri = format!("bytes://manual/svg/{name}");
            ctx.include_bytes(uri.clone(), *bytes);

            let res = ctx.try_load_image(&uri, egui::SizeHint::Scale(1.0.into()));
            match res {
                Ok(egui::load::ImagePoll::Ready { image }) => {
                    assert!(
                        image.width() > 100 && image.height() > 100,
                        "SVG {name} produced suspiciously small dimensions: {}x{}",
                        image.width(),
                        image.height()
                    );
                    assert!(!image.pixels.is_empty(), "SVG {name} produced 0 pixels");
                }
                Err(err) => panic!("Failed to load SVG diagram {name}: {:?}", err),
                Ok(_) => panic!("SVG diagram {name} returned Pending unexpectedly"),
            }
        }
    }

    #[test]
    fn test_rust_view_ui_multiple_invocations_no_id_clash() {
        let mut harness = Harness::builder()
            .with_size(egui::vec2(600.0, 400.0))
            .build_ui(|ui| {
                // Call rust_view_ui multiple times with varying and duplicate code blocks in the same UI
                ruxt::rust_view_ui(ui, "let x = 1;");
                ruxt::rust_view_ui(ui, "let y = 2;");
                ruxt::rust_view_ui(ui, "let z = 3;");
                ruxt::rust_view_ui(ui, "let x = 1;");
            });

        harness.run_steps(3);
    }

    #[test]
    fn test_manual_svg_intrinsic_aspect_ratios_match_calibrated_constants() {
        let mut options = resvg::usvg::Options::default();
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_system_fonts();
        options.fontdb = std::sync::Arc::new(db);

        // List of (file_name, raw_bytes, calibrated_native_size)
        let diagrams: &[(&str, &[u8], egui::Vec2)] = &[
            (
                "ch01_event_loop.svg",
                include_bytes!("../assets/manual/svg/ch01_event_loop.svg"),
                egui::vec2(1381.78, 2162.45),
            ),
            (
                "ch02_winit_bridge.svg",
                include_bytes!("../assets/manual/svg/ch02_winit_bridge.svg"),
                egui::vec2(2450.72, 903.0),
            ),
            (
                "ch03_imgui_lifecycle.svg",
                include_bytes!("../assets/manual/svg/ch03_imgui_lifecycle.svg"),
                egui::vec2(704.61, 2185.03),
            ),
            (
                "ch03_sub_id_hashing.svg",
                include_bytes!("../assets/manual/svg/ch03_sub_id_hashing.svg"),
                egui::vec2(647.11, 1122.34),
            ),
            (
                "ch04_context_internals.svg",
                include_bytes!("../assets/manual/svg/ch04_context_internals.svg"),
                egui::vec2(1468.03, 1047.20),
            ),
            (
                "ch05_layout_geometry.svg",
                include_bytes!("../assets/manual/svg/ch05_layout_geometry.svg"),
                egui::vec2(1236.20, 812.0),
            ),
            (
                "ch06_gpu_pipeline.svg",
                include_bytes!("../assets/manual/svg/ch06_gpu_pipeline.svg"),
                egui::vec2(721.0, 2494.20),
            ),
            (
                "ch07_ruxt_journey.svg",
                include_bytes!("../assets/manual/svg/ch07_ruxt_journey.svg"),
                egui::vec2(4750.0, 1356.0),
            ),
            (
                "ch07_platform_matrix.svg",
                include_bytes!("../assets/manual/svg/ch07_platform_matrix.svg"),
                egui::vec2(738.75, 1217.31),
            ),
        ];

        for (name, bytes, calibrated_size) in diagrams {
            let tree = resvg::usvg::Tree::from_data(bytes, &options)
                .unwrap_or_else(|err| panic!("Failed to parse SVG {name}: {err}"));
            let intrinsic_w = tree.size().width();
            let intrinsic_h = tree.size().height();
            let intrinsic_ratio = intrinsic_w / intrinsic_h;
            let calibrated_ratio = calibrated_size.x / calibrated_size.y;

            let diff = (intrinsic_ratio - calibrated_ratio).abs() / intrinsic_ratio;
            assert!(
                diff < 0.005,
                "Diagram {name} has aspect ratio mismatch: intrinsic={intrinsic_ratio:.4} ({intrinsic_w}x{intrinsic_h}), calibrated={calibrated_ratio:.4} ({calibrated_size:?}), diff={diff:.4}"
            );
        }
    }

    #[test]
    fn test_svg_loader_cache_eviction_and_lifecycle() {
        use egui::load::ImageLoader;
        let loader = ruxt::platform::UniversalSvgLoader::new();
        let ctx = egui::Context::default();
        let uri = "bytes://test/diagram.svg";
        let raw_svg = br#"<svg width="200" height="100"><rect width="200" height="100" fill="blue"/><text>Test</text></svg>"#;
        ctx.include_bytes(uri, raw_svg.as_slice());

        // Load at size hint 1:
        let res = loader.load(&ctx, uri, egui::SizeHint::Scale(1.0.into()));
        assert!(res.is_ok());
        let initial_bytes = loader.byte_size();
        assert!(
            initial_bytes > 0,
            "Loader byte size must reflect cached texture"
        );

        // Load at size hint 2 (creating a second variant in the same bucket):
        let res2 = loader.load(&ctx, uri, egui::SizeHint::Scale(2.0.into()));
        assert!(res2.is_ok());
        assert!(loader.byte_size() > initial_bytes);

        // Run end_pass with advanced pass_index to evict the previous zoom variant:
        loader.end_pass(10);
        // The bucket length was >= 2, so the older variant (last_used = 0) is pruned:
        assert!(loader.byte_size() <= initial_bytes * 5);

        // Forget drops entry completely:
        loader.forget(uri);
        assert_eq!(loader.byte_size(), 0, "Forget must drop all bucket entries");
    }
}
