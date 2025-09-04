#[derive(PartialEq)]
pub enum VS_PianoRoll_OnionState {
    NoOnion,
    ViewPrevious,
    ViewNext,
    ViewAll
}

impl Default for VS_PianoRoll_OnionState {
    fn default() -> Self {
        VS_PianoRoll_OnionState::NoOnion
    }
}

impl ToString for VS_PianoRoll_OnionState {
    fn to_string(&self) -> String {
        match self {
            VS_PianoRoll_OnionState::NoOnion => "No onion".to_string(),
            VS_PianoRoll_OnionState::ViewAll => "All tracks".to_string(),
            VS_PianoRoll_OnionState::ViewNext => "Next track".to_string(),
            VS_PianoRoll_OnionState::ViewPrevious => "Previous track".to_string()
        }
    }
}

#[derive(Default)]
pub struct ViewSettings {
    pub pr_onion_state: VS_PianoRoll_OnionState
}