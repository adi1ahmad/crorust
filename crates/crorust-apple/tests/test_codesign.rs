use apple_codesign::MachFile;
use crorust_apple::{AppleBundleContainer, DarwinPropertyList};
use crorust_common::IdentityKeypair;
use std::io::Read;
use tempfile::TempDir;

fn get_minimal_macho_executable() -> Vec<u8> {
    const GZIPPED_MACHO_B64: &str =
        "H4sICEk302oAA3Rlc3RfbWFjaG9feDg2AO2Zz0sCQRTH3+yuJv1cqENRhy1C6qRCB+lU5JKSVESCBDFZriiogSlZ\
        Jy8F3Tp2tP6SoKBj/0KXjuGpS0Q1szO26w+kWxTvA4+ZNzvzfe/N7mVnHt9ePvoAiAoACrNBZk+scwYGcCaYRZlR\
        urm8au6YWxvQAekc6oDrhAnX2TaT213WG20LpO+RpsphSstWtexMa9czVKE34vI1t64GNbdLaaV4nCumaa6YOeyi\
        F5R6Yd3xlR51UmplaaaUKljd88uOCr0ll6+61g8A7Ln1nP2Px9bXzEisqWUInSo4Prh8Aq3vZQagFuyR9zdMh8/j\
        NY4zCxuinzRECJYfbLYtUaD3nvRiWIS0v7tA5agUyOf2A+mTfFo+n5R5xOMRLZEIRa5v6veLC43XWTZmvxKt37Y5\
        Vh+fZ6it+n45L8iC+Fg7L31e109IRC+eH8x33g3d7t7ZezoFYm+bEryV+6Xx+GEQFpC58L5q10hA/1lYBEEQBEEQ\
        BEEQBEH+IGOyJadbQKo60Qf7fJcEgJ9j8POEq8anTfPcCpaH9XNlZcj7K9kiCIIgCIIgCPIvIRQ8oBRSuSJM00KW\
        WlXroFK2aNZKpa0S+FWoe123u3X5Q8JHRohz3+6zfXG7Ke5Gqa1Ju2hyvgCRA/R0eCAAAA==";

    let clean_b64 = GZIPPED_MACHO_B64.replace([' ', '\n'], "");
    let gzipped = base64::decode(clean_b64).expect("Valid base64");
    let mut gz = flate2::read::GzDecoder::new(&gzipped[..]);
    let mut decompressed = Vec::new();
    gz.read_to_end(&mut decompressed)
        .expect("Decompression succeeds");
    decompressed
}

#[test]
fn test_apple_appbundle_codesigning_and_entitlements_linux() -> anyhow::Result<()> {
    let temp = TempDir::new()?;
    let exe_bytes = get_minimal_macho_executable();
    let exe_path = temp.path().join("demo_app");
    std::fs::write(&exe_path, &exe_bytes)?;

    let info = DarwinPropertyList {
        bundle_name: Some("DemoApp".to_string()),
        bundle_identifier: Some("com.example.demoapp".to_string()),
        bundle_version: Some("1.0.0".to_string()),
        short_version_string: Some("1.0".to_string()),
        executable_name: Some("demo_app".to_string()),
        ..Default::default()
    };

    let mut bundle = AppleBundleContainer::new(temp.path(), info)?;
    bundle.stage_executable_binary(&exe_path)?;

    let entitlements_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.app-sandbox</key>
    <true/>
    <key>com.apple.security.network.client</key>
    <true/>
</dict>
</plist>"#;

    let entitlements: plist::Value = plist::from_reader_xml(entitlements_xml.as_bytes())?;
    bundle.set_entitlements(entitlements);

    let signer = IdentityKeypair::debug_identity().clone();
    bundle.finalize_bundle(Some(signer))?;

    let signed_exe = bundle.bundle_appdir().join("Contents/MacOS/demo_app");
    assert!(signed_exe.is_file(), "Signed executable must exist");

    let signed_data = std::fs::read(&signed_exe)?;
    let macho = MachFile::parse(&signed_data)?;
    let binary = macho
        .iter_macho()
        .next()
        .expect("Must contain at least one Mach-O binary architecture");
    let signature = binary.code_signature()?;
    assert!(
        signature.is_some(),
        "Executable must contain a Mach-O CodeSignature slot"
    );

    Ok(())
}
