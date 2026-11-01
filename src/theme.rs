use std::{fs, io, path::Path};

use serde_json::Value;
use slint::Color;

use crate::ColorPalette;

#[derive(Clone)]
pub struct Theme {
    pub name: String,
    pub palette: ColorPalette,
}

const COLOR_KEYS: &[&str] = &[
    "surface-0",
    "surface-1",
    "surface-2",
    "surface-3",
    "surface-4",
    "background",
    "sidebar",
    "top-bar",
    "bottom-bar",
    "text-base",
    "text-strong",
    "text-secondary",
    "text-tertiary",
    "text-muted",
    "text-subtle",
    "border-subtle",
    "border",
    "border-strong",
    "surface-hover",
    "surface-overlay",
    "elevate-1",
    "elevate-2",
    "primary",
    "primary-hover",
    "primary-strong",
    "primary-status",
    "secondary-soft",
    "secondary",
    "secondary-muted",
    "secondary-strong",
    "secondary-selected",
    "secondary-status",
    "secondary-overlay",
    "progress-1",
    "progress-2",
    "progress-3",
    "progress-4",
    "progress-5",
    "progress-6",
    "progress-7",
    "progress-8",
    "progress-9",
];
const OVERRIDE_KEYS: &[&str] = &[
    "dashboard.overdue",
    "dashboard.pending",
    "dashboard.done",
    "attachment.backdrop",
];

pub fn load_themes(
    data_dir: &Path,
    dark: ColorPalette,
    light: ColorPalette,
) -> io::Result<Vec<Theme>> {
    let directory = data_dir.join("themes");
    fs::create_dir_all(&directory)?;
    let mut themes = vec![
        Theme {
            name: "Dark".into(),
            palette: dark.clone(),
        },
        Theme {
            name: "Light".into(),
            palette: light,
        },
    ];
    for entry in fs::read_dir(directory)? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                log::warn!("Could not read a theme directory entry: {error}");
                continue;
            }
        };
        let path = entry.path();
        if !path.is_file()
            || !path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
        {
            continue;
        }
        let Some(name) = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .filter(|s| !s.trim().is_empty())
        else {
            log::warn!("Ignoring theme with invalid filename: {}", path.display());
            continue;
        };
        if themes
            .iter()
            .any(|theme| theme.name.eq_ignore_ascii_case(name))
        {
            log::warn!(
                "Ignoring duplicate or reserved theme name '{name}' from {}",
                path.display()
            );
            continue;
        }
        match read_theme(&path, &dark) {
            Ok(palette) => themes.push(Theme {
                name: name.to_owned(),
                palette,
            }),
            Err(error) => log::warn!("Ignoring invalid theme {}: {error}", path.display()),
        }
    }
    themes[2..].sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(themes)
}

fn parse_color(value: &Value, key: &str) -> Result<Color, String> {
    let value = value
        .as_str()
        .ok_or_else(|| format!("value for '{key}' must be a CSS color string"))?;
    let rgba = csscolorparser::parse(value)
        .map_err(|e| format!("invalid color for '{key}': {e}"))?
        .to_rgba8();
    Ok(Color::from_argb_u8(rgba[3], rgba[0], rgba[1], rgba[2]))
}

fn read_theme(path: &Path, dark: &ColorPalette) -> Result<ColorPalette, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let json: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    apply_theme(json, dark)
}

