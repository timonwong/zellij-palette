use crate::model::{
    CommandAction, PaletteAction, PaletteId, PaletteItem, PopupCoordinates, ThemeAction,
};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const CONFIG_EXTS: &[&str] = &["toml", "yaml", "json"];

#[derive(Clone, Debug, Default)]
pub struct UserConfig {
    pub commands: Vec<PaletteItem>,
    pub custom_palettes: HashMap<String, CustomPalette>,
    pub theme_names: Vec<String>,
    pub shortcut_overrides: HashMap<String, String>,
    pub alias_overrides: HashMap<String, Vec<String>>,
    pub hidden_titles: HashSet<String>,
}

#[derive(Clone, Debug, Default)]
pub struct CustomPalette {
    pub title: Option<String>,
    pub from: Vec<String>,
    pub from_category: Option<String>,
    pub command: Option<String>,
    pub template_action: Option<ConfigAction>,
    pub default_icon: Option<String>,
    pub default_icon_color: Option<String>,
    pub grouped: Option<bool>,
    pub empty_text: Option<String>,
    pub items: Vec<PaletteItem>,
}

#[derive(Debug, Deserialize)]
struct RawPaletteItem {
    title: String,
    description: Option<String>,
    category: Option<String>,
    group: Option<String>,
    aliases: Option<Vec<String>>,
    shortcut: Option<String>,
    icon: Option<String>,
    #[serde(rename = "iconColor", alias = "icon_color")]
    icon_color: Option<String>,
    action: ConfigAction,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum ConfigAction {
    Palette {
        palette: String,
    },
    Shell {
        shell: String,
    },
    Popup {
        popup: String,
        x: Option<String>,
        y: Option<String>,
        width: Option<String>,
        height: Option<String>,
        pinned: Option<bool>,
        borderless: Option<bool>,
    },
    Theme {
        theme: String,
    },
}

#[derive(Debug, Deserialize)]
struct RawCustomPalette {
    title: Option<String>,
    from: Option<Vec<String>>,
    #[serde(rename = "fromCategory", alias = "from_category")]
    from_category: Option<String>,
    #[serde(rename = "fromGroup", alias = "from_group")]
    from_group: Option<String>,
    command: Option<String>,
    action: Option<ConfigAction>,
    icon: Option<String>,
    #[serde(rename = "iconColor", alias = "icon_color")]
    icon_color: Option<String>,
    grouped: Option<bool>,
    #[serde(rename = "emptyText", alias = "empty_text")]
    empty_text: Option<String>,
    items: Option<Vec<RawPaletteItem>>,
}

pub fn load_user_config(home: Option<&Path>, theme_dir: Option<&Path>) -> UserConfig {
    // theme_dir is independent of home — a user can point at a themes
    // dir even when HOME is unavailable. Everything else still keys off
    // ~/.config/zellij-palette/.
    let theme_names = theme_dir.map(load_theme_names).unwrap_or_default();

    let Some(home) = home else {
        return UserConfig {
            theme_names,
            ..UserConfig::default()
        };
    };

    let config_root = home.join(".config").join("zellij-palette");
    let commands = load_commands(&config_root);
    let custom_palettes = load_custom_palettes(&config_root.join("palettes"));
    let shortcut_overrides = load_config_file(&config_root, "shortcuts").unwrap_or_default();
    let alias_overrides = load_config_file(&config_root, "aliases").unwrap_or_default();
    let hidden_titles = load_hidden(&config_root).into_iter().collect();

    UserConfig {
        commands,
        custom_palettes,
        theme_names,
        shortcut_overrides,
        alias_overrides,
        hidden_titles,
    }
}

// `commands` is a top-level array in JSON/YAML, but TOML disallows root
// arrays — for TOML we accept `[[commands]]` (array of tables wrapped under
// a `commands` key).
fn load_commands(config_root: &Path) -> Vec<PaletteItem> {
    #[derive(Deserialize)]
    struct TomlWrap {
        commands: Option<Vec<RawPaletteItem>>,
    }

    for ext in CONFIG_EXTS {
        let path = config_root.join(format!("commands.{ext}"));
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let parsed: Option<Vec<RawPaletteItem>> = match *ext {
            "toml" => toml::from_str::<TomlWrap>(&raw)
                .ok()
                .and_then(|w| w.commands),
            "yaml" => serde_yml::from_str(&raw).ok(),
            "json" => serde_json::from_str(&raw).ok(),
            _ => None,
        };
        return parsed
            .unwrap_or_default()
            .into_iter()
            .map(raw_item_to_palette_item)
            .collect();
    }
    Vec::new()
}

// Same TOML root-table constraint as `commands` — for TOML we accept
// `hidden = ["..."]`.
fn load_hidden(config_root: &Path) -> Vec<String> {
    #[derive(Deserialize)]
    struct TomlWrap {
        hidden: Option<Vec<String>>,
    }

    for ext in CONFIG_EXTS {
        let path = config_root.join(format!("hidden.{ext}"));
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let parsed: Option<Vec<String>> = match *ext {
            "toml" => toml::from_str::<TomlWrap>(&raw).ok().and_then(|w| w.hidden),
            "yaml" => serde_yml::from_str(&raw).ok(),
            "json" => serde_json::from_str(&raw).ok(),
            _ => None,
        };
        return parsed.unwrap_or_default();
    }
    Vec::new()
}

fn load_custom_palettes(dir: &Path) -> HashMap<String, CustomPalette> {
    let Ok(entries) = fs::read_dir(dir) else {
        return HashMap::new();
    };

    // stem -> (priority index in CONFIG_EXTS, path, ext)
    let mut best: HashMap<String, (usize, PathBuf, &'static str)> = HashMap::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        let Some(prio) = CONFIG_EXTS.iter().position(|candidate| *candidate == ext) else {
            continue;
        };
        let Some(stem) = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(str::to_owned)
        else {
            continue;
        };

        let ext_static = CONFIG_EXTS[prio];
        best.entry(stem)
            .and_modify(|cur| {
                if prio < cur.0 {
                    *cur = (prio, path.clone(), ext_static);
                }
            })
            .or_insert((prio, path, ext_static));
    }

