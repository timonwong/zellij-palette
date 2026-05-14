use crate::model::PaletteItem;

pub fn next_selectable(items: &[PaletteItem], current: usize, delta: isize) -> usize {
    let selectable: Vec<usize> = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| item.selectable.then_some(index))
        .collect();
    if selectable.is_empty() {
        return 0;
    }
    let current_index = selectable
        .iter()
        .position(|index| *index == current)
        .unwrap_or(0) as isize;
    let next = (current_index + delta).rem_euclid(selectable.len() as isize) as usize;
    selectable[next]
}

pub fn normalize_selection(items: &[PaletteItem], current: usize) -> usize {
    if items.is_empty() {
        return 0;
    }
    if items.get(current).is_some_and(|item| item.selectable) {
        return current;
    }
    items.iter().position(|item| item.selectable).unwrap_or(0)
}

pub fn list_offset(selected: usize, item_count: usize, list_rows: usize) -> usize {
    if item_count <= list_rows || selected < list_rows / 2 {
        return 0;
    }
    let max_offset = item_count.saturating_sub(list_rows);
    selected.saturating_sub(list_rows / 2).min(max_offset)
}

#[cfg(test)]
mod tests {
    use super::{list_offset, next_selectable, normalize_selection};
    use crate::model::{PaletteAction, PaletteItem};

    fn leaf(title: &str) -> PaletteItem {
        PaletteItem::leaf(title, PaletteAction::Noop)
    }

    fn group(title: &str) -> PaletteItem {
        PaletteItem::group(title)
    }

    #[test]
    fn next_selectable_steps_forward_and_skips_groups() {
        let items = vec![group("G"), leaf("a"), leaf("b"), group("G2"), leaf("c")];
        assert_eq!(next_selectable(&items, 1, 1), 2);
        assert_eq!(next_selectable(&items, 2, 1), 4);
    }

    #[test]
    fn next_selectable_wraps_at_the_end() {
        let items = vec![group("G"), leaf("a"), leaf("b"), group("G2"), leaf("c")];
        assert_eq!(next_selectable(&items, 4, 1), 1);
    }

    #[test]
    fn next_selectable_steps_backward_and_wraps() {
        let items = vec![group("G"), leaf("a"), leaf("b"), group("G2"), leaf("c")];
        assert_eq!(next_selectable(&items, 4, -1), 2);
        assert_eq!(next_selectable(&items, 2, -1), 1);
        assert_eq!(next_selectable(&items, 1, -1), 4);
    }

    #[test]
    fn next_selectable_returns_zero_when_no_item_is_selectable() {
        let items = vec![group("a"), group("b")];
        assert_eq!(next_selectable(&items, 0, 1), 0);
        assert_eq!(next_selectable(&items, 0, -1), 0);
    }

    #[test]
    fn next_selectable_returns_zero_on_empty_input() {
        assert_eq!(next_selectable(&[], 0, 1), 0);
        assert_eq!(next_selectable(&[], 0, -1), 0);
    }

    #[test]
    fn normalize_selection_keeps_a_valid_selectable_index() {
        let items = vec![group("g"), leaf("a"), leaf("b")];
        assert_eq!(normalize_selection(&items, 1), 1);
        assert_eq!(normalize_selection(&items, 2), 2);
    }

    #[test]
    fn normalize_selection_moves_off_a_group_header() {
        let items = vec![group("g"), leaf("a"), leaf("b")];
        assert_eq!(normalize_selection(&items, 0), 1);
    }

    #[test]
    fn normalize_selection_recovers_from_an_out_of_bounds_index() {
        let items = vec![group("g"), leaf("a")];
        assert_eq!(normalize_selection(&items, 99), 1);
    }

    #[test]
    fn normalize_selection_falls_back_to_zero_when_nothing_is_selectable() {
        let items = vec![group("g1"), group("g2")];
        assert_eq!(normalize_selection(&items, 0), 0);
        assert_eq!(normalize_selection(&items, 1), 0);
    }

    #[test]
    fn normalize_selection_handles_an_empty_list() {
        assert_eq!(normalize_selection(&[], 0), 0);
        assert_eq!(normalize_selection(&[], 99), 0);
    }

    #[test]
    fn list_offset_is_zero_when_the_list_fits_the_viewport() {
        assert_eq!(list_offset(0, 5, 10), 0);
        assert_eq!(list_offset(9, 10, 10), 0);
    }

    #[test]
    fn list_offset_is_zero_when_selection_is_near_the_top() {
        assert_eq!(list_offset(3, 100, 10), 0);
        assert_eq!(list_offset(4, 100, 10), 0);
    }

    #[test]
    fn list_offset_centers_the_selection_in_the_viewport() {
        assert_eq!(list_offset(50, 100, 10), 45);
    }

    #[test]
    fn list_offset_clamps_at_the_end_of_the_list() {
        assert_eq!(list_offset(99, 100, 10), 90);
    }

    #[test]
    fn list_offset_does_not_underflow_on_a_huge_viewport() {
        assert_eq!(list_offset(0, 5, 1000), 0);
    }
}
