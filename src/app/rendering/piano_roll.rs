use crate::app::rendering::Renderer;
use crate::audio::event_playback::PlaybackManager;
use eframe::egui::Vec2;
use eframe::glow;
use eframe::glow::HasContext;
use std::sync::{Arc, Mutex};
use crate::app::main_window::GhostNote;
use crate::app::rendering::{
    buffers::*,
    shaders::*
};
use crate::app::view_settings::{VS_PianoRoll_OnionState, ViewSettings};
use crate::editor::navigation::PianoRollNavigation;
use crate::midi::events::note::Note;
use crate::set_attribute;

const NOTE_BUFFER_SIZE: usize = 4096;

// Piano Roll Background
pub type BarStart = f32;
pub type BarLength = f32;
pub type BarNumber = u32;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct RenderPianoRollBar(BarStart, BarLength, BarNumber);

// Piano Roll Notes
pub type NoteRect = [f32; 4]; // (start, length, note bottom, note top)
pub type NoteColor = [f32; 3];

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct RenderPianoRollNote(NoteRect, NoteColor, NoteColor);

pub type Position = [f32; 2];

#[repr(C, packed)]
pub struct Vertex(Position);

pub const QUAD_VERTICES: [Vertex; 4] = [
    Vertex([0.0, 0.0]),
    Vertex([1.0, 0.0]),
    Vertex([1.0, 1.0]),
    Vertex([0.0, 1.0])
];

const QUAD_INDICES: [u32; 6] = [
    0, 1, 3,
    1, 2, 3
];

pub struct PianoRollRenderer {
    pub navigation: Arc<Mutex<PianoRollNavigation>>,
    pub playback_manager: Arc<Mutex<PlaybackManager>>,
    pub view_settings: Arc<Mutex<ViewSettings>>,
    pub window_size: Vec2<>,
    pub ppq: u16,

    pr_program: ShaderProgram,
    pr_vertex_buffer: Buffer,
    pr_vertex_array: VertexArray,
    pr_instance_buffer: Buffer,
    pr_index_buffer: Buffer,

    pr_notes_program: ShaderProgram,
    pr_notes_vbo: Buffer,
    pr_notes_vao: VertexArray,
    pr_notes_ibo: Buffer,
    pr_notes_ebo: Buffer,

    gl: Arc<glow::Context>,

    bars_render: Vec<RenderPianoRollBar>,
    notes_render: Vec<RenderPianoRollNote>,
    render_notes: Arc<Mutex<Vec<Vec<Vec<Note>>>>>,
    note_colors: Vec<[f32; 3]>,
    // per channel per track
    last_note_start: Vec<Vec<usize>>,
    first_render_note: Vec<Vec<usize>>,
    last_time: f32,

    pub ghost_notes: Option<Arc<Mutex<Vec<GhostNote>>>>,
    pub selected: Arc<Mutex<Vec<usize>>>,
    render_active: bool
}

