use crate::{EDITOR_STAGE, EDITOR_VERSION, app::{ui::dialog::{Dialog, DialogAction, DialogActionButtons, dialog_default_close_action, flags::*, names::DIALOG_NAME_EDITOR_INFO}, util::image_loader::ImageResources}};
use eframe::egui;

#[derive(Default)]
pub struct EditorInfo;

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
            DialogActionButtons::Ok(
                dialog_default_close_action()
            )
        )
    }

    fn get_flags(&self) -> u16 {
        DIALOG_NO_COLLAPSABLE | DIALOG_NO_RESIZABLE
    }
}