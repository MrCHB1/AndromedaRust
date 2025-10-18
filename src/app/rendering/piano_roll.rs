use crate::app::rendering::note_cull_helper::NoteCullHelper;
use crate::app::rendering::Renderer;
use crate::app::shared::NoteColors;
use crate::audio::event_playback::PlaybackManager;
use crate::editor::midi_bar_cacher::BarCacher;
use crate::editor::project_data::ProjectData;
use eframe::egui::Vec2;
use eframe::glow;
use eframe::glow::HasContext;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::{Arc, Mutex, RwLock};
use crate::editor::editing::note_editing::GhostNote;
use crate::app::rendering::{
    buffers::*,
    shaders::*
};
use crate::app::view_settings::{VS_PianoRoll_OnionColoring, VS_PianoRoll_OnionState, ViewSettings};
use crate::editor::navigation::PianoRollNavigation;
use crate::midi::events::note::Note;
use crate::set_attribute;

const NOTE_BUFFER_SIZE: usize = 8192;
// const NOTE_BORDER_DARKNESS: f32 = 0.5;

// Piano Roll Background
pub type BarStart = f32;
pub type BarLength = f32;
pub type BarNumber = u32;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct RenderPianoRollBar(BarStart, BarLength, BarNumber);

// Piano Roll Notes
pub type NoteRect = [f32; 4]; // (start, length, note bottom, note top)
pub type NoteMeta = u32; // first byte is note color index. 9th bit is the note selected bit, and 10th bit is note playing bit

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct RenderPianoRollNote(NoteRect, NoteMeta);

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
    0, 1, 2,
    0, 2, 3
];

pub struct PianoRollRenderer {
    pub navigation: Arc<Mutex<PianoRollNavigation>>,
    pub playback_manager: Arc<Mutex<PlaybackManager>>,
    pub view_settings: Arc<Mutex<ViewSettings>>,
    pub bar_cacher: Arc<Mutex<BarCacher>>,
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
    notes_render: [RenderPianoRollNote; NOTE_BUFFER_SIZE],
    render_notes: Arc<RwLock<Vec<Vec<Note>>>>,
    note_colors: Arc<Mutex<NoteColors>>,

    note_cull_helper: Arc<Mutex<NoteCullHelper>>,

    pub ghost_notes: Option<Arc<Mutex<Vec<GhostNote>>>>,
    pub selected: HashSet<usize>,
    render_active: bool,
}

