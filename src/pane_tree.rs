use crate::fuzzy::score_item;
use crate::model::{PaletteAction, PaletteItem, PaneTarget};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneRow {
    pub id: u32,
    pub tab_position: usize,
    pub tab_id: usize,
    pub title: String,
    pub is_plugin: bool,
    pub is_focused: bool,
    pub terminal_command: Option<String>,
    pub description: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabGroup {
    pub name: String,
    pub is_active: bool,
    pub panes: Vec<PaneRow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionGroup {
    pub name: String,
    pub is_current: bool,
    pub tabs: Vec<TabGroup>,
}

pub fn filter(tree: Vec<SessionGroup>, query: &str) -> Vec<SessionGroup> {
    if query.split_whitespace().next().is_none() {
        return tree;
    }
    tree.into_iter()
        .filter_map(|session| {
            let tabs: Vec<TabGroup> = session
                .tabs
                .into_iter()
                .filter_map(|tab| {
                    let panes: Vec<PaneRow> = tab
                        .panes
                        .into_iter()
                        .filter(|pane| pane_matches(&session.name, &tab.name, pane, query))
                        .collect();
                    if panes.is_empty() {
                        None
                    } else {
                        Some(TabGroup {
                            name: tab.name,
                            is_active: tab.is_active,
                            panes,
                        })
                    }
                })
                .collect();
            if tabs.is_empty() {
                None
            } else {
                Some(SessionGroup {
                    name: session.name,
                    is_current: session.is_current,
                    tabs,
                })
            }
        })
        .collect()
}

fn pane_matches(session_name: &str, tab_name: &str, pane: &PaneRow, query: &str) -> bool {
    score_item(&synthetic_item(session_name, tab_name, pane), query).is_some()
}

fn synthetic_item(session_name: &str, tab_name: &str, pane: &PaneRow) -> PaletteItem {
    let mut item = PaletteItem::leaf(pane.title.clone(), PaletteAction::Noop);
    item.description = Some(pane.description.clone());
    let raw_aliases = [
        session_name,
        tab_name,
        pane.title.as_str(),
        pane.terminal_command.as_deref().unwrap_or(""),
    ];
    item.aliases = raw_aliases
        .iter()
        .filter(|alias| !alias.is_empty())
        .map(|alias| (*alias).to_owned())
        .collect();
    item
}

pub fn flatten(tree: &[SessionGroup]) -> Vec<PaletteItem> {
    let mut out = Vec::new();
    for session in tree {
        let header_title = if session.is_current {
            format!("{} (current)", session.name)
        } else {
            session.name.clone()
        };
        out.push(PaletteItem::group(header_title));

        let tab_count = session.tabs.len();
        for (ti, tab) in session.tabs.iter().enumerate() {
            let is_last_tab = ti + 1 == tab_count;
            let tab_glyph = if is_last_tab { "└─ " } else { "├─ " };
            let tab_cont = if is_last_tab { "    " } else { "│   " };

            if tab.panes.len() == 1 {
                let pane = &tab.panes[0];
                out.push(pane_leaf(pane, session, tab, tab_glyph));
            } else {
                let mut header = PaletteItem::group(tab.name.clone());
                header.tree_prefix = Some(tab_glyph.to_owned());
                out.push(header);

                let pane_count = tab.panes.len();
                for (pi, pane) in tab.panes.iter().enumerate() {
                    let is_last_pane = pi + 1 == pane_count;
                    let pane_glyph = if is_last_pane { "└─ " } else { "├─ " };
                    let prefix = format!("{tab_cont}{pane_glyph}");
                    out.push(pane_leaf(pane, session, tab, &prefix));
                }
            }
        }
    }
    out
}

fn pane_leaf(pane: &PaneRow, session: &SessionGroup, tab: &TabGroup, prefix: &str) -> PaletteItem {
    let marker = if session.is_current && tab.is_active && pane.is_focused {
        "▶"
    } else if pane.is_focused {
        "●"
    } else {
        "○"
    };
    let title = format!("{marker} {}", pane.title);
    let mut item = PaletteItem::leaf(
        title,
        PaletteAction::FocusPane(PaneTarget {
            session_name: session.name.clone(),
            tab_position: pane.tab_position,
            tab_id: pane.tab_id,
            pane_id: pane.id,
            is_plugin: pane.is_plugin,
        }),
    )
    .with_category("Panes")
    .with_description(pane.description.clone())
    .with_tree_prefix(prefix);
    let raw_aliases = [
        session.name.as_str(),
        tab.name.as_str(),
        pane.title.as_str(),
        pane.terminal_command.as_deref().unwrap_or(""),
    ];
    item.aliases = raw_aliases
        .iter()
        .filter(|alias| !alias.is_empty())
        .map(|alias| (*alias).to_owned())
        .collect();
    item
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane(id: u32, title: &str, is_focused: bool) -> PaneRow {
        PaneRow {
            id,
            tab_position: 0,
            tab_id: 0,
            title: title.to_owned(),
            is_plugin: false,
            is_focused,
            terminal_command: None,
            description: format!("terminal · {title}"),
        }
    }

    fn tab(name: &str, is_active: bool, panes: Vec<PaneRow>) -> TabGroup {
        TabGroup {
            name: name.to_owned(),
            is_active,
            panes,
        }
    }

    fn session(name: &str, is_current: bool, tabs: Vec<TabGroup>) -> SessionGroup {
        SessionGroup {
            name: name.to_owned(),
            is_current,
            tabs,
        }
    }

    fn sample_tree() -> Vec<SessionGroup> {
        vec![session(
            "work",
            true,
            vec![
                tab(
                    "code",
                    true,
                    vec![pane(1, "vim", true), pane(2, "shell", false)],
                ),
                tab("notes", false, vec![pane(3, "obsidian", true)]),
            ],
        )]
    }

    #[test]
    fn flatten_emits_session_tab_and_pane_rows() {
        let items = flatten(&sample_tree());
        let titles: Vec<&str> = items.iter().map(|item| item.title.as_str()).collect();
        assert_eq!(
            titles,
            vec!["work (current)", "code", "▶ vim", "○ shell", "● obsidian",]
        );
    }

    #[test]
    fn flatten_uses_last_glyph_for_last_sibling() {
        let items = flatten(&sample_tree());
        // tab "code" is not the last tab -> ├─; tab "notes" is hoisted (single pane) at last -> └─
        assert_eq!(items[0].tree_prefix, None); // session header
        assert_eq!(items[1].tree_prefix.as_deref(), Some("├─ ")); // "code" tab header
        assert_eq!(items[2].tree_prefix.as_deref(), Some("│   ├─ ")); // vim
        assert_eq!(items[3].tree_prefix.as_deref(), Some("│   └─ ")); // shell (last pane)
        assert_eq!(items[4].tree_prefix.as_deref(), Some("└─ ")); // hoisted obsidian
    }

    #[test]
    fn flatten_hoists_single_pane_tab() {
        let tree = vec![session(
            "solo",
            false,
            vec![tab("only", true, vec![pane(7, "alone", true)])],
        )];
        let items = flatten(&tree);
        let titles: Vec<&str> = items.iter().map(|item| item.title.as_str()).collect();
        // session header, then a hoisted pane row (no tab header)
        assert_eq!(titles, vec!["solo", "● alone"]);
        assert_eq!(items[1].tree_prefix.as_deref(), Some("└─ "));
    }

    #[test]
    fn marker_distinguishes_current_active_inactive() {
        let tree = vec![
            session(
                "alpha",
                true,
                vec![tab("main", true, vec![pane(1, "current", true)])],
            ),
            session(
                "beta",
                false,
                vec![tab("main", true, vec![pane(2, "active-elsewhere", true)])],
            ),
            session(
                "gamma",
                false,
                vec![tab("main", true, vec![pane(3, "idle", false)])],
            ),
        ];
        let items = flatten(&tree);
        let pane_titles: Vec<&str> = items
            .iter()
            .filter(|item| item.selectable)
            .map(|item| item.title.as_str())
            .collect();
        assert_eq!(
            pane_titles,
            vec!["▶ current", "● active-elsewhere", "○ idle"]
        );
    }

    #[test]
    fn filter_keeps_ancestors_of_matching_pane() {
        let tree = sample_tree();
        let filtered = filter(tree, "obsidian");
        assert_eq!(filtered.len(), 1);
        let session = &filtered[0];
        assert_eq!(session.name, "work");
        // only the matching tab survives
        assert_eq!(session.tabs.len(), 1);
        assert_eq!(session.tabs[0].name, "notes");
        assert_eq!(session.tabs[0].panes.len(), 1);
        assert_eq!(session.tabs[0].panes[0].title, "obsidian");
    }

    #[test]
    fn filter_drops_non_matching_siblings_within_tab() {
        let tree = sample_tree();
        let filtered = filter(tree, "vim");
        assert_eq!(filtered.len(), 1);
        let tabs = &filtered[0].tabs;
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs[0].name, "code");
        assert_eq!(tabs[0].panes.len(), 1);
        assert_eq!(tabs[0].panes[0].title, "vim");
    }

    #[test]
    fn filter_drops_session_when_no_pane_matches() {
        let tree = sample_tree();
        let filtered = filter(tree, "nonexistent-query-token");
        assert!(filtered.is_empty());
    }

    #[test]
    fn filter_with_blank_query_returns_tree_unchanged() {
        let tree = sample_tree();
        let filtered = filter(tree.clone(), "   ");
        assert_eq!(filtered, tree);
    }

    #[test]
    fn filter_matches_via_session_alias() {
        let tree = vec![session(
            "deploy",
            false,
            vec![tab("main", true, vec![pane(1, "shell", false)])],
        )];
        let filtered = filter(tree, "deploy");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].tabs[0].panes[0].title, "shell");
    }

    #[test]
    fn filter_preserves_natural_order() {
        let tree = vec![session(
            "work",
            true,
            vec![tab(
                "code",
                true,
                vec![
                    pane(1, "alpha-shell", false),
                    pane(2, "beta-shell", false),
                    pane(3, "gamma-shell", false),
                ],
            )],
        )];
        let filtered = filter(tree, "shell");
        let titles: Vec<&str> = filtered[0].tabs[0]
            .panes
            .iter()
            .map(|p| p.title.as_str())
            .collect();
        assert_eq!(titles, vec!["alpha-shell", "beta-shell", "gamma-shell"]);
    }
}
