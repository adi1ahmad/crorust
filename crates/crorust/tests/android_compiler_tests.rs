use crorust::android::compiler::RClassGenerator;
use tempfile::TempDir;

#[test]
fn test_r_class_generator_multi_package_synthesis_and_symbols() -> anyhow::Result<()> {
    let tmp = TempDir::new()?;

    let appcompat_r_txt = tmp.path().join("appcompat_R.txt");
    std::fs::write(
        &appcompat_r_txt,
        r#"
int anim abc_fade_in 0x0
int attr colorPrimary 0x0
int attr colorSecondary 0x0
int drawable abc_btn_check 0x0
int string app_name 0x0
int[] styleable AppTheme { 0x10100da, 0x0, 0x0 }
int styleable AppTheme_android_focusable 0
int styleable AppTheme_colorPrimary 1
int styleable AppTheme_colorSecondary 2
"#,
    )?;

    let recycler_r_txt = tmp.path().join("recycler_R.txt");
    std::fs::write(
        &recycler_r_txt,
        r#"
int attr layoutManager 0x0
int id item_touch_helper_previous_elevation 0x0
int dimen fastscroll_default_thickness 0x0
int[] styleable RecyclerView { 0x0 }
int styleable RecyclerView_layoutManager 0
"#,
    )?;

    let r_txt_files = vec![
        ("androidx.appcompat".to_string(), appcompat_r_txt),
        ("androidx.recyclerview.widget".to_string(), recycler_r_txt),
    ];

    let mut generator = RClassGenerator::parse(&r_txt_files)?;

    let id_map = generator.assign_resource_ids();
    assert!(id_map.contains_key(&("anim".to_string(), "abc_fade_in".to_string())));
    assert!(id_map.contains_key(&("attr".to_string(), "colorPrimary".to_string())));
    assert!(id_map.contains_key(&("attr".to_string(), "layoutManager".to_string())));
    assert!(id_map.contains_key(&(
        "dimen".to_string(),
        "fastscroll_default_thickness".to_string()
    )));

    for id in id_map.values() {
        assert_eq!(
            *id >> 24,
            0x7f,
            "Resource ID must have 0x7f package ID prefix"
        );
    }

    let symbols = generator.export_resource_symbols();
    assert!(symbols.types.contains(&"anim".to_string()));
    assert!(symbols.types.contains(&"attr".to_string()));
    assert!(symbols.types.contains(&"drawable".to_string()));
    assert!(symbols.types.contains(&"dimen".to_string()));
    assert!(symbols.types.contains(&"id".to_string()));
    assert!(symbols.types.contains(&"string".to_string()));
    assert_eq!(symbols.id_map.len(), id_map.len());

    let out_dir = tmp.path().join("r_java_out");
    let java_files = generator.write_sources(&out_dir, "com.example.unified_app")?;

    assert_eq!(java_files.len(), 3);

    let root_r = std::fs::read_to_string(out_dir.join("com/example/unified_app/R.java"))?;
    assert!(root_r.contains("package com.example.unified_app;"));
    assert!(root_r.contains("public static int abc_fade_in"));
    assert!(root_r.contains("public static int layoutManager"));
    assert!(root_r.contains("public static int fastscroll_default_thickness"));

    assert!(root_r.contains("public static final class styleable"));
    assert!(root_r.contains("public static int[] AppTheme"));
    assert!(root_r.contains("public static int[] RecyclerView"));
    assert!(root_r.contains("public static int AppTheme_colorPrimary = 1;"));
    assert!(root_r.contains("public static int RecyclerView_layoutManager = 0;"));

    let javac = which::which("javac");
    if let Ok(javac_bin) = javac {
        let classes_out = tmp.path().join("classes_out");
        std::fs::create_dir_all(&classes_out)?;

        let mut cmd = std::process::Command::new(javac_bin);
        cmd.arg("-d").arg(&classes_out);
        for f in &java_files {
            cmd.arg(f);
        }

        let status = cmd.status()?;
        assert!(
            status.success(),
            "javac must successfully compile synthesized R.java files"
        );

        assert!(classes_out.join("com/example/unified_app/R.class").exists());
        assert!(classes_out
            .join("com/example/unified_app/R$styleable.class")
            .exists());
        assert!(classes_out.join("androidx/appcompat/R.class").exists());
        assert!(classes_out
            .join("androidx/recyclerview/widget/R.class")
            .exists());
    }

    Ok(())
}

