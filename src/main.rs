use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use zellij_palette::focus::{FocusPanePlan, plan_focus_pane, should_list_find_pane_item};
use zellij_palette::fuzzy::filter_items;
use zellij_palette::kdl::escape_kdl_string;
use zellij_palette::model::{
    CommandAction, PaletteAction, PaletteId, PaletteItem, PaneTarget, ThemeAction,
};
use zellij_palette::selection::{list_offset, next_selectable, normalize_selection};
use zellij_palette::state::{PermissionState, permission_placeholder_items};
use zellij_palette::user_config::{
    UserConfig, apply_item_overrides, filter_hidden_items, load_user_config,
    parse_command_palette_output, referenced_items_from_custom_palette, with_command_cwd,
};
use zellij_tile::prelude::actions::Action;
use zellij_tile::prelude::*;

const SEARCH_ROW: usize = 1;
const LIST_START_ROW: usize = 3;
const FOOTER_ROWS: usize = 2;

// Zellij 0.44.3 bakes these themes into the server binary via
// `include_dir!("$CARGO_MANIFEST_DIR/assets/themes")` in
// zellij-utils/src/consts.rs. Keep this list in sync when bumping the
// zellij-tile dep — and remember the user can shadow any of these by
// dropping a same-named file under ~/.config/zellij/themes/.
const BUILTIN_THEMES: &[&str] = &[
    "ansi",
    "ao",
    "atelier",
    "ayu-dark",
    "ayu-light",
    "ayu-mirage",
    "blade-runner",
    "catppuccin-frappe",
    "catppuccin-latte",
    "catppuccin-macchiato",
    "catppuccin-mocha",
    "cyber-noir",
    "dayfox",
    "dracula",
    "everforest-dark",
    "everforest-light",
    "flexoki-dark",
    "gruber-darker",
    "gruvbox-dark",
    "gruvbox-light",
    "iceberg-dark",
    "iceberg-light",
    "kanagawa",
    "lucario",
    "menace",
    "molokai-dark",
    "nightfox",
    "night-owl",
    "nord",
    "onedark",
    "one-half-dark",
    "pencil-light",
    "retro-wave",
    "solarized-dark",
    "solarized-light",
    "terafox",
    "tokyo-night",
    "tokyo-night-dark",
    "tokyo-night-light",
    "tokyo-night-storm",
    "vesper",
];

#[derive(Clone, Debug, Eq, PartialEq)]
enum ActivePalette {
    BuiltIn(PaletteId),
    Custom(String),
}

#[derive(Clone, Debug)]
struct PaletteSnapshot {
    active_palette: ActivePalette,
    query: String,
    selected: usize,
}

#[derive(Default)]
struct State {
    own_plugin_id: Option<u32>,
    own_client_id: Option<ClientId>,
    query: String,
    selected: usize,
    active_palette: Option<ActivePalette>,
    stack: Vec<PaletteSnapshot>,
    sessions: Vec<SessionInfo>,
    user_config: UserConfig,
    command_results: BTreeMap<String, Vec<PaletteItem>>,
    mode_info: Option<ModeInfo>,
    home_dir: Option<PathBuf>,
    caller_cwd: Option<PathBuf>,
    root_category: Option<String>,
    user_theme_dir: Option<String>,
    permission_state: PermissionState,
    message: Option<String>,
    last_rows: usize,
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        let plugin_ids = get_plugin_ids();
        self.own_plugin_id = Some(plugin_ids.plugin_id);
        self.own_client_id = Some(plugin_ids.client_id);
        self.active_palette = Some(active_palette_from_config(
            configuration.get("palette").map(String::as_str),
        ));
        self.caller_cwd = configuration.get("caller_cwd").map(PathBuf::from);
        self.root_category = configuration.get("category").cloned();
        self.user_theme_dir = configuration
            .get("theme_dir")
            .map(String::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        self.permission_state = PermissionState::Pending;
        self.message =
            Some("Approve the Zellij permission prompt to enable palette actions".to_owned());

        subscribe(&[
            EventType::Key,
            EventType::Mouse,
            EventType::ModeUpdate,
            EventType::SessionUpdate,
            EventType::PermissionRequestResult,
            EventType::RunCommandResult,
            EventType::Visible,
        ]);

        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::RunCommands,
            PermissionType::OpenTerminalsOrPlugins,
            PermissionType::Reconfigure,
            PermissionType::FullHdAccess,
            PermissionType::ReadSessionEnvironmentVariables,
            PermissionType::RunActionsAsUser,
        ]);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::ModeUpdate(mode_info) => {
                self.mode_info = Some(mode_info);
                true
            }
            Event::SessionUpdate(session_infos, _) => {
                self.sessions = session_infos;
                self.ensure_selection();
                true
            }
            Event::PermissionRequestResult(PermissionStatus::Granted) => {
                let first_grant = self.permission_state != PermissionState::Granted;
                self.permission_state = PermissionState::Granted;
                self.message = None;
                if first_grant {
                    self.finish_startup_after_permissions();
                }
                true
            }
            Event::PermissionRequestResult(PermissionStatus::Denied) => {
                self.permission_state = PermissionState::Denied;
                self.message =
                    Some("Permissions denied; reopen the palette and allow access".to_owned());
                true
            }
            Event::RunCommandResult(exit_code, stdout, stderr, context) => {
                self.handle_run_command_result(exit_code, stdout, stderr, context);
                true
            }
            Event::Key(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            Event::Visible(true) => {
                if self.permission_state == PermissionState::Granted {
                    self.refresh_sessions();
                }
                true
            }
            _ => false,
        }
    }

    fn render(&mut self, rows: usize, cols: usize) {
        self.last_rows = rows;
        self.ensure_selection();
        self.render_header(cols);
        self.render_search(cols);
        self.render_items(rows, cols);
        self.render_footer(rows, cols);
    }
}

