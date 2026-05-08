use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::errors::ViewerError;
use crate::ui::Theme;
use crate::APP_NAME;

pub const DEFAULT_CONTENT_MAX_WIDTH_PX: u16 = 1040;
const MIN_CONTENT_MAX_WIDTH_PX: u16 = 640;
const MAX_CONTENT_MAX_WIDTH_PX: u16 = 2400;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    Light,
    Dark,
}

impl Default for ThemePreference {
    fn default() -> Self {
        Self::Light
    }
}

impl From<ThemePreference> for Theme {
    fn from(preference: ThemePreference) -> Self {
        match preference {
            ThemePreference::Light => Theme::Light,
            ThemePreference::Dark => Theme::Dark,
        }
    }
}

impl From<Theme> for ThemePreference {
    fn from(theme: Theme) -> Self {
        match theme {
            Theme::Light => ThemePreference::Light,
            Theme::Dark => ThemePreference::Dark,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct BodyFontSettings {
    pub family_name: String,
    pub point_size_tenths: u16,
}

fn is_valid_body_font(bfs: &BodyFontSettings) -> bool {
    !bfs.family_name.is_empty() && bfs.point_size_tenths != 0
}

fn deserialize_body_font<'de, D>(deserializer: D) -> Result<Option<BodyFontSettings>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<BodyFontSettings>::deserialize(deserializer)?;
    Ok(opt.filter(is_valid_body_font))
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
#[serde(default)]
pub struct WindowGeometry {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

fn is_valid_window_geometry(g: &WindowGeometry) -> bool {
    g.right > g.left && g.bottom > g.top && g.left >= -32000 && g.top >= -32000
}

fn deserialize_window_geometry<'de, D>(
    deserializer: D,
) -> Result<Option<WindowGeometry>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<WindowGeometry>::deserialize(deserializer)?;
    Ok(opt.filter(is_valid_window_geometry))
}

fn default_content_max_width_px() -> u16 {
    DEFAULT_CONTENT_MAX_WIDTH_PX
}

fn normalize_content_max_width_px(px: u16) -> u16 {
    if (MIN_CONTENT_MAX_WIDTH_PX..=MAX_CONTENT_MAX_WIDTH_PX).contains(&px) {
        px
    } else {
        DEFAULT_CONTENT_MAX_WIDTH_PX
    }
}

fn is_default_content_max_width_px(px: &u16) -> bool {
    *px == DEFAULT_CONTENT_MAX_WIDTH_PX
}

fn deserialize_content_max_width_px<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    let px = match value {
        serde_json::Value::Number(number) => number
            .as_u64()
            .and_then(|v| u16::try_from(v).ok())
            .or_else(|| number.as_i64().and_then(|v| u16::try_from(v).ok())),
        _ => None,
    };

    Ok(px
        .map(normalize_content_max_width_px)
        .unwrap_or(DEFAULT_CONTENT_MAX_WIDTH_PX))
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
#[serde(default)]
pub struct Settings {
    pub theme: ThemePreference,
    #[serde(
        deserialize_with = "deserialize_body_font",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub body_font: Option<BodyFontSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_editor: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recent_files: Vec<PathBuf>,
    #[serde(
        deserialize_with = "deserialize_window_geometry",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub window_geometry: Option<WindowGeometry>,
    #[serde(
        default = "default_content_max_width_px",
        deserialize_with = "deserialize_content_max_width_px",
        skip_serializing_if = "is_default_content_max_width_px"
    )]
    pub content_max_width_px: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemePreference::default(),
            body_font: None,
            external_editor: None,
            recent_files: Vec::new(),
            window_geometry: None,
            content_max_width_px: DEFAULT_CONTENT_MAX_WIDTH_PX,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SettingsFile {
    path: PathBuf,
}

impl SettingsFile {
    pub fn new() -> Self {
        Self {
            path: settings_file_path(
                std::env::var_os("LOCALAPPDATA").map(PathBuf::from),
                std::env::temp_dir(),
            ),
        }
    }