#[test]
fn test_styleable_version_mismatch_and_compound_names() -> anyhow::Result<()> {
    let tmp = TempDir::new()?;

    // Library A: games-activity (older baseline with 2 attributes: focusable, attrA)
    let games_r_txt = tmp.path().join("games_R.txt");
    std::fs::write(
        &games_r_txt,
        r#"
int attr attrA 0x0
int attr attrB 0x0
int[] styleable CollidingTheme { 0x10100da, 0x0 }
int styleable CollidingTheme_android_focusable 0
int styleable CollidingTheme_attrA 1
"#,
    )?;

    // Library B: appcompat (newer baseline with 3 attributes: focusable, attrA, attrB)
    // Plus compound styleables with underscores
    let appcompat_r_txt = tmp.path().join("appcompat_R.txt");
    std::fs::write(
        &appcompat_r_txt,
        r#"
int attr attrA 0x0
int attr attrB 0x0
int attr layout_behavior 0x0
int[] styleable CollidingTheme { 0x10100da, 0x0, 0x0 }
int styleable CollidingTheme_android_focusable 0
int styleable CollidingTheme_attrA 1
int styleable CollidingTheme_attrB 2

int[] styleable CoordinatorLayout { 0x0 }
int styleable CoordinatorLayout_keylines 0

int[] styleable CoordinatorLayout_Layout { 0x10100b3, 0x0 }
int styleable CoordinatorLayout_Layout_android_gravity 0
int styleable CoordinatorLayout_Layout_layout_behavior 1
"#,
    )?;

    let r_txt_files = vec![
        ("androidx.games".to_string(), games_r_txt),
        ("androidx.appcompat".to_string(), appcompat_r_txt),
    ];

    let mut generator = RClassGenerator::parse(&r_txt_files)?;
    let id_map = generator.assign_resource_ids();
    generator.resolve_styleables(&id_map);

    // 1. Verify strict package isolation
    let games_theme = generator
        .pkg_styleable_arrays
        .get("androidx.games")
        .unwrap()
        .get("CollidingTheme")
        .unwrap();
    assert_eq!(
        games_theme.len(),
        2,
        "games-activity array must have exactly 2 elements"
    );

    let appcompat_theme = generator
        .pkg_styleable_arrays
        .get("androidx.appcompat")
        .unwrap()
        .get("CollidingTheme")
        .unwrap();
    assert_eq!(
        appcompat_theme.len(),
        3,
        "appcompat array must have exactly 3 elements"
    );
    assert_eq!(appcompat_theme[0], 0x10100da);
    assert_eq!(
        appcompat_theme[1],
        *id_map
            .get(&("attr".to_string(), "attrA".to_string()))
            .unwrap()
    );
    assert_eq!(
        appcompat_theme[2],
        *id_map
            .get(&("attr".to_string(), "attrB".to_string()))
            .unwrap()
    );

    // 2. Verify root package deterministic synthesis (sorted union)
    let root_theme = generator
        .root_styleable_arrays
        .get("CollidingTheme")
        .unwrap();
    assert_eq!(
        root_theme.len(),
        3,
        "Root array must have union of 3 elements"
    );
    assert_eq!(
        root_theme[0], 0x10100da,
        "Platform ID 0x0101xxxx must sort first"
    );
    assert!(
        root_theme[1] < root_theme[2],
        "App attributes must sort ascending"
    );

    let attr_b_idx = *generator
        .root_styleable_indices
        .get("CollidingTheme_attrB")
        .unwrap();
    assert!(
        (attr_b_idx as usize) < root_theme.len(),
        "Index {} must be strictly within root array bounds {}",
        attr_b_idx,
        root_theme.len()
    );

    // 3. Verify compound styleable longest-prefix matching
    let appcompat_indices = generator
        .pkg_styleable_indices
        .get("androidx.appcompat")
        .unwrap();
    assert_eq!(
        *appcompat_indices
            .get("CoordinatorLayout_Layout")
            .unwrap()
            .get("CoordinatorLayout_Layout_layout_behavior")
            .unwrap(),
        1,
        "CoordinatorLayout_Layout_layout_behavior must associate with CoordinatorLayout_Layout"
    );
    assert_eq!(
        *appcompat_indices
            .get("CoordinatorLayout")
            .unwrap()
            .get("CoordinatorLayout_keylines")
            .unwrap(),
        0,
        "CoordinatorLayout_keylines must associate with CoordinatorLayout"
    );

    // 4. Verify bytecode generation
    let classes_dir = tmp.path().join("classes");
    generator.emit_bytecode(&classes_dir, "com.example.app")?;

    assert!(classes_dir
        .join("androidx/games/R$styleable.class")
        .is_file());
    assert!(classes_dir
        .join("androidx/appcompat/R$styleable.class")
        .is_file());
    assert!(classes_dir
        .join("com/example/app/R$styleable.class")
        .is_file());

    Ok(())
}