impl State {
    fn finish_startup_after_permissions(&mut self) {
        self.load_user_config_from_home();
        self.refresh_sessions();
        if let Some(ActivePalette::Custom(name)) = self.active_palette.clone() {
            self.maybe_request_custom_palette_data(&name);
        }
        if let Some(plugin_id) = self.own_plugin_id {
            rename_plugin_pane(plugin_id, "Palette");
        }
    }

    fn load_user_config_from_home(&mut self) {
        if self.home_dir.is_none() {
            let env_vars = get_session_environment_variables();
            self.home_dir = env_vars.get("HOME").map(PathBuf::from);
        }
        let theme_dir = self
            .user_theme_dir
            .as_deref()
            .map(|raw| expand_user_path(raw, self.home_dir.as_deref()));
        self.user_config = load_user_config(self.home_dir.as_deref(), theme_dir.as_deref());
    }

    fn refresh_sessions(&mut self) {
        if let Ok(snapshot) = get_session_list() {
            self.sessions = snapshot.live_sessions;
        }
    }

    fn handle_key(&mut self, key: KeyWithModifier) -> bool {
        match key.bare_key {
            BareKey::Char(c) if key.has_no_modifiers() => {
                self.query.push(c);
                self.selected = 0;
                true
            }
            BareKey::Backspace if key.has_no_modifiers() => {
                self.query.pop();
                self.selected = 0;
                true
            }
            BareKey::Esc if key.has_no_modifiers() => {
                self.go_back_or_close();
                true
            }
            BareKey::Char('c') if key.has_modifiers(&[KeyModifier::Ctrl]) => {
                self.go_back_or_close();
                true
            }
            BareKey::Up if key.has_no_modifiers() => {
                self.move_selection(-1);
                true
            }
            BareKey::Down if key.has_no_modifiers() => {
                self.move_selection(1);
                true
            }
            BareKey::Char('p') if key.has_modifiers(&[KeyModifier::Ctrl]) => {
                self.move_selection(-1);
                true
            }
            BareKey::Char('n') if key.has_modifiers(&[KeyModifier::Ctrl]) => {
                self.move_selection(1);
                true
            }
            BareKey::Enter if key.has_no_modifiers() => {
                self.activate_selected();
                true
            }
            _ => false,
        }
    }

    fn handle_mouse(&mut self, mouse: Mouse) -> bool {
        match mouse {
            Mouse::ScrollUp(_) => {
                self.move_selection(-1);
                true
            }
            Mouse::ScrollDown(_) => {
                self.move_selection(1);
                true
            }
            Mouse::LeftClick(line, _) => {
                self.select_line(line);
                self.activate_selected();
                true
            }
            Mouse::Hover(line, _) if line >= LIST_START_ROW as isize => {
                self.select_line(line);
                true
            }
            _ => false,
        }
    }

    fn go_back_or_close(&mut self) {
        if !self.query.is_empty() {
            self.query.clear();
            self.selected = 0;
            return;
        }
        if let Some(previous) = self.stack.pop() {
            self.active_palette = Some(previous.active_palette);
            self.query = previous.query;
            self.selected = previous.selected;
            return;
        }
        close_self();
    }

    fn move_selection(&mut self, delta: isize) {
        let visible = self.visible_items();
        self.selected = next_selectable(&visible, self.selected, delta);
    }

    fn select_line(&mut self, line: isize) {
        let visible = self.visible_items();
        if visible.is_empty() || line < LIST_START_ROW as isize || self.last_rows == 0 {
            return;
        }
        let max_rows = self.list_rows(self.last_rows);
        let start = list_offset(self.selected, visible.len(), max_rows);
        let relative_line = line as usize - LIST_START_ROW;
        let index = start + relative_line;
        if index < visible.len() && visible[index].selectable {
            self.selected = index;
        }
    }

    fn activate_selected(&mut self) {
        let visible = self.visible_items();
        let Some(item) = visible.get(self.selected).cloned() else {
            return;
        };
        self.execute_action(item.action);
    }

