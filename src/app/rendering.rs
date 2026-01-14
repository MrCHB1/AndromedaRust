use std::{cell::RefCell, rc::Rc, sync::{Arc, Mutex, RwLock}};
use eframe::egui::Vec2;
use eframe::glow;
use crate::{app::{rendering::{note_cull_helper::NoteCullHelper, piano_roll::PianoRollRenderer, track_view::TrackViewRenderer}, shared::NoteColors, view_settings::ViewSettings}, audio::event_playback::PlaybackManager, editor::{editing::SharedSelectedNotes, midi_bar_cacher::BarCacher, navigation::{PianoRollNavigation, TrackViewNavigation}, project::project_manager::{self, ProjectManager}}, midi::events::note::Note};
use crate::editor::project::project_data::ProjectData;

pub mod buffers;
pub mod piano_roll;
pub mod shaders;
pub mod track_view;
pub mod data_view;
pub mod note_cull_helper;

pub trait Renderer {
    fn draw(&mut self);
    fn set_ghost_notes(&mut self, _notes: Arc<Mutex<Vec<Note>>>) {}
    fn clear_ghost_notes(&mut self) {}
    fn set_selected(&mut self, _selected_ids: &Arc<RwLock<SharedSelectedNotes>>) {}
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
    pub fn init_renderers(
        &mut self,
        project_manager: Arc<RwLock<ProjectManager>>,
        gl: Option<Arc<glow::Context>>,
        nav: Arc<Mutex<PianoRollNavigation>>,
        track_view_nav: Arc<Mutex<TrackViewNavigation>>,
        view_settings: Arc<Mutex<ViewSettings>>,
        playback_manager: Arc<Mutex<PlaybackManager>>,
        bar_cacher: Arc<Mutex<BarCacher>>,
        colors: &Arc<Mutex<NoteColors>>,
        note_cull_helper: &Arc<Mutex<NoteCullHelper>>,
        shared_selected_notes: &Arc<RwLock<SharedSelectedNotes>>
    ) {
        // initialize piano roll renderer
        {
            let gl = gl.as_ref().unwrap();
            //let project_data = project_data.clone();
            //let project_data = project_data.lock().unwrap();

            println!("init piano roll renderer");
            let piano_roll_renderer = Arc::new(Mutex::new(unsafe {
                PianoRollRenderer::new(
                    &project_manager,
                    &view_settings,
                    &nav,
                    &gl,
                    &playback_manager,
                    &bar_cacher,
                    colors,
                    note_cull_helper,
                    shared_selected_notes,
                )
            }));

            println!("init track view renderer");
            let track_view_renderer = Arc::new(Mutex::new(unsafe {
                TrackViewRenderer::new(
                    &project_manager,
                    &track_view_nav,
                    &nav,
                    &gl,
                    &bar_cacher,
                    colors,
                    shared_selected_notes
                )
            }));

            self.renderers.push(piano_roll_renderer);
            self.renderers.push(track_view_renderer);
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

    pub fn set_ppq(&mut self, ppq: u16) {
        for renderer in self.renderers.iter_mut() {
            let mut renderer = renderer.lock().unwrap();
            renderer.update_ppq(ppq);
        }
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