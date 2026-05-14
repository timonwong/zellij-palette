use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaletteId {
    Commands,
    FindPane,
    Sessions,
    MovePane,
    Themes,
    Custom,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ThemeAction {
    Toggle,
    SetDark,
    SetLight,
    SetNamed(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneTarget {
    pub session_name: String,
    pub tab_position: usize,
    pub tab_id: usize,
    pub pane_id: u32,
    pub is_plugin: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandAction {
    pub command: String,
    pub cwd: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PopupCoordinates {
    pub x: Option<String>,
    pub y: Option<String>,
    pub width: Option<String>,
    pub height: Option<String>,
    pub pinned: Option<bool>,
    pub borderless: Option<bool>,
}

impl PopupCoordinates {
    pub fn new(
        x: Option<String>,
        y: Option<String>,
        width: Option<String>,
        height: Option<String>,
        pinned: Option<bool>,
        borderless: Option<bool>,
    ) -> Option<Self> {
        if x.is_none()
            && y.is_none()
            && width.is_none()
            && height.is_none()
            && pinned.is_none()
            && borderless.is_none()
        {
            None
        } else {
            Some(Self {
                x,
                y,
                width,
                height,
                pinned,
                borderless,
            })
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PaletteAction {
    Noop,
    OpenPalette(PaletteId),
    OpenCustomPalette(String),
    FocusPane(PaneTarget),
    MovePaneToNewTab(PaneTarget),
    MovePaneToTab {
        source: PaneTarget,
        target_tab_id: usize,
    },
    SplitRight,
    SplitDown,
    ToggleFocusedPaneFullscreen(PaneTarget),
    ToggleFocusedPaneEmbedOrFloat(PaneTarget),
    ClosePane(PaneTarget),
    NewTab {
        cwd: Option<PathBuf>,
    },
    NextTab,
    PreviousTab,
    CloseTab {
        tab_id: usize,
    },
    SwitchSession {
        session_name: String,
    },
    Detach,
    Theme(ThemeAction),
    RunShell(CommandAction),
    OpenCommandPane {
        command: CommandAction,
        coordinates: Option<PopupCoordinates>,
        floating: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaletteItem {
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub aliases: Vec<String>,
    pub shortcut: Option<String>,
    pub icon: Option<String>,
    pub icon_color: Option<String>,
    pub tree_prefix: Option<String>,
    pub selectable: bool,
    pub action: PaletteAction,
}

impl PaletteItem {
    pub fn leaf(title: impl Into<String>, action: PaletteAction) -> Self {
        Self {
            title: title.into(),
            description: None,
            category: None,
            aliases: Vec::new(),
            shortcut: None,
            icon: None,
            icon_color: None,
            tree_prefix: None,
            selectable: true,
            action,
        }
    }

    pub fn group(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: None,
            category: None,
            aliases: Vec::new(),
            shortcut: None,
            icon: None,
            icon_color: None,
            tree_prefix: None,
            selectable: false,
            action: PaletteAction::Noop,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_aliases<const N: usize>(mut self, aliases: [&str; N]) -> Self {
        self.aliases = aliases.into_iter().map(str::to_owned).collect();
        self
    }

    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn with_icon_color(mut self, icon_color: impl Into<String>) -> Self {
        self.icon_color = Some(icon_color.into());
        self
    }

    pub fn with_tree_prefix(mut self, tree_prefix: impl Into<String>) -> Self {
        self.tree_prefix = Some(tree_prefix.into());
        self
    }
}
