pub mod fuzzy;
pub mod model;
pub mod state;
pub mod user_config;

#[cfg(test)]
mod tests {
    use crate::fuzzy::filter_items;
    use crate::model::{PaletteAction, PaletteId, PaletteItem};
    use crate::state::PaletteState;

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
    fn selection_skips_non_selectable_rows() {
        let items = vec![
            PaletteItem::group("Panes"),
            item("Find Pane"),
            item("Split Right"),
        ];
        let mut state = PaletteState::new(PaletteId::Commands, items);
        assert_eq!(state.selected_title(), Some("Find Pane"));
        state.move_next();
        assert_eq!(state.selected_title(), Some("Split Right"));
    }

    #[test]
    fn back_restores_previous_filter_and_selection() {
        let mut state = PaletteState::new(
            PaletteId::Commands,
            vec![
                item("Find Pane"),
                PaletteItem::leaf(
                    "Move Pane to...",
                    PaletteAction::OpenPalette(PaletteId::MovePane),
                ),
                item("Split Right"),
            ],
        );
        state.set_query("move");
        state.move_next();
        state.push_palette(
            PaletteId::MovePane,
            vec![item("Tab 1"), item("Tab 2"), item("New Tab")],
        );
        state.set_query("tab 2");
        state.move_next();
        state.pop_palette();

        assert_eq!(state.query(), "move");
        assert_eq!(state.selected_title(), Some("Move Pane to..."));
    }
}
