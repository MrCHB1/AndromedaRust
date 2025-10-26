use crate::app::{ui::dialog::Dialog, util::image_loader::ImageResources};
use eframe::egui;

const EDITOR_VERSION: &'static str = "2.2";
const EDITOR_STAGE: &'static str = "Beta";

#[derive(Default)]
pub struct EditorInfo {
    showing: bool
}

impl Dialog for EditorInfo {
    fn show(&mut self) -> () {
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
    }
}