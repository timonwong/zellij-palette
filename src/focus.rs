use crate::model::PaneTarget;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FocusPanePlan {
    CurrentSession {
        pane_id: u32,
        is_plugin: bool,
    },
    OtherSession {
        session_name: String,
        tab_position: usize,
        pane_id: u32,
        is_plugin: bool,
    },
}

pub fn plan_focus_pane(current_session_name: Option<&str>, target: &PaneTarget) -> FocusPanePlan {
    if current_session_name == Some(target.session_name.as_str()) {
        FocusPanePlan::CurrentSession {
            pane_id: target.pane_id,
            is_plugin: target.is_plugin,
        }
    } else {
        FocusPanePlan::OtherSession {
            session_name: target.session_name.clone(),
            tab_position: target.tab_position,
            pane_id: target.pane_id,
            is_plugin: target.is_plugin,
        }
    }
}

pub fn should_list_find_pane_item(
    pane_id: u32,
    is_plugin: bool,
    is_selectable: bool,
    is_suppressed: bool,
    own_plugin_id: Option<u32>,
) -> bool {
    is_selectable && !is_suppressed && !(is_plugin && own_plugin_id == Some(pane_id))
}
