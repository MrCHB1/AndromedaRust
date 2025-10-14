use std::{cell::RefCell, rc::Rc};

use mlua::Lua;

use crate::{app::ui::dialog::Dialog, editor::plugins::plugin_lua::PluginLua};

pub struct PluginDialog {
    plugin: Rc<RefCell<PluginLua>>,
    lua: Rc<Lua>,
    showing: bool
}

impl PluginDialog {
    pub fn lua_mut(&mut self) -> &mut Rc<Lua> {
        &mut self.lua
    }
}

impl Dialog for PluginDialog {
    fn show(&mut self) -> () {
        self.showing = true;
    }

    fn close(&mut self) -> () {
        self.showing = false;
    }

    fn is_showing(&self) -> bool {
        self.showing
    }

    fn draw(&mut self, ctx: &eframe::egui::Context, image_resources: &crate::app::util::image_loader::ImageResources) -> () {
        
    }
}