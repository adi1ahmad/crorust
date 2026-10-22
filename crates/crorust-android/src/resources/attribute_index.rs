pub const ATTR_THEME: u32 = 0x0101_0000;
pub const ATTR_LABEL: u32 = 0x0101_0001;
pub const ATTR_ICON: u32 = 0x0101_0002;
pub const ATTR_NAME: u32 = 0x0101_0003;
pub const ATTR_PERMISSION: u32 = 0x0101_0006;
pub const ATTR_PROTECTION_LEVEL: u32 = 0x0101_0009;
pub const ATTR_HAS_CODE: u32 = 0x0101_000c;
pub const ATTR_DEBUGGABLE: u32 = 0x0101_000f;
pub const ATTR_EXPORTED: u32 = 0x0101_0010;
pub const ATTR_PROCESS: u32 = 0x0101_0011;
pub const ATTR_TASK_AFFINITY: u32 = 0x0101_0012;
pub const ATTR_MULTIPROCESS: u32 = 0x0101_0013;
pub const ATTR_FINISH_ON_TASK_LAUNCH: u32 = 0x0101_0014;
pub const ATTR_CLEAR_TASK_ON_LAUNCH: u32 = 0x0101_0015;
pub const ATTR_STATE_NOT_NEEDED: u32 = 0x0101_0016;
pub const ATTR_SCREEN_ORIENTATION: u32 = 0x0101_001e;
pub const ATTR_CONFIG_CHANGES: u32 = 0x0101_001f;
pub const ATTR_VALUE: u32 = 0x0101_0024;
pub const ATTR_RESOURCE: u32 = 0x0101_0025;
pub const ATTR_WINDOW_NO_TITLE: u32 = 0x0101_0058;
pub const ATTR_MIN_SDK_VERSION: u32 = 0x0101_020c;
pub const ATTR_VERSION_CODE: u32 = 0x0101_021b;
pub const ATTR_VERSION_NAME: u32 = 0x0101_021c;
pub const ATTR_WINDOW_SOFT_INPUT_MODE: u32 = 0x0101_022b;
pub const ATTR_TARGET_SDK_VERSION: u32 = 0x0101_0270;
pub const ATTR_MAX_SDK_VERSION: u32 = 0x0101_0271;
pub const ATTR_WINDOW_ACTION_BAR: u32 = 0x0101_02ce;
pub const ATTR_WINDOW_ACTION_BAR_OVERLAY: u32 = 0x0101_02cf;
pub const ATTR_HARDWARE_ACCELERATED: u32 = 0x0101_02d3;
pub const ATTR_EXTRACT_NATIVE_LIBS: u32 = 0x0101_04ea;
pub const ATTR_APP_COMPONENT_FACTORY: u32 = 0x0101_057a;
pub const ATTR_AUTHORITIES: u32 = 0x0101_0018;
pub const ATTR_LAUNCH_MODE: u32 = 0x0101_001d;
pub const ATTR_COMPILE_SDK_VERSION: u32 = 0x0101_0572;
pub const ATTR_COMPILE_SDK_VERSION_CODENAME: u32 = 0x0101_0573;
pub const ATTR_ROUND_ICON: u32 = 0x0101_052c;
pub const ATTR_SUPPORTS_RTL: u32 = 0x0101_03af;
pub const ATTR_ALLOW_BACKUP: u32 = 0x0101_0280;
pub const ATTR_BANNER: u32 = 0x0101_03f2;
pub const ATTR_FULL_BACKUP_CONTENT: u32 = 0x0101_0486;
pub const ATTR_DATA_EXTRACTION_RULES: u32 = 0x0101_061c;
pub const ATTR_NETWORK_SECURITY_CONFIG: u32 = 0x0101_0529;
pub const ATTR_LOCALE_CONFIG: u32 = 0x0101_062a;

