use crorust_apple::bundle::director::AppBundleDirector;
use crorust_apple::plist::DarwinPropertyList;
use tempfile::tempdir;

#[test]
fn test_darwin_property_list_macos_serialization() {
    let plist = DarwinPropertyList::new_macos_app("com.example.demo", "Demo", "demo", "2.0.0");

    let xml = plist.to_xml_bytes().expect("serialize plist to xml");
    let xml_str = std::str::from_utf8(&xml).expect("valid utf-8");

    assert!(xml_str.contains("<key>CFBundleIdentifier</key>"));
    assert!(xml_str.contains("<string>com.example.demo</string>"));
    assert!(xml_str.contains("<key>CFBundleName</key>"));
    assert!(xml_str.contains("<string>Demo</string>"));
    assert!(xml_str.contains("<key>CFBundleExecutable</key>"));
    assert!(xml_str.contains("<string>demo</string>"));
    assert!(xml_str.contains("<key>LSMinimumSystemVersion</key>"));
    assert!(xml_str.contains("<string>11.0</string>"));
    assert!(!plist.is_ios());
}

#[test]
fn test_darwin_property_list_ios_serialization() {
    let plist = DarwinPropertyList::new_ios_app(
        "com.example.demo.ios",
        "DemoMobile",
        "demo_mobile",
        "1.5.0",
    );

    assert!(plist.is_ios());
    let xml = plist.to_xml_bytes().expect("serialize ios plist");
    let xml_str = std::str::from_utf8(&xml).expect("valid utf-8");

    assert!(xml_str.contains("<key>LSRequiresIPhoneOS</key>"));
    assert!(xml_str.contains("<true/>"));
    assert!(xml_str.contains("<key>MinimumOSVersion</key>"));
    assert!(xml_str.contains("<string>14.0</string>"));
}

#[test]
fn test_app_bundle_director_macos_structure() {
    let tmp = tempdir().expect("tempdir");
    let plist = DarwinPropertyList::new_macos_app(
        "com.example.director",
        "DirectorTest",
        "director_exe",
        "1.0.0",
    );

    let mut director = AppBundleDirector::new(tmp.path(), plist).expect("create director");
    let dummy_exe = tmp.path().join("director_exe");
    std::fs::write(&dummy_exe, b"Mach-O mock").expect("write exe");

    director
        .stage_bundle_executable(&dummy_exe)
        .expect("add executable");
    director.seal_and_sign(None).expect("seal bundle");

    let bundle_root = tmp.path().join("DirectorTest.app");
    assert!(bundle_root.is_dir());
    assert!(bundle_root.join("Contents/MacOS/director_exe").is_file());
    assert!(bundle_root.join("Contents/Info.plist").is_file());
    assert!(bundle_root.join("Contents/PkgInfo").is_file());
}

#[test]
fn test_app_bundle_director_ios_structure() {
    let tmp = tempdir().expect("tempdir");
    let plist = DarwinPropertyList::new_ios_app(
        "com.example.iosdirector",
        "IosDirectorTest",
        "ios_exe",
        "1.0.0",
    );

    let mut director = AppBundleDirector::new(tmp.path(), plist).expect("create ios director");
    let dummy_exe = tmp.path().join("ios_exe");
    std::fs::write(&dummy_exe, b"Mach-O mock").expect("write exe");

    director
        .stage_bundle_executable(&dummy_exe)
        .expect("add executable");
    director.seal_and_sign(None).expect("seal ios bundle");

    let bundle_root = tmp.path().join("IosDirectorTest.app");
    assert!(bundle_root.is_dir());
    assert!(bundle_root.join("ios_exe").is_file());
    assert!(bundle_root.join("Info.plist").is_file());
    assert!(
        !bundle_root.join("Contents").exists(),
        "iOS bundles must have flat structure"
    );
}
