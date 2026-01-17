use mlua::{Error, Function, Lua, Table, Value};
use crate::{editor::{plugins::PluginType}};
use std::{path::PathBuf, rc::Rc};
use regex::Regex;

pub struct PluginInfo {
    pub author: Option<String>,
    pub description: Option<String>,
}

pub struct PluginLua {
    pub plugin_name: String,
    pub plugin_type: PluginType,
    pub plugin_info: Option<PluginInfo>,
    plugin_path: Option<PathBuf>,

    pub on_apply_fn: Option<Function>,
    pub lua: Rc<Lua>,
    pub dialog_field_table: Option<Table>,

    loaded: bool,
    is_builtin: bool,
}

impl PluginLua {
    pub fn new() -> Self {
        Self {
            plugin_name: "unnamed plugin".into(),
            plugin_type: PluginType::Manipluate,
            plugin_info: None,
            plugin_path: None,
            lua: Rc::new(Lua::new()),
            on_apply_fn: None,
            dialog_field_table: None,

            loaded: false,
            is_builtin: true
        }
    }

    pub fn load_plugin_from_path(&mut self, path: PathBuf) -> Result<(), Error> {
        if self.loaded { return Ok(()); }

        // read file contents
        match std::fs::read_to_string(&path) {
            Ok(src_code) => {
                self.plugin_path = Some(path);
                self.load_plugin_from_str(&src_code)?;
                self.is_builtin = false;
            },
            Err(e) => {
                return Err(Error::external(std::io::Error::new(std::io::ErrorKind::Other, "Failed to read plugin")))
            }
        }
        
        Ok(())
    }

    pub fn reload_plugin(&mut self) -> Result<(), Error> {
        if self.is_builtin {
            println!("Skipping {} reload because it is a builtin plugin", self.plugin_name);
            return Ok(());
        }

        if self.plugin_path.is_none() {
            return Err(Error::RuntimeError("This plugin's path is invalid.".into()));
        }

        self.loaded = false;
        let path = self.plugin_path.take().unwrap();
        let result = self.load_plugin_from_path(path);

        if let Err(ref e) = result {
            println!("[PluginError] (while reloading {}): \n--> {}", self.plugin_name, e.to_string());
        }

        result
    }

    pub fn load_plugin_from_str(&mut self, src_code: &String) -> Result<(), Error> {
        if self.loaded { return Ok(()); }
        let lua = &mut self.lua;

        let disallowed_modules = vec!["socket", "os", "package"];
        let globals = lua.globals();
        let require: Function = globals.get("require")?;
        let require_strip = lua.create_function(move |_, name: String| {
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

        let src_code = Self::preprocess_plugin_src(src_code);
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

    fn preprocess_plugin_src(source: &str) -> String {
        let re = Regex::new(r"(?m)^\s*local\s+P\s*=\s*\{\s*\}\s*;?\s*$").unwrap();
        re.replace_all(source, r#"local P = {}
function get_field_value(f_id)
    for i,f in ipairs(P.dialog_fields) do
        if f.id == f_id then
            return f[1].value
        end
    end
end"#).to_string()
    }
}