fn apply_theme(json: Value, dark: &ColorPalette) -> Result<ColorPalette, String> {
    let object = json.as_object().ok_or("theme must be a JSON object")?;
    for key in object.keys() {
        if key != "colors" && key != "override" {
            return Err(format!("unknown theme section '{key}'"));
        }
    }
    let mut palette = dark.clone();
    let colors = object
        .get("colors")
        .map(|value| value.as_object().ok_or("colors must be a JSON object"))
        .transpose()?;
    if let Some(colors) = colors {
        for key in colors.keys() {
            if !COLOR_KEYS.contains(&key.as_str()) {
                return Err(format!("unknown default color '{key}'"));
            }
        }
        for (key, value) in colors {
            let color = parse_color(value, key)?;
            match key.as_str() {
                "surface-0" => palette.colors.surface_0 = color,
                "surface-1" => palette.colors.surface_1 = color,
                "surface-2" => palette.colors.surface_2 = color,
                "surface-3" => palette.colors.surface_3 = color,
                "surface-4" => palette.colors.surface_4 = color,
                "background" => palette.colors.background = color,
                "sidebar" => palette.colors.sidebar = color,
                "top-bar" => palette.colors.top_bar = color,
                "bottom-bar" => palette.colors.bottom_bar = color,
                "text-base" => palette.colors.text_base = color,
                "text-strong" => palette.colors.text_strong = color,
                "text-secondary" => palette.colors.text_secondary = color,
                "text-tertiary" => palette.colors.text_tertiary = color,
                "text-muted" => palette.colors.text_muted = color,
                "text-subtle" => palette.colors.text_subtle = color,
                "border-subtle" => palette.colors.border_subtle = color,
                "border" => palette.colors.border = color,
                "border-strong" => palette.colors.border_strong = color,
                "surface-hover" => palette.colors.surface_hover = color,
                "surface-overlay" => palette.colors.surface_overlay = color,
                "elevate-1" => palette.colors.elevate_1 = color,
                "elevate-2" => palette.colors.elevate_2 = color,
                "primary" => palette.colors.primary = color,
                "primary-hover" => palette.colors.primary_hover = color,
                "primary-strong" => palette.colors.primary_strong = color,
                "primary-status" => palette.colors.primary_status = color,
                "secondary-soft" => palette.colors.secondary_soft = color,
                "secondary" => palette.colors.secondary = color,
                "secondary-muted" => palette.colors.secondary_muted = color,
                "secondary-strong" => palette.colors.secondary_strong = color,
                "secondary-selected" => palette.colors.secondary_selected = color,
                "secondary-status" => palette.colors.secondary_status = color,
                "secondary-overlay" => palette.colors.secondary_overlay = color,
                "progress-1" => palette.colors.progress_1 = color,
                "progress-2" => palette.colors.progress_2 = color,
                "progress-3" => palette.colors.progress_3 = color,
                "progress-4" => palette.colors.progress_4 = color,
                "progress-5" => palette.colors.progress_5 = color,
                "progress-6" => palette.colors.progress_6 = color,
                "progress-7" => palette.colors.progress_7 = color,
                "progress-8" => palette.colors.progress_8 = color,
                "progress-9" => palette.colors.progress_9 = color,
                _ => unreachable!("validated default color key"),
            }
        }
    }
    let has_color_override = |key: &str| colors.is_some_and(|colors| colors.contains_key(key));
    if !has_color_override("background") {
        palette.colors.background = palette.colors.surface_2;
    }
    if !has_color_override("sidebar") {
        palette.colors.sidebar = palette.colors.surface_0;
    }
    if !has_color_override("top-bar") {
        palette.colors.top_bar = palette.colors.surface_1;
    }
    if !has_color_override("bottom-bar") {
        palette.colors.bottom_bar = palette.colors.surface_1;
    }
    if !has_color_override("elevate-1") {
        palette.colors.elevate_1 = palette.colors.surface_3;
    }
    if !has_color_override("elevate-2") {
        palette.colors.elevate_2 = palette.colors.surface_4;
    }
    // Screen-specific defaults are aliases into the shared palette; explicit overrides win.
    palette.overrides.dashboard_overdue = palette.colors.primary_status;
    palette.overrides.dashboard_pending = palette.colors.secondary_status;
    palette.overrides.dashboard_done = palette.colors.progress_9;
    palette.overrides.attachment_backdrop = palette.colors.secondary_overlay;
    let overrides = object
        .get("override")
        .map(|value| value.as_object().ok_or("override must be a JSON object"))
        .transpose()?;
    if let Some(overrides) = overrides {
        for key in overrides.keys() {
            if !OVERRIDE_KEYS.contains(&key.as_str()) {
                return Err(format!("unknown override key '{key}'"));
            }
        }
        for (key, value) in overrides {
            let color = parse_color(value, key)?;
            match key.as_str() {
                "dashboard.overdue" => palette.overrides.dashboard_overdue = color,
                "dashboard.pending" => palette.overrides.dashboard_pending = color,
                "attachment.backdrop" => palette.overrides.attachment_backdrop = color,
                "dashboard.done" => palette.overrides.dashboard_done = color,
                _ => unreachable!("validated override key"),
            }
        }
    }
    Ok(palette)
}

#[cfg(test)]
mod tests {
    use super::apply_theme;
    use crate::{ColorOverrides, ColorPalette, DefaultColors};
    use serde_json::json;
    use slint::Color;

    fn color(red: u8) -> Color {
        Color::from_argb_u8(255, red, 2, 3)
    }

