use std::{cell::RefCell, rc::Rc, sync::{Arc, RwLock}};

use mlua::UserData;
use crate::editor::{playhead::Playhead, project::{project_data::ProjectData, project_manager::ProjectManager}};

// provides functions that can be called from lua plugins
pub struct AndromedaObj {
    project_manager: Arc<RwLock<ProjectManager>>,
    // for getting the playhead pos
    playhead: Rc<RefCell<Playhead>>
}

impl AndromedaObj {
    pub fn new(project_manager: &Arc<RwLock<ProjectManager>>, playhead: &Rc<RefCell<Playhead>>) -> Self {
        Self {
            project_manager: project_manager.clone(),
            playhead: playhead.clone()
        }
    }
}

impl UserData for AndromedaObj {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method::<_, _, mlua::Number>("ticks_to_secs", |_, this, tick: mlua::Number| {
            let project_manager = this.project_manager.read().unwrap();
            let tempo_map = project_manager.get_tempo_map().read().unwrap();
            let secs = tempo_map.ticks_to_secs_from_map(project_manager.get_ppq(), tick as f32);
            Ok(secs as mlua::Number)
        });

        methods.add_method::<_, _, mlua::Number>("secs_to_ticks", |_, this, secs: mlua::Number| {
            let project_manager = this.project_manager.read().unwrap();
            let tempo_map = project_manager.get_tempo_map().read().unwrap();
            let ticks = tempo_map.secs_to_ticks_from_map(project_manager.get_ppq(), secs as f32);
            Ok(ticks as mlua::Number)
        });

        methods.add_method::<_, _, mlua::Number>("get_ppq", |_, this, _: ()| {
            let project_manager = this.project_manager.read().unwrap();
            let ppq = project_manager.get_ppq();
            Ok(ppq as mlua::Number)
        });

        methods.add_method::<_, _, mlua::Number>("get_playhead_tick_pos", |_, this, _: ()| {
            let playhead = this.playhead.try_borrow().unwrap();
            Ok(playhead.start_tick as mlua::Number)
        });

        methods.add_method::<_, _, mlua::Number>("get_playhead_secs_pos", |_, this, _: ()| {
            let playhead = this.playhead.try_borrow().unwrap();
            let project_manager = this.project_manager.read().unwrap();
            let tempo_map = project_manager.get_tempo_map().read().unwrap();
            let secs = tempo_map.ticks_to_secs_from_map(project_manager.get_ppq(), playhead.start_tick as f32);

            Ok(secs as mlua::Number)
        });
    }
}