impl PianoRollRenderer {
    pub unsafe fn new(
        project_data: &Rc<RefCell<ProjectData>>,
        view_settings: &Arc<Mutex<ViewSettings>>,
        nav: &Arc<Mutex<PianoRollNavigation>>,
        gl: &Arc<glow::Context>,
        playback_manager: &Arc<Mutex<PlaybackManager>>,
        bar_cacher: &Arc<Mutex<BarCacher>>,
        colors: &Arc<Mutex<NoteColors>>,
        note_cull_helper: &Arc<Mutex<NoteCullHelper>>,
    ) -> Self {
        let pr_program = ShaderProgram::create_from_files(gl.clone(), "./assets/shaders/piano_roll_bg");
        let pr_notes_program = ShaderProgram::create_from_files(gl.clone(), "./assets/shaders/piano_roll_note");

        // -------- PIANO ROLL BAR --------

        let pr_vertex_buffer = Buffer::new(gl.clone(), glow::ARRAY_BUFFER);
        pr_vertex_buffer.set_data(&QUAD_VERTICES, glow::STATIC_DRAW);

        let pr_index_buffer = Buffer::new(gl.clone(), glow::ELEMENT_ARRAY_BUFFER);
        pr_index_buffer.set_data(&QUAD_INDICES, glow::STATIC_DRAW);

        let pr_vertex_array = VertexArray::new(gl.clone());
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

        gl.vertex_attrib_divisor(0, 1);
        gl.vertex_attrib_divisor(1, 1);
        gl.vertex_attrib_divisor(2, 1);

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
                1: 0
            }; NOTE_BUFFER_SIZE
        ];
        pr_notes_ibo.set_data(pr_notes_render.as_slice(), glow::DYNAMIC_DRAW);

        let pr_note_rect = pr_notes_program.get_attrib_location("noteRect").unwrap();
        set_attribute!(glow::FLOAT, pr_notes_vao, pr_note_rect, RenderPianoRollNote::0);
        let pr_note_meta = pr_notes_program.get_attrib_location("noteMeta").unwrap();
        set_attribute!(glow::UNSIGNED_INT, pr_notes_vao, pr_note_meta, RenderPianoRollNote::1);

        gl.vertex_attrib_divisor(1, 1);
        gl.vertex_attrib_divisor(2, 1);

        /*let mut pr_notes_color_tex = Texture::new(gl.clone(), glow::TEXTURE_2D);
        pr_notes_color_tex.bind();
        pr_notes_color_tex.set_wrapping(glow::REPEAT);
        pr_notes_color_tex.set_filtering(glow::NEAREST);
        pr_notes_color_tex.load_texture("./shaders/textures/notes.png", 16, 1);*/

        let notes = {
            let project_data = project_data.borrow();
            project_data.notes.clone()
        };

        Self {
            playback_manager: playback_manager.clone(),
            navigation: nav.clone(),
            view_settings: view_settings.clone(),
            bar_cacher: bar_cacher.clone(),
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

            gl: gl.clone(),
            bars_render: pr_bars_render.to_vec(),
            notes_render: pr_notes_render,
            render_notes: notes,

            ppq: 960,
            note_colors: colors.clone(),

            note_cull_helper: note_cull_helper.clone(),

            ghost_notes: None,
            selected: HashSet::new(),
            render_active: false
        }
    }

    fn get_time(&self) -> f32 {
        let nav = self.navigation.lock().unwrap();

        /*let nav_ticks = {
            let playback_manager = self.playback_manager.lock().unwrap();
            if playback_manager.playing {
                playback_manager.get_playback_ticks() as f32
            } else {
                nav.tick_pos_smoothed
            }
        };*/

        nav.tick_pos_smoothed
    }
}

