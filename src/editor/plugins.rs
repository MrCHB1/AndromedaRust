pub mod plugin_lua;
pub mod plugin_dialog;
pub mod plugin_andromeda_obj;

use std::path::Path;
use crate::editor::plugins::plugin_lua::PluginLua;
use std::fs::{self, DirEntry};
use std::io::Result;

use std::rc::Rc;
use std::cell::RefCell;
use include_dir::include_dir;

static BUILTIN_PLUGIN_NAMES: &[&'static str] = &[
    "flip_x",
    "flip_y",
];

pub enum PluginType {
    Manipluate,
    Generate
}

pub struct PluginLoader {
    pub manip_plugins: Vec<Rc<RefCell<PluginLua>>>,
    pub gen_plugins: Vec<Rc<RefCell<PluginLua>>>,
}

impl PluginLoader {
    pub fn new() -> Self {
        let mut plugin_loader = Self { manip_plugins: Vec::new(), gen_plugins: Vec::new() };
        // very first thing to do: load built-in plugins

        let builtin_plugins_dir = include_dir!("assets/plugins/builtin");
        
        for file_name in BUILTIN_PLUGIN_NAMES.iter() {
            if let Some(file) = builtin_plugins_dir.get_file(format!("{}.lua", file_name)) {
                let src_code = file.contents_utf8().unwrap();
                plugin_loader.push_plugin_raw_str(file_name, src_code.into()).unwrap();
            }
        }

        plugin_loader
    }

    pub fn load_plugins(&mut self, dir: &Path) -> Result<()> {
        let read_dir = fs::read_dir(dir)?;
        for entry in read_dir {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.load_plugins(&path)?;
            } else {
                self.push_plugin(&entry)?;
            }
        }
        Ok(())
    }

    fn push_plugin(&mut self, plugin_entry: &DirEntry) -> Result<()> {
        let plugin_file_name = plugin_entry.file_name();
        let plugin_file_name = plugin_file_name.to_str().unwrap();

        let plugin_path = plugin_entry.path();
        let mut plugin = PluginLua::new();
        match plugin.load_plugin_from_path(plugin_path) {
            Ok(_) => {
                self.push_inner(plugin);
                Ok(())
            },
            Err(lua_err) => {
                println!("[PluginError] (in {}): \n--> {}", plugin_file_name, lua_err.to_string());
                Ok(())
            }
        }
    }

    fn push_plugin_raw_str(&mut self, plugin_name: &str, plugin_str: String) -> Result<()> {
        let mut plugin = PluginLua::new();
        match plugin.load_plugin_from_str(&plugin_str) {
            Ok(_) => {
                self.push_inner(plugin);
                Ok(())
            },
            Err(lua_err) => {
                println!("[PluginError] (in {}): \n--> {}", plugin_name, lua_err.to_string());
                Ok(())
            }
        }
    }

    fn push_inner(&mut self, plugin: PluginLua) {
        match plugin.plugin_type {
            PluginType::Manipluate => {
                self.manip_plugins.push(Rc::new(RefCell::new(plugin)));
            },
            PluginType::Generate => {
                self.gen_plugins.push(Rc::new(RefCell::new(plugin)));
            }
        }
    }
}