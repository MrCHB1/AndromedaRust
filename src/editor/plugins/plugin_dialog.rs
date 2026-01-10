use std::{cell::RefCell, collections::HashMap, hash::{DefaultHasher, Hash, Hasher}, rc::Rc, sync::{Arc, Mutex}};

use as_any::AsAny;
use eframe::egui::{self, Align2};
use mlua::{FromLua, Table, Value};

use crate::{app::{custom_widgets::{NumberField, NumericField}, ui::dialog::{Dialog, DialogAction, DialogActionButtons, names::{DIALOG_NAME_PLUGIN_DIALOG, DIALOG_NAME_PLUGIN_ERROR_DIALOG}}}, editor::{actions::EditorActions, editing::{lua_note_editing::LuaNoteEditing, note_editing::NoteEditing}, plugins::{plugin_error_dialog::PluginErrorDialog, plugin_lua::PluginLua}}};

fn hash_table_address(table: &Table) -> u64 {
    let ptr = table.to_pointer() as usize;
    let mut hasher = DefaultHasher::new();
    ptr.hash(&mut hasher);
    hasher.finish()
}

pub enum DialogField {
    Label { contents: String },
    Number { field_id: String, label: String, field: NumericField<f64> },
    Slider { field_id: String, label: String, value: f64, min: f64, max: f64, step: Option<f64> },
    TextField { field_id: String, label: String, value: String },
    Toggle { field_id: String, label: String, value: bool },
    Dropdown { field_id: String, label: String, value: usize, value_labels: Vec<String> },
    Separator
}

#[derive(Default)]
pub struct PluginDialog {
    plugin: Option<Rc<RefCell<PluginLua>>>,
    // field id, and field itself
    fields: Vec<DialogField>,
    pub curr_track: usize,

    note_editing: Arc<Mutex<NoteEditing>>,
    editor_actions: Rc<RefCell<EditorActions>>,
    plugin_run_result: Option<Result<(), mlua::Error>>,
    showing: bool
}

impl PluginDialog {
    pub fn init(&mut self, editor_actions: &Rc<RefCell<EditorActions>>, note_editing: &Arc<Mutex<NoteEditing>>) {
        // println!("[PluginDialog::init()] Track count: {}", note_editing.lock().unwrap().get_tracks().read().unwrap().len());
        self.editor_actions = editor_actions.clone();
        self.note_editing = note_editing.clone();
        self.plugin_run_result = Some(Ok(()));
    }