    #[cfg(test)]
    pub fn with_path(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Settings {
        let content = match fs::read_to_string(&self.path) {
            Ok(content) => content,
            Err(_error) => {
                crate::debug_log!(
                    "failed to read settings file {}: {_error}",
                    self.path.display()
                );
                return Settings::default();
            }
        };

        match serde_json::from_str::<Settings>(&content) {
            Ok(settings) => settings,
            Err(_error) => {
                crate::debug_log!(
                    "failed to parse settings file {}: {_error}",
                    self.path.display()
                );
                Settings::default()
            }
        }
    }

    pub fn save(&self, settings: &Settings) -> Result<(), ViewerError> {
        if let Some(parent) = self.path.parent() {
            if let Err(_error) = fs::create_dir_all(parent) {
                crate::debug_log!(
                    "failed to create settings directory {}: {_error}",
                    parent.display()
                );
                return Err(ViewerError::settings_save(
                    &self.path,
                    format!(
                        "failed to create settings directory {}: {_error}",
                        parent.display()
                    ),
                ));
            }
        }

        let serialized = match serde_json::to_string_pretty(settings) {
            Ok(serialized) => serialized,
            Err(_error) => {
                crate::debug_log!(
                    "failed to serialize settings for {}: {_error}",
                    self.path.display()
                );
                return Err(ViewerError::settings_save(
                    &self.path,
                    format!(
                        "failed to serialize settings for {}: {_error}",
                        self.path.display()
                    ),
                ));
            }
        };

        if let Err(_error) = fs::write(&self.path, serialized) {
            crate::debug_log!(
                "failed to write settings file {}: {_error}",
                self.path.display()
            );
            return Err(ViewerError::settings_save(
                &self.path,
                format!(
                    "failed to write settings file {}: {_error}",
                    self.path.display()
                ),
            ));
        }
        Ok(())
    }
}

fn settings_file_path(local_app_data: Option<PathBuf>, temp_dir: PathBuf) -> PathBuf {
    local_app_data
        .unwrap_or(temp_dir)
        .join(APP_NAME)
        .join("settings.json")
}

#[cfg(test)]
mod tests {
    use super::{
        settings_file_path, BodyFontSettings, Settings, SettingsFile, ThemePreference,
        DEFAULT_CONTENT_MAX_WIDTH_PX,
    };
    use crate::errors::ViewerError;
    use crate::ui::Theme;
    use crate::APP_NAME;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn theme_preference_serializes_as_lowercase_string() {
        let light = serde_json::to_string(&ThemePreference::Light).expect("serialize light");
        let dark = serde_json::to_string(&ThemePreference::Dark).expect("serialize dark");

        assert_eq!(light, "\"light\"");
        assert_eq!(dark, "\"dark\"");
    }

    #[test]
    fn theme_preference_deserializes_from_lowercase_string() {
        let light: ThemePreference = serde_json::from_str("\"light\"").expect("parse light");
        let dark: ThemePreference = serde_json::from_str("\"dark\"").expect("parse dark");

        assert_eq!(light, ThemePreference::Light);
        assert_eq!(dark, ThemePreference::Dark);
    }

    #[test]
    fn theme_preference_converts_from_theme() {
        assert_eq!(ThemePreference::from(Theme::Light), ThemePreference::Light);
        assert_eq!(ThemePreference::from(Theme::Dark), ThemePreference::Dark);
    }

    #[test]
    fn theme_preference_converts_into_theme() {
        let light: Theme = ThemePreference::Light.into();
        let dark: Theme = ThemePreference::Dark.into();

        assert_eq!(light, Theme::Light);
        assert_eq!(dark, Theme::Dark);
    }

    #[test]
    fn settings_default_theme_is_light() {
        assert_eq!(Settings::default().theme, ThemePreference::Light);
    }

    #[test]
    fn settings_default_content_max_width_px_is_1040() {
        assert_eq!(Settings::default().content_max_width_px, 1040);
    }

    #[test]
    fn settings_file_path_prefers_local_app_data_directory() {
        let path = settings_file_path(
            Some(PathBuf::from(r"C:\Users\ashim\AppData\Local")),
            PathBuf::from(r"C:\Temp"),
        );

        assert_eq!(
            path,
            PathBuf::from(format!(
                r"C:\Users\ashim\AppData\Local\{}\settings.json",
                APP_NAME
            ))
        );
    }

    #[test]
    fn settings_file_path_falls_back_to_temp_directory() {
        let path = settings_file_path(None, PathBuf::from(r"C:\Temp"));

        assert_eq!(
            path,
            PathBuf::from(format!(r"C:\Temp\{}\settings.json", APP_NAME))
        );
    }