    fn execute_action(&mut self, action: PaletteAction) {
        match action {
            PaletteAction::Noop => {}
            PaletteAction::OpenPalette(palette_id) => {
                self.push_palette(ActivePalette::BuiltIn(palette_id));
            }
            PaletteAction::OpenCustomPalette(name) => {
                self.push_palette(ActivePalette::Custom(name));
            }
            PaletteAction::FocusPane(target) => {
                match plan_focus_pane(self.current_session_name(), &target) {
                    FocusPanePlan::CurrentSession { .. } => {
                        focus_pane_with_id(pane_id(&target), true, true);
                    }
                    FocusPanePlan::OtherSession {
                        session_name,
                        tab_position,
                        pane_id,
                        is_plugin,
                    } => {
                        switch_session_with_focus(
                            &session_name,
                            Some(tab_position),
                            Some((pane_id, is_plugin)),
                        );
                    }
                }
                close_self();
            }
            PaletteAction::MovePaneToNewTab(source) => {
                break_panes_to_new_tab(&[pane_id(&source)], None, true);
                close_self();
            }
            PaletteAction::MovePaneToTab {
                source,
                target_tab_id,
            } => {
                break_panes_to_tab_with_id(&[pane_id(&source)], target_tab_id, true);
                close_self();
            }
            PaletteAction::SplitRight => {
                self.run_split(Direction::Right);
                close_self();
            }
            PaletteAction::SplitDown => {
                self.run_split(Direction::Down);
                close_self();
            }
            PaletteAction::ToggleFocusedPaneFullscreen(target) => {
                toggle_pane_id_fullscreen(pane_id(&target));
                close_self();
            }
            PaletteAction::ToggleFocusedPaneEmbedOrFloat(target) => {
                toggle_pane_embed_or_eject_for_pane_id(pane_id(&target));
                close_self();
            }
            PaletteAction::ClosePane(target) => {
                close_pane_with_id(pane_id(&target));
                close_self();
            }
            PaletteAction::NewTab { cwd } => {
                let cwd_str = cwd.as_ref().map(|path| path.display().to_string());
                new_tab::<String>(None, cwd_str);
                close_self();
            }
            PaletteAction::NextTab => {
                go_to_next_tab();
                close_self();
            }
            PaletteAction::PreviousTab => {
                go_to_previous_tab();
                close_self();
            }
            PaletteAction::CloseTab { tab_id } => {
                close_tab_with_id(tab_id as u64);
                close_self();
            }
            PaletteAction::SwitchSession { session_name } => {
                switch_session(Some(&session_name));
                close_self();
            }
            PaletteAction::Detach => {
                detach();
                close_self();
            }
            PaletteAction::Theme(theme_action) => {
                self.apply_theme(theme_action);
                close_self();
            }
            PaletteAction::RunShell(command) => {
                self.run_shell(command, false, None);
                close_self();
            }
            PaletteAction::OpenCommandPane {
                command,
                coordinates,
                floating,
            } => {
                self.run_shell(command, floating, coordinates);
                close_self();
            }
        }
    }

    fn run_split(&self, direction: Direction) {
        let tab_id = self.current_tab_id();
        run_action(
            Action::NewTiledPane {
                direction: Some(direction),
                command: None,
                pane_name: None,
                near_current_pane: false,
                borderless: None,
                tab_id,
            },
            BTreeMap::new(),
        );
    }

    fn apply_theme(&self, action: ThemeAction) {
        match action {
            ThemeAction::Toggle => run_action(Action::ToggleTheme, BTreeMap::new()),
            ThemeAction::SetDark => run_action(Action::SetDarkTheme, BTreeMap::new()),
            ThemeAction::SetLight => run_action(Action::SetLightTheme, BTreeMap::new()),
            ThemeAction::SetNamed(name) => {
                reconfigure(format!("theme \"{}\"", escape_kdl_string(&name)), false)
            }
        }
    }

    fn run_shell(
        &self,
        command: CommandAction,
        floating: bool,
        coordinates: Option<zellij_palette::model::PopupCoordinates>,
    ) {
        let mut shell = CommandToRun::new_with_args("sh", vec!["-lc", command.command.as_str()]);
        shell.cwd = command.cwd.clone();
        if floating {
            open_command_pane_floating(shell, floating_coordinates(coordinates), BTreeMap::new());
        } else {
            run_command_with_env_variables_and_cwd(
                &["sh", "-lc", &command.command],
                BTreeMap::new(),
                command.cwd.unwrap_or_else(|| PathBuf::from(".")),
                BTreeMap::new(),
            );
        }
    }

    fn push_palette(&mut self, active_palette: ActivePalette) {
        let previous = PaletteSnapshot {
            active_palette: self
                .active_palette
                .clone()
                .unwrap_or(ActivePalette::BuiltIn(PaletteId::Commands)),
            query: self.query.clone(),
            selected: self.selected,
        };
        self.stack.push(previous);
        self.active_palette = Some(active_palette.clone());
        self.query.clear();
        self.selected = 0;
        if let ActivePalette::Custom(name) = active_palette {
            self.maybe_request_custom_palette_data(&name);
        }
    }

    fn maybe_request_custom_palette_data(&self, palette_name: &str) {
        let Some(custom_palette) = self.user_config.custom_palettes.get(palette_name) else {
            return;
        };
        let Some(command) = &custom_palette.command else {
            return;
        };
        let mut context = BTreeMap::new();
        context.insert("kind".to_owned(), "custom_palette".to_owned());
        context.insert("palette".to_owned(), palette_name.to_owned());
        run_command(&["sh", "-lc", command], context);
    }