impl PianoRollRenderer {
    pub unsafe fn new(notes: Arc<Mutex<Vec<Vec<Vec<Note>>>>>, view_settings: Arc<Mutex<ViewSettings>>, nav: Arc<Mutex<PianoRollNavigation>>, gl: Arc<glow::Context>, playback_manager: Arc<Mutex<PlaybackManager>>) -> Self {
        let pr_program = ShaderProgram::create_from_files(gl.clone(), "./shaders/piano_roll_bg");
        let pr_notes_program = ShaderProgram::create_from_files(gl.clone(), "./shaders/piano_roll_note");

        // -------- PIANO ROLL BAR --------

        let pr_vertex_buffer = Buffer::new(gl.clone(), glow::ARRAY_BUFFER);
        pr_vertex_buffer.set_data(&QUAD_VERTICES, glow::STATIC_DRAW);

        let pr_index_buffer = Buffer::new(gl.clone(), glow::ELEMENT_ARRAY_BUFFER);
        pr_index_buffer.set_data(&QUAD_INDICES, glow::STATIC_DRAW);

        let pr_vertex_array = VertexArray::new(gl.clone());
        let pos_attrib = pr_program.get_attrib_location("vPos").unwrap();
        set_attribute!(glow::FLOAT, pr_vertex_array, pos_attrib, Vertex::0);

        let pr_instance_buffer = Buffer::new(gl.clone(), glow::ARRAY_BUFFER);
        let pr_bars_render = [
            RenderPianoRollBar {
                0: 0.0,
                1: 1.0,
                2: 0
            }; 32
        ];
        pr_instance_buffer.set_data(pr_bars_render.as_slice(), glow::DYNAMIC_DRAW);

        let pr_bar_start = pr_program.get_attrib_location("barStart").unwrap();
        set_attribute!(glow::FLOAT, pr_vertex_array, pr_bar_start, RenderPianoRollBar::0);
        let pr_bar_length = pr_program.get_attrib_location("barLength").unwrap();
        set_attribute!(glow::FLOAT, pr_vertex_array, pr_bar_length, RenderPianoRollBar::1);
        let pr_bar_number = pr_program.get_attrib_location("barNumber").unwrap();
        set_attribute!(glow::UNSIGNED_INT, pr_vertex_array, pr_bar_number, RenderPianoRollBar::2);

        gl.vertex_attrib_divisor(1, 1);
        gl.vertex_attrib_divisor(2, 1);
        gl.vertex_attrib_divisor(3, 1);

        // -------- PIANO ROLL NOTES --------

        let pr_notes_vbo = Buffer::new(gl.clone(), glow::ARRAY_BUFFER);
        pr_notes_vbo.set_data(&QUAD_VERTICES, glow::STATIC_DRAW);

        let pr_notes_ebo = Buffer::new(gl.clone(), glow::ELEMENT_ARRAY_BUFFER);
        pr_notes_ebo.set_data(&QUAD_INDICES, glow::STATIC_DRAW);

        let pr_notes_vao = VertexArray::new(gl.clone());
        // let pos_attrib = pr_notes_program.get_attrib_location("vPos").unwrap();
        set_attribute!(glow::FLOAT, pr_notes_vao, 0, Vertex::0);

        let pr_notes_ibo = Buffer::new(gl.clone(), glow::ARRAY_BUFFER);
        let pr_notes_render = [
            RenderPianoRollNote {
                0: [0.0, 1.0, 0.0, 1.0],
                1: [1.0, 0.0, 0.0],
                2: [0.0, 0.0, 0.0]
            }; NOTE_BUFFER_SIZE
        ];
        pr_notes_ibo.set_data(pr_notes_render.as_slice(), glow::DYNAMIC_DRAW);

        let pr_note_rect = pr_notes_program.get_attrib_location("noteRect").unwrap();
        set_attribute!(glow::FLOAT, pr_notes_vao, pr_note_rect, RenderPianoRollNote::0);
        let pr_note_color = pr_notes_program.get_attrib_location("noteColor").unwrap();
        set_attribute!(glow::FLOAT, pr_notes_vao, pr_note_color, RenderPianoRollNote::1);
        let pr_note_color = pr_notes_program.get_attrib_location("noteColor2").unwrap();
        set_attribute!(glow::FLOAT, pr_notes_vao, pr_note_color, RenderPianoRollNote::2);

        gl.vertex_attrib_divisor(1, 1);
        gl.vertex_attrib_divisor(2, 1);
        gl.vertex_attrib_divisor(3, 1);

        let last_note_start = {
            let notes = notes.lock().unwrap();
            vec![vec![0usize; 16]; notes.len()]
        };

        let first_render_note = {
            let notes = notes.lock().unwrap();
            vec![vec![0usize; 16]; notes.len()]
        };

        Self {
            playback_manager,
            navigation: nav,
            view_settings: view_settings,
            window_size: Vec2::new(0.0, 0.0),
            pr_program,
            pr_vertex_buffer,
            pr_vertex_array,
            pr_instance_buffer,
            pr_index_buffer,

            pr_notes_program,
            pr_notes_vao,
            pr_notes_vbo,
            pr_notes_ebo,
            pr_notes_ibo,

            gl,
            bars_render: pr_bars_render.to_vec(),
            notes_render: pr_notes_render.to_vec(),
            render_notes: notes.clone(),

            ppq: 960,
            note_colors: vec![
                [1.0, 0.0, 0.0],
                [1.0, 0.25, 0.0],
                [1.0, 0.5, 0.0],
                [1.0, 0.75, 0.0],
                [1.0, 1.0, 0.0],
                [0.5, 1.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 0.5],
                [0.0, 1.0, 1.0],
                [0.0, 0.75, 1.0],
                [0.0, 0.5, 1.0],
                [0.0, 0.25, 1.0],
                [0.0, 0.0, 1.0],
                [0.25, 0.0, 1.0],
                [0.5, 0.0, 1.0],
                [0.75, 0.0, 1.0],
                [1.0, 0.0, 1.0]
            ],

            last_note_start,
            first_render_note,
            last_time: 0.0,
            ghost_notes: None,
            selected: Arc::new(Mutex::new(Vec::new())),
            render_active: false
        }
    }
}

