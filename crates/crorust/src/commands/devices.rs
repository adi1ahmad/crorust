use crate::devices::TargetDevice;
use anyhow::Result;

pub fn list_connected_devices() -> Result<()> {
    let discovered = TargetDevice::discover_available_devices()?;

    if discovered.is_empty() {
        println!("No connected hardware or emulator devices found.");
        println!("Default compilation and execution target is host desktop.");
        return Ok(());
    }

    println!(
        "{:<45} {:<20} {:<22} DETAILS",
        "DEVICE ID", "NAME", "PLATFORM / ARCH"
    );
    println!("{}", "-".repeat(105));

    for target in discovered {
        let label = target.to_string();
        let name_str = target.device_name().unwrap_or_else(|_| "Unknown".into());
        let platform = target
            .target_platform()
            .map(|p| p.to_string())
            .unwrap_or_default();
        let arch = target
            .target_arch()
            .map(|a| a.to_string())
            .unwrap_or_default();
        let target_str = format!("{platform} ({arch})");
        let detail_str = target.device_details().unwrap_or_default();

        println!("{label:<45} {name_str:<20} {target_str:<22} {detail_str}");
    }

    Ok(())
}
