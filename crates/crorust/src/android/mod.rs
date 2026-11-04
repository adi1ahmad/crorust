pub mod activity;
pub mod compiler;
pub mod gradle;
pub mod mvn;

pub use activity::{
    detect_android_activity_version, parse_android_activity_from_lock,
    parse_android_activity_from_manifest, resolve_recommended_games_activity_version,
};
pub use compiler::AndroidBytecodeCompiler;
pub use mvn::AndroidMvnResolver;