    let mut palettes = HashMap::new();
    for (name, (_, path, ext)) in best {
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Some(parsed) = parse_by_ext::<RawCustomPalette>(ext, &raw) else {
            continue;
        };
        palettes.insert(name, custom_palette_from_raw(parsed));
    }
    palettes
}

fn custom_palette_from_raw(parsed: RawCustomPalette) -> CustomPalette {
    CustomPalette {
        title: parsed.title,
        from: parsed.from.unwrap_or_default(),
        from_category: parsed.from_category.or(parsed.from_group),
        command: parsed.command,
        template_action: parsed.action,
        default_icon: parsed.icon,
        default_icon_color: parsed.icon_color,
        grouped: parsed.grouped,
        empty_text: parsed.empty_text,
        items: parsed
            .items
            .unwrap_or_default()
            .into_iter()
            .map(raw_item_to_palette_item)
            .collect(),
    }
}

fn load_theme_names(path: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(path) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("kdl") {
                return None;
            }
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_owned)
        })
        .collect();
    names.sort();
    names
}

pub fn parse_command_palette_output(
    output: &str,
    template_action: Option<&ConfigAction>,
    default_icon: Option<&str>,
    default_icon_color: Option<&str>,
) -> Vec<PaletteItem> {
    if let Ok(parsed) = serde_json::from_str::<Vec<RawPaletteItem>>(output) {
        return parsed.into_iter().map(raw_item_to_palette_item).collect();
    }

    let Some(template_action) = template_action.cloned() else {
        return Vec::new();
    };

    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            plain_line_to_palette_item(
                line,
                template_action.clone(),
                default_icon,
                default_icon_color,
            )
        })
        .collect()
}

