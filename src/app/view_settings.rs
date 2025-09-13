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

pub enum VS_PianoRoll_OnionColoring {
    GrayedOut,
    PartialColor,
    FullColor
}

impl Default for VS_PianoRoll_OnionColoring {
    fn default() -> Self {
        VS_PianoRoll_OnionColoring::PartialColor
    }
}

// data view (for viewing note velocities, cc event stuff, etc.)
#[derive(PartialEq, Clone, Copy)]
pub enum VS_PianoRoll_DataViewState {
    Hidden,
    NoteVelocities,
    PitchBend
}

impl Default for VS_PianoRoll_DataViewState {
    fn default() -> Self {
        VS_PianoRoll_DataViewState::NoteVelocities
    }
}

impl ToString for VS_PianoRoll_DataViewState {
    fn to_string(&self) -> String {
        match self {
            VS_PianoRoll_DataViewState::Hidden => "None".to_string(),
            VS_PianoRoll_DataViewState::NoteVelocities => "Velocity".to_string(),
            VS_PianoRoll_DataViewState::PitchBend => "Pitch bend".to_string()
        }
    }
}

pub struct ViewSettings {
    pub pr_onion_state: VS_PianoRoll_OnionState,
    pub pr_dataview_state: VS_PianoRoll_DataViewState,
    pub pr_dataview_size: f32
}

impl Default for ViewSettings {
    fn default() -> Self {
        Self {
            pr_dataview_size: 0.25,
            pr_onion_state: Default::default(),
            pr_dataview_state: Default::default()
        }
    }
}