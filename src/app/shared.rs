use std::sync::Arc;

use eframe::glow;

use crate::app::rendering::buffers::Texture;

type NoteColor = [f32; 3];

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

#[derive(PartialEq)]
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

impl ToString for NoteColorIndexing {
    fn to_string(&self) -> String {
        match self {
            NoteColorIndexing::Channel => "Channel",
            NoteColorIndexing::Track => "Track",
            NoteColorIndexing::ChannelTrack => "Channel & Track"
        }.to_string()
    }
}

pub struct NoteColors {
    index_type: NoteColorIndexing,
    note_texture: Option<Texture>
}

impl Default for NoteColors {
    fn default() -> Self {
        Self {
            index_type: Default::default(),
            note_texture: None
        }
    }
}

impl NoteColors {
    pub fn new(gl: &Arc<glow::Context>) -> Self {
        let mut note_texture = Texture::new(gl.clone(), glow::TEXTURE_2D);
        
        let note_colors = DEFAULT_COLORS;
        let note_data = Self::generate_texture_data(note_colors);
        note_texture.bind();
        note_texture.set_wrapping(glow::REPEAT);
        note_texture.set_filtering(glow::NEAREST);
        note_texture.load_raw(note_data.as_slice(), 16, 1);

        Self {
            index_type: Default::default(),
            note_texture: Some(note_texture)
        }
    }

    /*pub fn with_index_type(gl: &Arc<glow::Context>, index_type: NoteColorIndexing) -> Self {
        let mut note_texture = Texture::new(gl.clone(), glow::TEXTURE_2D);

        let note_colors = DEFAULT_COLORS;
        let note_data = Self::generate_texture_data(note_colors);
        note_texture.bind();
        note_texture.set_wrapping(glow::REPEAT);
        note_texture.set_filtering(glow::NEAREST);
        note_texture.load_raw(note_data.as_slice(), 16, 1);

        Self {
            index_type,
            note_texture: Some(note_texture)
        }
    }*/

    pub fn load_from_image(&mut self, path: &str) {
        if let Some(tex) = self.note_texture.as_mut() {
            tex.update_texture(path);
        }
    }

    #[inline(always)]
    pub fn get_index(&self, trk_chan: usize) -> usize {
        let (trk, chn) = self.decode_track_channel(trk_chan);

        match &self.index_type {
            NoteColorIndexing::Channel => {
                chn
            },
            NoteColorIndexing::Track => {
                trk & 0xF
            },
            NoteColorIndexing::ChannelTrack => {
                (trk + chn) & 0xF
            }
        }
    }

    pub fn generate_texture_data(colors: [[f32; 3]; 16]) -> Vec<u8> {
        let mut data = vec![0; 16 * 3];
        for (i, color) in colors.iter().enumerate() {
            let index = i * 3;
            let (r, g, b) = ((color[0] * 255.0) as u8, (color[1] * 255.0) as u8, (color[2] * 255.0) as u8);
            data[index] = r;
            data[index + 1] = g;
            data[index + 2] = b;
        }
        data
    }

    #[inline(always)]
    fn decode_track_channel(&self, trk_chan: usize) -> (usize, usize) {
        (trk_chan >> 4, trk_chan & 0xF)
    }

    #[inline(always)]
    pub fn get_texture(&mut self) -> &mut Texture {
        self.note_texture.as_mut().unwrap()
    }

    #[inline(always)]
    pub fn get_index_type_mut(&mut self) -> &mut NoteColorIndexing {
        &mut self.index_type
    }
}