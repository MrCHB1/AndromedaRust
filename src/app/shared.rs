type NoteColor = [f32; 3];

pub const DARK: NoteColor = [0.75, 0.75, 0.75];
pub const DARKER: NoteColor = [0.5, 0.5, 0.5];
pub const DARKEST: NoteColor = [0.25, 0.25, 0.25];

pub const WHITE: NoteColor = [1.0, 1.0, 1.0];
pub const BLACK: NoteColor = [0.0, 0.0, 0.0];
pub const SELECTED: NoteColor = [1.0, 0.5, 0.5];

const DEFAULT_COLORS: [NoteColor; 16] = [
    [1.00, 0.00, 0.00],
    [1.00, 0.25, 0.00],
    [1.00, 0.50, 0.00],
    [1.00, 0.75, 0.00],
    [1.00, 1.00, 0.00],
    [0.75, 1.00, 0.00],
    [0.50, 1.00, 0.00],
    [0.25, 1.00, 0.00],
    [0.00, 1.00, 0.00],
    [0.00, 1.00, 0.25],
    [0.00, 1.00, 0.50],
    [0.00, 1.00, 0.75],
    [0.00, 1.00, 1.00],
    [0.00, 0.50, 1.00],
    [0.50, 0.00, 1.00],
    [0.75, 0.00, 1.00],
];

/// This defines how note colors are indexed.
pub enum NoteColorIndexing {
    Channel,
    Track,
    ChannelTrack
}

impl Default for NoteColorIndexing {
    fn default() -> Self {
        NoteColorIndexing::Channel
    }
}

pub struct NoteColors {
    note_colors: [[f32; 3]; 16],
    index_type: NoteColorIndexing
}

impl Default for NoteColors {
    fn default() -> Self {
        Self {
            note_colors: DEFAULT_COLORS,
            index_type: Default::default()
        }
    }
}

impl NoteColors {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_index_type(index_type: NoteColorIndexing) -> Self {
        Self {
            note_colors: DEFAULT_COLORS,
            index_type
        }
    }

    #[inline]
    pub fn get(&self, trk_chan: usize) -> &NoteColor {
        let (trk, chn) = self.decode_track_channel(trk_chan);

        &self.note_colors[match &self.index_type {
            NoteColorIndexing::Channel => {
                chn
            },
            NoteColorIndexing::Track => {
                trk & 0xF
            },
            NoteColorIndexing::ChannelTrack => {
                ((trk >> 4) + chn) & 0xF
            }
        }]
    }

    #[inline]
    pub fn get_mut(&mut self, trk_chan: usize) -> &mut NoteColor {
        let (trk, chn) = self.decode_track_channel(trk_chan);

        &mut self.note_colors[match &self.index_type {
            NoteColorIndexing::Channel => {
                chn
            },
            NoteColorIndexing::Track => {
                trk & 0xF
            },
            NoteColorIndexing::ChannelTrack => {
                ((trk >> 4) + chn) & 0xF
            }
        }]
    }

    pub fn get_and_mix(&self, trk_chan: usize, b: &NoteColor, factor: f32) -> NoteColor {
        let a = self.get(trk_chan);
        [
            a[0] * (1.0 - factor) + b[0] * factor,
            a[1] * (1.0 - factor) + b[1] * factor,
            a[2] * (1.0 - factor) + b[2] * factor
        ]
    }

    

    #[inline(always)]
    fn decode_track_channel(&self, trk_chan: usize) -> (usize, usize) {
        (trk_chan >> 4, trk_chan & 0xF)
    }
}