    fn handle_run_command_result(
        &mut self,
        exit_code: Option<i32>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        context: BTreeMap<String, String>,
    ) {
        if context.get("kind").map(String::as_str) != Some("custom_palette") {
            return;
        }
        let Some(palette_name) = context.get("palette") else {
            return;
        };
        let Some(custom_palette) = self.user_config.custom_palettes.get(palette_name) else {
            return;
        };

        if exit_code == Some(0) {
            let output = String::from_utf8_lossy(&stdout);
            let items = parse_command_palette_output(
                &output,
                custom_palette.template_action.as_ref(),
                custom_palette.default_icon.as_deref(),
                custom_palette.default_icon_color.as_deref(),
            );
            self.command_results.insert(palette_name.to_owned(), items);
            self.message = None;
        } else {
            let stderr = String::from_utf8_lossy(&stderr);
            self.message = Some(format!(
                "Palette command failed: {}",
                stderr.lines().next().unwrap_or("unknown error")
            ));
        }
    }

    fn render_header(&self, cols: usize) {
        let title = format!(" Zellij Palette · {} ", self.palette_title());
        let header = Text::new(truncate_line(&title, cols)).color_all(2);
        print_text_with_coordinates(header, 0, 0, Some(cols), None);
    }

    fn render_search(&self, cols: usize) {
        let line = format!("> {}_", self.query);
        let search = if self.query.is_empty() {
            Text::new(truncate_line("> _", cols))
        } else {
            Text::new(truncate_line(&line, cols))
        };
        print_text_with_coordinates(search, 0, SEARCH_ROW, Some(cols), None);
    }

    fn render_items(&mut self, rows: usize, cols: usize) {
        let visible = self.visible_items();
        if visible.is_empty() {
            let empty = Text::new(truncate_line(&self.empty_state_text(), cols)).color_all(1);
            print_text_with_coordinates(empty, 0, LIST_START_ROW, Some(cols), None);
            return;
        }

        self.ensure_selection();
        let list_rows = self.list_rows(rows);
        let offset = list_offset(self.selected, visible.len(), list_rows);
        let end = (offset + list_rows).min(visible.len());
        for (line, item) in visible[offset..end].iter().enumerate() {
            let row = LIST_START_ROW + line;
            if !item.selectable {
                let text = Text::new(truncate_line(&format!("{}:", item.title), cols)).color_all(2);
                print_text_with_coordinates(text, 0, row, Some(cols), None);
                continue;
            }

            let text = item_text(item, cols, offset + line == self.selected);
            print_text_with_coordinates(text, 0, row, Some(cols), None);
        }
    }

    fn render_footer(&self, rows: usize, cols: usize) {
        let help = Text::new("Enter select  Esc back  Ctrl-C close  Up/Down move")
            .color_range(2, 0..5)
            .color_range(2, 14..17)
            .color_range(2, 24..30)
            .color_range(2, 32..39);
        print_text_with_coordinates(help, 0, rows.saturating_sub(2), Some(cols), None);
        if let Some(message) = &self.message {
            let text = Text::new(truncate_line(message, cols)).color_all(1);
            print_text_with_coordinates(text, 0, rows.saturating_sub(1), Some(cols), None);
        } else {
            let path = match self.active_palette {
                Some(ActivePalette::Custom(ref name)) => format!("Custom palette · {name}"),
                _ => self.palette_title(),
            };
            let text = Text::new(truncate_line(&path, cols)).color_all(2);
            print_text_with_coordinates(text, 0, rows.saturating_sub(1), Some(cols), None);
        }
    }

    fn empty_state_text(&self) -> String {
        match self.active_palette.as_ref() {
            Some(ActivePalette::Custom(name)) => self
                .user_config
                .custom_palettes
                .get(name)
                .and_then(|palette| palette.empty_text.clone())
                .unwrap_or_else(|| "No results".to_owned()),
            _ => "No results".to_owned(),
        }
    }

    fn visible_items(&self) -> Vec<PaletteItem> {
        let items = self.palette_items();
        if self.query.trim().is_empty() {
            return items;
        }
        let selectable: Vec<_> = items.into_iter().filter(|item| item.selectable).collect();
        filter_items(&selectable, &self.query)
    }

    fn palette_items(&self) -> Vec<PaletteItem> {
        if self.permission_state != PermissionState::Granted {
            return self.permission_placeholder_items();
        }
        let items = match self
            .active_palette
            .clone()
            .unwrap_or(ActivePalette::BuiltIn(PaletteId::Commands))
        {
            ActivePalette::BuiltIn(PaletteId::Commands) => self.command_palette_items(),
            ActivePalette::BuiltIn(PaletteId::FindPane) => self.find_pane_items(),
            ActivePalette::BuiltIn(PaletteId::Sessions) => self.session_items(),
            ActivePalette::BuiltIn(PaletteId::MovePane) => self.move_pane_items(),
            ActivePalette::BuiltIn(PaletteId::Themes) => self.theme_items(),
            ActivePalette::BuiltIn(PaletteId::Custom) => Vec::new(),
            ActivePalette::Custom(name) => self.custom_palette_items(&name),
        };
        apply_item_overrides(
            items,
            &self.user_config.shortcut_overrides,
            &self.user_config.alias_overrides,
        )
    }

