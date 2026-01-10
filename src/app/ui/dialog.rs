use std::any::Any;

use as_any::AsAny;
use eframe::egui::Ui;
use crate::app::{ui::dialog_manager::MaybeDlgAction, util::image_loader::ImageResources};

// Dialog flags
pub mod flags {
    pub const DIALOG_NO_COLLAPSABLE: u16 = 0x1;
    pub const DIALOG_NO_RESIZABLE: u16 = 0x2;
}

// Dialog names
pub mod names {
    pub const DIALOG_NAME_EF_STRETCH: &'static str = "EFStretchDialog";
    pub const DIALOG_NAME_EF_CHOP: &'static str = "EFChopDialog";
    pub const DIALOG_NAME_EF_GLUE: &'static str = "EFGlueDialog";
    pub const DIALOG_NAME_EDITOR_SETTINGS: &'static str = "EditorSettings";
    pub const DIALOG_NAME_PROJECT_SETTINGS: &'static str = "ProjectSettings";
    pub const DIALOG_NAME_INSERT_META: &'static str = "InsertMeta";
    pub const DIALOG_NAME_EDITOR_MANUAL: &'static str = "EditorManual";
    pub const DIALOG_NAME_EDITOR_INFO: &'static str = "EditorInfo";
    pub const DIALOG_NAME_PLUGIN_DIALOG: &'static str = "LuaPluginDialog";
    pub const DIALOG_NAME_PLUGIN_ERROR_DIALOG: &'static str = "LuaPluginErrorDialog";
}

pub enum DialogAction {
    // dialog_id, args
    Open(&'static str, Vec<Box<dyn Any>>),
    Close(&'static str)
}

type DlgButtonAction = Box<dyn FnMut(&mut Box<dyn Dialog>) -> MaybeDlgAction + 'static>;

pub enum DialogActionButtons {
    YesNo(DlgButtonAction, DlgButtonAction),
    Ok(DlgButtonAction),
    OkCancel(DlgButtonAction, DlgButtonAction),
    ApplyClose(DlgButtonAction, DlgButtonAction)
}

use flags::*;

pub trait Dialog: AsAny {
    /// Called before the dialog is shown.
    /// If initialization was unsuccessful, the resulting [Err] will contain the message explaining why it failed.
    fn init_dialog(&mut self, _: Vec<Box<dyn Any>>) -> Result<(), &'static str> { Ok(()) }
    
    // fn draw(&mut self, ctx: &egui::Context, image_resources: &ImageResources) -> Option<DialogAction>;
    fn draw(&mut self, ui: &mut Ui, image_resources: &ImageResources) -> Option<DialogAction>;
    /// Called before the dialog closes.
    /// If cleanup was unsuccessful, the resulting [Err] will contain the message explaining why it failed.
    fn cleanup_dialog(&mut self) -> Result<(), &'static str> { Ok(()) }

    /// Gets the Dialog's internal name (NOT the dialog's window title)
    fn get_dialog_name(&self) -> &'static str;
    fn get_dialog_title(&self) -> String;
    fn get_action_buttons(&self) -> Option<DialogActionButtons> { None }
    fn get_flags(&self) -> u16 { DIALOG_NO_COLLAPSABLE | DIALOG_NO_RESIZABLE }
    fn flag_enabled(&self, flag: u16) -> bool {
        self.get_flags() & flag != 0
    }
}