    fn dark_defaults() -> ColorPalette {
        let default = color(1);
        ColorPalette {
            colors: DefaultColors {
                surface_0: color(10),
                surface_1: color(20),
                surface_2: color(30),
                surface_3: color(40),
                surface_4: color(50),
                background: default,
                sidebar: default,
                top_bar: default,
                bottom_bar: default,
                text_base: default,
                text_strong: default,
                text_secondary: default,
                text_tertiary: default,
                text_muted: default,
                text_subtle: default,
                border_subtle: default,
                border: default,
                border_strong: default,
                surface_hover: default,
                surface_overlay: default,
                elevate_1: default,
                elevate_2: default,
                primary: default,
                primary_hover: default,
                primary_strong: default,
                primary_status: color(60),
                secondary_soft: default,
                secondary: default,
                secondary_muted: default,
                secondary_strong: default,
                secondary_selected: default,
                secondary_status: color(70),
                secondary_overlay: color(80),
                progress_1: default,
                progress_2: default,
                progress_3: default,
                progress_4: default,
                progress_5: default,
                progress_6: default,
                progress_7: default,
                progress_8: default,
                progress_9: color(90),
            },
            overrides: ColorOverrides {
                dashboard_overdue: default,
                dashboard_pending: default,
                attachment_backdrop: default,
                dashboard_done: default,
            },
        }
    }

    #[test]
    fn missing_sections_use_dark_defaults_and_surface_aliases() {
        let dark = dark_defaults();
        let result = apply_theme(json!({}), &dark).unwrap();
        assert_eq!(result.colors.surface_0, dark.colors.surface_0);
        assert_eq!(result.colors.text_base, dark.colors.text_base);
        assert_eq!(result.colors.sidebar, dark.colors.surface_0);
        assert_eq!(result.colors.background, dark.colors.surface_2);
        assert_eq!(result.colors.elevate_1, dark.colors.surface_3);
        assert_eq!(result.colors.elevate_2, dark.colors.surface_4);
        assert_eq!(
            result.overrides.dashboard_overdue,
            dark.colors.primary_status
        );
        assert_eq!(
            result.overrides.dashboard_pending,
            dark.colors.secondary_status
        );
        assert_eq!(result.overrides.dashboard_done, dark.colors.progress_9);
        assert_eq!(
            result.overrides.attachment_backdrop,
            dark.colors.secondary_overlay
        );
    }

    #[test]
    fn partial_default_colors_flow_to_screen_defaults_and_overrides_win() {
        let result = apply_theme(
            json!({
                "colors": {"surface-0":"#112233", "surface-2":"#445566", "text-base":"#010203", "primary-status":"#778899", "secondary-status":"#8899aa"},
                "override": {"dashboard.overdue":"#abcdef"}
            }),
            &dark_defaults(),
        )
        .unwrap();
        assert_eq!(result.colors.surface_0.to_argb_u8().red, 17);
        assert_eq!(result.colors.background.to_argb_u8().red, 68);
        assert_eq!(result.colors.text_base.to_argb_u8().red, 1);
        assert_eq!(result.overrides.dashboard_overdue.to_argb_u8().red, 171);
        assert_eq!(result.overrides.dashboard_pending.to_argb_u8().red, 136);
    }

    #[test]
    fn dotted_semantic_keys_and_alpha_are_supported() {
        let result = apply_theme(
            json!({"override":{"dashboard.overdue":"rgb(10 20 30 / 50%)"}}),
            &dark_defaults(),
        )
        .unwrap();
        let rgba = result.overrides.dashboard_overdue.to_argb_u8();
        assert_eq!(
            (rgba.alpha, rgba.red, rgba.green, rgba.blue),
            (128, 10, 20, 30)
        );
    }

    #[test]
    fn rejects_unknown_keys_and_invalid_sections() {
        let dark = dark_defaults();
        assert!(apply_theme(json!({"unknown":{}}), &dark).is_err());
        assert!(apply_theme(json!({"colors":{"surface-5":"red"}}), &dark).is_err());
        assert!(apply_theme(json!({"override":{"unknown":"red"}}), &dark).is_err());
        assert!(apply_theme(json!({"override":{"text.primary":"red"}}), &dark).is_err());
        assert!(apply_theme(json!({"colors":null}), &dark).is_err());
        assert!(apply_theme(json!({"override":{"dashboard.overdue":null}}), &dark).is_err());
    }
}
