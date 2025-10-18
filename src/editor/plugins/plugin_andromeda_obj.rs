use std::{cell::RefCell, rc::Rc};

use mlua::UserData;
use crate::editor::{playhead::Playhead, project_data::ProjectData};

// provides functions that can be called from lua plugins
pub struct AndromedaObj {
    project_data: Rc<RefCell<ProjectData>>,
    // for getting the playhead pos
    playhead: Rc<RefCell<Playhead>>
}

impl AndromedaObj {
    pub fn new(project_data: &Rc<RefCell<ProjectData>>, playhead: &Rc<RefCell<Playhead>>) -> Self {
        Self {
            project_data: project_data.clone(),
            playhead: playhead.clone()
        }
    }
}

impl UserData for AndromedaObj {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method::<_, _, mlua::Number>("ticks_to_secs", |_, this, tick: mlua::Number| {
            let project_data = this.project_data.borrow();
            let tempo_map = project_data.tempo_map.read().unwrap();
            let secs = tempo_map.ticks_to_secs_from_map(project_data.project_info.ppq, tick as f32);
            Ok(secs as mlua::Number)
        });

        methods.add_method::<_, _, mlua::Number>("secs_to_ticks", |_, this, secs: mlua::Number| {
            let project_data = this.project_data.borrow();
            let tempo_map = project_data.tempo_map.read().unwrap();
            let ticks = tempo_map.secs_to_ticks_from_map(project_data.project_info.ppq, secs as f32);
            Ok(ticks as mlua::Number)
        });

        methods.add_method::<_, _, mlua::Number>("get_ppq", |_, this, _: ()| {
            let project_data = this.project_data.borrow();
            let ppq = project_data.project_info.ppq;
            Ok(ppq as mlua::Number)
        });

        methods.add_method::<_, _, mlua::Number>("get_playhead_tick_pos", |_, this, _: ()| {
            let playhead = this.playhead.borrow();
            Ok(playhead.start_tick as mlua::Number)
        });

        methods.add_method::<_, _, mlua::Number>("get_playhead_secs_pos", |_, this, _: ()| {
            let playhead = this.playhead.borrow();
            let project_data = this.project_data.borrow();
            let tempo_map = project_data.tempo_map.read().unwrap();
            let secs = tempo_map.ticks_to_secs_from_map(project_data.project_info.ppq, playhead.start_tick as f32);

            Ok(secs as mlua::Number)
        });
    }
}