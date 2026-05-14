use crate::model::{
    CommandAction, PaletteAction, PaletteId, PaletteItem, PopupCoordinates, ThemeAction,
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

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
    #[serde(rename = "iconColor")]
    icon_color: Option<String>,
    action: ConfigAction,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum ConfigAction {
    Palette { palette: String },
    Shell { shell: String },
    Popup {
        popup: String,
        x: Option<String>,
        y: Option<String>,
        width: Option<String>,
        height: Option<String>,
        pinned: Option<bool>,
        borderless: Option<bool>,
    },
    Theme { theme: String },
}

#[derive(Debug, Deserialize)]
struct RawCustomPalette {
    title: Option<String>,
    from: Option<Vec<String>>,
    #[serde(rename = "fromCategory")]
    from_category: Option<String>,
    #[serde(rename = "fromGroup")]
    from_group: Option<String>,
    command: Option<String>,
    action: Option<ConfigAction>,
    icon: Option<String>,
    #[serde(rename = "iconColor")]
    icon_color: Option<String>,
    grouped: Option<bool>,
    #[serde(rename = "emptyText")]
    empty_text: Option<String>,
    items: Option<Vec<RawPaletteItem>>,
}

pub fn load_user_config(home: Option<&Path>) -> UserConfig {
    let Some(home) = home else {
        return UserConfig::default();
    };

    let config_root = home.join(".config").join("zellij-palette");
    let commands = load_commands(&config_root.join("commands.json"));
    let custom_palettes = load_custom_palettes(&config_root.join("palettes"));
    let shortcut_overrides = load_json_file(&config_root.join("shortcuts.json")).unwrap_or_default();
    let alias_overrides = load_json_file(&config_root.join("aliases.json")).unwrap_or_default();
    let hidden_titles = load_json_file::<Vec<String>>(&config_root.join("hidden.json"))
        .unwrap_or_default()
        .into_iter()
        .collect();
    let theme_names = load_theme_names(&home.join(".config").join("zellij").join("themes"));

    UserConfig {
        commands,
        custom_palettes,
        theme_names,
        shortcut_overrides,
        alias_overrides,
        hidden_titles,
    }
}

fn load_commands(path: &Path) -> Vec<PaletteItem> {
    let Some(parsed) = load_json_file::<Vec<RawPaletteItem>>(path) else {
        return Vec::new();
    };
    parsed
        .into_iter()
        .map(raw_item_to_palette_item)
        .collect()
}

fn load_custom_palettes(path: &Path) -> HashMap<String, CustomPalette> {
    let Ok(entries) = fs::read_dir(path) else {
        return HashMap::new();
    };

    let mut palettes = HashMap::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Some(parsed) = load_json_file::<RawCustomPalette>(&path) else {
            continue;
        };
        let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        palettes.insert(
            name.to_owned(),
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
            },
        );
    }
    palettes
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
        [icon, title] => (Some((*icon).to_owned()), default_icon_color.map(str::to_owned), (*title).to_owned()),
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

pub fn filter_hidden_items(items: Vec<PaletteItem>, hidden_titles: &HashSet<String>) -> Vec<PaletteItem> {
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

fn load_json_file<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    let Ok(raw) = fs::read_to_string(path) else {
        return None;
    };
    serde_json::from_str(&raw).ok()
}

#[cfg(test)]
mod tests {
    use super::{
        ConfigAction, CustomPalette, apply_item_overrides, filter_hidden_items,
        load_user_config, parse_command_palette_output, referenced_items_from_custom_palette,
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
        fs::write(
            home.join(".config").join("zellij").join("themes").join("nord.kdl"),
            "theme nord {}",
        )
        .unwrap();

        let user_config = load_user_config(Some(&home));

        assert_eq!(user_config.commands.len(), 1);
        assert_eq!(user_config.commands[0].category.as_deref(), Some("Tools"));
        assert_eq!(user_config.commands[0].shortcut.as_deref(), Some("Cmd-L"));
        assert_eq!(user_config.commands[0].icon.as_deref(), Some("󰌱"));
        assert_eq!(user_config.commands[0].icon_color.as_deref(), Some("#22cc22"));
        assert_eq!(
            user_config.shortcut_overrides.get("Find Pane").map(String::as_str),
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