fn raw_item_to_palette_item(item: RawPaletteItem) -> PaletteItem {
    let mut palette_item =
        PaletteItem::leaf(item.title, config_action_to_palette_action(item.action));
    if let Some(description) = item.description {
        palette_item = palette_item.with_description(description);
    }
    if let Some(category) = item.category.or(item.group) {
        palette_item.category = Some(category);
    }
    if let Some(aliases) = item.aliases {
        palette_item.aliases = aliases;
    }
    if let Some(shortcut) = item.shortcut {
        palette_item.shortcut = Some(shortcut);
    }
    if let Some(icon) = item.icon {
        palette_item.icon = Some(icon);
    }
    if let Some(icon_color) = item.icon_color {
        palette_item.icon_color = Some(icon_color);
    }
    palette_item
}

fn plain_line_to_palette_item(
    line: &str,
    template_action: ConfigAction,
    default_icon: Option<&str>,
    default_icon_color: Option<&str>,
) -> PaletteItem {
    let parts: Vec<&str> = line.split('\t').collect();
    let (icon, icon_color, title) = match parts.as_slice() {
        [title] => (
            default_icon.map(str::to_owned),
            default_icon_color.map(str::to_owned),
            (*title).to_owned(),
        ),
        [icon, title] => (
            Some((*icon).to_owned()),
            default_icon_color.map(str::to_owned),
            (*title).to_owned(),
        ),
        [icon, icon_color, rest @ ..] => (
            Some((*icon).to_owned()),
            Some((*icon_color).to_owned()),
            rest.join("\t"),
        ),
        [] => (None, None, String::new()),
    };

    let mut item = PaletteItem::leaf(
        title.clone(),
        config_action_to_palette_action(interpolate_action(template_action, &title)),
    );
    item.icon = icon.filter(|value| !value.is_empty());
    item.icon_color = icon_color.filter(|value| !value.is_empty());
    item
}

fn interpolate_action(action: ConfigAction, value: &str) -> ConfigAction {
    match action {
        ConfigAction::Palette { palette } => ConfigAction::Palette {
            palette: palette.replace("{}", value),
        },
        ConfigAction::Shell { shell } => ConfigAction::Shell {
            shell: shell.replace("{}", value),
        },
        ConfigAction::Popup {
            popup,
            x,
            y,
            width,
            height,
            pinned,
            borderless,
        } => ConfigAction::Popup {
            popup: popup.replace("{}", value),
            x,
            y,
            width,
            height,
            pinned,
            borderless,
        },
        ConfigAction::Theme { theme } => ConfigAction::Theme {
            theme: theme.replace("{}", value),
        },
    }
}

pub fn config_action_to_palette_action(action: ConfigAction) -> PaletteAction {
    match action {
        ConfigAction::Palette { palette } => match palette.as_str() {
            "commands" => PaletteAction::OpenPalette(PaletteId::Commands),
            "find-pane" => PaletteAction::OpenPalette(PaletteId::FindPane),
            "move-pane" => PaletteAction::OpenPalette(PaletteId::MovePane),
            "sessions" => PaletteAction::OpenPalette(PaletteId::Sessions),
            "themes" => PaletteAction::OpenPalette(PaletteId::Themes),
            _ => PaletteAction::OpenCustomPalette(palette),
        },
        ConfigAction::Shell { shell } => PaletteAction::RunShell(CommandAction {
            command: shell,
            cwd: None,
        }),
        ConfigAction::Popup {
            popup,
            x,
            y,
            width,
            height,
            pinned,
            borderless,
        } => PaletteAction::OpenCommandPane {
            command: CommandAction {
                command: popup,
                cwd: None,
            },
            coordinates: PopupCoordinates::new(x, y, width, height, pinned, borderless),
            floating: true,
        },
        ConfigAction::Theme { theme } => match theme.as_str() {
            "dark" => PaletteAction::Theme(ThemeAction::SetDark),
            "light" => PaletteAction::Theme(ThemeAction::SetLight),
            "toggle" => PaletteAction::Theme(ThemeAction::Toggle),
            other => PaletteAction::Theme(ThemeAction::SetNamed(other.to_owned())),
        },
    }
}