    /// Returns [`true`] if the dialog has fields and would need to be shown
    pub fn load_plugin_dialog(&mut self, plugin: &Rc<RefCell<PluginLua>>) -> Result<bool, mlua::Error> {
        self.plugin = Some(plugin.clone());

        self.fields.clear();
        
        let plugin = plugin.try_borrow().unwrap();
        if let Some(fields) = plugin.dialog_field_table.as_ref() {
            if fields.is_empty() {
                drop(plugin);
                return Ok(false);
            }

            for (idx, field) in fields.sequence_values::<Value>().enumerate() {
                let field = field?;
                let Some(field_table) = field.as_table() else {
                    println!("[PluginWarning] skipping field {idx} because it is not a table");
                    continue;
                };

                if field_table.is_empty() {
                    self.add_separator();
                    continue;
                }

                let field_id = field_table.get::<String>("id");
                let field_id = match field_id {
                    Ok(field_id) => field_id,
                    Err(_) => { // no field id present, maybe it's a label?
                        // so let's expect a label
                        let field_type = field_table.get::<String>("type")?;
                        if field_type != "label" {
                            return Err(mlua::Error::runtime(
                                format!("[PluginError] expected unnested field type to be label, not {field_type}")
                            ));
                        }
                        // ... it is a label, so push it to the fields
                        let field_label = field_table.get::<String>("label").unwrap_or("".into());
                        self.add_label(field_label);
                        continue;
                    }
                };

                let Ok(field_contents) = field_table.get::<Table>(1) else {
                    println!("[PluginWarning] skipping field {field_id} because the contents are empty");
                    continue;
                };

                let field_type = field_contents.get::<String>("type")?;

                match field_type.as_str() {
                    "separator" => {
                        self.add_separator();
                    },
                    "label" => { 
                        let label = field_contents.get::<String>("label").unwrap_or("".into()); 
                        self.add_label(label);
                    }, 
                    "number" => {
                        self.add_number(field_id, &field_contents);
                    },
                    "slider" => {
                        self.add_slider(field_id, &field_contents);
                    },
                    "textedit" => {
                        self.add_textedit(field_id, &field_contents);
                    }, 
                    "toggle" => {
                        self.add_toggle(field_id, &field_contents);
                    },
                    "dropdown" => {
                        self.add_dropdown(field_id, &field_contents)?;
                    }, 
                    _ => {
                        println!("[PluginWarning] Unknown field type \"{}\", skipping...", field_type);
                        continue;
                    }
                }
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn run_plugin(&mut self) -> Result<(), mlua::Error> {
        if self.plugin.is_none() { return Ok(()); }
        let plugin = self.plugin.as_ref().unwrap();

        let (lua, apply_fn) = {
            let p = plugin.try_borrow().unwrap();
            (p.lua.clone(), p.on_apply_fn.clone())
        };

        if apply_fn.is_none() { return Ok(()); }
        let apply_fn = apply_fn.unwrap();

        let mut lua_note_editing = LuaNoteEditing::new(&self.note_editing);

        match lua.scope(|scope| {
            let note_track_ref = scope.create_userdata_ref_mut(&mut lua_note_editing)?;
            apply_fn.call::<()>(note_track_ref)?;
            Ok(())
        }) {
            Ok(_) => {
                let mut editor_actions = self.editor_actions.try_borrow_mut().unwrap();
                lua_note_editing.apply_changes(self.curr_track as u16, &mut editor_actions);
                return Ok(());
            },
            Err(lua_error) => {
                let plugin = plugin.try_borrow().unwrap();
                println!("[PluginError] (While running {}): \n{}", plugin.plugin_name, lua_error);
                return Err(lua_error);
            }
        }
    }

    fn add_separator(&mut self) {
        self.fields.push(DialogField::Separator);
    }

    fn add_label(&mut self, label: String) {
        self.fields.push(DialogField::Label { contents: label });
    }

    fn add_number(&mut self, field_id: String, field_contents: &Table) {
        let (label, value) = self.get_label_and_value(field_contents);

        let (min, max) = if let Ok(number_range) = field_contents.get::<Table>("range") {
            (
                number_range.get::<f64>("min").ok(),
                number_range.get::<f64>("max").ok()
            )
        } else {
            (None, None)
        };

        let numeric_field = NumericField::new(value, min, max);
        self.fields.push(DialogField::Number { field_id, label, field: numeric_field });
    }

    fn add_slider(&mut self, field_id: String, field_contents: &Table) {
        let (label, value) = self.get_label_and_value(field_contents);

        let (min, max) = if let Ok(slider_range) = field_contents.get::<Table>("range") {
            (
                slider_range.get::<f64>("min").unwrap_or(0.0),
                slider_range.get::<f64>("max").unwrap_or(1.0)
            )
        } else {
            (0.0, 1.0)
        };

        let step = field_contents.get::<mlua::Number>("step").ok();
        self.fields.push(DialogField::Slider { field_id, label, value, min, max, step });
    }

    fn add_textedit(&mut self, field_id: String, field_contents: &Table) {
        let (label, value) = self.get_label_and_value(field_contents);
        self.fields.push(DialogField::TextField { field_id, label, value });
    }

    fn add_toggle(&mut self, field_id: String, field_contents: &Table) {
        let (label, value) = self.get_label_and_value(field_contents);
        self.fields.push(DialogField::Toggle { field_id, label, value });
    }

    fn add_dropdown(&mut self, field_id: String, field_contents: &Table) -> Result<(), mlua::Error> {
        let (label, mut value) = self.get_label_and_value(field_contents);

        let mut value_labels = Vec::new();
        if let Ok(val_labels) = field_contents.get::<Table>("value_labels") {
            let val_labels_len = val_labels.len().unwrap() as usize;
            if val_labels_len == 0 { return Err(mlua::Error::RuntimeError("Dropdown widget must contain at least one value".into())); }
            if value > val_labels_len { value = val_labels_len - 1; }

            for label in val_labels.sequence_values::<Value>() {
                value_labels.push(label.unwrap().to_string().unwrap());
            }
        } else {
            return Err(mlua::Error::RuntimeError("Dropdown widget must contain at least one value".into()));
        }

        self.fields.push(DialogField::Dropdown { field_id: field_id, label, value, value_labels });
        Ok(())
    }

    fn get_label_and_value<T: Default + FromLua>(&self, field_contents: &Table) -> (String, T) {
        let label = field_contents.get::<String>("label").unwrap_or("".into());
        let value = field_contents.get::<T>("value").unwrap_or_default();
        (label, value)
    }

    fn get_field_by_id(plugin: &Rc<RefCell<PluginLua>>, id: String) -> Option<Table> {
        let plugin = plugin.borrow();
        let dialog_fields = plugin.dialog_field_table.as_ref().unwrap();
        for field in dialog_fields.sequence_values::<Table>() {
            let field = field.unwrap();
            if let Ok(id_) = field.get::<String>("id") {
                if id == id_ { return Some(field.clone()); }
            }
        }
        None
    }

    pub fn get_plugin_run_result(&self) -> Result<(), mlua::Error> {
        if let Some(result) = self.plugin_run_result.as_ref() {
            result.clone()
        } else {
            Ok(())
        }
    }
}

impl Dialog for PluginDialog {
    fn draw(&mut self, ui: &mut egui::Ui, image_resources: &crate::app::util::image_loader::ImageResources) -> Option<crate::app::ui::dialog::DialogAction> {
        {
            let plugin = self.plugin.as_ref().unwrap();
            let plugin = plugin.try_borrow().unwrap();
            if let Some(plugin_info) = plugin.plugin_info.as_ref() {
                if let Some(desc) = plugin_info.description.as_ref() {
                    ui.label(desc);
                    ui.separator();
                }
            }
        }
        
        for field in self.fields.iter_mut() {
            match field {
                DialogField::Separator => {
                    ui.separator();
                },
                DialogField::Label { contents } => {
                    ui.label(contents.as_str());
                },
                DialogField::Number { field_id, label, field } => {
                    field.show(&label, ui, None);
                    if field.changed() {
                        let val = field.value();
                        Self::get_field_by_id(
                            self.plugin.as_ref().unwrap(),
                            field_id.to_string()
                        ).unwrap()
                            .get::<Table>(1).unwrap()
                            .set("value", val).unwrap();
                    }
                },
                DialogField::Slider { field_id, label, value, min, max, step } => {
                    ui.horizontal(|ui| {
                        ui.label(&*label);
                        let mut slider = egui::Slider::new(value, *min..=*max);
                        if let Some(step) = step { slider = slider.step_by(*step); }
                        
                        if ui.add(slider).changed() {
                            let plugin = self.plugin.as_ref().unwrap();
                            let field_id = field_id.to_string();

                            Self::get_field_by_id(plugin, field_id).unwrap()
                                .get::<Table>(1).unwrap()
                                .set("value", *value)
                                .unwrap();
                        }
                    });
                },
                DialogField::TextField { field_id, label, value } => {
                    ui.horizontal(|ui| {
                        ui.label(&*label);
                        if ui.text_edit_singleline(value).changed() {
                            let plugin = self.plugin.as_ref().unwrap();
                            let field_id = field_id.to_string();
                            
                            Self::get_field_by_id(plugin, field_id).unwrap()
                                .get::<Table>(1).unwrap()
                                .set("value", value.clone())
                                .unwrap();
                        }
                    });
                },
                DialogField::Toggle { field_id, label, value } => {
                    ui.horizontal(|ui| {
                        ui.label(&*label);
                        if ui.checkbox(value, "").changed() {
                            let plugin = self.plugin.as_ref().unwrap();
                            let field_id = field_id.to_string();
                            
                            Self::get_field_by_id(plugin, field_id).unwrap()
                                .get::<Table>(1).unwrap()
                                .set("value", *value)
                                .unwrap();
                        }
                    });
                },
                DialogField::Dropdown { field_id, label, value, value_labels } => {
                    ui.horizontal(|ui| {
                        ui.label(&*label);
                        if egui::ComboBox::from_id_salt(&*field_id)
                            .selected_text(&value_labels[*value])
                            .show_index(ui, &mut *value, value_labels.len(), |i| &value_labels[i]).changed() {
                                let plugin = self.plugin.as_ref().unwrap();
                                let field_id = field_id.to_string();
                                
                                Self::get_field_by_id(plugin, field_id).unwrap()
                                    .get::<Table>(1).unwrap()
                                    .set("value", *value)
                                    .unwrap();
                            }
                    });
                }
            }
        }

        None
    }

    fn get_dialog_name(&self) -> &'static str {
        DIALOG_NAME_PLUGIN_DIALOG
    }

    fn get_dialog_title(&self) -> String {
        let plugin = self.plugin.as_ref().unwrap();
        let plugin = plugin.try_borrow().unwrap();
        plugin.plugin_name.clone()
    }

    fn get_action_buttons(&self) -> Option<DialogActionButtons> {
        Some(
            DialogActionButtons::ApplyClose(
                // apply n run
                Box::new(|dlg| {
                    let dlg_name = dlg.get_dialog_name();
                    let dlg_plugin_lua = dlg.as_mut().as_any_mut().downcast_mut::<PluginDialog>().unwrap();
                    match dlg_plugin_lua.run_plugin() {
                        Ok(_) => Some(DialogAction::Close(dlg_name)),
                        Err(lua_err) => {
                            let plugin = dlg_plugin_lua.plugin.as_ref().unwrap();
                            let plugin_name = plugin.borrow().plugin_name.clone();
                            Some(DialogAction::Open(DIALOG_NAME_PLUGIN_ERROR_DIALOG, vec![
                                Box::new(plugin_name),
                                Box::new(lua_err.to_string())
                            ]))
                        }
                    }
                }),
                Box::new(|dlg| {
                    let dlg_name = dlg.get_dialog_name();
                    Some(DialogAction::Close(dlg_name))
                })
            )
        )
    }
    /*fn show(&mut self) -> () {
        self.showing = true;
    }

    fn close(&mut self) -> () {
        self.showing = false;
    }

    fn is_showing(&self) -> bool {
        self.showing
    }

    fn draw(&mut self, ctx: &eframe::egui::Context, _: &crate::app::util::image_loader::ImageResources) -> () {
        if !self.showing { return; }

        let plugin_window_title = {
            let plugin = self.plugin.as_ref().unwrap();
            let plugin = plugin.try_borrow().unwrap();
            plugin.plugin_name.clone()
        };

        egui::Window::new(plugin_window_title)
            .resizable(false)
            .collapsible(false)
            .pivot(Align2::CENTER_CENTER)
            .show(ctx, |ui| {
                {
                    let plugin = self.plugin.as_ref().unwrap();
                    let plugin = plugin.try_borrow().unwrap();
                    if let Some(plugin_info) = plugin.plugin_info.as_ref() {
                        if let Some(desc) = plugin_info.description.as_ref() {
                            ui.label(desc);
                            ui.separator();
                        }
                    }
                }
                
                for field in self.fields.iter_mut() {
                    match field {
                        DialogField::Separator => {
                            ui.separator();
                        },
                        DialogField::Label { contents } => {
                            ui.label(contents.as_str());
                        },
                        DialogField::Number { field_id, label, field } => {
                            field.show(&label, ui, None);
                            if field.changed() {
                                let val = field.value();
                                Self::get_field_by_id(
                                    self.plugin.as_ref().unwrap(),
                                    field_id.to_string()
                                ).unwrap()
                                    .get::<Table>(1).unwrap()
                                    .set("value", val).unwrap();
                            }
                        },
                        DialogField::Slider { field_id, label, value, min, max, step } => {
                            ui.horizontal(|ui| {
                                ui.label(&*label);
                                let mut slider = egui::Slider::new(value, *min..=*max);
                                if let Some(step) = step { slider = slider.step_by(*step); }
                                
                                if ui.add(slider).changed() {
                                    /*let plugin = self.plugin.as_ref().unwrap();
                                    let mut plugin = plugin.try_borrow_mut().unwrap();
                                    let dialog_fields = plugin.dialog_field_table.as_mut().unwrap();*/

                                    Self::get_field_by_id(
                                        self.plugin.as_ref().unwrap(),
                                        field_id.to_string()).unwrap()
                                        .set("value", *value)
                                        .unwrap();
                                }
                            });
                        },
                        DialogField::TextField { field_id, label, value } => {
                            ui.horizontal(|ui| {
                                ui.label(&*label);
                                if ui.text_edit_singleline(value).changed() {
                                    let plugin = self.plugin.as_ref().unwrap();
                                    let field_id = field_id.to_string();
                                    
                                    Self::get_field_by_id(plugin, field_id).unwrap()
                                        .set("value", value.clone())
                                        .unwrap();
                                }
                            });
                        },
                        DialogField::Toggle { field_id, label, value } => {
                            ui.horizontal(|ui| {
                                ui.label(&*label);
                                if ui.checkbox(value, "").changed() {
                                    let plugin = self.plugin.as_ref().unwrap();
                                    let field_id = field_id.to_string();
                                    
                                    Self::get_field_by_id(plugin, field_id).unwrap()
                                        .set("value", *value)
                                        .unwrap();
                                }
                            });
                        },
                        DialogField::Dropdown { field_id, label, value, value_labels } => {
                            ui.horizontal(|ui| {
                                ui.label(&*label);
                                if egui::ComboBox::from_id_salt(&*field_id)
                                    .selected_text(&value_labels[*value])
                                    .show_index(ui, &mut *value, value_labels.len(), |i| &value_labels[i]).changed() {
                                        let plugin = self.plugin.as_ref().unwrap();
                                        let field_id = field_id.to_string();
                                        
                                        Self::get_field_by_id(plugin, field_id).unwrap()
                                            .set("value", *value)
                                            .unwrap();
                                    }
                            });
                        }
                    }
                }

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Apply").clicked() {
                        self.run_plugin();
                        self.close();
                    }

                    if ui.button("Cancel").clicked() {
                        self.close();
                    }
                });
                
            });
    }*/
}