impl Renderer for PianoRollRenderer {
    fn draw(&mut self) {
        if !self.render_active { return; }

        unsafe {
            let nav = self.navigation.lock().unwrap();

            let is_playing = {
                let playback_manager = self.playback_manager.lock().unwrap();
                playback_manager.playing
            };

            // only used for when playing the midi
            let view_offset = {
                if is_playing {
                    -nav.zoom_ticks_smoothed / 2.0
                } else {
                    0.0
                }
            };

            let nav_ticks = {
                let playback_manager = self.playback_manager.lock().unwrap();
                if is_playing {
                    playback_manager.get_playback_ticks() as f32
                } else {
                    nav.tick_pos_smoothed
                }
            };

            let nav_ticks_offs = {
                let nav_ticks_offs = nav_ticks + view_offset;
                if nav_ticks_offs < 0.0 { 0.0 }
                else { nav_ticks_offs }
            };

            // RENDER BARS
            {
                self.gl.use_program(Some(self.pr_program.program));

                let mut curr_bar_tick = 0.0;
                let mut bar_id = 0;
                let mut bar_num = 0;
                {
                    let key_start = nav.key_pos_smoothed;
                    let key_end = nav.key_pos_smoothed + nav.zoom_keys_smoothed;

                    self.pr_program.set_float("prBarBottom", -key_start / (key_end - key_start));
                    self.pr_program.set_float("prBarTop", (128.0 - key_start) / (key_end - key_start));
                    self.pr_program.set_float("width", self.window_size.x);
                    self.pr_program.set_float("height", self.window_size.y);

                    while curr_bar_tick < nav.zoom_ticks_smoothed + nav_ticks_offs {
                        bar_num += 1;
                        if (bar_num as f32) * ((self.ppq as f32) * 4.0) < nav_ticks_offs {
                            curr_bar_tick += self.ppq as f32 * 4.0;
                            continue;
                        }
                        self.bars_render[bar_id] = RenderPianoRollBar {
                            0: ((curr_bar_tick - nav_ticks_offs) / nav.zoom_ticks_smoothed),
                            1: ((self.ppq as f32 * 4.0) / nav.zoom_ticks_smoothed),
                            2: bar_num as u32 - 1
                        };
                        bar_id += 1;
                        if bar_id >= 32 {
                            self.pr_vertex_array.bind();
                            self.pr_instance_buffer.bind();
                            self.pr_vertex_buffer.bind();
                            self.pr_index_buffer.bind();
                            self.pr_instance_buffer.set_data(self.bars_render.as_slice(), glow::DYNAMIC_DRAW);
                            self.gl.draw_elements_instanced(
                                glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, 32);
                            bar_id = 0;
                        }
                        curr_bar_tick += self.ppq as f32 * 4.0;
                    }
                }

                if bar_id != 0 {
                    self.pr_vertex_array.bind();
                    self.pr_instance_buffer.bind();
                    self.pr_vertex_buffer.bind();
                    self.pr_index_buffer.bind();
                    self.pr_instance_buffer.set_data(self.bars_render.as_slice(), glow::DYNAMIC_DRAW);

                    self.gl.use_program(Some(self.pr_program.program));
                    self.gl.draw_elements_instanced(
                            glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, bar_id as i32);
                }

                self.gl.use_program(None);
            }

            // RENDER NOTES
            {
                self.gl.use_program(Some(self.pr_notes_program.program));

                {
                    let all_render_notes = self.render_notes.lock().unwrap();
                    // resize last_note_start and first_render_note if notes changed size
                    if self.last_note_start.len() != all_render_notes.len() {
                        self.last_note_start = vec![vec![0usize; 16]; all_render_notes.len()];
                        self.first_render_note = vec![vec![0usize; 16]; all_render_notes.len()];
                    }

                    self.pr_notes_program.set_float("width", self.window_size.x);
                    self.pr_notes_program.set_float("height", self.window_size.y);

                    // iterate through each track, and each channel
                    {
                        let view_settings = self.view_settings.lock().unwrap();

                        //let mut view_all_tracks = true;
                        
                        let tracks_to_iter = match view_settings.pr_onion_state {
                            VS_PianoRoll_OnionState::NoOnion => {
                                &all_render_notes[nav.curr_track as usize..nav.curr_track as usize]
                            },
                            VS_PianoRoll_OnionState::ViewAll => {
                                //view_all_tracks = true;
                                &all_render_notes[..]
                            },
                            VS_PianoRoll_OnionState::ViewPrevious => {
                                //view_all_tracks = false;
                                if nav.curr_track == 0 { &all_render_notes[nav.curr_track as usize..nav.curr_track as usize] }
                                else { &all_render_notes[(nav.curr_track - 1) as usize..(nav.curr_track as usize)] }
                            },
                            VS_PianoRoll_OnionState::ViewNext => {
                                if nav.curr_track == (all_render_notes.len() - 1) as u16 { &all_render_notes[nav.curr_track as usize..nav.curr_track as usize] }
                                else { &all_render_notes[(nav.curr_track) as usize..(nav.curr_track + 1) as usize] }
                            }
                        };

                        { 
                            let mut note_id = 0;
                            let mut curr_track = 0;

                            let mut rendered_notes = 0;

                            // 1. draw all notes that is not the current track
                            for note_track in tracks_to_iter {
                                // skip track if it has nothing or its the navigation's current track
                                if note_track.is_empty() || curr_track == nav.curr_track {
                                    curr_track += 1;
                                    continue;
                                }

                                let mut curr_channel = 0;
                                // iterate through the 16 channels
                                for notes in note_track.iter() {
                                    if notes.is_empty() { curr_channel += 1; continue; }

                                    let mut curr_note = 0;

                                    let mut n_off = self.first_render_note[curr_track as usize][curr_channel];
                                    if self.last_time > nav_ticks_offs {
                                        if n_off == 0 {
                                            for note in &notes[0..notes.len()] {
                                                if (note.start + note.length) as f32 > nav_ticks_offs { break; }
                                                n_off += 1;
                                            }
                                        } else {
                                            for note in notes[0..n_off].iter().rev() {
                                                if ((note.start + note.length) as f32) <= nav_ticks_offs { break; }
                                                n_off -= 1;
                                            }
                                        }
                                        self.first_render_note[curr_track as usize][curr_channel] = n_off;
                                    } else if self.last_time < nav_ticks_offs {
                                        for note in &notes[n_off..notes.len()] {
                                            if (note.start + note.length) as f32 > nav_ticks_offs { break; }
                                            n_off += 1;
                                        }
                                        self.first_render_note[curr_track as usize][curr_channel] = n_off;
                                    }

                                    let mut note_idx = n_off;

                                    let note_end = {
                                        let mut e = n_off;
                                        for note in &notes[n_off..notes.len()] {
                                            if (note.start as f32) > nav_ticks_offs + nav.zoom_ticks_smoothed { break; }
                                            e += 1;
                                        }
                                        e
                                    };

                                    for note in &notes[n_off..note_end] {
                                        //n_off += 1;
                                        //if n_off == notes.len() { break; }

                                        if note.key as f32 + 1.0 < nav.key_pos_smoothed || (note.key as f32) > nav.key_pos_smoothed + nav.zoom_keys_smoothed {
                                            curr_note += 1;
                                            continue;
                                        }

                                        // if note.start as f32 > nav.tick_pos_smoothed + nav.zoom_ticks_smoothed { break; }
                                        {
                                            let sel_lock = self.selected.lock().unwrap();
                                            let note_bottom = (note.key as f32 - nav.key_pos_smoothed) / nav.zoom_keys_smoothed;
                                            let note_top = ((note.key as f32 + 1.0) - nav.key_pos_smoothed) / nav.zoom_keys_smoothed;

                                            let highlight_note_play_size = nav.zoom_ticks_smoothed * 0.001;
                                            let note_playing = (note.start as f32) < nav_ticks + highlight_note_play_size && (note.start as f32 + note.length as f32) > nav_ticks - highlight_note_play_size && is_playing;

                                            self.notes_render[note_id] = RenderPianoRollNote {
                                                0: [(note.start as f32 - nav_ticks_offs) / nav.zoom_ticks_smoothed,
                                                    (note.length as f32) / nav.zoom_ticks_smoothed,
                                                    (note_bottom),
                                                    (note_top)],
                                                1: {
                                                    let mut color = self.note_colors[curr_channel as usize % self.note_colors.len()];
                                                    if sel_lock.contains(&note_idx) {
                                                        color = [1.0, 0.5, 0.5];
                                                    } else {
                                                        color = [color[0] / 128.0 * note.velocity as f32, color[1] / 128.0 * note.velocity as f32, color[2] / 128.0 * note.velocity as f32];
                                                    }

                                                    if note_playing {
                                                        color = [color[0] + 0.5, color[1] + 0.5, color[2] + 0.5];
                                                    }

                                                    color
                                                },
                                                2: {
                                                    let mut color = self.note_colors[curr_channel as usize % self.note_colors.len()];
                                                    color = [color[0] / 128.0 * (127 - note.velocity) as f32, color[1] / 128.0 * (127 - note.velocity) as f32, color[2] / 128.0 * (127 - note.velocity) as f32];
                                                    if note_playing {
                                                        color = [color[0] + 0.5, color[1] + 0.5, color[2] + 0.5];
                                                    }
                                                    color
                                                }
                                            };

                                            note_id += 1;
                                            note_idx += 1;

                                            // flush if note_id is now the note draw buffer size and reset it to zero
                                            if note_id >= NOTE_BUFFER_SIZE {
                                                self.pr_notes_vao.bind();
                                                self.pr_notes_ibo.bind();
                                                self.pr_notes_vbo.bind();
                                                self.pr_notes_ebo.bind();
                                                self.pr_notes_ibo.set_data(self.notes_render.as_slice(), glow::DYNAMIC_DRAW);

                                                self.gl.use_program(Some(self.pr_notes_program.program));
                                                self.gl.draw_elements_instanced(
                                                    glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, NOTE_BUFFER_SIZE as i32);
                                                rendered_notes += note_id;
                                                note_id = 0;
                                            }

                                            curr_note += 1;
                                            if curr_note >= notes.len() {
                                                break;
                                            }
                                        }
                                    }
                                }

                                curr_track += 1;
                            }

                            // 2. draw current track on top
                            let notes_curr_track = &all_render_notes[nav.curr_track as usize];

                            let mut curr_channel = 0;
                            for notes in notes_curr_track {
                                // skip channel if its empty
                                if notes.is_empty() { curr_channel += 1; continue; }

                                let mut curr_note = 0;

                                let mut n_off = self.first_render_note[nav.curr_track as usize][curr_channel];
                                if self.last_time > nav_ticks_offs {
                                    if n_off == 0 {
                                        for note in &notes[0..notes.len()] {
                                            if (note.start + note.length) as f32 > nav_ticks_offs { break; }
                                            n_off += 1;
                                        }
                                    } else { // backwards instead of forwards
                                        for note in notes[0..n_off].iter().rev() {
                                            if ((note.start + note.length) as f32) <= nav_ticks_offs { break; }
                                            n_off -= 1;
                                        }
                                    }
                                    self.first_render_note[nav.curr_track as usize][curr_channel] = n_off;
                                } else if self.last_time < nav_ticks_offs {
                                    for note in &notes[n_off..notes.len()] {
                                        if (note.start + note.length) as f32 > nav_ticks_offs { break; }
                                        n_off += 1;
                                    }
                                    self.first_render_note[nav.curr_track as usize][curr_channel] = n_off;
                                }

                                let mut note_idx = n_off;

                                let note_end = {
                                    let mut e = n_off;
                                    for note in &notes[n_off..notes.len()] {
                                        if (note.start as f32) > nav_ticks_offs + nav.zoom_ticks_smoothed { break; }
                                        e += 1;
                                    }
                                    e
                                };
                                
                                for note in &notes[n_off..note_end] {
                                    //n_off += 1;
                                    //if n_off == notes.len() { break; }

                                    if note.key as f32 + 1.0 < nav.key_pos_smoothed || (note.key as f32) > nav.key_pos_smoothed + nav.zoom_keys_smoothed {
                                        curr_note += 1;
                                        continue;
                                    }

                                    // if note.start as f32 > nav.tick_pos_smoothed + nav.zoom_ticks_smoothed { break; }
                                    {
                                        let sel_lock = self.selected.lock().unwrap();
                                        let note_bottom = (note.key as f32 - nav.key_pos_smoothed) / nav.zoom_keys_smoothed;
                                        let note_top = ((note.key as f32 + 1.0) - nav.key_pos_smoothed) / nav.zoom_keys_smoothed;
                                        self.notes_render[note_id] = RenderPianoRollNote {
                                            0: [(note.start as f32 - nav_ticks_offs) / nav.zoom_ticks_smoothed,
                                                (note.length as f32) / nav.zoom_ticks_smoothed,
                                                (note_bottom),
                                                (note_top)],
                                            1: {
                                                let color = self.note_colors[curr_channel as usize % self.note_colors.len()];
                                                if sel_lock.contains(&note_idx) {
                                                    [1.0, 0.5, 0.5]
                                                } else {
                                                    [color[0] / 128.0 * note.velocity as f32, color[1] / 128.0 * note.velocity as f32, color[2] / 128.0 * note.velocity as f32]
                                                }
                                            },
                                            2: {
                                                let color = self.note_colors[curr_channel as usize % self.note_colors.len()];
                                                [color[0] / 128.0 * (127 - note.velocity) as f32, color[1] / 128.0 * (127 - note.velocity) as f32, color[2] / 128.0 * (127 - note.velocity) as f32]
                                            }
                                        };

                                        note_id += 1;
                                        note_idx += 1;

                                        // flush if note_id is now the note draw buffer size and reset it to zero
                                        if note_id >= NOTE_BUFFER_SIZE {
                                            self.pr_notes_vao.bind();
                                            self.pr_notes_ibo.bind();
                                            self.pr_notes_vbo.bind();
                                            self.pr_notes_ebo.bind();
                                            self.pr_notes_ibo.set_data(self.notes_render.as_slice(), glow::DYNAMIC_DRAW);

                                            self.gl.use_program(Some(self.pr_notes_program.program));
                                            self.gl.draw_elements_instanced(
                                                glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, NOTE_BUFFER_SIZE as i32);
                                            rendered_notes += note_id;
                                            note_id = 0;
                                        }

                                        curr_note += 1;
                                        if curr_note >= notes.len() {
                                            break;
                                        }
                                    }

                                    rendered_notes += 1;
                                }

                                curr_channel += 1;
                            }
                            
                            // 3. flush remaining notes
                            if note_id != 0 {
                                self.pr_notes_vao.bind();
                                self.pr_notes_ibo.bind();
                                self.pr_notes_vbo.bind();
                                self.pr_notes_ebo.bind();
                                self.pr_notes_ibo.set_data(self.notes_render.as_slice(), glow::DYNAMIC_DRAW);

                                self.gl.use_program(Some(self.pr_notes_program.program));
                                self.gl.draw_elements_instanced(
                                    glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, note_id as i32);
                                rendered_notes += note_id;
                            }
                        }
                    }

                    if let Some(ghost_notes) = &self.ghost_notes {
                        let mut note_id = 0;
                        let notes = ghost_notes.lock().unwrap();
                        for note in notes.iter() {
                            let note = note.get_note();
                            let note_bottom = (note.key as f32 - nav.key_pos_smoothed) / (nav.zoom_keys_smoothed);
                            let note_top = ((note.key as f32 + 1.0) - nav.key_pos_smoothed) / (nav.zoom_keys_smoothed);

                            self.notes_render[note_id] = RenderPianoRollNote {
                                0: [(note.start as f32 - nav_ticks_offs) / nav.zoom_ticks_smoothed,
                                    (note.length as f32) / nav.zoom_ticks_smoothed,
                                    (note_bottom),
                                    (note_top)],
                                1: {
                                    let color = self.note_colors[nav.curr_track as usize * 16 + nav.curr_channel as usize % self.note_colors.len()];
                                    [color[0] / 128.0 * note.velocity as f32, color[1] / 128.0 * note.velocity as f32, color[2] / 128.0 * note.velocity as f32]
                                },
                                2: {
                                        let color = self.note_colors[nav.curr_track as usize * 16 + nav.curr_channel as usize % self.note_colors.len()];
                                    [color[0] / 128.0 * (127 - note.velocity) as f32, color[1] / 128.0 * (127 - note.velocity) as f32, color[2] / 128.0 * (127 - note.velocity) as f32]
                                }
                            };
                            note_id += 1;
                            if note_id >= NOTE_BUFFER_SIZE {
                                self.pr_notes_vao.bind();
                                self.pr_notes_ibo.bind();
                                self.pr_notes_vbo.bind();
                                self.pr_notes_ebo.bind();
                                self.pr_notes_ibo.set_data(self.notes_render.as_slice(), glow::DYNAMIC_DRAW);
                                self.gl.use_program(Some(self.pr_notes_program.program));
                                self.gl.draw_elements_instanced(
                                    glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, NOTE_BUFFER_SIZE as i32);
                                note_id = 0;
                            }
                        }

                        if note_id != 0 {
                            self.pr_notes_vao.bind();
                            self.pr_notes_ibo.bind();
                            self.pr_notes_vbo.bind();
                            self.pr_notes_ebo.bind();
                            self.pr_notes_ibo.set_data(self.notes_render.as_slice(), glow::DYNAMIC_DRAW);
                            self.gl.use_program(Some(self.pr_notes_program.program));
                            self.gl.draw_elements_instanced(
                                glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, note_id as i32);
                        }
                    }

                    self.last_time = nav_ticks_offs;
                }

                self.gl.use_program(None);
            }
        }
    }

    fn set_ghost_notes(&mut self, notes: Arc<Mutex<Vec<GhostNote>>>) {
        self.ghost_notes = Some(notes);
    }

    fn clear_ghost_notes(&mut self) {
        self.ghost_notes = None;
    }

    fn window_size(&mut self, size: Vec2) {
        self.window_size = size;
    }

    /*fn time_changed(&mut self, time: u64) {
        //if self.last_time > time { return; }
        self.last_time = time as f32;
        /*{
            let note = self.render_notes.lock().unwrap();
            self.first_render_note = vec![vec![0; 16]; note.len()];
            self.last_note_start = vec![vec![0; 16]; note.len()];
        }*/
    }*/

    fn set_selected(&mut self, selected_ids: Arc<Mutex<Vec<usize>>>) {
        self.selected = selected_ids;
    }

    fn set_active(&mut self, is_active: bool) {
        self.render_active = is_active;
    }
}