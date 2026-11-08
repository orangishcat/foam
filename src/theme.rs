use std::{fs, path::Path};

use serde_json::Value;
use slint::Color;

use crate::ColorPalette;
use anyhow;

#[derive(Clone)]
pub struct Theme {
    pub name: String,
    pub palette: ColorPalette,
}

pub fn load_themes(
    data_dir: &Path,
    dark: ColorPalette,
    light: ColorPalette,
) -> anyhow::Result<Vec<Theme>> {
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
    themes[2..].sort_by_key(|a| a.name.to_lowercase());
    Ok(themes)
}

fn parse_color(value: &Value, key: &str) -> anyhow::Result<Color, String> {
    let value = value
        .as_str()
        .ok_or_else(|| format!("value for '{key}' must be a CSS color string"))?;
    let rgba = csscolorparser::parse(value)
        .map_err(|e| format!("invalid color for '{key}': {e}"))?
        .to_rgba8();
    Ok(Color::from_argb_u8(rgba[3], rgba[0], rgba[1], rgba[2]))
}

fn read_theme(path: &Path, dark: &ColorPalette) -> anyhow::Result<ColorPalette, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let json: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    apply_theme(json, dark)
}

fn apply_theme(json: Value, dark: &ColorPalette) -> anyhow::Result<ColorPalette, String> {
    let object = json.as_object().ok_or("theme must be a JSON object")?;
    for key in object.keys() {
        if key != "colors" && key != "override" {
            return Err(format!("unknown theme section '{key}'"));
        }
    }

    // 1. set up fallbacks for palette colors
    let mut palette = dark.clone();
    let colors_option = object
        .get("colors")
        .map(|value| value.as_object().ok_or("colors must be a JSON object"))
        .transpose()?;

    // 2. apply palette color overrides
    if let Some(colors) = colors_option {
        for (key, value) in colors {
            apply_color(key, &mut palette, parse_color(value, key)?);
        }
    }

    // 3. set up fallbacks for secondary palette colors
    set_defaults(&mut palette);

    // 4. override fallbacks if exist
    if let Some(colors) = colors_option {
        for (key, value) in colors {
            apply_palette_override(key, &mut palette, parse_color(value, key)?);
        }
    }

    // 5. override screen specific
    let overrides_option = object
        .get("override")
        .map(|value| value.as_object().ok_or("override must be a JSON object"))
        .transpose()?;
    if let Some(overrides) = overrides_option {
        for (key, value) in overrides {
            apply_screen_specific_override(key, &mut palette, parse_color(value, key)?);
        }
    }
    Ok(palette)
}

fn apply_color(key: &str, palette: &mut ColorPalette, color: Color) {
    // not having fields exposed dynamically in rust kinda sucks
    // todo: is there a better way to do this?
    // i can't inject deserialize and serialize into the slint generated types either
    match key {
        "surface.0" => palette.colors.surface_0 = color,
        "surface.1" => palette.colors.surface_1 = color,
        "surface.2" => palette.colors.surface_2 = color,
        "surface.3" => palette.colors.surface_3 = color,
        "surface.4" => palette.colors.surface_4 = color,
        "text.base" => palette.colors.text_base = color,
        "text.strong" => palette.colors.text_strong = color,
        "text.secondary" => palette.colors.text_secondary = color,
        "text.tertiary" => palette.colors.text_tertiary = color,
        "text.muted" => palette.colors.text_muted = color,
        "text.subtle" => palette.colors.text_subtle = color,
        "border" => palette.colors.border = color,
        "border.subtle" => palette.colors.border_subtle = color,
        "border.strong" => palette.colors.border_strong = color,
        "surface.hover" => palette.colors.surface_hover = color,
        "surface.overlay" => palette.colors.surface_overlay = color,
        "primary" => palette.colors.primary = color,
        "primary.hover" => palette.colors.primary_hover = color,
        "primary.strong" => palette.colors.primary_strong = color,
        "primary.status" => palette.colors.primary_status = color,
        "secondary" => palette.colors.secondary = color,
        "secondary.soft" => palette.colors.secondary_soft = color,
        "secondary.muted" => palette.colors.secondary_muted = color,
        "secondary.strong" => palette.colors.secondary_strong = color,
        "secondary.selected" => palette.colors.secondary_selected = color,
        "secondary.status" => palette.colors.secondary_status = color,
        "secondary.overlay" => palette.colors.secondary_overlay = color,
        "progress.1" => palette.colors.progress_1 = color,
        "progress.2" => palette.colors.progress_2 = color,
        "progress.3" => palette.colors.progress_3 = color,
        "progress.4" => palette.colors.progress_4 = color,
        "progress.5" => palette.colors.progress_5 = color,
        "progress.6" => palette.colors.progress_6 = color,
        "progress.7" => palette.colors.progress_7 = color,
        "progress.8" => palette.colors.progress_8 = color,
        "progress.9" => palette.colors.progress_9 = color,
        _ => log::warn!("Unknown theme key: {key}"),
    }
}

fn set_defaults(palette: &mut ColorPalette) {
    palette.colors.background = palette.colors.surface_2;
    palette.colors.sidebar = palette.colors.surface_0;
    palette.colors.top_bar = palette.colors.surface_1;
    palette.colors.bottom_bar = palette.colors.surface_1;
    palette.colors.elevate_1 = palette.colors.surface_3;
    palette.colors.elevate_2 = palette.colors.surface_4;
    palette.overrides.dashboard_overdue = palette.colors.primary_status;
    palette.overrides.dashboard_pending = palette.colors.secondary_status;
    palette.overrides.dashboard_done = palette.colors.progress_9;
    palette.overrides.attachment_backdrop = palette.colors.secondary_overlay;
}

fn apply_palette_override(key: &str, palette: &mut ColorPalette, color: Color) {
    match key {
        "background" => palette.colors.background = color,
        "topbar" => palette.colors.top_bar = color,
        "sidebar" => palette.colors.sidebar = color,
        "bottombar" => palette.colors.bottom_bar = color,
        "elevate.1" => palette.colors.elevate_1 = color,
        "elevate.2" => palette.colors.elevate_2 = color,
        _ => log::warn!("Unknown override key: {key}"),
    }
}

fn apply_screen_specific_override(key: &str, palette: &mut ColorPalette, color: Color) {
    match key {
        "dashboard.overdue" => palette.overrides.dashboard_overdue = color,
        "dashboard.pending" => palette.overrides.dashboard_pending = color,
        "attachment.backdrop" => palette.overrides.attachment_backdrop = color,
        "dashboard.done" => palette.overrides.dashboard_done = color,
        _ => log::warn!("Unknown override key: {key}"),
    }
}