pub fn with_command_cwd(mut action: PaletteAction, cwd: Option<PathBuf>) -> PaletteAction {
    match &mut action {
        PaletteAction::RunShell(command) | PaletteAction::OpenCommandPane { command, .. } => {
            command.cwd = cwd;
        }
        PaletteAction::NewTab { cwd: destination } => *destination = cwd,
        _ => {}
    }
    action
}

pub fn apply_item_overrides(
    items: Vec<PaletteItem>,
    shortcuts: &HashMap<String, String>,
    aliases: &HashMap<String, Vec<String>>,
) -> Vec<PaletteItem> {
    items
        .into_iter()
        .map(|mut item| {
            if item.shortcut.is_none() {
                item.shortcut = shortcuts.get(&item.title).cloned();
            }
            if let Some(extra_aliases) = aliases.get(&item.title) {
                item.aliases.extend(extra_aliases.iter().cloned());
            }
            item
        })
        .collect()
}

pub fn filter_hidden_items(
    items: Vec<PaletteItem>,
    hidden_titles: &HashSet<String>,
) -> Vec<PaletteItem> {
    items
        .into_iter()
        .filter(|item| !hidden_titles.contains(&item.title))
        .collect()
}

pub fn referenced_items_from_custom_palette(
    base_items: &[PaletteItem],
    custom_palette: &CustomPalette,
) -> Vec<PaletteItem> {
    let mut items = Vec::new();
    for title in &custom_palette.from {
        if let Some(item) = base_items.iter().find(|item| item.title == *title) {
            items.push(item.clone());
        }
    }
    if let Some(category) = &custom_palette.from_category {
        for item in base_items
            .iter()
            .filter(|item| item.category.as_ref() == Some(category))
        {
            items.push(item.clone());
        }
    }
    items
}

fn parse_by_ext<T: DeserializeOwned>(ext: &str, raw: &str) -> Option<T> {
    match ext {
        "toml" => toml::from_str(raw).ok(),
        "yaml" => serde_yml::from_str(raw).ok(),
        "json" => serde_json::from_str(raw).ok(),
        _ => None,
    }
}

