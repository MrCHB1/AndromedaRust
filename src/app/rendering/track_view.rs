use crate::audio::event_playback::PlaybackManager;
use crate::editor::midi_bar_cacher::BarCacher;
use crate::editor::navigation::TrackViewNavigation;
use crate::editor::project_data::ProjectData;
use crate::midi::events::meta_event::MetaEvent;
use crate::midi::events::note::Note;
use std::sync::{Arc, Mutex, RwLock};
use eframe::egui::Vec2;
use eframe::glow;
use eframe::glow::HasContext;

use crate::app::rendering::{
    buffers::*,
    shaders::*, Renderer
};

use crate::set_attribute;

const NOTE_BUFFER_SIZE: usize = 4096;
const BAR_BUFFER_SIZE: usize = 32;

// track view background
pub type BarStart = f32;
pub type BarLength = f32;
pub type BarNumber = u32;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct RenderTrackViewBar(BarStart, BarLength, BarNumber);

// track view notes
pub type NoteRect = [f32; 4]; // (start, length, note bottom, note top)
pub type NoteColor = [f32; 3];

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct RenderTrackViewNote(NoteRect, NoteColor);

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

pub struct TrackViewRenderer {
    pub navigation: Arc<Mutex<TrackViewNavigation>>,
    pub playback_manager: Arc<Mutex<PlaybackManager>>,
    pub bar_cacher: Arc<Mutex<BarCacher>>,
    pub window_size: Vec2<>,
    pub ppq: u16,

    tv_program: ShaderProgram,
    tv_vertex_buffer: Buffer,
    tv_vertex_array: VertexArray,
    tv_instance_buffer: Buffer,
    tv_index_buffer: Buffer,

    tv_notes_program: ShaderProgram,
    tv_notes_vbo: Buffer,
    tv_notes_vao: VertexArray,
    tv_notes_ibo: Buffer,
    tv_notes_ebo: Buffer,

    gl: Arc<glow::Context>,

    bars_render: Vec<RenderTrackViewBar>,
    notes_render: Vec<RenderTrackViewNote>,
    render_notes: Arc<RwLock<Vec<Vec<Vec<Note>>>>>,
    global_metas: Arc<Mutex<Vec<MetaEvent>>>,
    note_colors: Vec<[f32; 3]>,

    // per channel per track
    last_note_start: Vec<Vec<usize>>,
    first_render_note: Vec<Vec<usize>>,
    last_time: f32,

    render_active: bool
}