    fn permission_placeholder_items(&self) -> Vec<PaletteItem> {
        permission_placeholder_items(self.permission_state)
    }

    fn command_palette_items(&self) -> Vec<PaletteItem> {
        let items = filter_hidden_items(
            self.command_palette_base_items(),
            &self.user_config.hidden_titles,
        );
        if let Some(category) = &self.root_category {
            let mut filtered = vec![PaletteItem::group(category.clone())];
            filtered.extend(
                items
                    .into_iter()
                    .filter(|item| item.category.as_ref() == Some(category)),
            );
            return filtered;
        }
        items
    }

    fn command_palette_base_items(&self) -> Vec<PaletteItem> {
        let caller = self.caller_pane_target();
        let caller_cwd = self.caller_cwd.clone();
        let active_tab_id = self.current_tab_id().unwrap_or_default();
        let mut items = vec![
            PaletteItem::group("Panes"),
            PaletteItem::leaf("Find Pane", PaletteAction::OpenPalette(PaletteId::FindPane))
                .with_icon("󰍉")
                .with_category("Panes")
                .with_description("jump across sessions, tabs, and panes")
                .with_aliases(["pane", "jump"]),
            PaletteItem::leaf("Split Right", PaletteAction::SplitRight)
                .with_icon("")
                .with_category("Panes")
                .with_description("open a pane to the right")
                .with_aliases(["split horizontal", "sh"]),
            PaletteItem::leaf("Split Down", PaletteAction::SplitDown)
                .with_icon("")
                .with_category("Panes")
                .with_description("open a pane below")
                .with_aliases(["split vertical", "sv"]),
            PaletteItem::group("Tabs"),
            PaletteItem::leaf(
                "New Tab",
                PaletteAction::NewTab {
                    cwd: caller_cwd.clone(),
                },
            )
            .with_icon("󰝰")
            .with_category("Tabs")
            .with_description("new tab in the caller cwd")
            .with_aliases(["tab", "window"]),
            PaletteItem::leaf("Next Tab", PaletteAction::NextTab)
                .with_icon("󰁔")
                .with_category("Tabs"),
            PaletteItem::leaf("Previous Tab", PaletteAction::PreviousTab)
                .with_icon("󰁍")
                .with_category("Tabs"),
            PaletteItem::leaf(
                "Close Tab",
                PaletteAction::CloseTab {
                    tab_id: active_tab_id,
                },
            )
            .with_icon("󰅖")
            .with_category("Tabs"),
            PaletteItem::group("Sessions"),
            PaletteItem::leaf(
                "Switch Session...",
                PaletteAction::OpenPalette(PaletteId::Sessions),
            )
            .with_icon("󱂬")
            .with_category("Sessions")
            .with_aliases(["session"]),
            PaletteItem::leaf("Detach Session", PaletteAction::Detach)
                .with_icon("󰍃")
                .with_category("Sessions"),
            PaletteItem::group("Appearance"),
            PaletteItem::leaf(
                "Switch Theme...",
                PaletteAction::OpenPalette(PaletteId::Themes),
            )
            .with_icon("")
            .with_category("Appearance")
            .with_aliases(["theme"]),
        ];

        if let Some(target) = caller {
            items.insert(
                4,
                PaletteItem::leaf(
                    "Move Pane to...",
                    PaletteAction::OpenPalette(PaletteId::MovePane),
                )
                .with_icon("󰁁")
                .with_category("Panes")
                .with_aliases(["move", "join", "break"]),
            );
            items.insert(
                4,
                PaletteItem::leaf("Close Pane", PaletteAction::ClosePane(target.clone()))
                    .with_icon("󰅖")
                    .with_category("Panes"),
            );
            items.insert(
                4,
                PaletteItem::leaf(
                    "Float / Embed Pane",
                    PaletteAction::ToggleFocusedPaneEmbedOrFloat(target.clone()),
                )
                .with_icon("◫")
                .with_category("Panes"),
            );
            items.insert(
                4,
                PaletteItem::leaf(
                    "Toggle Fullscreen",
                    PaletteAction::ToggleFocusedPaneFullscreen(target),
                )
                .with_icon("󰍉")
                .with_category("Panes"),
            );
        }

        if !self.user_config.commands.is_empty() {
            items.push(PaletteItem::group("Custom"));
            for item in &self.user_config.commands {
                items.push(apply_default_cwd(item.clone(), caller_cwd.clone()));
            }
        }

        items
    }

