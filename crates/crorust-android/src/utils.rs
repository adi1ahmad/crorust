use anyhow::Result;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AndroidAbi {
    ArmV7a,
    Arm64V8a,
    X86,
    X86_64,
}

impl AndroidAbi {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ArmV7a => "armeabi-v7a",
            Self::Arm64V8a => "arm64-v8a",
            Self::X86 => "x86",
            Self::X86_64 => "x86_64",
        }
    }

    pub const fn android_abi(self) -> &'static str {
        self.as_str()
    }

    pub fn from_ndk_name(name: &str) -> Option<Self> {
        match name {
            "armeabi-v7a" | "armv7" => Some(Self::ArmV7a),
            "arm64-v8a" | "aarch64" => Some(Self::Arm64V8a),
            "x86" | "i686" => Some(Self::X86),
            "x86_64" => Some(Self::X86_64),
            _ => None,
        }
    }
}

impl std::fmt::Display for AndroidAbi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AndroidVersionCode {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

impl AndroidVersionCode {
    pub const fn new(major: u8, minor: u8, patch: u8) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub fn from_semver(version: &str) -> Result<Self> {
        let clean = version.split(['-', '+']).next().unwrap_or(version);
        let mut parts = clean.split('.');

        let major_str = parts.next().unwrap_or("0");
        let minor_str = parts.next().unwrap_or("0");
        let patch_str = parts.next().unwrap_or("0");

        let major: u8 = major_str
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid semver major: '{major_str}'"))?;
        let minor: u8 = minor_str
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid semver minor: '{minor_str}'"))?;
        let patch: u8 = patch_str
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid semver patch: '{patch_str}'"))?;

        Ok(Self::new(major, minor, patch))
    }

    pub const fn to_code(&self, apk_id: u8) -> u32 {
        ((apk_id as u32) << 24)
            | ((self.major as u32) << 16)
            | ((self.minor as u32) << 8)
            | (self.patch as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_semver_components() {
        let vc = AndroidVersionCode::from_semver("1.2.3").unwrap();
        assert_eq!(vc, AndroidVersionCode::new(1, 2, 3));
        assert_eq!(vc.to_code(1), (1 << 24) | (1 << 16) | (2 << 8) | 3);

        let pre = AndroidVersionCode::from_semver("2.5.10-beta.1+build.42").unwrap();
        assert_eq!(pre, AndroidVersionCode::new(2, 5, 10));
    }
}
