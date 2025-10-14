use std::{cell::RefCell, rc::Rc};

use mlua::UserData;
use crate::editor::project_data::ProjectData;

// provides functions that can be called from lua plugins
pub struct AndromedaObj {
    project_data: Rc<RefCell<ProjectData>>
}

impl AndromedaObj {
    pub fn new(project_data: &Rc<RefCell<ProjectData>>) -> Self {
        Self {
            project_data: project_data.clone()
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
    }
}