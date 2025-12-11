use crate::app::{ui::dialog::{Dialog, DialogAction, DialogActionButtons, flags::*, names::DIALOG_NAME_EDITOR_INFO}, util::image_loader::ImageResources};
use eframe::egui;

const EDITOR_VERSION: &'static str = "2.3";
const EDITOR_STAGE: &'static str = "Beta";

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
        /*ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Close").clicked() {
                self.close();
            }
        });*/
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

    /*fn show(&mut self) -> () {
        self.showing = true;
    }

    fn close(&mut self) -> () {
        self.showing = false;
    }

    fn is_showing(&self) -> bool {
        self.showing
    }

    fn draw(&mut self, ctx: &eframe::egui::Context, images: &ImageResources) -> () {
        if !self.is_showing() { return; }
        egui::Window::new("About Andromeda")
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.heading("Andromeda");
                            ui.label(format!("VERSION {}-{}", EDITOR_VERSION, EDITOR_STAGE));
                        });
                        ui.separator();
                        ui.image(&*images.get_image_handle(String::from("logo_medium")));
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Close").clicked() {
                            self.close();
                        }
                    });
                })
            });
    }*/
}