// Try each accepted extension in priority order (toml > yaml > json) and
// return the first one whose file exists and parses. If a file exists but
// fails to parse, we stop the chain — falling through to a lower-priority
// stale file would silently substitute behind the user's back.
fn load_config_file<T: DeserializeOwned>(dir: &Path, stem: &str) -> Option<T> {
    for ext in CONFIG_EXTS {
        let path = dir.join(format!("{stem}.{ext}"));
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        return parse_by_ext::<T>(ext, &raw);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        ConfigAction, CustomPalette, apply_item_overrides, filter_hidden_items, load_user_config,
        parse_command_palette_output, referenced_items_from_custom_palette,
    };
    use crate::model::{PaletteAction, PaletteItem};
    use std::collections::{HashMap, HashSet};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn load_user_config_reads_overlay_files_and_category_aliases() {
        let home = temp_home("load-user-config");
        let config_root = home.join(".config").join("zellij-palette");
        fs::create_dir_all(config_root.join("palettes")).unwrap();
        fs::create_dir_all(home.join(".config").join("zellij").join("themes")).unwrap();

        fs::write(
            config_root.join("commands.json"),
            r##"[{"title":"Open Logs","group":"Tools","shortcut":"Cmd-L","icon":"󰌱","iconColor":"#22cc22","action":{"popup":"tail -f logs.txt"}}]"##,
        )
        .unwrap();
        fs::write(
            config_root.join("shortcuts.json"),
            r#"{"Find Pane":"Ctrl-P"}"#,
        )
        .unwrap();
        fs::write(
            config_root.join("aliases.json"),
            r#"{"Find Pane":["jump","locator"]}"#,
        )
        .unwrap();
        fs::write(config_root.join("hidden.json"), r#"["Split Down"]"#).unwrap();
        fs::write(
            config_root.join("palettes").join("tools.json"),
            r##"{"title":"Tools","fromCategory":"Tools","icon":"󰆍","iconColor":"#ffaa00","grouped":true,"emptyText":"No tools","action":{"popup":"echo {}"}}"##,
        )
        .unwrap();
        let theme_dir = home.join(".config").join("zellij").join("themes");
        fs::write(theme_dir.join("nord.kdl"), "theme nord {}").unwrap();

        let user_config = load_user_config(Some(&home), Some(&theme_dir));

        assert_eq!(user_config.commands.len(), 1);
        assert_eq!(user_config.commands[0].category.as_deref(), Some("Tools"));
        assert_eq!(user_config.commands[0].shortcut.as_deref(), Some("Cmd-L"));
        assert_eq!(user_config.commands[0].icon.as_deref(), Some("󰌱"));
        assert_eq!(
            user_config.commands[0].icon_color.as_deref(),
            Some("#22cc22")
        );
        assert_eq!(
            user_config
                .shortcut_overrides
                .get("Find Pane")
                .map(String::as_str),
            Some("Ctrl-P")
        );
        assert_eq!(
            user_config
                .alias_overrides
                .get("Find Pane")
                .map(|aliases| aliases.as_slice()),
            Some(["jump".to_owned(), "locator".to_owned()].as_slice())
        );
        assert!(user_config.hidden_titles.contains("Split Down"));
        assert_eq!(user_config.theme_names, vec!["nord".to_owned()]);

        let tools = user_config.custom_palettes.get("tools").unwrap();
        assert_eq!(tools.from_category.as_deref(), Some("Tools"));
        assert_eq!(tools.default_icon.as_deref(), Some("󰆍"));
        assert_eq!(tools.default_icon_color.as_deref(), Some("#ffaa00"));
        assert_eq!(tools.grouped, Some(true));
        assert_eq!(tools.empty_text.as_deref(), Some("No tools"));
    }

    // Proves the old implicit `~/.config/zellij/themes` scan is gone:
    // theme files sitting at that path no longer reach the palette when
    // the caller passes `None` for theme_dir.
    #[test]
    fn theme_dir_must_be_passed_explicitly() {
        let home = temp_home("theme-dir-explicit");
        let legacy_theme_dir = home.join(".config").join("zellij").join("themes");
        fs::create_dir_all(&legacy_theme_dir).unwrap();
        fs::write(legacy_theme_dir.join("nord.kdl"), "theme nord {}").unwrap();
        fs::write(legacy_theme_dir.join("dracula.kdl"), "theme dracula {}").unwrap();

        let without_dir = load_user_config(Some(&home), None);
        assert!(
            without_dir.theme_names.is_empty(),
            "theme files at the legacy path must not leak in when theme_dir is None",
        );

        let with_dir = load_user_config(Some(&home), Some(&legacy_theme_dir));
        assert_eq!(
            with_dir.theme_names,
            vec!["dracula".to_owned(), "nord".to_owned()]
        );
    }

    #[test]
    fn load_user_config_accepts_toml_overlay_files() {
        let home = temp_home("load-user-config-toml");
        let config_root = home.join(".config").join("zellij-palette");
        fs::create_dir_all(config_root.join("palettes")).unwrap();

        fs::write(
            config_root.join("commands.toml"),
            r##"
[[commands]]
title = "Open Logs"
group = "Tools"
shortcut = "Cmd-L"
icon = "L"
icon_color = "#22cc22"
action = { popup = "tail -f logs.txt" }
"##,
        )
        .unwrap();
        fs::write(
            config_root.join("shortcuts.toml"),
            r#""Find Pane" = "Ctrl-P""#,
        )
        .unwrap();
        fs::write(
            config_root.join("aliases.toml"),
            r#""Find Pane" = ["jump", "locator"]"#,
        )
        .unwrap();
        fs::write(
            config_root.join("hidden.toml"),
            r#"hidden = ["Split Down"]"#,
        )
        .unwrap();
        fs::write(
            config_root.join("palettes").join("tools.toml"),
            r##"
title = "Tools"
from_category = "Tools"
icon = "T"
icon_color = "#ffaa00"
grouped = true
empty_text = "No tools"
action = { popup = "echo {}" }
"##,
        )
        .unwrap();

        let user_config = load_user_config(Some(&home), None);

        assert_eq!(user_config.commands.len(), 1);
        assert_eq!(user_config.commands[0].category.as_deref(), Some("Tools"));
        assert_eq!(user_config.commands[0].shortcut.as_deref(), Some("Cmd-L"));
        assert_eq!(user_config.commands[0].icon.as_deref(), Some("L"));
        assert_eq!(
            user_config.commands[0].icon_color.as_deref(),
            Some("#22cc22")
        );
        assert_eq!(
            user_config
                .shortcut_overrides
                .get("Find Pane")
                .map(String::as_str),
            Some("Ctrl-P")
        );
        assert_eq!(
            user_config
                .alias_overrides
                .get("Find Pane")
                .map(|aliases| aliases.as_slice()),
            Some(["jump".to_owned(), "locator".to_owned()].as_slice())
        );
        assert!(user_config.hidden_titles.contains("Split Down"));

        let tools = user_config.custom_palettes.get("tools").unwrap();
        assert_eq!(tools.from_category.as_deref(), Some("Tools"));
        assert_eq!(tools.default_icon.as_deref(), Some("T"));
        assert_eq!(tools.default_icon_color.as_deref(), Some("#ffaa00"));
        assert_eq!(tools.grouped, Some(true));
        assert_eq!(tools.empty_text.as_deref(), Some("No tools"));
    }

    #[test]
    fn load_user_config_prefers_toml_over_json_for_overlays() {
        let home = temp_home("load-user-config-priority");
        let config_root = home.join(".config").join("zellij-palette");
        fs::create_dir_all(&config_root).unwrap();

        fs::write(
            config_root.join("shortcuts.toml"),
            r#""Find Pane" = "Ctrl-T""#,
        )
        .unwrap();
        fs::write(
            config_root.join("shortcuts.json"),
            r#"{"Find Pane":"Ctrl-J"}"#,
        )
        .unwrap();
        fs::write(
            config_root.join("shortcuts.yaml"),
            r#""Find Pane": "Ctrl-Y""#,
        )
        .unwrap();

        let user_config = load_user_config(Some(&home), None);

        // TOML wins over both YAML and JSON.
        assert_eq!(
            user_config
                .shortcut_overrides
                .get("Find Pane")
                .map(String::as_str),
            Some("Ctrl-T")
        );
    }

    #[test]
    fn broken_higher_priority_file_does_not_fall_through_to_lower_priority() {
        let home = temp_home("load-user-config-broken");
        let config_root = home.join(".config").join("zellij-palette");
        fs::create_dir_all(&config_root).unwrap();

        // Syntactically invalid TOML.
        fs::write(
            config_root.join("shortcuts.toml"),
            "this is = not = valid = toml",
        )
        .unwrap();
        // A perfectly valid JSON file that should NOT be substituted.
        fs::write(
            config_root.join("shortcuts.json"),
            r#"{"Find Pane":"Ctrl-J"}"#,
        )
        .unwrap();

        let user_config = load_user_config(Some(&home), None);

        assert!(user_config.shortcut_overrides.is_empty());
    }

    #[test]
    fn load_custom_palettes_picks_highest_priority_per_stem() {
        let home = temp_home("load-user-config-palettes-mix");
        let palettes_dir = home.join(".config").join("zellij-palette").join("palettes");
        fs::create_dir_all(&palettes_dir).unwrap();

        // `tools` has both .toml and .json — TOML must win.
        fs::write(
            palettes_dir.join("tools.toml"),
            r#"
title = "Tools (TOML)"
action = { popup = "echo {}" }
"#,
        )
        .unwrap();
        fs::write(
            palettes_dir.join("tools.json"),
            r#"{"title":"Tools (JSON)","action":{"popup":"echo {}"}}"#,
        )
        .unwrap();
        // `panes` only as .yaml.
        fs::write(
            palettes_dir.join("panes.yaml"),
            "title: Panes (YAML)\naction:\n  popup: echo {}\n",
        )
        .unwrap();
        // `.yml` is intentionally unsupported and must be ignored.
        fs::write(
            palettes_dir.join("ignored.yml"),
            "title: Ignored\naction:\n  popup: echo {}\n",
        )
        .unwrap();

        let user_config = load_user_config(Some(&home), None);

        assert_eq!(
            user_config
                .custom_palettes
                .get("tools")
                .and_then(|p| p.title.as_deref()),
            Some("Tools (TOML)")
        );
        assert_eq!(
            user_config
                .custom_palettes
                .get("panes")
                .and_then(|p| p.title.as_deref()),
            Some("Panes (YAML)")
        );
        assert!(!user_config.custom_palettes.contains_key("ignored"));
    }

    // Stage the bundled examples/ into a temp HOME and verify they load
    // through the real loader. Guards against TOML/escape regressions in
    // the user-facing samples.
    #[test]
    fn bundled_examples_toml_files_parse() {
        let home = temp_home("load-user-config-examples");
        let config_root = home.join(".config").join("zellij-palette");
        fs::create_dir_all(config_root.join("palettes")).unwrap();

        let project_examples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples");
        for name in ["commands", "shortcuts", "aliases", "hidden"] {
            fs::copy(
                project_examples.join(format!("{name}.toml")),
                config_root.join(format!("{name}.toml")),
            )
            .unwrap();
        }
        fs::copy(
            project_examples.join("palettes").join("github-prs.toml"),
            config_root.join("palettes").join("github-prs.toml"),
        )
        .unwrap();

        let user_config = load_user_config(Some(&home), None);

        assert!(
            user_config
                .commands
                .iter()
                .any(|item| item.title == "lazygit")
        );
        assert!(
            user_config
                .commands
                .iter()
                .any(|item| item.title == "Tail logs")
        );
        assert_eq!(
            user_config
                .shortcut_overrides
                .get("Find Pane")
                .map(String::as_str),
            Some("Ctrl-F")
        );
        assert_eq!(
            user_config
                .alias_overrides
                .get("Tail logs")
                .map(|aliases| aliases.as_slice()),
            Some(["journal".to_owned()].as_slice())
        );
        assert!(user_config.hidden_titles.contains("Detach Session"));

        let gh = user_config.custom_palettes.get("github-prs").unwrap();
        assert_eq!(gh.title.as_deref(), Some("GitHub PRs"));
        assert_eq!(gh.from_category.as_deref(), Some("Tools"));
        // The jq escapes survive a TOML round-trip.
        assert!(
            gh.command
                .as_deref()
                .unwrap()
                .contains(r##""#\(.number) \(.title)""##)
        );
    }

    #[test]
    fn plain_line_output_supports_default_and_inline_icon_metadata() {
        let items = parse_command_palette_output(
            "first\n󰍉\tsecond\n●\t#00ff00\tthird",
            Some(&ConfigAction::Popup {
                popup: "echo {}".to_owned(),
                x: Some("10%".to_owned()),
                y: Some("5%".to_owned()),
                width: Some("80%".to_owned()),
                height: Some("70%".to_owned()),
                pinned: Some(true),
                borderless: Some(false),
            }),
            Some("󰊠"),
            Some("#ffaa00"),
        );

        assert_eq!(items.len(), 3);
        assert_eq!(items[0].title, "first");
        assert_eq!(items[0].icon.as_deref(), Some("󰊠"));
        assert_eq!(items[0].icon_color.as_deref(), Some("#ffaa00"));
        assert_eq!(items[1].icon.as_deref(), Some("󰍉"));
        assert_eq!(items[1].icon_color.as_deref(), Some("#ffaa00"));
        assert_eq!(items[2].icon.as_deref(), Some("●"));
        assert_eq!(items[2].icon_color.as_deref(), Some("#00ff00"));

        match &items[2].action {
            PaletteAction::OpenCommandPane {
                command,
                coordinates,
                floating,
            } => {
                assert!(floating);
                assert_eq!(command.command, "echo third");
                let coordinates = coordinates.as_ref().unwrap();
                assert_eq!(coordinates.x.as_deref(), Some("10%"));
                assert_eq!(coordinates.width.as_deref(), Some("80%"));
                assert_eq!(coordinates.pinned, Some(true));
            }
            action => panic!("unexpected action: {action:?}"),
        }
    }

    #[test]
    fn item_overrides_preserve_explicit_shortcuts_and_append_aliases() {
        let items = vec![
            PaletteItem::leaf("Find Pane", PaletteAction::Noop)
                .with_shortcut("Enter")
                .with_aliases(["pane"]),
            PaletteItem::leaf("Switch Theme", PaletteAction::Noop),
        ];
        let shortcuts = HashMap::from([
            ("Find Pane".to_owned(), "Ctrl-P".to_owned()),
            ("Switch Theme".to_owned(), "Ctrl-T".to_owned()),
        ]);
        let aliases = HashMap::from([
            ("Find Pane".to_owned(), vec!["jump".to_owned()]),
            ("Switch Theme".to_owned(), vec!["appearance".to_owned()]),
        ]);

        let items = apply_item_overrides(items, &shortcuts, &aliases);

        assert_eq!(items[0].shortcut.as_deref(), Some("Enter"));
        assert_eq!(items[0].aliases, vec!["pane".to_owned(), "jump".to_owned()]);
        assert_eq!(items[1].shortcut.as_deref(), Some("Ctrl-T"));
        assert_eq!(items[1].aliases, vec!["appearance".to_owned()]);
    }

    #[test]
    fn hidden_items_filter_by_title() {
        let items = vec![
            PaletteItem::leaf("Find Pane", PaletteAction::Noop),
            PaletteItem::leaf("Split Down", PaletteAction::Noop),
        ];
        let hidden = HashSet::from(["Split Down".to_owned()]);

        let filtered = filter_hidden_items(items, &hidden);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "Find Pane");
    }

    #[test]
    fn custom_palette_references_titles_then_categories() {
        let base_items = vec![
            PaletteItem::leaf("Find Pane", PaletteAction::Noop).with_category("Panes"),
            PaletteItem::leaf("Open Logs", PaletteAction::Noop).with_category("Tools"),
            PaletteItem::leaf("Switch Theme", PaletteAction::Noop).with_category("Appearance"),
        ];
        let custom_palette = CustomPalette {
            from: vec!["Find Pane".to_owned()],
            from_category: Some("Tools".to_owned()),
            ..CustomPalette::default()
        };

        let referenced = referenced_items_from_custom_palette(&base_items, &custom_palette);
        let titles: Vec<_> = referenced.into_iter().map(|item| item.title).collect();

        assert_eq!(titles, vec!["Find Pane".to_owned(), "Open Logs".to_owned()]);
    }

    fn temp_home(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("zellij-palette-{prefix}-{nanos}"));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