pub fn lookup_framework_attribute(name: &str) -> Option<u32> {
    match name {
        "theme" => Some(ATTR_THEME),
        "label" => Some(ATTR_LABEL),
        "icon" => Some(ATTR_ICON),
        "name" => Some(ATTR_NAME),
        "permission" => Some(ATTR_PERMISSION),
        "protectionLevel" => Some(ATTR_PROTECTION_LEVEL),
        "hasCode" => Some(ATTR_HAS_CODE),
        "debuggable" => Some(ATTR_DEBUGGABLE),
        "exported" => Some(ATTR_EXPORTED),
        "process" => Some(ATTR_PROCESS),
        "taskAffinity" => Some(ATTR_TASK_AFFINITY),
        "multiprocess" => Some(ATTR_MULTIPROCESS),
        "finishOnTaskLaunch" => Some(ATTR_FINISH_ON_TASK_LAUNCH),
        "clearTaskOnLaunch" => Some(ATTR_CLEAR_TASK_ON_LAUNCH),
        "stateNotNeeded" => Some(ATTR_STATE_NOT_NEEDED),
        "screenOrientation" => Some(ATTR_SCREEN_ORIENTATION),
        "configChanges" => Some(ATTR_CONFIG_CHANGES),
        "value" => Some(ATTR_VALUE),
        "resource" => Some(ATTR_RESOURCE),
        "windowNoTitle" => Some(ATTR_WINDOW_NO_TITLE),
        "minSdkVersion" => Some(ATTR_MIN_SDK_VERSION),
        "versionCode" => Some(ATTR_VERSION_CODE),
        "versionName" => Some(ATTR_VERSION_NAME),
        "windowSoftInputMode" => Some(ATTR_WINDOW_SOFT_INPUT_MODE),
        "targetSdkVersion" => Some(ATTR_TARGET_SDK_VERSION),
        "maxSdkVersion" => Some(ATTR_MAX_SDK_VERSION),
        "windowActionBar" => Some(ATTR_WINDOW_ACTION_BAR),
        "windowActionBarOverlay" => Some(ATTR_WINDOW_ACTION_BAR_OVERLAY),
        "hardwareAccelerated" => Some(ATTR_HARDWARE_ACCELERATED),
        "extractNativeLibs" => Some(ATTR_EXTRACT_NATIVE_LIBS),
        "appComponentFactory" => Some(ATTR_APP_COMPONENT_FACTORY),
        "authorities" => Some(ATTR_AUTHORITIES),
        "launchMode" => Some(ATTR_LAUNCH_MODE),
        "compileSdkVersion" => Some(ATTR_COMPILE_SDK_VERSION),
        "compileSdkVersionCodename" => Some(ATTR_COMPILE_SDK_VERSION_CODENAME),
        "roundIcon" => Some(ATTR_ROUND_ICON),
        "supportsRtl" => Some(ATTR_SUPPORTS_RTL),
        "allowBackup" => Some(ATTR_ALLOW_BACKUP),
        "banner" => Some(ATTR_BANNER),
        "fullBackupContent" => Some(ATTR_FULL_BACKUP_CONTENT),
        "dataExtractionRules" => Some(ATTR_DATA_EXTRACTION_RULES),
        "networkSecurityConfig" => Some(ATTR_NETWORK_SECURITY_CONFIG),
        "localeConfig" => Some(ATTR_LOCALE_CONFIG),
        _ => None,
    }
}

