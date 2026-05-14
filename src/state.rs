use crate::model::{PaletteId, PaletteItem};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PermissionState {
    #[default]
    Pending,
    Granted,
    Denied,
}

pub fn permission_placeholder_items(permission_state: PermissionState) -> Vec<PaletteItem> {
    match permission_state {
        PermissionState::Pending => vec![PaletteItem::group("Waiting for plugin permissions")],
        PermissionState::Denied => vec![PaletteItem::group("Permissions denied")],
        PermissionState::Granted => Vec::new(),
    }
}

pub struct PaletteState {
    query: String,
    selected: usize,
    palette_id: PaletteId,
    items: Vec<PaletteItem>,
    stack: Vec<PaletteSnapshot>,
}

struct PaletteSnapshot {
    query: String,
    selected: usize,
    palette_id: PaletteId,
    items: Vec<PaletteItem>,
}

impl PaletteState {
    pub fn new(palette_id: PaletteId, items: Vec<PaletteItem>) -> Self {
        let mut state = Self {
            query: String::new(),
            selected: 0,
            palette_id,
            items,
            stack: Vec::new(),
        };
        state.ensure_selectable();
        state
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
    }

    pub fn move_next(&mut self) {
        if self.items.is_empty() {
            self.selected = 0;
            return;
        }

        let mut next = self.selected;
        for _ in 0..self.items.len() {
            next = (next + 1) % self.items.len();
            if self.items[next].selectable {
                self.selected = next;
                return;
            }
        }
    }

    pub fn selected_title(&self) -> Option<&str> {
        self.items
            .get(self.selected)
            .map(|item| item.title.as_str())
    }

    pub fn push_palette(&mut self, palette_id: PaletteId, items: Vec<PaletteItem>) {
        self.stack.push(PaletteSnapshot {
            query: self.query.clone(),
            selected: self.selected,
            palette_id: self.palette_id,
            items: self.items.clone(),
        });
        self.palette_id = palette_id;
        self.items = items;
        self.query.clear();
        self.selected = 0;
        self.ensure_selectable();
    }

    pub fn pop_palette(&mut self) {
        if let Some(snapshot) = self.stack.pop() {
            self.query = snapshot.query;
            self.selected = snapshot.selected;
            self.palette_id = snapshot.palette_id;
            self.items = snapshot.items;
            self.ensure_selectable();
        }
    }

    fn ensure_selectable(&mut self) {
        if self
            .items
            .get(self.selected)
            .is_some_and(|item| item.selectable)
        {
            return;
        }
        if let Some(index) = self.items.iter().position(|item| item.selectable) {
            self.selected = index;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PermissionState, permission_placeholder_items};

    #[test]
    fn pending_permissions_render_placeholder_items() {
        let items = permission_placeholder_items(PermissionState::Pending);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Waiting for plugin permissions");
        assert!(!items[0].selectable);
    }

    #[test]
    fn denied_permissions_render_guidance() {
        let items = permission_placeholder_items(PermissionState::Denied);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Permissions denied");
        assert!(!items[0].selectable);
    }
}
