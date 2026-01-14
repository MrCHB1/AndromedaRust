use crate::app::rendering::note_cull_helper::NoteCullHelper;
use crate::app::shared::NoteColors;
use crate::app::view_settings::{VS_PianoRoll_DataViewState, 
    VS_PianoRoll_OnionColoring, 
    VS_PianoRoll_OnionState, 
ViewSettings};
use crate::audio::event_playback::PlaybackManager;
use crate::editor::editing::SharedSelectedNotes;
use crate::editor::midi_bar_cacher::BarCacher;
//use crate::editor::note_editing::GhostNote;
use crate::editor::editing::note_editing::GhostNote;
use crate::editor::project::project_data::ProjectData;
use crate::editor::project::project_manager::ProjectManager;
use crate::midi::events::note::Note;
use crate::midi::midi_track::MIDITrack;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex, RwLock};
use eframe::egui::Vec2;
use eframe::glow;
use eframe::glow::HasContext;
use crate::app::rendering::{
    buffers::*,
    shaders::*, Renderer
};

use crate::editor::navigation::PianoRollNavigation;
use crate::set_attribute;

const HANDLE_BUFFER_SIZE: usize = 2048;
const BAR_BUFFER_SIZE: usize = 32;

// data view background
pub type BarStart = f32;
pub type BarLength = f32;
pub type BarNumber = u32;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct RenderDataViewBar(BarStart, BarLength, BarNumber);

pub type HandleRect = [f32; 4]; // (tick, length, handle_center (0.0 -> 1.0), handle_value (0.0 -> 1.0))
pub type HandleMeta = u32;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct RenderDataViewHandle(HandleRect, HandleMeta);

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

pub struct DataViewRenderer {
    pub navigation: Arc<Mutex<PianoRollNavigation>>,
    // pub playback_manager: Arc<Mutex<PlaybackManager>>,
    pub bar_cacher: Arc<Mutex<BarCacher>>,
    pub window_size: Vec2<>,
    pub ppq: u16,
    view_settings: Arc<Mutex<ViewSettings>>,
    note_colors: Arc<Mutex<NoteColors>>,

    dv_program: ShaderProgram,
    dv_vertex_buffer: Buffer,
    dv_vertex_array: VertexArray,
    dv_instance_buffer: Buffer,
    dv_index_buffer: Buffer,

    dv_handles_program: ShaderProgram,
    dv_handles_vbo: Buffer,
    dv_handles_vao: VertexArray,
    dv_handles_ibo: Buffer,
    dv_handles_ebo: Buffer,

    gl: Arc<glow::Context>,

    bars_render: Vec<RenderDataViewBar>,
    dv_handles_render: Vec<RenderDataViewHandle>,
    // notes: Arc<RwLock<Vec<Vec<Note>>>>,
    all_tracks: Arc<RwLock<Vec<MIDITrack>>>,

    note_cull_helper: Arc<Mutex<NoteCullHelper>>,

    pub ghost_notes: Option<Arc<Mutex<Vec<Note>>>>,
    selected: Arc<RwLock<SharedSelectedNotes>>
}