    fn find_pane_items(&self) -> Vec<PaletteItem> {
        let mut items = Vec::new();
        let own_plugin_id = self.own_plugin_id;
        for session in &self.sessions {
            items.push(PaletteItem::group(if session.is_current_session {
                format!("{} (current)", session.name)
            } else {
                session.name.clone()
            }));
            for tab in &session.tabs {
                if let Some(panes) = session.panes.panes.get(&tab.position) {
                    for pane in panes.iter().filter(|pane| {
                        should_list_find_pane_item(
                            pane.id,
                            pane.is_plugin,
                            pane.is_selectable,
                            pane.is_suppressed,
                            own_plugin_id,
                        )
                    }) {
                        let aliases = [
                            session.name.as_str(),
                            tab.name.as_str(),
                            pane.title.as_str(),
                            pane.terminal_command.as_deref().unwrap_or(""),
                        ];
                        let mut item = PaletteItem::leaf(
                            format!("[{}] {}", tab.name, pane.title),
                            PaletteAction::FocusPane(PaneTarget {
                                session_name: session.name.clone(),
                                tab_position: tab.position,
                                tab_id: tab.tab_id,
                                pane_id: pane.id,
                                is_plugin: pane.is_plugin,
                            }),
                        )
                        .with_icon(if pane.is_plugin { "" } else { "󰆍" })
                        .with_category("Panes")
                        .with_description(description_for_pane(session, tab, pane));
                        item.aliases = aliases
                            .into_iter()
                            .filter(|value| !value.is_empty())
                            .map(str::to_owned)
                            .collect();
                        items.push(item);
                    }
                }
            }
        }
        items
    }

    fn session_items(&self) -> Vec<PaletteItem> {
        let mut items = vec![PaletteItem::group("Sessions")];
        for session in &self.sessions {
            let mut item = PaletteItem::leaf(
                session.name.clone(),
                PaletteAction::SwitchSession {
                    session_name: session.name.clone(),
                },
            )
            .with_icon("󱂬")
            .with_category("Sessions");
            item.description = Some(format!(
                "{} tabs · {} clients",
                session.tabs.len(),
                session.connected_clients
            ));
            items.push(item);
        }
        items
    }

    fn move_pane_items(&self) -> Vec<PaletteItem> {
        let Some(source) = self.caller_pane_target() else {
            return vec![PaletteItem::group("No pane to move")];
        };
        let Some(current_session) = self
            .sessions
            .iter()
            .find(|session| session.is_current_session)
        else {
            return vec![PaletteItem::group("No active session")];
        };

        let mut items = vec![
            PaletteItem::group("Targets"),
            PaletteItem::leaf("New Tab", PaletteAction::MovePaneToNewTab(source.clone()))
                .with_icon("󰝰")
                .with_category("Tabs"),
        ];
        for tab in &current_session.tabs {
            if tab.tab_id == source.tab_id {
                continue;
            }
            items.push(
                PaletteItem::leaf(
                    tab.name.clone(),
                    PaletteAction::MovePaneToTab {
                        source: source.clone(),
                        target_tab_id: tab.tab_id,
                    },
                )
                .with_icon("󰖲")
                .with_category("Tabs")
                .with_description(format!("tab {}", tab.position + 1)),
            );
        }
        items
    }

    fn theme_items(&self) -> Vec<PaletteItem> {
        let mut items = vec![
            PaletteItem::group("Theme Mode"),
            PaletteItem::leaf(
                "Toggle Dark / Light",
                PaletteAction::Theme(ThemeAction::Toggle),
            )
            .with_icon("◐")
            .with_category("Appearance"),
            PaletteItem::leaf("Use Dark Theme", PaletteAction::Theme(ThemeAction::SetDark))
                .with_icon("●")
                .with_category("Appearance"),
            PaletteItem::leaf(
                "Use Light Theme",
                PaletteAction::Theme(ThemeAction::SetLight),
            )
            .with_icon("○")
            .with_category("Appearance"),
        ];

        // A user-supplied theme file shadows the built-in of the same
        // name (Zellij merges user themes on top of the embedded set),
        // so hide the built-in entry in that case.
        let user_themes: std::collections::HashSet<&str> = self
            .user_config
            .theme_names
            .iter()
            .map(String::as_str)
            .collect();

        items.push(PaletteItem::group("Built-in Themes"));
        for name in BUILTIN_THEMES {
            if user_themes.contains(name) {
                continue;
            }
            items.push(
                PaletteItem::leaf(
                    (*name).to_owned(),
                    PaletteAction::Theme(ThemeAction::SetNamed((*name).to_owned())),
                )
                .with_icon("●")
                .with_category("Appearance"),
            );
        }

        if !self.user_config.theme_names.is_empty() {
            items.push(PaletteItem::group("User Themes"));
            for theme_name in &self.user_config.theme_names {
                items.push(
                    PaletteItem::leaf(
                        theme_name.clone(),
                        PaletteAction::Theme(ThemeAction::SetNamed(theme_name.clone())),
                    )
                    .with_icon("●")
                    .with_category("Appearance"),
                );
            }
        }

        items
    }

    fn custom_palette_items(&self, name: &str) -> Vec<PaletteItem> {
        let Some(custom_palette) = self.user_config.custom_palettes.get(name) else {
            return vec![PaletteItem::group("Unknown custom palette")];
        };

        let base_commands = self.command_palette_base_items();
        let caller_cwd = self.caller_cwd.clone();

        let mut items = Vec::new();
        if !custom_palette.from.is_empty() || custom_palette.from_category.is_some() {
            items.push(PaletteItem::group(
                custom_palette
                    .title
                    .clone()
                    .unwrap_or_else(|| name.to_owned()),
            ));
        }

        for item in referenced_items_from_custom_palette(&base_commands, custom_palette) {
            items.push(apply_default_cwd(item, caller_cwd.clone()));
        }
        for item in &custom_palette.items {
            items.push(apply_default_cwd(item.clone(), caller_cwd.clone()));
        }
        if let Some(command_items) = self.command_results.get(name) {
            for item in command_items {
                items.push(apply_default_cwd(item.clone(), caller_cwd.clone()));
            }
        }
        if items.is_empty() {
            if custom_palette.command.is_some() {
                return vec![PaletteItem::group("Loading custom palette...")];
            }
            return vec![PaletteItem::group("Empty custom palette")];
        }
        if custom_palette.grouped.unwrap_or(false) {
            group_items_by_category(items)
        } else {
            items
        }
    }

