use mlua::{Error, Function, Lua, Table};
use crate::{editor::{plugins::PluginType}};
use std::{path::PathBuf, rc::Rc};

// will have to eventually use this for everything else
// pub struct LuaNotes(pub Arc<RwLock<Vec<Vec<Note>>>>, pub Arc<Mutex<Vec<usize>>>);

/*impl IntoLua for &mut Note {
    fn into_lua(self, lua: &Lua) -> mlua::Result<mlua::Value> {
        Ok(self.))
    }
}*/

pub struct PluginLua {
    pub plugin_name: String,
    pub plugin_type: PluginType,
    pub on_apply_fn: Option<Function>,
    pub lua: Rc<Lua>,
    loaded: bool,
}

impl PluginLua {
    pub fn new() -> Self {
        Self {
            plugin_name: "unnamed plugin".into(),
            plugin_type: PluginType::Manipluate,
            lua: Rc::new(Lua::new()),
            on_apply_fn: None,
            loaded: false
        }
    }

    pub fn load_plugin_from_path(&mut self, path: PathBuf) -> Result<(), Error> {
        if self.loaded { return Ok(()); }

        // read file contents
        let src_code = std::fs::read_to_string(path).unwrap();
        
        self.load_plugin_from_str(&src_code)?;
        Ok(())
    }

    pub fn load_plugin_from_str(&mut self, src_code: &String) -> Result<(), Error> {
        if self.loaded { return Ok(()); }

        let lua = &mut self.lua;
        let globals = lua.load(src_code).eval::<Table>()?;

        let plugin_name = globals.get::<String>("plugin_name");
        if plugin_name.is_err() {
            return Err(Error::RuntimeError("plugin requires a name to be defined, which is missing (maybe try defining P.plugin_name)".into()))
        }

        let plugin_type: PluginType = {
            let plugin_type_str: String = globals.get::<String>("plugin_type")?;
            let plugin_type_str = plugin_type_str.as_str();
            match plugin_type_str {
                "manipulate" => {
                    PluginType::Manipluate
                },
                "generate" => {
                    PluginType::Generate
                },
                _ => {
                    PluginType::Manipluate
                }
            }
        };

        let on_apply = globals.get::<mlua::Function>("on_apply")?;
        
        self.plugin_name = plugin_name.unwrap();
        self.plugin_type = plugin_type;
        self.on_apply_fn = Some(on_apply);
        self.loaded = true;

        Ok(())
    }
}