use eframe::egui::{self, RichText};

use crate::{app::ui::dialog::Dialog, editor::project_data::ProjectData};
use std::{cell::RefCell, rc::Rc};

pub struct ProjectSettings {
    pub project_data: Rc<RefCell<ProjectData>>,
    is_showing: bool,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            project_data: Default::default(),
            is_showing: false
        }
    }
}

impl ProjectSettings {
    pub fn new(project_data: &Rc<RefCell<ProjectData>>) -> Self {
        Self { 
            project_data: project_data.clone(),
            is_showing: false
        }
    }
}

impl Dialog for ProjectSettings {
    fn show(&mut self) {
        self.is_showing = true;
    }

    fn close(&mut self) {
        self.is_showing = false;
    }

    fn is_showing(&self) -> bool {
        self.is_showing
    }

    fn draw(&mut self, ctx: &egui::Context, _image_resources: &crate::app::util::image_loader::ImageResources) {
        if !self.is_showing { return; }

        egui::Window::new(RichText::new("Project Information").size(15.0))
            .collapsible(false)
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    let mut project_data = self.project_data.borrow_mut();

                    ui.horizontal(|ui| {
                        ui.label("Name");
                        let mut project_name = project_data.project_info.name;
                        ui.add(egui::TextEdit::singleline(&mut project_name));
                        //ui.text_edit_singleline(&mut project_data.project_info.name);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Author");
                        ui.text_edit_singleline(&mut project_data.project_info.author);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Description");
                        ui.text_edit_multiline(&mut project_data.project_info.description);
                    });

                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("PPQ");
                        egui::ComboBox::from_label(format!("{}", project_data.project_info.ppq))
                            .selected_text(format!("{}", project_data.project_info.ppq))
                            .show_ui(ui, |ui| {
                                let ppq_values = [96, 120, 192, 240, 384, 480, 768, 960, 1920, 3840];
                                for ppq in ppq_values {
                                    ui.selectable_value(&mut project_data.project_info.ppq, ppq, format!("{}", ppq));
                                }
                            });
                    });

                    ui.horizontal(|ui| {
                        if ui.button("Close").clicked() {
                            self.is_showing = false;
                        }
                    });
                });
            });
    }
}