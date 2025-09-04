use std::sync::{Arc, Mutex};
use eframe::egui::Vec2;
use eframe::glow;
use crate::{app::{main_window::{GhostNote, MainWindow}, rendering::{piano_roll::PianoRollRenderer, track_view::TrackViewRenderer}, view_settings::ViewSettings}, editor::{navigation::{PianoRollNavigation, TrackViewNavigation}, project_data::ProjectData}};

pub mod buffers;
pub mod piano_roll;
pub mod shaders;
pub mod track_view;

pub trait Renderer {
    fn draw(&mut self);
    fn set_ghost_notes(&mut self, _notes: Arc<Mutex<Vec<GhostNote>>>) {}
    fn clear_ghost_notes(&mut self) {}
    fn set_selected(&mut self, _selected_ids: Arc<Mutex<Vec<usize>>>) {}
    fn window_size(&mut self, _size: Vec2) {}
    fn update_ppq(&mut self, _ppq: u16) {}
    fn time_changed(&mut self, _time: u64) {}
    fn set_active(&mut self, _is_active: bool) {}
}

#[derive(PartialEq, Clone, Copy)]
pub enum RenderType {
    PianoRoll,
    TrackView
}

pub struct RenderManager {
    pub render_type: RenderType,
    renderers: Vec<Arc<Mutex<dyn Renderer + Send + Sync>>>
}

impl Default for RenderManager {
    fn default() -> Self {
        Self {
            render_type: RenderType::PianoRoll,
            renderers: Vec::new()
        }
    }
}

impl RenderManager {
    pub fn init_renderers(&mut self, project_data: Arc<Mutex<ProjectData>>, gl: Option<Arc<glow::Context>>, nav: Arc<Mutex<PianoRollNavigation>>, track_view_nav: Arc<Mutex<TrackViewNavigation>>, view_settings: Arc<Mutex<ViewSettings>>) {
        // initialize piano roll renderer
        {
            let gl = gl.as_ref().unwrap();
            let project_data = project_data.clone();
            let project_data = project_data.lock().unwrap();

            let piano_roll_renderer = unsafe {
                PianoRollRenderer::new(
                    project_data.notes.clone(),
                    view_settings.clone(),
                    nav.clone(),
                    gl.clone()
                )
            };

            let track_view_renderer = unsafe {
                TrackViewRenderer::new(
                    project_data.notes.clone(),
                    track_view_nav.clone(),
                    gl.clone()
                )
            };

            self.renderers.push(Arc::new(Mutex::new(piano_roll_renderer)));
            self.renderers.push(Arc::new(Mutex::new(track_view_renderer)));
        }
    }

    pub fn switch_renderer(&mut self, render_type: RenderType) {
        /*match render_type {
            RenderType::PianoRoll => {
                if self.render_type == RenderType::PianoRoll { return; }
                self.render_type = RenderType::PianoRoll;
            },
            RenderType::TrackView => {
                if self.render_type == RenderType::TrackView { return; }
                self.render_type = RenderType::TrackView;
            }
        }*/

        self.set_active(render_type);
    }

    pub fn get_active_renderer(&mut self) -> &mut Arc<std::sync::Mutex<(dyn Renderer + Send + Sync + 'static)>> {
        match self.render_type {
            RenderType::PianoRoll => {
                &mut self.renderers[0]
            },
            RenderType::TrackView => {
                &mut self.renderers[1]
            }
        }
    }

    pub fn get_render_type(&self) -> &RenderType {
        &self.render_type
    }

    fn get_renderer(&mut self, render_type: RenderType) -> &mut Arc<std::sync::Mutex<(dyn Renderer + Send + Sync + 'static)>> {
        match render_type {
            RenderType::PianoRoll => {
                &mut self.renderers[0]
            },
            RenderType::TrackView => {
                &mut self.renderers[1]
            }
        }
    }

    fn set_active(&mut self, render_type: RenderType) {
        match render_type {
            RenderType::PianoRoll => {
                let tmp = self.get_renderer(RenderType::TrackView);
                tmp.lock().unwrap().set_active(false);
                self.render_type = RenderType::PianoRoll;
            },
            RenderType::TrackView => {
                let tmp = self.get_renderer(RenderType::PianoRoll);
                tmp.lock().unwrap().set_active(false);
                self.render_type = RenderType::TrackView;
            }
        }
        
        self.get_renderer(render_type).lock().unwrap().set_active(true);
    }
}