impl TrackViewRenderer {
    pub unsafe fn new(
        project_data: &Arc<Mutex<ProjectData>>,
        nav: Arc<Mutex<TrackViewNavigation>>,
        gl: Arc<glow::Context>,
        playback_manager: &Arc<Mutex<PlaybackManager>>,
        bar_cacher: &Arc<Mutex<BarCacher>>
    ) -> Self {
        let tv_program = ShaderProgram::create_from_files(gl.clone(), "./shaders/track_view_bg");
        let tv_notes_program = ShaderProgram::create_from_files(gl.clone(), "./shaders/track_view_note");

        // -------- TRACK VIEW BAR --------

        let tv_vertex_buffer = Buffer::new(gl.clone(), glow::ARRAY_BUFFER);
        tv_vertex_buffer.set_data(&QUAD_VERTICES, glow::STATIC_DRAW);

        let tv_index_buffer = Buffer::new(gl.clone(), glow::ELEMENT_ARRAY_BUFFER);
        tv_index_buffer.set_data(&QUAD_INDICES, glow::STATIC_DRAW);

        let tv_vertex_array = VertexArray::new(gl.clone());
        let pos_attrib = tv_program.get_attrib_location("vPos").unwrap();
        set_attribute!(glow::FLOAT, tv_vertex_array, pos_attrib, Vertex::0);

        let tv_instance_buffer = Buffer::new(gl.clone(), glow::ARRAY_BUFFER);
        let tv_bars_render = [
            RenderTrackViewBar {
                0: 0.0,
                1: 1.0,
                2: 0
            }; BAR_BUFFER_SIZE
        ];
        tv_instance_buffer.set_data(tv_bars_render.as_slice(), glow::DYNAMIC_DRAW);

        let tv_bar_start = tv_program.get_attrib_location("barStart").unwrap();
        set_attribute!(glow::FLOAT, tv_vertex_array, tv_bar_start, RenderTrackViewBar::0);
        let tv_bar_length = tv_program.get_attrib_location("barLength").unwrap();
        set_attribute!(glow::FLOAT, tv_vertex_array, tv_bar_length, RenderTrackViewBar::1);
        let tv_bar_number = tv_program.get_attrib_location("barNumber").unwrap();
        set_attribute!(glow::UNSIGNED_INT, tv_vertex_array, tv_bar_number, RenderTrackViewBar::2);

        gl.vertex_attrib_divisor(1, 1);
        gl.vertex_attrib_divisor(2, 1);
        gl.vertex_attrib_divisor(3, 1);

        // -------- TRACK VIEW NOTES --------

        let tv_notes_vbo = Buffer::new(gl.clone(), glow::ARRAY_BUFFER);
        tv_notes_vbo.set_data(&QUAD_VERTICES, glow::STATIC_DRAW);

        let tv_notes_ebo = Buffer::new(gl.clone(), glow::ELEMENT_ARRAY_BUFFER);
        tv_notes_ebo.set_data(&QUAD_INDICES, glow::STATIC_DRAW);

        let tv_notes_vao = VertexArray::new(gl.clone());
        // let pos_attrib = pr_notes_program.get_attrib_location("vPos").unwrap();
        set_attribute!(glow::FLOAT, tv_notes_vao, 0, Vertex::0);

        let tv_notes_ibo = Buffer::new(gl.clone(), glow::ARRAY_BUFFER);
        let tv_notes_render = vec![
            RenderTrackViewNote {
                0: [0.0, 1.0, 0.0, 1.0],
                1: [1.0, 0.0, 0.0]
            }; NOTE_BUFFER_SIZE
        ];
        tv_notes_ibo.set_data(tv_notes_render.as_slice(), glow::DYNAMIC_DRAW);

        let tv_note_rect = tv_notes_program.get_attrib_location("noteRect").unwrap();
        set_attribute!(glow::FLOAT, tv_notes_vao, tv_note_rect, RenderTrackViewNote::0);
        let tv_note_color = tv_notes_program.get_attrib_location("noteColor").unwrap();
        set_attribute!(glow::FLOAT, tv_notes_vao, tv_note_color, RenderTrackViewNote::1);

        gl.vertex_attrib_divisor(1, 1);
        gl.vertex_attrib_divisor(2, 1);

        let (notes, global_metas) = {
            let project_data = project_data.lock().unwrap();
            (project_data.notes.clone(), project_data.global_metas.clone())
        };

        let last_note_start = {
            let notes = notes.read().unwrap();
            vec![vec![0usize; 16]; notes.len()]
        };

        let first_render_note = {
            let notes = notes.read().unwrap();
            vec![vec![0usize; 16]; notes.len()]
        };

        Self {
            navigation: nav,
            window_size: Vec2::new(0.0, 0.0),
            playback_manager: playback_manager.clone(),
            bar_cacher: bar_cacher.clone(),
            tv_program,
            tv_vertex_buffer,
            tv_vertex_array,
            tv_instance_buffer,
            tv_index_buffer,

            tv_notes_program,
            tv_notes_vao,
            tv_notes_vbo,
            tv_notes_ebo,
            tv_notes_ibo,

            gl,
            bars_render: tv_bars_render.to_vec(),
            notes_render: tv_notes_render.to_vec(),
            render_notes: notes,
            global_metas,

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
            render_active: false,
            last_time: 0.0
        }
    }

    fn get_time(&self) -> f32 {
        let nav = self.navigation.lock().unwrap();

        let is_playing = {
            let playback_manager = self.playback_manager.lock().unwrap();
            playback_manager.playing
        };

        let nav_ticks = {
            let mut playback_manager = self.playback_manager.lock().unwrap();
            if is_playing {
                playback_manager.get_playback_ticks() as f32
            } else {
                nav.tick_pos_smoothed
            }
        };

        nav_ticks
    }
}