impl Renderer for PianoRollRenderer {
    fn draw(&mut self) {
        if !self.render_active { return; }

        unsafe {
            let tick_pos = self.get_time();

            let (zoom_ticks, key_pos, zoom_keys) = {
                let nav = self.navigation.lock().unwrap();
                (nav.zoom_ticks_smoothed, nav.key_pos_smoothed, nav.zoom_keys_smoothed)
            };
            
            let nav_curr_track = {
                let nav = self.navigation.lock().unwrap();
                nav.curr_track
            };

            let (is_playing, playback_pos) = {
                let playback_manager = self.playback_manager.lock().unwrap();
                /*let mut view_offset = self.last_view_offset;
                if playback_manager.playing && !self.started_playing {
                    let nav = self.navigation.lock().unwrap();
                    view_offset = nav.tick_pos_smoothed - playback_manager.playback_start_pos as f32;
                    self.last_view_offset = view_offset;
                    self.last_zoom = zoom_ticks;
                    self.started_playing = true;
                } else if !playback_manager.playing {
                    self.started_playing = false;
                    self.last_view_offset = 0.0;
                }

                view_offset = if self.last_zoom > 0.0 {
                    view_offset * (zoom_ticks / self.last_zoom)
                } else {
                    view_offset
                };
                
                (playback_manager.playing, view_offset)*/
                (playback_manager.playing, playback_manager.get_playback_ticks() as f32)
            };

            let tick_pos_offs = tick_pos;

            // RENDER BARS
            {
                self.gl.use_program(Some(self.pr_program.program));

                let mut curr_bar_tick = 0.0;
                let mut bar_id = 0;
                let mut bar_num = 0;
                {
                    let key_start = key_pos;
                    let key_end = key_pos + zoom_keys;

                    self.pr_program.set_float("prBarBottom", -key_start / (key_end - key_start));
                    self.pr_program.set_float("prBarTop", (128.0 - key_start) / (key_end - key_start));
                    self.pr_program.set_float("width", self.window_size.x);
                    self.pr_program.set_float("height", self.window_size.y);
                    self.pr_program.set_float("ppqNorm", self.ppq as f32 / zoom_ticks);
                    self.pr_program.set_float("keyZoom", zoom_keys / 128.0);

                    // bind before loop
                    self.pr_vertex_array.bind();
                    self.pr_instance_buffer.bind();
                    self.pr_vertex_buffer.bind();
                    self.pr_index_buffer.bind();

                    while curr_bar_tick < zoom_ticks + tick_pos_offs {
                        let (bar_tick, bar_length) = {
                            let mut bar_cacher = self.bar_cacher.lock().unwrap();
                            let interval = bar_cacher.get_bar_interval(bar_num);
                            interval
                        };
                        // let (bar_tick, bar_length) = (bar_num * self.ppq as u32 * 4, self.ppq as u32 * 4);

                        if ((bar_tick + bar_length) as f32) < tick_pos_offs {
                            curr_bar_tick += bar_length as f32;
                            bar_num += 1;
                            continue;
                        }

                        self.bars_render[bar_id] = RenderPianoRollBar {
                            0: ((curr_bar_tick - tick_pos_offs) / zoom_ticks),
                            1: (bar_length as f32 / zoom_ticks),
                            2: bar_num as u32
                        };
                        bar_id += 1;
                        if bar_id >= 32 {
                            self.pr_instance_buffer.set_data(self.bars_render.as_slice(), glow::DYNAMIC_DRAW);
                            self.gl.draw_elements_instanced(
                                glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, 32);
                            bar_id = 0;
                        }

                        curr_bar_tick += bar_length as f32;
                        bar_num += 1;
                    }
                }

                if bar_id != 0 {
                    self.pr_instance_buffer.set_data(self.bars_render.as_slice(), glow::DYNAMIC_DRAW);
                    self.gl.draw_elements_instanced(
                            glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, bar_id as i32);
                }

                self.gl.use_program(None);
            }

            // RENDER NOTES
            {
                self.gl.use_program(Some(self.pr_notes_program.program));

                {
                    let mut note_colors = self.note_colors.lock().unwrap();

                    let all_render_notes = self.render_notes.read().unwrap();
                    // resize last_note_start and first_render_note if notes changed size
                    /*if self.last_note_start.len() != all_render_notes.len() {
                        self.last_note_start = vec![0; all_render_notes.len()];
                        self.first_render_note = vec![0; all_render_notes.len()];
                    }*/

                    self.gl.active_texture(glow::TEXTURE0);
                    note_colors.get_texture().bind();

                    // self.pr_notes_color_tex.bind();
                    self.pr_notes_program.set_int("noteColorTexture", 0);
                    self.pr_notes_program.set_float("width", self.window_size.x);
                    self.pr_notes_program.set_float("height", self.window_size.y);

                    // iterate through each track, and each channel
                    {
                        let view_settings = self.view_settings.lock().unwrap();

                        //let mut view_all_tracks = true;
                        
                        let tracks_to_iter = match view_settings.pr_onion_state {
                            VS_PianoRoll_OnionState::NoOnion => {
                                &all_render_notes[nav_curr_track as usize..nav_curr_track as usize]
                            },
                            VS_PianoRoll_OnionState::ViewAll => {
                                //view_all_tracks = true;
                                &all_render_notes[..]
                            },
                            VS_PianoRoll_OnionState::ViewPrevious => {
                                //view_all_tracks = false;
                                if nav_curr_track == 0 { &all_render_notes[nav_curr_track as usize..nav_curr_track as usize] }
                                else { &all_render_notes[(nav_curr_track - 1) as usize..=(nav_curr_track as usize)] }
                            },
                            VS_PianoRoll_OnionState::ViewNext => {
                                if nav_curr_track == (all_render_notes.len() - 1) as u16 { &all_render_notes[nav_curr_track as usize..nav_curr_track as usize] }
                                else { &all_render_notes[(nav_curr_track) as usize..=(nav_curr_track + 1) as usize] }
                            }
                        };

                        let onion_track_color_meta = match view_settings.pr_onion_coloring {
                            VS_PianoRoll_OnionColoring::FullColor => { 0b00 },
                            VS_PianoRoll_OnionColoring::PartialColor => { 0b01 },
                            VS_PianoRoll_OnionColoring::GrayedOut => { 0b10 }
                        };

                        {
                            let mut note_id = 0;
                            let mut curr_track = 0;

                            // bind before rendering all notes
                            self.pr_notes_vao.bind();
                            self.pr_notes_ibo.bind();
                            self.pr_notes_vbo.bind();
                            self.pr_notes_ebo.bind();

                            let mut note_culler = self.note_cull_helper.lock().unwrap();
                            note_culler.sync_cull_array_lengths();
                            // note_culler.update_cull(tick_pos, zoom_ticks);

                            // 1. draw all notes that is not the current track
                            for notes in tracks_to_iter {
                                // skip track if it has nothing or its the navigation's current track
                                if notes.is_empty() || curr_track == nav_curr_track {
                                    curr_track += 1;
                                    continue;
                                }

                                note_culler.update_cull_for_track(curr_track, tick_pos_offs, zoom_ticks, false);
                                let (note_start, mut note_end) = note_culler.get_track_cull_range(curr_track);
                                let mut n_off = note_start;

                                let mut curr_note = 0;
                                
                                if note_end > notes.len() { 
                                    note_culler.update_cull_for_track(curr_track, tick_pos_offs, zoom_ticks, true);
                                    (n_off, note_end) = note_culler.get_track_cull_range(curr_track);
                                }

                                for note in &notes[n_off..note_end] {
                                    if note.key() as f32 + 1.0 < key_pos || note.key() as f32 > key_pos + zoom_keys {
                                        curr_note += 1;
                                        continue;
                                    }

                                    let trk_chan = ((curr_track as usize) << 4) | (note.channel() as usize);

                                    {
                                        let note_bottom = (note.key() as f32 - key_pos) / zoom_keys;
                                        let note_top = ((note.key() as f32 + 1.0) - key_pos) / zoom_keys;

                                        let highlight_note_play_size = zoom_ticks * 0.001;
                                        let note_playing = (note.start() as f32) < playback_pos + highlight_note_play_size
                                            && (note.end() as f32) > playback_pos - highlight_note_play_size
                                            && is_playing;

                                        self.notes_render[note_id] = RenderPianoRollNote {
                                            0: [(note.start as f32 - tick_pos_offs) / zoom_ticks,
                                                (note.length as f32) / zoom_ticks,
                                                (note_bottom),
                                                (note_top)],
                                            1: {
                                                let mut note_meta = note_colors.get_index(trk_chan) as u32;
                                                note_meta |= (note.velocity() as u32) << 4;
                                                if note_playing {
                                                    note_meta |= 1 << 12;
                                                }

                                                note_meta |= onion_track_color_meta << 14;
                                                note_meta
                                                /* let color = self.note_colors.get_and_mix(trk_chan, &WHITE, 1.0 - (note.velocity() as f32 / 128.0));
                                                
                                                if note_playing {
                                                    [color[0] + 0.5, color[1] + 0.5, color[2] + 0.5]
                                                } else {
                                                    color
                                                } */
                                                //let color = self.note_colors.get_and_mix(curr_channel);
                                            },
                                            /*2: {
                                                // let mut color = self.note_colors[curr_channel as usize % self.note_colors.len()];
                                                // color = [color[0] / 128.0 * (127 - note.velocity) as f32, color[1] / 128.0 * (127 - note.velocity) as f32, color[2] / 128.0 * (127 - note.velocity) as f32];
                                                let color = self.note_colors.get_and_mix(trk_chan, &BLACK, NOTE_BORDER_DARKNESS);

                                                if note_playing {
                                                    [color[0] + 0.5, color[1] + 0.5, color[2] + 0.5]
                                                } else{
                                                    color
                                                }
                                            }*/
                                        };
                                    }

                                    note_id += 1;

                                    if note_id >= NOTE_BUFFER_SIZE {
                                        self.pr_notes_ibo.set_data(self.notes_render.as_slice(), glow::DYNAMIC_DRAW);

                                        self.gl.use_program(Some(self.pr_notes_program.program));
                                        self.gl.draw_elements_instanced(
                                            glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, NOTE_BUFFER_SIZE as i32);
                                        // rendered_notes += note_id;
                                        note_id = 0;
                                    }

                                    curr_note += 1;
                                    if curr_note >= notes.len() {
                                        break;
                                    }
                                }
                                
                                curr_track += 1;
                            }

                            // 2. draw current track on top
                            let notes = &all_render_notes[nav_curr_track as usize];

                            if !notes.is_empty() {
                                let mut curr_note = 0;

                                note_culler.update_cull_for_track(nav_curr_track, tick_pos_offs, zoom_ticks, false);
                                let (note_start, note_end) = note_culler.get_track_cull_range(nav_curr_track);
                                let n_off = note_start;
                                let mut note_idx = n_off;
                                /*let mut n_off = self.first_render_note[nav_curr_track as usize];
                                if self.last_time > tick_pos_offs {
                                    if n_off == 0 {
                                        for note in &notes[0..notes.len()] {
                                            if note.end() as f32 > tick_pos_offs { break; }
                                            n_off += 1;
                                        }
                                    } else { // backwards instead of forwards
                                        for note in notes[0..n_off].iter().rev() {
                                            if (note.end() as f32) <= tick_pos_offs { break; }
                                            n_off -= 1;
                                        }
                                    }
                                    self.first_render_note[nav_curr_track as usize] = n_off;
                                } else if self.last_time < tick_pos_offs {
                                    for note in &notes[n_off..notes.len()] {
                                        if note.end() as f32 > tick_pos_offs { break; }
                                        n_off += 1;
                                    }
                                    self.first_render_note[nav_curr_track as usize] = n_off;
                                }

                                let mut note_idx = n_off;

                                let note_end = {
                                    let mut e = n_off;
                                    for note in &notes[n_off..notes.len()] {
                                        if note.start() as f32 > tick_pos_offs + zoom_ticks { break; }
                                        e += 1;
                                    }
                                    e
                                };*/
                                for note in &notes[n_off..note_end] {
                                    if note.key() as f32 + 1.0 < key_pos || (note.key() as f32) > key_pos + zoom_keys {
                                        curr_note += 1;
                                        note_idx += 1;
                                        continue;
                                    }

                                    let trk_chan = ((nav_curr_track as usize) << 4) | (note.channel() as usize);

                                    {
                                        let note_bottom = (note.key as f32 - key_pos) / zoom_keys;
                                        let note_top = ((note.key as f32 + 1.0) - key_pos) / zoom_keys;

                                        let highlight_note_play_size = zoom_ticks * 0.001;
                                        let note_playing = (note.start() as f32) < playback_pos + highlight_note_play_size
                                            && (note.end() as f32) > playback_pos - highlight_note_play_size
                                            && is_playing;
                

                                        self.notes_render[note_id] = RenderPianoRollNote {
                                            0: [(note.start as f32 - tick_pos_offs) / zoom_ticks,
                                                (note.length as f32) / zoom_ticks,
                                                (note_bottom),
                                                (note_top)],
                                            1: {
                                                let mut note_meta = note_colors.get_index(trk_chan) as u32;
                                                note_meta |= (note.velocity() as u32) << 4;
                                                if note_playing {
                                                    note_meta |= 1 << 12;
                                                }

                                                if self.selected.contains(&note_idx) {
                                                    note_meta |= 1 << 13;
                                                }

                                                note_meta
                                            },
                                        };
                                    }

                                    note_id += 1;
                                    note_idx += 1;

                                    // flush if note_id is now the note draw buffer size and reset it to zero
                                    if note_id >= NOTE_BUFFER_SIZE {
                                        self.pr_notes_ibo.set_data(self.notes_render.as_slice(), glow::DYNAMIC_DRAW);

                                        self.gl.use_program(Some(self.pr_notes_program.program));
                                        self.gl.draw_elements_instanced(
                                            glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, NOTE_BUFFER_SIZE as i32);
                                        note_id = 0;
                                    }

                                    curr_note += 1;
                                    if curr_note >= notes.len() {
                                        break;
                                    }
                                }
                            }
                            
                            // for notes in notes_curr_track {
                                // skip channel if its empty
                                /*if notes.is_empty() { curr_channel += 1; continue; }

                                let mut curr_note = 0;

                                let mut n_off = self.first_render_note[nav_curr_track as usize][curr_channel];
                                if self.last_time > tick_pos_offs {
                                    if n_off == 0 {
                                        for note in &notes[0..notes.len()] {
                                            if (note.start + note.length) as f32 > tick_pos_offs { break; }
                                            n_off += 1;
                                        }
                                    } else { // backwards instead of forwards
                                        for note in notes[0..n_off].iter().rev() {
                                            if ((note.start + note.length) as f32) <= tick_pos_offs { break; }
                                            n_off -= 1;
                                        }
                                    }
                                    self.first_render_note[nav_curr_track as usize][curr_channel] = n_off;
                                } else if self.last_time < tick_pos_offs {
                                    for note in &notes[n_off..notes.len()] {
                                        if (note.start + note.length) as f32 > tick_pos_offs { break; }
                                        n_off += 1;
                                    }
                                    self.first_render_note[nav_curr_track as usize][curr_channel] = n_off;
                                }

                                let mut note_idx = n_off;

                                let note_end = {
                                    let mut e = n_off;
                                    for note in &notes[n_off..notes.len()] {
                                        if (note.start as f32) > tick_pos_offs + zoom_ticks { break; }
                                        e += 1;
                                    }
                                    e
                                };
                                
                                let trk_chan = ((nav_curr_track as usize) << 4) | curr_channel;
                                
                                for note in &notes[n_off..note_end] {
                                    //n_off += 1;
                                    //if n_off == notes.len() { break; }

                                    if note.key as f32 + 1.0 < key_pos || (note.key as f32) > key_pos + zoom_keys {
                                        curr_note += 1;
                                        note_idx += 1;
                                        continue;
                                    }

                                    // if note.start as f32 > nav.tick_pos_smoothed + nav.zoom_ticks_smoothed { break; }
                                    {
                                        let sel_lock = self.selected.lock().unwrap();
                                        let note_bottom = (note.key as f32 - key_pos) / zoom_keys;
                                        let note_top = ((note.key as f32 + 1.0) - key_pos) / zoom_keys;

                                        let highlight_note_play_size = zoom_ticks * 0.001;
                                        let note_playing = (note.start as f32) < tick_pos + highlight_note_play_size
                                            && (note.start as f32 + note.length as f32) > tick_pos - highlight_note_play_size
                                            && is_playing;
                

                                        self.notes_render[note_id] = RenderPianoRollNote {
                                            0: [(note.start as f32 - tick_pos_offs) / zoom_ticks,
                                                (note.length as f32) / zoom_ticks,
                                                (note_bottom),
                                                (note_top)],
                                            1: {
                                                let mut color = if sel_lock.contains(&note_idx) {
                                                    SELECTED
                                                } else {
                                                    self.note_colors.get_and_mix(trk_chan, &WHITE, 1.0 - (note.velocity() as f32 / 128.0))
                                                };

                                                if note_playing {
                                                    [color[0] + 0.5, color[1] + 0.5, color[2] + 0.5]
                                                } else {
                                                    color
                                                }
                                            },
                                            2: {
                                                let color = self.note_colors.get_and_mix(trk_chan, &BLACK, note.velocity() as f32 / 128.0);
                                                if note_playing {
                                                    [color[0] + 0.5, color[1] + 0.5, color[2] + 0.5]
                                                } else {
                                                    color
                                                }
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

                                curr_channel += 1;*/
                            // }
                            
                            // 3. flush remaining notes
                            if note_id != 0 {
                                
                                self.pr_notes_ibo.set_data(self.notes_render.as_slice(), glow::DYNAMIC_DRAW);

                                self.gl.use_program(Some(self.pr_notes_program.program));
                                self.gl.draw_elements_instanced(
                                    glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, note_id as i32);
                                // rendered_notes += note_id;
                            }
                        }
                    }

                    if let Some(ghost_notes) = &self.ghost_notes {
                        let mut note_id = 0;
                        let notes = ghost_notes.lock().unwrap();

                        self.pr_notes_vao.bind();
                        self.pr_notes_ibo.bind();
                        self.pr_notes_vbo.bind();
                        self.pr_notes_ebo.bind();

                        for note in notes.iter() {
                            let note = note.get_note();
                            let note_bottom = (note.key as f32 - key_pos) / zoom_keys;
                            let note_top = ((note.key as f32 + 1.0) - key_pos) / zoom_keys;

                            let trk_chan = ((nav_curr_track as usize) << 4) | (note.channel() as usize);

                            self.notes_render[note_id] = RenderPianoRollNote {
                                0: [(note.start as f32 - tick_pos) / zoom_ticks,
                                    (note.length as f32) / zoom_ticks,
                                    (note_bottom),
                                    (note_top)],
                                1: {
                                    let mut note_meta = note_colors.get_index(trk_chan) as u32;
                                    note_meta |= (note.velocity() as u32) << 4;
                                    note_meta
                                    // self.note_colors.get_and_mix(trk_chan, &WHITE, 1.0 - (note.velocity() as f32 / 128.0))
                                },
                                /* 2: {
                                    self.note_colors.get_and_mix(trk_chan, &BLACK, NOTE_BORDER_DARKNESS)
                                } */
                            };
                            note_id += 1;
                            if note_id >= NOTE_BUFFER_SIZE {
                                self.pr_notes_ibo.set_data(self.notes_render.as_slice(), glow::DYNAMIC_DRAW);
                                self.gl.use_program(Some(self.pr_notes_program.program));
                                self.gl.draw_elements_instanced(
                                    glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, NOTE_BUFFER_SIZE as i32);
                                note_id = 0;
                            }
                        }

                        if note_id != 0 {
                            self.pr_notes_ibo.set_data(self.notes_render.as_slice(), glow::DYNAMIC_DRAW);
                            self.gl.use_program(Some(self.pr_notes_program.program));
                            self.gl.draw_elements_instanced(
                                glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, note_id as i32);
                        }
                    }

                    // self.last_time = tick_pos_offs;
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

    fn update_ppq(&mut self, ppq: u16) {
        self.ppq = ppq;
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

    fn set_selected(&mut self, selected_ids: &Arc<Mutex<Vec<usize>>>) {
        let sel = selected_ids.lock().unwrap();
        self.selected = HashSet::from_iter((*sel).clone());
    }

    fn set_active(&mut self, is_active: bool) {
        self.render_active = is_active;
    }
}