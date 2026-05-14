pub mod focus;
pub mod fuzzy;
pub mod kdl;
pub mod model;
pub mod selection;
pub mod state;
pub mod user_config;

#[cfg(test)]
mod tests {
    use crate::focus::{FocusPanePlan, plan_focus_pane, should_list_find_pane_item};
    use crate::fuzzy::filter_items;
    use crate::model::{PaletteAction, PaletteItem, PaneTarget};

    fn item(title: &str) -> PaletteItem {
        PaletteItem::leaf(title, PaletteAction::Noop)
    }

    #[test]
    fn filter_matches_title_initials() {
        let items = vec![item("Split Horizontal"), item("New Tab")];
        let filtered = filter_items(&items, "sh");
        let titles: Vec<_> = filtered.iter().map(|item| item.title.as_str()).collect();
        assert_eq!(titles, vec!["Split Horizontal"]);
    }

    #[test]
    fn filter_matches_explicit_aliases() {
        let items = vec![
            PaletteItem::leaf("Find Pane", PaletteAction::Noop).with_aliases(["locator", "jump"]),
            item("New Tab"),
        ];
        let filtered = filter_items(&items, "jump");
        let titles: Vec<_> = filtered.iter().map(|item| item.title.as_str()).collect();
        assert_eq!(titles, vec!["Find Pane"]);
    }

    #[test]
    fn filter_requires_every_query_part() {
        let items = vec![item("Split Horizontal Pane"), item("Switch Session")];
        let filtered = filter_items(&items, "split pane");
        let titles: Vec<_> = filtered.iter().map(|item| item.title.as_str()).collect();
        assert_eq!(titles, vec!["Split Horizontal Pane"]);
    }

    #[test]
    fn filter_matches_category_and_shortcut_text() {
        let items = vec![
            PaletteItem::leaf("Open Logs", PaletteAction::Noop)
                .with_category("Tools")
                .with_shortcut("Ctrl-L"),
            item("Split Right"),
        ];

        let by_category = filter_items(&items, "tools");
        let by_shortcut = filter_items(&items, "ctrl");

        assert_eq!(by_category[0].title, "Open Logs");
        assert_eq!(by_shortcut[0].title, "Open Logs");
    }

    #[test]
    fn empty_query_preserves_original_order() {
        let items = vec![item("Themes"), item("Find Pane"), item("Split Right")];
        let filtered = filter_items(&items, "");
        let titles: Vec<_> = filtered.iter().map(|item| item.title.as_str()).collect();
        assert_eq!(titles, vec!["Themes", "Find Pane", "Split Right"]);
    }

    #[test]
    fn focus_plan_uses_direct_focus_for_current_session() {
        let target = PaneTarget {
            session_name: "work".to_owned(),
            tab_position: 2,
            tab_id: 17,
            pane_id: 42,
            is_plugin: false,
        };

        let plan = plan_focus_pane(Some("work"), &target);

        assert_eq!(
            plan,
            FocusPanePlan::CurrentSession {
                pane_id: 42,
                is_plugin: false,
            }
        );
    }

    #[test]
    fn focus_plan_switches_session_for_other_session_targets() {
        let target = PaneTarget {
            session_name: "ops".to_owned(),
            tab_position: 3,
            tab_id: 21,
            pane_id: 9,
            is_plugin: true,
        };

        let plan = plan_focus_pane(Some("work"), &target);

        assert_eq!(
            plan,
            FocusPanePlan::OtherSession {
                session_name: "ops".to_owned(),
                tab_position: 3,
                pane_id: 9,
                is_plugin: true,
            }
        );
    }

    #[test]
    fn find_pane_items_exclude_the_palette_plugin_itself() {
        assert!(!should_list_find_pane_item(7, true, true, false, Some(7)));
    }

    #[test]
    fn find_pane_items_keep_other_selectable_panes() {
        assert!(should_list_find_pane_item(8, true, true, false, Some(7)));
        assert!(should_list_find_pane_item(7, false, true, false, Some(7)));
    }
}
