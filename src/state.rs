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