impl DataViewRenderer {
    pub unsafe fn new(
        project_manager: &Arc<RwLock<ProjectManager>>,
        view_settings: &Arc<Mutex<ViewSettings>>,
        nav: &Arc<Mutex<PianoRollNavigation>>,
        gl: &Arc<glow::Context>,
        _playback_manager: &Arc<Mutex<PlaybackManager>>,
        bar_cacher: &Arc<Mutex<BarCacher>>,
        note_colors: &Arc<Mutex<NoteColors>>,
        note_cull_helper: &Arc<Mutex<NoteCullHelper>>,
        shared_selected_notes: &Arc<RwLock<SharedSelectedNotes>>
    ) -> Self {
        let dv_program = ShaderProgram::create_from_files(gl.clone(), "./assets/shaders/data_view_bg");
        let dv_handles_program = ShaderProgram::create_from_files(gl.clone(), "./assets/shaders/data_view_handles");
        
        // -------- DATA VIEW BARS --------

        let dv_vertex_buffer = Buffer::new(gl.clone(), glow::ARRAY_BUFFER);
        dv_vertex_buffer.set_data(&QUAD_VERTICES, glow::STATIC_DRAW);

        let dv_index_buffer = Buffer::new(gl.clone(), glow::ELEMENT_ARRAY_BUFFER);
        dv_index_buffer.set_data(&QUAD_INDICES, glow::STATIC_DRAW);

        let dv_vertex_array = VertexArray::new(gl.clone());
        let pos_attrib = dv_program.get_attrib_location("vPos").unwrap();
        set_attribute!(glow::FLOAT, dv_vertex_array, pos_attrib, Vertex::0);

        let dv_instance_buffer = Buffer::new(gl.clone(), glow::ARRAY_BUFFER);
        let dv_bars_render = vec![
            RenderDataViewBar {
                0: 0.0,
                1: 1.0,
                2: 0
            }; BAR_BUFFER_SIZE
        ];
        dv_instance_buffer.set_data(dv_bars_render.as_slice(), glow::DYNAMIC_DRAW);

        let tv_bar_start = dv_program.get_attrib_location("barStart").unwrap();
        set_attribute!(glow::FLOAT, dv_vertex_array, tv_bar_start, RenderDataViewBar::0);
        let tv_bar_length = dv_program.get_attrib_location("barLength").unwrap();
        set_attribute!(glow::FLOAT, dv_vertex_array, tv_bar_length, RenderDataViewBar::1);
        let tv_bar_number = dv_program.get_attrib_location("barNumber").unwrap();
        set_attribute!(glow::UNSIGNED_INT, dv_vertex_array, tv_bar_number, RenderDataViewBar::2);

        gl.vertex_attrib_divisor(1, 1);
        gl.vertex_attrib_divisor(2, 1);
        gl.vertex_attrib_divisor(3, 1);

        // -------- DATA VIEW HANDLES --------

        let dv_handles_vbo = Buffer::new(gl.clone(), glow::ARRAY_BUFFER);
        dv_handles_vbo.set_data(&QUAD_VERTICES, glow::STATIC_DRAW);

        let dv_handles_ebo = Buffer::new(gl.clone(), glow::ELEMENT_ARRAY_BUFFER);
        dv_handles_ebo.set_data(&QUAD_INDICES, glow::STATIC_DRAW);

        let dv_handles_vao = VertexArray::new(gl.clone());
        // let pos_attrib = dv_handles_program.get_attrib_location("vPos").unwrap();
        set_attribute!(glow::FLOAT, dv_handles_vao, 0, Vertex::0);

        let dv_handles_ibo = Buffer::new(gl.clone(), glow::ARRAY_BUFFER);
        let dv_handles_render = vec![
            RenderDataViewHandle {
                0: [0.0, 1.0, 0.5, 0.5],
                1: 0
            }; HANDLE_BUFFER_SIZE
        ];
        dv_handles_ibo.set_data(&dv_handles_render, glow::DYNAMIC_DRAW);

        let dv_handles_rect = dv_handles_program.get_attrib_location("handleRect").unwrap();
        set_attribute!(glow::FLOAT, dv_handles_vao, dv_handles_rect, RenderDataViewHandle::0);
        let dv_handles_meta = dv_handles_program.get_attrib_location("handleMeta").unwrap();
        set_attribute!(glow::UNSIGNED_INT, dv_handles_vao, dv_handles_meta, RenderDataViewHandle::1);

        gl.vertex_attrib_divisor(1, 1);
        gl.vertex_attrib_divisor(2, 1);

        let tracks = {
            let project_manager = project_manager.read().unwrap();
            project_manager.get_tracks().clone()
        };

        Self {
            navigation: nav.clone(),
            bar_cacher: bar_cacher.clone(),
            window_size: Vec2::new(0.0, 0.0),
            gl: gl.clone(),
            // playback_manager: playback_manager.clone(),
            ppq: 960,

            dv_program,
            dv_vertex_buffer,
            dv_vertex_array,
            dv_instance_buffer,
            dv_index_buffer,

            dv_handles_program,
            dv_handles_vbo,
            dv_handles_vao,
            dv_handles_ebo,
            dv_handles_ibo,
            dv_handles_render,

            bars_render: dv_bars_render.to_vec(),

            all_tracks: tracks,
            view_settings: view_settings.clone(),
            note_colors: note_colors.clone(),

            note_cull_helper: note_cull_helper.clone(),
            selected: shared_selected_notes.clone(),
            ghost_notes: None
        }
    }

