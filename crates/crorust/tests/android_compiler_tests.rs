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