impl Renderer for TrackViewRenderer {
    fn draw(&mut self) {
        if !self.render_active { return; }
        unsafe {
            let tick_pos = self.get_time();

            let (zoom_ticks, track_pos, zoom_tracks) = {
                let nav = self.navigation.lock().unwrap();
                (nav.zoom_ticks_smoothed, nav.track_pos_smoothed, nav.zoom_tracks_smoothed)
            };

            // RENDER BARS
            {
                self.gl.use_program(Some(self.tv_program.program));

                // render from top to bottom
                let mut curr_track = 0;
                
                while (curr_track as f32) < track_pos + zoom_tracks {
                    let mut curr_bar_tick = 0.0;
                    let mut bar_num = 0;
                    let mut bar_id = 0;

                    let num_bars = zoom_tracks;

                    let bar_top = (zoom_tracks - curr_track as f32) + track_pos;
                    let bar_bottom = (zoom_tracks - curr_track as f32 - 1.0) + track_pos;

                    self.tv_program.set_float("width", self.window_size.x);
                    self.tv_program.set_float("height", self.window_size.y);
                    self.tv_program.set_float("tvBarTop", bar_top / num_bars);
                    self.tv_program.set_float("tvBarBottom", bar_bottom / num_bars);
                    self.tv_program.set_float("ppqNorm", self.ppq as f32 / zoom_ticks);

                    while curr_bar_tick <= zoom_ticks + tick_pos {
                        // TODO: proper bar position calculation because of signature change events
                        let (bar_tick, bar_length) = {
                            let mut bar_cacher = self.bar_cacher.lock().unwrap();
                            let interval = bar_cacher.get_bar_interval(bar_num);
                            interval
                        };

                        if ((bar_tick + bar_length) as f32) < tick_pos{
                            curr_bar_tick += bar_length as f32;
                            bar_num += 1;
                            continue;
                        }
                        
                        self.bars_render[bar_id] = RenderTrackViewBar {
                            0: ((curr_bar_tick - tick_pos) / zoom_ticks),
                            1: ((bar_length as f32) / zoom_ticks),
                            2: bar_num as u32
                        };

                        bar_id += 1;
                        if bar_id >= BAR_BUFFER_SIZE {
                            self.tv_vertex_array.bind();
                            self.tv_instance_buffer.bind();
                            self.tv_vertex_buffer.bind();
                            self.tv_index_buffer.bind();
                            self.tv_instance_buffer.set_data(self.bars_render.as_slice(), glow::DYNAMIC_DRAW);
                            // self.gl.use_program(Some(self.tv_program.program));
                            self.gl.draw_elements_instanced(
                                glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, BAR_BUFFER_SIZE as i32);
                            bar_id = 0;
                        }

                        curr_bar_tick += bar_length as f32;
                        bar_num += 1;
                    }

                    if bar_id != 0 {
                        self.tv_vertex_array.bind();
                        self.tv_instance_buffer.bind();
                        self.tv_vertex_buffer.bind();
                        self.tv_index_buffer.bind();
                        self.tv_instance_buffer.set_data(self.bars_render.as_slice(), glow::DYNAMIC_DRAW);
                        // self.gl.use_program(Some(self.tv_program.program));
                        self.gl.draw_elements_instanced(
                            glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, bar_id as i32);
                    }

                    curr_track += 1;
                }

                self.gl.use_program(None);
            }

            // RENDER NOTES
            {
                self.gl.use_program(Some(self.tv_notes_program.program));
                self.tv_notes_program.set_float("width", self.window_size.x);
                self.tv_notes_program.set_float("height", self.window_size.y);

                let all_render_notes = self.render_notes.read().unwrap();

                if self.last_note_start.len() != all_render_notes.len() {
                    self.last_note_start = vec![vec![0usize; 16]; all_render_notes.len()];
                    self.first_render_note = vec![vec![0usize; 16]; all_render_notes.len()];
                }
                
                let track_start = {
                    let mut track_start = 0;
                    for track in 0..all_render_notes.len() {
                        if track as f32 >= track_pos { break; }
                        track_start += 1;
                    }
                    track_start
                };

                let track_end = {
                    let mut track_end = track_start;
                    for track in track_start..all_render_notes.len() {
                        if track as f32 >= track_pos + zoom_tracks { break; }
                        track_end += 1;
                    }
                    track_end
                };

                let mut note_id = 0;
                let mut curr_track = track_start;

                for note_track in &all_render_notes[track_start..track_end] {
                    if note_track.is_empty() {
                        curr_track += 1;
                        continue;
                    }

                    let mut curr_channel = 0;

                    for notes in note_track.iter() {
                        if notes.is_empty() { curr_channel += 1; continue; }

                        let mut curr_note = 0;

                        let mut n_off = self.first_render_note[curr_track as usize][curr_channel];
                        if self.last_time > tick_pos {
                            if n_off == 0 {
                                for note in &notes[0..notes.len()] {
                                    if (note.start + note.length) as f32 > tick_pos { break; }
                                    n_off += 1;
                                }
                            } else {
                                for note in notes[0..n_off].iter().rev() {
                                    if ((note.start + note.length) as f32) <= tick_pos { break; }
                                    n_off -= 1;
                                }
                            }
                            self.first_render_note[curr_track as usize][curr_channel] = n_off;
                        } else if self.last_time < tick_pos {
                            for note in &notes[n_off..notes.len()] {
                                if (note.start + note.length) as f32 > tick_pos { break; }
                                n_off += 1;
                            }
                            self.first_render_note[curr_track as usize][curr_channel] = n_off;
                        }

                        let mut note_idx = n_off;

                        let note_end = {
                            let mut e = n_off;
                            for note in &notes[n_off..notes.len()] {
                                if (note.start as f32) > tick_pos + zoom_ticks { break; }
                                e += 1;
                            }
                            e
                        };

                        for note in &notes[n_off..note_end] {
                            {
                                let note_top =    (zoom_tracks - curr_track as f32 - (1.0 - ((note.key as f32 + 1.0) / 128.0))) + track_pos;
                                let note_bottom = (zoom_tracks - curr_track as f32 - (1.0 - ((note.key as f32 - 1.0) / 128.0))) + track_pos;
                                self.notes_render[note_id] = RenderTrackViewNote {
                                    0: [(note.start as f32 - tick_pos) / zoom_ticks,
                                        (note.length as f32) / zoom_ticks,
                                        note_bottom / zoom_tracks,
                                        note_top / zoom_tracks],
                                    1: {
                                        let color = self.note_colors[curr_channel as usize % self.note_colors.len()];
                                        [color[0], color[1], color[2]]
                                    }
                                };

                                note_id += 1;

                                if note_id >= NOTE_BUFFER_SIZE {
                                    self.tv_notes_vao.bind();
                                    self.tv_notes_ibo.bind();
                                    self.tv_notes_vbo.bind();
                                    self.tv_notes_ebo.bind();
                                    self.tv_notes_ibo.set_data(self.notes_render.as_slice(), glow::DYNAMIC_DRAW);
                                    self.gl.draw_elements_instanced(
                                        glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, NOTE_BUFFER_SIZE as i32);
                                    note_id = 0;
                                }
                            }
                        }

                        curr_channel += 1;
                    }

                    curr_track += 1;
                }

                if note_id != 0 {
                    self.tv_notes_vao.bind();
                    self.tv_notes_ibo.bind();
                    self.tv_notes_vbo.bind();
                    self.tv_notes_ebo.bind();
                    self.tv_notes_ibo.set_data(self.notes_render.as_slice(), glow::DYNAMIC_DRAW);
                    self.gl.draw_elements_instanced(
                        glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, note_id as i32);
                }

                self.gl.use_program(None);
            }

            self.last_time = tick_pos;
        }
    }

    fn window_size(&mut self, size: Vec2) {
        self.window_size = size;
    }

    fn set_active(&mut self, is_active: bool) {
        self.render_active = is_active;
    }

    /*fn set_ghost_notes(&mut self, _notes: Arc<Mutex<Vec<crate::app::main_window::GhostNote>>>) {
        
    }

    fn clear_ghost_notes(&mut self) {
        
    }*/
}