    fn get_time(&self) -> f32 {
        let nav = self.navigation.lock().unwrap();

        /*let is_playing = {
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
        };*/

        nav.tick_pos_smoothed
    }

    fn draw_note_velocities(&mut self, tick_pos: f32, zoom_ticks: f32) {
        let view_settings = self.view_settings.lock().unwrap();

        let nav_curr_track = {
            let nav = self.navigation.lock().unwrap();
            nav.curr_track
        };

        let tracks = self.all_tracks.read().unwrap();
        if tracks.is_empty() { return; }

        let tracks_to_iter = match view_settings.pr_onion_state {
            VS_PianoRoll_OnionState::NoOnion => {
                &tracks[nav_curr_track as usize..nav_curr_track as usize]
            },
            VS_PianoRoll_OnionState::ViewAll => {
                //view_all_tracks = true;
                &tracks[..]
            },
            VS_PianoRoll_OnionState::ViewPrevious => {
                //view_all_tracks = false;
                if nav_curr_track == 0 { &tracks[nav_curr_track as usize..nav_curr_track as usize] }
                else { &tracks[(nav_curr_track - 1) as usize..=(nav_curr_track as usize)] }
            },
            VS_PianoRoll_OnionState::ViewNext => {
                if nav_curr_track == (tracks.len() - 1) as u16 { &tracks[nav_curr_track as usize..nav_curr_track as usize] }
                else { &tracks[(nav_curr_track) as usize..=(nav_curr_track + 1) as usize] }
            }
        };

        let onion_track_color_meta = match view_settings.pr_onion_coloring {
            VS_PianoRoll_OnionColoring::FullColor => { 0b00 },
            VS_PianoRoll_OnionColoring::PartialColor => { 0b01 },
            VS_PianoRoll_OnionColoring::GrayedOut => { 0b10 }
        };
        
        {
            let note_colors = self.note_colors.lock().unwrap();

            let mut handle_id = 0;
            let mut curr_track = 0;

            let tick_pos_offs = tick_pos;

            // bind before rendering
            self.dv_handles_vao.bind();
            self.dv_handles_ibo.bind();
            self.dv_handles_vbo.bind();
            self.dv_handles_ebo.bind();

            let mut note_culler = self.note_cull_helper.lock().unwrap();
            note_culler.sync_cull_array_lengths();

            // 1. draw all note velocities that is not the current track
            for track in tracks_to_iter {
                let notes = track.get_notes();
                if notes.is_empty() || curr_track == nav_curr_track {
                    curr_track += 1;
                    continue;
                }

                note_culler.update_cull_for_track(curr_track, tick_pos_offs, zoom_ticks, false);
                let (note_start, mut note_end) = note_culler.get_track_cull_range(curr_track);
                let mut n_off = note_start;
                
                if note_end > notes.len() { 
                    note_culler.update_cull_for_track(curr_track, tick_pos_offs, zoom_ticks, true);
                    (n_off, note_end) = note_culler.get_track_cull_range(curr_track);
                }

                let mut curr_handle = 0;

                for note in &notes[n_off..note_end] {
                    let trk_chan = ((curr_track as usize) << 4) | (note.channel() as usize);

                    {
                        let dv_handle_val = (note.velocity() as f32) / 127.0;
                        self.dv_handles_render[handle_id] = RenderDataViewHandle {
                            0: [(note.start as f32 - tick_pos_offs) / zoom_ticks,
                                (note.length as f32) / zoom_ticks,
                                0.0,
                                dv_handle_val],
                            1: {
                                let mut note_meta = note_colors.get_index(trk_chan) as u32;
                                note_meta |= (note.velocity() as u32) << 4;
                                note_meta |= onion_track_color_meta << 14;
                                note_meta
                            }
                        };
                    }

                    handle_id += 1;

                    if handle_id >= HANDLE_BUFFER_SIZE {
                        self.dv_handles_ibo.set_data(self.dv_handles_render.as_slice(), glow::DYNAMIC_DRAW);

                        unsafe {
                            self.gl.use_program(Some(self.dv_handles_program.program));
                            self.gl.draw_elements_instanced(
                                glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, HANDLE_BUFFER_SIZE as i32
                            );
                        }

                        handle_id = 0;
                    }

                    curr_handle += 1;
                    if curr_handle >= notes.len() { break; }
                }

                curr_track += 1;
            }

            // 2. draw current track on top
            let notes = tracks[nav_curr_track as usize].get_notes();
            if !notes.is_empty() {
                let mut curr_handle = 0;

                note_culler.update_cull_for_track(nav_curr_track, tick_pos_offs, zoom_ticks, false);
                let (note_start, note_end) = note_culler.get_track_cull_range(nav_curr_track);
                let n_off = note_start;
                let mut note_idx = n_off;
                
                let shared_sel_notes = self.selected.read().unwrap();

                let empty: &[usize] = &[];
                let sel_ids = shared_sel_notes
                    .get_selected_ids_in_track(nav_curr_track)
                    .map(|v| v.as_slice())
                    .unwrap_or(empty);

                let mut sel_idx = 0;
                for note in &notes[n_off..note_end] {
                    let trk_chan = ((nav_curr_track as usize) << 4) | (note.channel() as usize);
                    {
                        let dv_handle_val = (note.velocity() as f32) / 127.0;
                        self.dv_handles_render[handle_id] = RenderDataViewHandle {
                            0: [(note.start as f32 - tick_pos_offs) / zoom_ticks,
                                (note.length as f32) / zoom_ticks,
                                0.0,
                                dv_handle_val],
                            1: {
                                let mut note_meta = note_colors.get_index(trk_chan) as u32;
                                note_meta |= (note.velocity() as u32) << 4;

                                if sel_idx < sel_ids.len() && note_idx == sel_ids[sel_idx] {
                                    note_meta |= 1 << 13;
                                    sel_idx += 1;
                                }   

                                note_meta
                            }
                        };
                    }

                    note_idx += 1;
                    handle_id += 1;

                    if handle_id >= HANDLE_BUFFER_SIZE {
                        self.dv_handles_ibo.set_data(self.dv_handles_render.as_slice(), glow::DYNAMIC_DRAW);

                        unsafe {
                            self.gl.use_program(Some(self.dv_handles_program.program));
                            self.gl.draw_elements_instanced(
                                glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, HANDLE_BUFFER_SIZE as i32
                            );
                        }

                        handle_id = 0;
                    }

                    curr_handle += 1;
                    if curr_handle >= notes.len() { break; }
                }
            }

            // 3. draw ghost handles
            if let Some(ghost_notes) = &self.ghost_notes {
                let notes = ghost_notes.lock().unwrap();

                for note in notes.iter() {
                    // let note = note.get_note();

                    let trk_chan = ((nav_curr_track as usize) << 4) | (note.channel() as usize);
                    let dv_handle_val = (note.velocity() as f32) / 127.0;

                    self.dv_handles_render[handle_id] = RenderDataViewHandle {
                        0: [(note.start as f32 - tick_pos_offs) / zoom_ticks,
                            (note.length as f32) / zoom_ticks,
                            0.0,
                            dv_handle_val],
                        1: {
                            let mut note_meta = note_colors.get_index(trk_chan) as u32;
                            note_meta |= (note.velocity() as u32) << 4;
                            // note_meta |= onion_track_color_meta << 14;
                            note_meta
                        }
                    };

                    handle_id += 1;
                    // note_idx += 1;

                    if handle_id >= HANDLE_BUFFER_SIZE {
                        self.dv_handles_ibo.set_data(self.dv_handles_render.as_slice(), glow::DYNAMIC_DRAW);

                        unsafe {
                            self.gl.use_program(Some(self.dv_handles_program.program));
                            self.gl.draw_elements_instanced(
                                glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, HANDLE_BUFFER_SIZE as i32
                            );
                        }

                        handle_id = 0;
                    }
                }
            }
            
            // 4. flush remaining handles
            if handle_id != 0 {
                self.dv_handles_ibo.set_data(self.dv_handles_render.as_slice(), glow::DYNAMIC_DRAW);

                unsafe {
                    self.gl.use_program(Some(self.dv_handles_program.program));
                    self.gl.draw_elements_instanced(
                        glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, handle_id as i32
                    );
                }
            }

            // note_culler.update_time_and_zoom(tick_pos_offs, zoom_ticks);
            // self.last_time = tick_pos_offs;
        }
    }

