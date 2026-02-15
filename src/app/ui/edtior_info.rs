use crate::app::{ui::dialog::{Dialog, DialogAction, DialogActionButtons, flags::*, names::DIALOG_NAME_EDITOR_INFO}, util::image_loader::ImageResources};
use eframe::egui;

pub const EDITOR_VERSION: &'static str = "2.5p3";
pub const EDITOR_STAGE: &'static str = "Beta";

#[derive(Default)]
pub struct EditorInfo {
    showing: bool
}

impl Dialog for EditorInfo {
    fn draw(&mut self, ui: &mut egui::Ui, image_resources: &ImageResources) -> Option<super::dialog::DialogAction> {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.heading("Andromeda");
                ui.label(format!("VERSION {}-{}", EDITOR_VERSION, EDITOR_STAGE));
            });
            ui.separator();
            ui.image(&*image_resources.get_image_handle(String::from("logo_medium")));
        });

        None
    }

    fn get_dialog_name(&self) -> &'static str {
        DIALOG_NAME_EDITOR_INFO
    }

    fn get_dialog_title(&self) -> String {
        "Editor Info".into()
    }

    fn get_action_buttons(&self) -> Option<super::dialog::DialogActionButtons> {
        Some(
            DialogActionButtons::Ok(Box::new(|dlg| {
                let dlg_name = dlg.get_dialog_name();
                Some(DialogAction::Close(dlg_name))
            }))
        )
    }

    fn get_flags(&self) -> u16 {
        DIALOG_NO_COLLAPSABLE | DIALOG_NO_RESIZABLE
    }
}