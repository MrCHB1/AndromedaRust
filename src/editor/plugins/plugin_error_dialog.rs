use std::{cell::RefCell, rc::Rc};

use eframe::egui::{self, Align2};

use crate::{app::ui::dialog::{Dialog, DialogAction, DialogActionButtons, flags::*, names::DIALOG_NAME_PLUGIN_ERROR_DIALOG}, editor::plugins::plugin_lua::PluginLua};

/// This shows whenever a plugin ran into an error while running.
pub struct PluginErrorDialog {
    plugin_name: String,
    error_msg: String,
}

impl PluginErrorDialog {
    pub fn new() -> Self {
        Self {
            plugin_name: "".into(),
            error_msg: "".into()
        }
    }
}

impl Dialog for PluginErrorDialog {
    fn init_dialog(&mut self, args: Vec<Box<dyn std::any::Any>>) -> Result<(), &'static str> {
        self.plugin_name = args[0].downcast_ref::<String>().unwrap().clone();
        self.error_msg = args[1].downcast_ref::<String>().unwrap().clone();
        Ok(())
    }

    fn draw(&mut self, ui: &mut egui::Ui, _: &crate::app::util::image_loader::ImageResources) -> Option<crate::app::ui::dialog::DialogAction> {
        ui.label(format!("Plugin '{}' failed to run. See error below.", self.plugin_name));
        ui.separator();
        ui.code(&self.error_msg);
        ui.label("Fix the error(s) mentioned above, then reload plugins and try again.");
        None
    }

    fn get_action_buttons(&self) -> Option<crate::app::ui::dialog::DialogActionButtons> {
        Some(DialogActionButtons::Ok(
            Box::new(|dlg| {
                let dlg_name = dlg.get_dialog_name();
                Some(DialogAction::Close(dlg_name))
            })
        ))
    }

    fn get_dialog_name(&self) -> &'static str {
        DIALOG_NAME_PLUGIN_ERROR_DIALOG
    }

    fn get_dialog_title(&self) -> String {
        "Plugin Error!".into()
    }

    fn get_flags(&self) -> u16 {
        DIALOG_NO_COLLAPSABLE
    }
}