use crate::audio::event_playback::PlaybackManager;
use crate::editor::midi_bar_cacher::BarCacher;
use crate::editor::project_data::ProjectData;
use crate::midi::events::note::Note;
use std::sync::{Arc, Mutex};
use eframe::egui::Vec2;
use eframe::glow;
use eframe::glow::HasContext;
use crate::app::rendering::{
    buffers::*,
    shaders::*, Renderer
};

use crate::editor::navigation::PianoRollNavigation;
use crate::set_attribute;

const HANDLE_BUFFER_SIZE: usize = 4096;
const BAR_BUFFER_SIZE: usize = 32;

// data view background
pub type BarStart = f32;
pub type BarLength = f32;
pub type BarNumber = u32;

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct RenderDataViewBar(BarStart, BarLength, BarNumber);

pub type HandleRect = [f32; 4]; // (tick, length, handle_center (0.0 -> 1.0), handle_value (0.0 -> 1.0))
pub type HandleColor = [f32; 3];

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct RenderDataViewHandle(HandleRect, HandleColor);

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
    pub playback_manager: Arc<Mutex<PlaybackManager>>,
    pub bar_cacher: Arc<Mutex<BarCacher>>,
    pub window_size: Vec2<>,
    pub ppq: u16,

    dv_program: ShaderProgram,
    dv_vertex_buffer: Buffer,
    dv_vertex_array: VertexArray,
    dv_instance_buffer: Buffer,
    dv_index_buffer: Buffer,

    /*dv_data_program: ShaderProgram,
    dv_data_vbo: Buffer,
    dv_data_vao: VertexArray,
    dv_data_ibo: Buffer,
    dv_data_ebo: Buffer,*/

    gl: Arc<glow::Context>,

    bars_render: Vec<RenderDataViewBar>,

    started_playing: bool,
    last_view_offset: f32,
    last_zoom: f32
}

impl DataViewRenderer {
    pub unsafe fn new(
        project_data: &Arc<Mutex<ProjectData>>,
        nav: &Arc<Mutex<PianoRollNavigation>>,
        gl: &Arc<glow::Context>,
        playback_manager: &Arc<Mutex<PlaybackManager>>,
        bar_cacher: &Arc<Mutex<BarCacher>>
    ) -> Self {
        let dv_program = ShaderProgram::create_from_files(gl.clone(), "./shaders/data_view_bg");
        
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

        Self {
            navigation: nav.clone(),
            bar_cacher: bar_cacher.clone(),
            window_size: Vec2::new(0.0, 0.0),
            gl: gl.clone(),
            playback_manager: playback_manager.clone(),
            ppq: 960,

            dv_program,
            dv_vertex_buffer,
            dv_vertex_array,
            dv_instance_buffer,
            dv_index_buffer,

            bars_render: dv_bars_render.to_vec(),

            started_playing: false,
            last_view_offset: 0.0,
            last_zoom: 0.0
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

impl Renderer for DataViewRenderer {
    fn draw(&mut self) {
        unsafe {
            let tick_pos = self.get_time();

            let (zoom_ticks, key_pos, zoom_keys) = {
                let nav = self.navigation.lock().unwrap();
                (nav.zoom_ticks_smoothed, nav.key_pos_smoothed, nav.zoom_keys_smoothed)
            };
            
            let (nav_curr_track, nav_curr_channel) = {
                let nav = self.navigation.lock().unwrap();
                (nav.curr_track, nav.curr_channel)
            };

            let (is_playing, view_offset) = {
                let playback_manager = self.playback_manager.lock().unwrap();
                let mut view_offset = self.last_view_offset;
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

                (playback_manager.playing, view_offset)
            };

            let tick_pos_offs = tick_pos + view_offset;

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
        }
    }

    fn window_size(&mut self, size: Vec2) {
        self.window_size = size;
    }
}