    pub fn set_ghost_notes(&mut self, notes: Arc<Mutex<Vec<Note>>>) {
        self.ghost_notes = Some(notes);
    }

    pub fn clear_ghost_notes(&mut self) {
        self.ghost_notes = None;
    }
}

impl Renderer for DataViewRenderer {
    fn draw(&mut self) {
        {
            let view_settings = self.view_settings.lock().unwrap();
            if view_settings.pr_dataview_state == VS_PianoRoll_DataViewState::Hidden { 
                drop(view_settings);
                return;
            }
        }

        unsafe {
            let tick_pos = self.get_time();

            let zoom_ticks = {
                let nav = self.navigation.lock().unwrap();
                nav.zoom_ticks_smoothed
            };

            let tick_pos_offs = tick_pos;

            // RENDER BARS
            {
                self.gl.use_program(Some(self.dv_program.program));

                self.dv_program.set_float("width", self.window_size.x);
                self.dv_program.set_float("height", self.window_size.y);
                self.dv_program.set_float("ppqNorm", self.ppq as f32 / zoom_ticks);

                let mut curr_bar_tick = 0.0;
                let mut bar_num = 0;
                let mut bar_id = 0;

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

                    self.bars_render[bar_id] = RenderDataViewBar {
                        0: ((curr_bar_tick - tick_pos_offs) / zoom_ticks),
                        1: (bar_length as f32 / zoom_ticks),
                        2: bar_num as u32
                    };
                    bar_id += 1;
                    if bar_id >= 32 {
                        self.dv_vertex_array.bind();
                        self.dv_instance_buffer.bind();
                        self.dv_vertex_buffer.bind();
                        self.dv_index_buffer.bind();
                        self.dv_instance_buffer.set_data(self.bars_render.as_slice(), glow::DYNAMIC_DRAW);
                        self.gl.draw_elements_instanced(
                            glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, 32);
                        bar_id = 0;
                    }

                    curr_bar_tick += bar_length as f32;
                    bar_num += 1;
                }

                if bar_id != 0 {
                    self.dv_vertex_array.bind();
                    self.dv_instance_buffer.bind();
                    self.dv_vertex_buffer.bind();
                    self.dv_index_buffer.bind();
                    self.dv_instance_buffer.set_data(self.bars_render.as_slice(), glow::DYNAMIC_DRAW);
                    // self.gl.use_program(Some(self.tv_program.program));
                    self.gl.draw_elements_instanced(
                        glow::TRIANGLES, 6, glow::UNSIGNED_INT, 0, bar_id as i32);
                }

                self.gl.use_program(None);
            }

            // RENDER DATA VIEW HANDLES
            {
                self.gl.use_program(Some(self.dv_handles_program.program));
                {
                    {
                        let mut note_colors = self.note_colors.lock().unwrap();
                        self.gl.active_texture(glow::TEXTURE0);
                        note_colors.get_texture().bind();
                    }

                    self.dv_handles_program.set_int("noteColorTexture", 0);
                    self.dv_handles_program.set_float("width", self.window_size.x);
                    self.dv_handles_program.set_float("height", self.window_size.y);

                    let curr_data_view = {
                        let view_settings = self.view_settings.lock().unwrap();
                        view_settings.pr_dataview_state
                    };

                    match curr_data_view {
                        VS_PianoRoll_DataViewState::NoteVelocities => {
                            self.draw_note_velocities(tick_pos, zoom_ticks);
                        },
                        _ => {}
                    }
                }
                self.gl.use_program(None);
            }
        }
    }

    fn window_size(&mut self, size: Vec2) {
        self.window_size = size;
    }

    fn update_ppq(&mut self, ppq: u16) {
        self.ppq = ppq;
    }
}