    #[test]
    fn settings_file_load_returns_default_when_file_is_missing() {
        let dir = unique_test_dir("settings-missing");
        let settings = SettingsFile::with_path(dir.join("settings.json")).load();

        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn settings_file_load_returns_default_on_invalid_json() {
        let dir = unique_test_dir("settings-invalid");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let path = dir.join("settings.json");
        fs::write(&path, "{invalid json").expect("write invalid json");

        let settings = SettingsFile::with_path(path).load();
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn settings_file_save_writes_pretty_json() {
        let dir = unique_test_dir("settings-save");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let path = dir.join("settings.json");
        let settings_file = SettingsFile::with_path(path.clone());

        settings_file
            .save(&Settings {
                theme: ThemePreference::Dark,
                body_font: None,
                external_editor: None,
                recent_files: vec![],
                window_geometry: None,
                content_max_width_px: DEFAULT_CONTENT_MAX_WIDTH_PX,
            })
            .expect("save settings");

        let content = fs::read_to_string(path).expect("read settings file");
        assert_eq!(content, "{\n  \"theme\": \"dark\"\n}");
    }

    #[test]
    fn settings_file_save_creates_parent_directories() {
        let dir = unique_test_dir("settings-parent");
        let path = dir.join("nested").join("settings.json");
        let settings_file = SettingsFile::with_path(path.clone());

        settings_file
            .save(&Settings::default())
            .expect("save settings");

        assert!(path.exists());
    }

    fn unique_test_dir(name: &str) -> TestDir {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("mdluma-{name}-{nonce}"));
        TestDir { path }
    }

    #[derive(Debug)]
    struct TestDir {
        path: PathBuf,
    }

    impl AsRef<Path> for TestDir {
        fn as_ref(&self) -> &Path {
            &self.path
        }
    }

    impl std::ops::Deref for TestDir {
        type Target = PathBuf;

        fn deref(&self) -> &Self::Target {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            if self.path.exists() {
                let _ = fs::remove_dir_all(&self.path);
            }
        }
    }

    #[test]
    fn body_font_settings_serializes_to_json() {
        let bfs = BodyFontSettings {
            family_name: "Yu Gothic UI".to_string(),
            point_size_tenths: 120,
        };
        let json = serde_json::to_string(&bfs).expect("serialize BodyFontSettings");
        assert_eq!(
            json,
            r#"{"family_name":"Yu Gothic UI","point_size_tenths":120}"#
        );
    }

    #[test]
    fn body_font_settings_deserializes_from_json() {
        let bfs: BodyFontSettings =
            serde_json::from_str(r#"{"family_name":"Segoe UI","point_size_tenths":100}"#)
                .expect("deserialize BodyFontSettings");
        assert_eq!(bfs.family_name, "Segoe UI");
        assert_eq!(bfs.point_size_tenths, 100);
    }

    #[test]
    fn settings_default_body_font_is_none() {
        assert_eq!(Settings::default().body_font, None);
    }

    #[test]
    fn settings_deserializes_body_font_when_present() {
        let json =
            r#"{"theme":"dark","body_font":{"family_name":"Consolas","point_size_tenths":110}}"#;
        let settings: Settings = serde_json::from_str(json).expect("parse settings with body_font");
        assert_eq!(settings.theme, ThemePreference::Dark);
        let bfs = settings.body_font.expect("body_font should be present");
        assert_eq!(bfs.family_name, "Consolas");
        assert_eq!(bfs.point_size_tenths, 110);
    }

    #[test]
    fn settings_deserializes_without_body_font_backward_compat() {
        let json = r#"{"theme":"light"}"#;
        let settings: Settings =
            serde_json::from_str(json).expect("parse settings without body_font");
        assert_eq!(settings.theme, ThemePreference::Light);
        assert_eq!(settings.body_font, None);
        assert_eq!(settings.content_max_width_px, DEFAULT_CONTENT_MAX_WIDTH_PX);
    }

    #[test]
    fn settings_deserializes_content_max_width_px_when_present() {
        let json = r#"{"theme":"light","content_max_width_px":980}"#;
        let settings: Settings = serde_json::from_str(json).expect("parse settings with width");
        assert_eq!(settings.content_max_width_px, 980);
    }

    #[test]
    fn settings_normalizes_out_of_range_content_max_width_px_to_default() {
        let json = r#"{"theme":"light","content_max_width_px":320}"#;
        let settings: Settings =
            serde_json::from_str(json).expect("parse settings with invalid width");
        assert_eq!(settings.content_max_width_px, DEFAULT_CONTENT_MAX_WIDTH_PX);
    }

    #[test]
    fn settings_invalid_content_max_width_px_does_not_drop_other_fields() {
        let json = r#"{"theme":"dark","content_max_width_px":-10}"#;
        let settings: Settings =
            serde_json::from_str(json).expect("parse settings with negative width");
        assert_eq!(settings.theme, ThemePreference::Dark);
        assert_eq!(settings.content_max_width_px, DEFAULT_CONTENT_MAX_WIDTH_PX);
    }

    #[test]
    fn settings_non_integer_content_max_width_px_falls_back_to_default() {
        let json = r#"{"theme":"light","content_max_width_px":980.5}"#;
        let settings: Settings =
            serde_json::from_str(json).expect("parse settings with floating width");
        assert_eq!(settings.content_max_width_px, DEFAULT_CONTENT_MAX_WIDTH_PX);
    }

    #[test]
    fn settings_normalizes_empty_family_name_to_none() {
        let json = r#"{"theme":"light","body_font":{"family_name":"","point_size_tenths":120}}"#;
        let settings: Settings =
            serde_json::from_str(json).expect("parse settings with empty family");
        assert_eq!(settings.body_font, None);
    }

    #[test]
    fn settings_normalizes_zero_point_size_to_none() {
        let json = r#"{"theme":"light","body_font":{"family_name":"Arial","point_size_tenths":0}}"#;
        let settings: Settings = serde_json::from_str(json).expect("parse settings with zero size");
        assert_eq!(settings.body_font, None);
    }

    #[test]
    fn settings_normalizes_both_invalid_fields_to_none() {
        let json = r#"{"theme":"dark","body_font":{"family_name":"","point_size_tenths":0}}"#;
        let settings: Settings =
            serde_json::from_str(json).expect("parse settings with all invalid");
        assert_eq!(settings.body_font, None);
    }

    #[test]
    fn settings_file_save_and_load_roundtrip_with_body_font() {
        let dir = unique_test_dir("settings-body-font-roundtrip");
        let path = dir.join("settings.json");
        let settings_file = SettingsFile::with_path(path.clone());

        let save_settings = Settings {
            theme: ThemePreference::Dark,
            body_font: Some(BodyFontSettings {
                family_name: "Yu Gothic UI".to_string(),
                point_size_tenths: 120,
            }),
            external_editor: None,
            recent_files: vec![],
            window_geometry: None,
            content_max_width_px: DEFAULT_CONTENT_MAX_WIDTH_PX,
        };
        settings_file.save(&save_settings).expect("save settings");

        let loaded = settings_file.load();
        assert_eq!(loaded, save_settings);
    }

    #[test]
    fn settings_file_save_writes_body_font_in_json() {
        let dir = unique_test_dir("settings-body-font-save");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let path = dir.join("settings.json");
        let settings_file = SettingsFile::with_path(path.clone());

        settings_file
            .save(&Settings {
                theme: ThemePreference::Light,
                body_font: Some(BodyFontSettings {
                    family_name: "Consolas".to_string(),
                    point_size_tenths: 110,
                }),
                external_editor: None,
                recent_files: vec![],
                window_geometry: None,
                content_max_width_px: DEFAULT_CONTENT_MAX_WIDTH_PX,
            })
            .expect("save settings");

        let content = fs::read_to_string(path).expect("read settings file");
        let parsed: serde_json::Value = serde_json::from_str(&content).expect("parse json");
        assert_eq!(parsed["theme"], "light");
        assert_eq!(parsed["body_font"]["family_name"], "Consolas");
        assert_eq!(parsed["body_font"]["point_size_tenths"], 110);
    }

    #[test]
    fn settings_file_load_reads_legacy_file_without_body_font() {
        let dir = unique_test_dir("settings-legacy-no-body-font");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let path = dir.join("settings.json");
        fs::write(&path, "{\n  \"theme\": \"dark\"\n}").expect("write legacy settings");

        let loaded = SettingsFile::with_path(path).load();
        assert_eq!(loaded.theme, ThemePreference::Dark);
        assert_eq!(loaded.body_font, None);
    }

    #[test]
    fn settings_file_load_normalizes_invalid_body_font_to_none() {
        let dir = unique_test_dir("settings-invalid-body-font");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let path = dir.join("settings.json");
        fs::write(&path, "{\n  \"theme\": \"light\",\n  \"body_font\": {\"family_name\": \"\", \"point_size_tenths\": 0}\n}")
            .expect("write settings with invalid body_font");

        let loaded = SettingsFile::with_path(path).load();
        assert_eq!(loaded.body_font, None);
    }

    #[test]
    fn settings_default_external_editor_is_none() {
        assert_eq!(Settings::default().external_editor, None);
    }

    #[test]
    fn settings_deserializes_external_editor_when_present() {
        let json = r#"{"theme":"dark","external_editor":"C:\\path\\to\\editor.exe"}"#;
        let settings: Settings =
            serde_json::from_str(json).expect("parse settings with external_editor");
        assert_eq!(settings.theme, ThemePreference::Dark);
        assert_eq!(
            settings.external_editor,
            Some(PathBuf::from(r"C:\path\to\editor.exe"))
        );
    }

    #[test]
    fn settings_deserializes_without_external_editor_backward_compat() {
        let json = r#"{"theme":"light"}"#;
        let settings: Settings =
            serde_json::from_str(json).expect("parse settings without external_editor");
        assert_eq!(settings.theme, ThemePreference::Light);
        assert_eq!(settings.external_editor, None);
    }

    #[test]
    fn settings_serialization_omits_external_editor_when_none() {
        let settings = Settings::default();
        let json = serde_json::to_string(&settings).expect("serialize settings");
        assert!(!json.contains("external_editor"));
    }

    #[test]
    fn settings_file_save_and_load_roundtrip_with_external_editor() {
        let dir = unique_test_dir("settings-external-editor-roundtrip");
        let path = dir.join("settings.json");
        let settings_file = SettingsFile::with_path(path.clone());

        let save_settings = Settings {
            theme: ThemePreference::Dark,
            body_font: None,
            external_editor: Some(PathBuf::from(r"C:\Tools\vscode\Code.exe")),
            recent_files: vec![],
            window_geometry: None,
            content_max_width_px: DEFAULT_CONTENT_MAX_WIDTH_PX,
        };
        settings_file.save(&save_settings).expect("save settings");

        let loaded = settings_file.load();
        assert_eq!(loaded, save_settings);
    }

    #[test]
    fn settings_file_save_and_load_roundtrip_preserves_all_fields() {
        let dir = unique_test_dir("settings-all-fields-roundtrip");
        let path = dir.join("settings.json");
        let settings_file = SettingsFile::with_path(path.clone());

        let save_settings = Settings {
            theme: ThemePreference::Light,
            body_font: Some(BodyFontSettings {
                family_name: "Consolas".to_string(),
                point_size_tenths: 110,
            }),
            external_editor: Some(PathBuf::from(r"C:\Tools\notepadpp\notepad++.exe")),
            recent_files: vec![],
            window_geometry: None,
            content_max_width_px: DEFAULT_CONTENT_MAX_WIDTH_PX,
        };
        settings_file.save(&save_settings).expect("save settings");

        let loaded = settings_file.load();
        assert_eq!(loaded, save_settings);
    }

    #[test]
    fn save_returns_error_when_directory_cannot_be_created() {
        let dir = unique_test_dir("settings-dir-create-fail");
        fs::write(dir.as_ref(), "block").expect("create file at parent path");
        let path = dir.join("settings.json");
        let settings_file = SettingsFile::with_path(path);

        let result = settings_file.save(&Settings::default());

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, ViewerError::SettingsSave { .. }));
        let diag = error.operator_diagnostic();
        assert!(
            diag.contains("settings save failed"),
            "diagnostic must mention settings save: {diag}"
        );
        assert!(
            diag.contains("create settings directory"),
            "diagnostic must mention directory creation: {diag}"
        );
    }

    #[test]
    fn save_returns_error_when_file_cannot_be_written() {
        let dir = unique_test_dir("settings-write-fail");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let path = dir.join("settings.json");
        fs::create_dir_all(&path).expect("create directory at settings path");
        let settings_file = SettingsFile::with_path(path);

        let result = settings_file.save(&Settings::default());

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, ViewerError::SettingsSave { .. }));
        let diag = error.operator_diagnostic();
        assert!(
            diag.contains("settings save failed"),
            "diagnostic must mention settings save: {diag}"
        );
        assert!(
            diag.contains("write settings file"),
            "diagnostic must mention file write: {diag}"
        );
    }
}