    fn ensure_selection(&mut self) {
        let visible = self.visible_items();
        self.selected = normalize_selection(&visible, self.selected);
    }

    fn list_rows(&self, rows: usize) -> usize {
        rows.saturating_sub(LIST_START_ROW + FOOTER_ROWS).max(1)
    }

    fn palette_title(&self) -> String {
        match self
            .active_palette
            .clone()
            .unwrap_or(ActivePalette::BuiltIn(PaletteId::Commands))
        {
            ActivePalette::BuiltIn(PaletteId::Commands) => self
                .root_category
                .clone()
                .unwrap_or_else(|| "Commands".to_owned()),
            ActivePalette::BuiltIn(PaletteId::FindPane) => "Find Pane".to_owned(),
            ActivePalette::BuiltIn(PaletteId::Sessions) => "Sessions".to_owned(),
            ActivePalette::BuiltIn(PaletteId::MovePane) => "Move Pane".to_owned(),
            ActivePalette::BuiltIn(PaletteId::Themes) => "Themes".to_owned(),
            ActivePalette::BuiltIn(PaletteId::Custom) => "Custom".to_owned(),
            ActivePalette::Custom(name) => name,
        }
    }

    fn current_tab_id(&self) -> Option<usize> {
        self.sessions
            .iter()
            .find(|session| session.is_current_session)
            .and_then(|session| {
                session
                    .tabs
                    .iter()
                    .find(|tab| tab.active)
                    .map(|tab| tab.tab_id)
            })
    }

    fn current_session_name(&self) -> Option<&str> {
        self.sessions
            .iter()
            .find(|session| session.is_current_session)
            .map(|session| session.name.as_str())
    }

    fn caller_pane_target(&self) -> Option<PaneTarget> {
        let client_id = self.own_client_id?;
        let own_plugin_id = self.own_plugin_id?;
        let current_session = self
            .sessions
            .iter()
            .find(|session| session.is_current_session)?;
        let history = current_session.pane_history.get(&client_id)?;
        let target_pane = history
            .iter()
            .rev()
            .find(|pane_id| !matches!(pane_id, PaneId::Plugin(id) if *id == own_plugin_id))
            .cloned()?;

        for tab in &current_session.tabs {
            if let Some(panes) = current_session.panes.panes.get(&tab.position) {
                for pane in panes {
                    let matches = match target_pane {
                        PaneId::Terminal(id) => !pane.is_plugin && pane.id == id,
                        PaneId::Plugin(id) => pane.is_plugin && pane.id == id,
                    };
                    if matches {
                        return Some(PaneTarget {
                            session_name: current_session.name.clone(),
                            tab_position: tab.position,
                            tab_id: tab.tab_id,
                            pane_id: pane.id,
                            is_plugin: pane.is_plugin,
                        });
                    }
                }
            }
        }
        None
    }
}

fn group_items_by_category(items: Vec<PaletteItem>) -> Vec<PaletteItem> {
    let mut grouped = Vec::new();
    let mut last_category: Option<String> = None;

    for item in items {
        if !item.selectable {
            last_category = None;
            grouped.push(item);
            continue;
        }
        if let Some(category) = &item.category {
            if last_category.as_deref() != Some(category.as_str()) {
                grouped.push(PaletteItem::group(category.clone()));
                last_category = Some(category.clone());
            }
        }
        grouped.push(item);
    }

    grouped
}

fn floating_coordinates(
    coordinates: Option<zellij_palette::model::PopupCoordinates>,
) -> Option<FloatingPaneCoordinates> {
    let coordinates = coordinates?;
    FloatingPaneCoordinates::new(
        coordinates.x,
        coordinates.y,
        coordinates.width,
        coordinates.height,
        coordinates.pinned,
        coordinates.borderless,
    )
}

fn active_palette_from_config(palette: Option<&str>) -> ActivePalette {
    match palette.unwrap_or("commands") {
        "commands" => ActivePalette::BuiltIn(PaletteId::Commands),
        "find-pane" => ActivePalette::BuiltIn(PaletteId::FindPane),
        "sessions" => ActivePalette::BuiltIn(PaletteId::Sessions),
        "move-pane" => ActivePalette::BuiltIn(PaletteId::MovePane),
        "themes" => ActivePalette::BuiltIn(PaletteId::Themes),
        custom => ActivePalette::Custom(custom.to_owned()),
    }
}

