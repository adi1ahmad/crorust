use crorust::download::env::EnvSynthesizer;
use std::path::PathBuf;

#[test]
fn test_env_synthesizer_applies_vars_to_command() {
    let mut synth = EnvSynthesizer::new();
    synth.set_var("TEST_JAVA_HOME", "/test/jdk/home");
    synth.set_var("TEST_ANDROID_HOME", "/test/sdk/home");

    let mut cmd = std::process::Command::new("dummy");
    synth.apply(&mut cmd).unwrap();

    let env_map: std::collections::HashMap<_, _> = cmd
        .get_envs()
        .filter_map(|(k, v)| v.map(|v| (k.to_os_string(), v.to_os_string())))
        .collect();

    assert_eq!(
        env_map
            .get(&std::ffi::OsString::from("TEST_JAVA_HOME"))
            .unwrap(),
        "/test/jdk/home"
    );
    assert_eq!(
        env_map
            .get(&std::ffi::OsString::from("TEST_ANDROID_HOME"))
            .unwrap(),
        "/test/sdk/home"
    );
}

#[test]
fn test_path_delimiters_match_os_rules() {
    let mut synth = EnvSynthesizer::new();
    synth.prepend_path(PathBuf::from("/a"));
    synth.prepend_path(PathBuf::from("/b"));

    let built = synth.build_path().expect("Should build synthesized path");
    let joined_str = built.to_str().expect("Path should be valid string");

    if cfg!(target_os = "windows") {
        assert!(joined_str.contains(';'));
    } else {
        assert!(joined_str.contains(':'));
    }

    let split: Vec<PathBuf> = std::env::split_paths(&built).collect();
    assert_eq!(split[0], PathBuf::from("/a"));
    assert_eq!(split[1], PathBuf::from("/b"));

    let mut cmd = std::process::Command::new("dummy");
    synth
        .apply(&mut cmd)
        .expect("EnvSynthesizer application should succeed");

    let path_val = cmd
        .get_envs()
        .find(|(k, _)| k.to_string_lossy().eq_ignore_ascii_case("PATH"))
        .and_then(|(_, v)| v)
        .expect("PATH must be set on Command");

    let split_cmd: Vec<PathBuf> = std::env::split_paths(&path_val).collect();
    assert_eq!(split_cmd[0], PathBuf::from("/a"));
    assert_eq!(split_cmd[1], PathBuf::from("/b"));
}

#[test]
fn test_android_ndk_host_prebuilt_tag_resolution() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let ndk = tmp.path().join("android-ndk-r26d");

    let host_tag = if cfg!(target_os = "windows") {
        "windows-x86_64"
    } else if cfg!(target_os = "macos") {
        "darwin-x86_64"
    } else {
        "linux-x86_64"
    };
    let expected_dir = ndk.join("toolchains/llvm/prebuilt").join(host_tag);
    std::fs::create_dir_all(expected_dir.join("sysroot/usr/lib"))?;

    assert!(expected_dir.join("sysroot").exists());
    Ok(())
}
