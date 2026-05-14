use crate::model::PaletteItem;

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
