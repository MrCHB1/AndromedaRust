use mlua::{Error, Function, Lua, Table, Value};
use crate::{editor::{plugins::PluginType}};
use std::{path::PathBuf, rc::Rc};

pub struct PluginInfo {
    pub author: Option<String>,
    pub description: Option<String>,
}

pub struct PluginLua {
    pub plugin_name: String,
    pub plugin_type: PluginType,
    pub plugin_info: Option<PluginInfo>,
    pub on_apply_fn: Option<Function>,
    pub lua: Rc<Lua>,
    pub dialog_field_table: Option<Table>,
    loaded: bool,
}

impl PluginLua {
    pub fn new() -> Self {
        Self {
            plugin_name: "unnamed plugin".into(),
            plugin_type: PluginType::Manipluate,
            plugin_info: None,
            lua: Rc::new(Lua::new()),
            on_apply_fn: None,
            dialog_field_table: None,
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

        let disallowed_modules = vec!["socket", "os", "package"];
        let globals = lua.globals();
        let require: Function = globals.get("require")?;
        let require_strip = lua.create_function(move |lua, name: String| {
            if disallowed_modules.iter().any(|&m| m == name) {
                return Err(mlua::Error::RuntimeError(format!(
                    "for security reasons, usage of module '{}' is not allowed",
                    name
                )));
            }

            require.call::<Value>(name)
        })?;

        globals.set("require", require_strip)?;
        globals.set("os", Value::Nil)?;
        globals.set("package", Value::Nil)?;

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

        // plugin info
        if let Ok(plugin_info) = globals.get::<Table>("plugin_info") {
            let author = plugin_info.get::<String>("author").ok();
            let description = plugin_info.get::<String>("description").ok();
            self.plugin_info = Some(PluginInfo {
                author,
                description
            });
        }

        let on_apply = globals.get::<mlua::Function>("on_apply")?;
        let dialog_field_table = globals.get::<Table>("dialog_fields").ok();
        
        self.plugin_name = plugin_name.unwrap();
        self.plugin_type = plugin_type;
        self.on_apply_fn = Some(on_apply);
        self.dialog_field_table = dialog_field_table;
        self.loaded = true;

        Ok(())
    }
}