// The plugin lives in a wasm sandbox, so `~` is not expanded by the
// shell or the host. Accept `~` and `~/...` against the HOME we got
// from `get_session_environment_variables()`; pass anything else
// through unchanged.
fn expand_user_path(raw: &str, home: Option<&Path>) -> PathBuf {
    if raw == "~" {
        return home
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(raw));
    }
    if let (Some(rest), Some(home)) = (raw.strip_prefix("~/"), home) {
        return home.join(rest);
    }
    PathBuf::from(raw)
}

fn pane_id(target: &PaneTarget) -> PaneId {
    if target.is_plugin {
        PaneId::Plugin(target.pane_id)
    } else {
        PaneId::Terminal(target.pane_id)
    }
}

fn description_for_pane(session: &SessionInfo, tab: &TabInfo, pane: &PaneInfo) -> String {
    let kind = if pane.is_plugin { "plugin" } else { "terminal" };
    let marker = if session.is_current_session && tab.active && pane.is_focused {
        "current"
    } else {
        kind
    };
    let command = pane
        .terminal_command
        .clone()
        .unwrap_or_else(|| kind.to_owned());
    format!("{marker} · {command}")
}

#[derive(Clone, Copy)]
enum LineStyle {
    Accent,
    Alias,
    Muted,
    Success,
}

struct StyledRange {
    start: usize,
    end: usize,
    style: LineStyle,
}

struct RenderedLine {
    text: String,
    ranges: Vec<StyledRange>,
}

fn item_text(item: &PaletteItem, cols: usize, is_selected: bool) -> Text {
    let rendered = item_line(item, cols);
    let mut text = Text::new(rendered.text);
    for range in rendered.ranges {
        text = match range.style {
            LineStyle::Accent => text.color_range(2, range.start..range.end),
            LineStyle::Alias => text.color_range(3, range.start..range.end),
            LineStyle::Muted => text.dim_range(range.start..range.end),
            LineStyle::Success => text.success_color_range(range.start..range.end),
        };
    }
    if is_selected { text.selected() } else { text }
}

fn item_line(item: &PaletteItem, cols: usize) -> RenderedLine {
    let shortcut = item.shortcut.as_deref().unwrap_or("");
    let shortcut_width = shortcut.width();
    let reserved_gap = usize::from(!shortcut.is_empty());
    let max_left_width = cols.saturating_sub(shortcut_width + reserved_gap);

    let mut text = String::new();
    let mut ranges = Vec::new();
    let mut left_width = 0usize;
    let mut char_len = 0usize;

    let mut segments: Vec<(String, Option<LineStyle>)> = Vec::new();
    if let Some(icon) = &item.icon {
        let style = if item.icon_color.is_some() {
            LineStyle::Success
        } else {
            LineStyle::Accent
        };
        segments.push((icon.clone(), Some(style)));
        segments.push(("  ".to_owned(), None));
    }
    segments.push((item.title.clone(), None));
    if let Some(alias) = item.aliases.first() {
        segments.push((format!("  [{alias}]"), Some(LineStyle::Alias)));
    }
    if let Some(description) = &item.description {
        segments.push((format!("  {description}"), Some(LineStyle::Muted)));
    }

    for (segment, style) in segments {
        let remaining = max_left_width.saturating_sub(left_width);
        if remaining == 0 {
            break;
        }
        let clipped = truncate_fragment(&segment, remaining);
        if clipped.is_empty() {
            break;
        }
        let segment_width = clipped.width();
        let segment_chars = clipped.chars().count();
        let start = char_len;
        text.push_str(&clipped);
        char_len += segment_chars;
        left_width += segment_width;
        if let Some(style) = style {
            ranges.push(StyledRange {
                start,
                end: char_len,
                style,
            });
        }
        if segment_width < segment.width() {
            break;
        }
    }

    if !shortcut.is_empty() && cols > 0 {
        let gap = if text.is_empty() {
            cols.saturating_sub(shortcut_width)
        } else {
            cols.saturating_sub(text.width() + shortcut_width).max(1)
        };
        text.push_str(&" ".repeat(gap));
        let start = char_len + gap;
        text.push_str(shortcut);
        char_len = start + shortcut.chars().count();
        ranges.push(StyledRange {
            start,
            end: char_len,
            style: LineStyle::Accent,
        });
    }

    if text.width() < cols {
        text.push_str(&" ".repeat(cols - text.width()));
    }

    RenderedLine { text, ranges }
}

fn truncate_line(line: &str, cols: usize) -> String {
    if line.width() <= cols {
        return line.to_owned();
    }
    let mut truncated = String::new();
    for ch in line.chars() {
        if truncated.width() + ch.width().unwrap_or(0) + 1 >= cols {
            break;
        }
        truncated.push(ch);
    }
    truncated.push('…');
    truncated
}

fn truncate_fragment(fragment: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if fragment.width() <= max_width {
        return fragment.to_owned();
    }
    if max_width == 1 {
        return "…".to_owned();
    }

    let mut truncated = String::new();
    for ch in fragment.chars() {
        if truncated.width() + ch.width().unwrap_or(0) + 1 > max_width {
            break;
        }
        truncated.push(ch);
    }
    truncated.push('…');
    truncated
}

fn apply_default_cwd(item: PaletteItem, cwd: Option<PathBuf>) -> PaletteItem {
    let mut next = item;
    next.action = with_command_cwd(next.action, cwd);
    next
}