#[test]
fn test_compound_styleable_anti_bleeding() -> anyhow::Result<()> {
    let tmp = TempDir::new()?;

    let r_txt = tmp.path().join("R.txt");
    std::fs::write(
        &r_txt,
        r#"
int attr keylines 0x0
int attr statusBarBackground 0x0
int attr layout_behavior 0x0
int attr layout_anchor 0x0

int[] styleable CoordinatorLayout { 0x0, 0x0 }
int styleable CoordinatorLayout_keylines 0
int styleable CoordinatorLayout_statusBarBackground 1

int[] styleable CoordinatorLayout_Layout { 0x10100b3, 0x0, 0x0 }
int styleable CoordinatorLayout_Layout_android_gravity 0
int styleable CoordinatorLayout_Layout_layout_behavior 1
int styleable CoordinatorLayout_Layout_layout_anchor 2
"#,
    )?;

    let r_txt_files = vec![("androidx.coordinator".to_string(), r_txt)];
    let mut generator = RClassGenerator::parse(&r_txt_files)?;
    let id_map = generator.assign_resource_ids();
    generator.resolve_styleables(&id_map);

    let pkg_indices = generator
        .pkg_styleable_indices
        .get("androidx.coordinator")
        .unwrap();

    // Verify CoordinatorLayout only contains its own 2 attributes
    let base_indices = pkg_indices.get("CoordinatorLayout").unwrap();
    assert_eq!(base_indices.len(), 2);
    assert!(base_indices.contains_key("CoordinatorLayout_keylines"));
    assert!(base_indices.contains_key("CoordinatorLayout_statusBarBackground"));
    assert!(!base_indices.contains_key("CoordinatorLayout_Layout_layout_behavior"));

    // Verify CoordinatorLayout_Layout only contains its own 3 attributes
    let compound_indices = pkg_indices.get("CoordinatorLayout_Layout").unwrap();
    assert_eq!(compound_indices.len(), 3);
    assert!(compound_indices.contains_key("CoordinatorLayout_Layout_layout_behavior"));
    assert!(compound_indices.contains_key("CoordinatorLayout_Layout_layout_anchor"));

    // Verify bytecode generation
    let classes_dir = tmp.path().join("classes");
    generator.emit_bytecode(&classes_dir, "com.example.app")?;
    assert!(classes_dir
        .join("androidx/coordinator/R$styleable.class")
        .is_file());

    Ok(())
}