pub const COLOR_DARKER_GRAY: u32 = 0x0106_0000;
pub const COLOR_PRIMARY_TEXT_DARK: u32 = 0x0106_0001;
pub const COLOR_PRIMARY_TEXT_LIGHT: u32 = 0x0106_0002;
pub const COLOR_SECONDARY_TEXT_LIGHT: u32 = 0x0106_0003;
pub const COLOR_SECONDARY_TEXT_DARK: u32 = 0x0106_0004;
pub const COLOR_WHITE: u32 = 0x0106_000b;
pub const COLOR_BLACK: u32 = 0x0106_000c;
pub const COLOR_TRANSPARENT: u32 = 0x0106_000d;
pub const COLOR_BACKGROUND_DARK: u32 = 0x0106_000e;
pub const COLOR_BACKGROUND_LIGHT: u32 = 0x0106_000f;
pub const COLOR_HOLO_BLUE_LIGHT: u32 = 0x0106_0012;
pub const COLOR_HOLO_BLUE_DARK: u32 = 0x0106_0013;
pub const COLOR_HOLO_GREEN_LIGHT: u32 = 0x0106_0014;
pub const COLOR_HOLO_GREEN_DARK: u32 = 0x0106_0015;
pub const COLOR_HOLO_RED_LIGHT: u32 = 0x0106_0016;
pub const COLOR_HOLO_RED_DARK: u32 = 0x0106_0017;
pub const COLOR_HOLO_ORANGE_LIGHT: u32 = 0x0106_0018;
pub const COLOR_HOLO_ORANGE_DARK: u32 = 0x0106_0019;
pub const COLOR_HOLO_PURPLE: u32 = 0x0106_001a;
pub const COLOR_HOLO_BLUE_BRIGHT: u32 = 0x0106_001b;

pub fn lookup_framework_color(name: &str) -> Option<u32> {
    match name {
        "white" => Some(COLOR_WHITE),
        "black" => Some(COLOR_BLACK),
        "transparent" => Some(COLOR_TRANSPARENT),
        "background_dark" => Some(COLOR_BACKGROUND_DARK),
        "background_light" => Some(COLOR_BACKGROUND_LIGHT),
        "darker_gray" => Some(COLOR_DARKER_GRAY),
        "primary_text_dark" => Some(COLOR_PRIMARY_TEXT_DARK),
        "primary_text_light" => Some(COLOR_PRIMARY_TEXT_LIGHT),
        "secondary_text_dark" => Some(COLOR_SECONDARY_TEXT_DARK),
        "secondary_text_light" => Some(COLOR_SECONDARY_TEXT_LIGHT),
        "holo_blue_light" => Some(COLOR_HOLO_BLUE_LIGHT),
        "holo_blue_dark" => Some(COLOR_HOLO_BLUE_DARK),
        "holo_green_light" => Some(COLOR_HOLO_GREEN_LIGHT),
        "holo_green_dark" => Some(COLOR_HOLO_GREEN_DARK),
        "holo_red_light" => Some(COLOR_HOLO_RED_LIGHT),
        "holo_red_dark" => Some(COLOR_HOLO_RED_DARK),
        "holo_orange_light" => Some(COLOR_HOLO_ORANGE_LIGHT),
        "holo_orange_dark" => Some(COLOR_HOLO_ORANGE_DARK),
        "holo_purple" => Some(COLOR_HOLO_PURPLE),
        "holo_blue_bright" => Some(COLOR_HOLO_BLUE_BRIGHT),
        _ => None,
    }
}

pub const ID_CONTENT: u32 = 0x0102_0002;
pub const ID_ICON: u32 = 0x0102_0006;
pub const ID_EMPTY: u32 = 0x0102_0009;
pub const ID_LIST: u32 = 0x0102_000a;
pub const ID_SUMMARY: u32 = 0x0102_0010;
pub const ID_TABCONTENT: u32 = 0x0102_0011;
pub const ID_TITLE: u32 = 0x0102_0016;
pub const ID_CUSTOM: u32 = 0x0102_002c;

pub fn lookup_framework_id(name: &str) -> Option<u32> {
    match name {
        "content" => Some(ID_CONTENT),
        "icon" => Some(ID_ICON),
        "empty" => Some(ID_EMPTY),
        "list" => Some(ID_LIST),
        "summary" => Some(ID_SUMMARY),
        "tabcontent" => Some(ID_TABCONTENT),
        "title" => Some(ID_TITLE),
        "custom" => Some(ID_CUSTOM),
        _ => None,
    }
}

pub fn lookup_framework_resource(ty: &str, name: &str) -> Option<u32> {
    match ty {
        "attr" => lookup_framework_attribute(name),
        "color" => lookup_framework_color(name),
        "id" => lookup_framework_id(name),
        _ => None,
    }
}
