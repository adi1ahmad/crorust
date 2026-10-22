use crorust::android::gradle::{
    escape_groovy_string, generate_gradle_properties, generate_root_build_gradle,
    generate_settings_gradle, generate_settings_gradle_with_env,
};
use crorust::config::{AndroidPlatformConfig, GradleConfig, RepositoryEntry};

#[test]
fn test_generate_root_build_gradle_custom_versions() {
    let config = AndroidPlatformConfig {
        agp_version: Some("8.8.2".to_string()),
        kotlin_version: Some("2.1.20".to_string()),
        ..Default::default()
    };

    let output = generate_root_build_gradle(&config);

    assert!(output.contains("id 'com.android.application' version '8.8.2' apply false"));
    assert!(output.contains("id 'com.android.library' version '8.8.2' apply false"));
    assert!(output.contains("id 'org.jetbrains.kotlin.android' version '2.1.20' apply false"));
    assert!(output.contains("task clean(type: Delete)"));
}

#[test]
fn test_generate_settings_gradle_default_repositories() {
    let config = AndroidPlatformConfig::default();
    let output = generate_settings_gradle(&config);

    assert!(output.contains("repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)"));
    assert!(output.contains("google()"));
    assert!(output.contains("mavenCentral()"));
    assert!(output.contains("include ':app'"));
}

#[test]
fn test_generate_settings_gradle_custom_maven_with_env_credentials() {
    let mut env_map = std::collections::HashMap::new();
    env_map.insert("TEST_NEXUS_USER", "svc_account_deployer");
    env_map.insert("TEST_NEXUS_TOKEN", "n3xus_s3cr3t_t0k3n");

    let config = AndroidPlatformConfig {
        repositories: vec![
            RepositoryEntry::Standard {
                standard: "google".to_string(),
            },
            RepositoryEntry::Maven {
                maven: "https://jitpack.io".to_string(),
                username_env: None,
                password_env: None,
            },
            RepositoryEntry::Maven {
                maven: "https://nexus.internal.company/repository/maven-releases/".to_string(),
                username_env: Some("TEST_NEXUS_USER".to_string()),
                password_env: Some("TEST_NEXUS_TOKEN".to_string()),
            },
        ],
        ..Default::default()
    };

    let output =
        generate_settings_gradle_with_env(&config, |k| env_map.get(k).map(|&s| s.to_string()));

    assert!(output.contains("maven { url 'https://jitpack.io' }"));

    assert!(output.contains("url 'https://nexus.internal.company/repository/maven-releases/'"));
    assert!(output.contains("username = 'svc_account_deployer'"));
    assert!(output.contains("password = 'n3xus_s3cr3t_t0k3n'"));
}

#[test]
fn test_generate_gradle_properties_custom_jvm_args() {
    let gradle_cfg = GradleConfig {
        enabled: None,
        version: "8.8".to_string(),
        distribution_url: None,
        jvm_args: "-Xmx8192m -XX:+UseG1GC".to_string(),
    };

    let output = generate_gradle_properties(Some(&gradle_cfg));

    assert!(output.contains("org.gradle.jvmargs=-Xmx8192m -XX:+UseG1GC"));
    assert!(output.contains("android.useAndroidX=true"));
    assert!(output.contains("android.nonTransitiveRClass=true"));
}

#[test]
fn test_groovy_string_escaping_prevents_injection() {
    let raw_injection = "https://example.com/repo'; System.exit(1); //";
    let escaped = escape_groovy_string(raw_injection);

    assert_eq!(
        escaped, "https://example.com/repo\\'; System.exit(1); //",
        "Single quote must be escaped with backslash"
    );

    let raw_backslashes = "C:\\Tools\\Android\\";
    let escaped_bs = escape_groovy_string(raw_backslashes);
    assert_eq!(escaped_bs, "C:\\\\Tools\\\\Android\\\\");
}
