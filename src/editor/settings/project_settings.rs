use eframe::egui::{self, RichText};

use crate::{app::ui::dialog::Dialog, editor::{midi_bar_cacher::BarCacher, project::{project_data::ProjectData, project_manager::ProjectManager}}};
use core::f32;
use std::sync::{Arc, RwLock, Mutex};
use crate::app::custom_widgets::{NumberField, NumericField};

pub struct ProjectSettings {
    pub project_manager: Arc<RwLock<ProjectManager>>,
    is_showing: bool,

    custom_ppq_field: NumericField<u16>,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            project_manager: Default::default(),
            is_showing: false,
            custom_ppq_field: NumericField::new(960, Some(1), Some(0x7FFF))
        }
    }
}

impl ProjectSettings {
    pub fn new(project_data: &Arc<RwLock<ProjectManager>>) -> Self {
        Self { 
            project_manager: project_data.clone(),
            is_showing: false,
            custom_ppq_field: NumericField::new(project_data.read().unwrap().project_data.ppq, Some(1), Some(0x7FFF))
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

        let mut project_manager = self.project_manager.write().unwrap();

        egui::Window::new(RichText::new("Project Information").size(15.0))
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                // ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    {
                        let project_info = project_manager.get_project_info_mut();
                        
                        ui.horizontal(|ui| {
                            ui.label("Name");
                            ui.text_edit_singleline(&mut project_info.name);
                        });

                        ui.horizontal(|ui| {
                            ui.label("Author");
                            ui.text_edit_singleline(&mut project_info.author);
                        });

                        ui.horizontal(|ui| {
                            ui.label("Description");
                            ui.text_edit_multiline(&mut project_info.description);
                        });
                    }

                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("PPQ");

                        // let mut project_manager = self.project_manager.write().unwrap();
                        let ppq = project_manager.get_ppq();
                        egui::ComboBox::from_label("")
                            .selected_text(format!("{}", ppq))
                            .show_ui(ui, |ui| {
                                let ppq_values = [96, 120, 192, 240, 384, 480, 768, 960, 1920, 3840];
                                let old_value = ppq;
                                let mut new_value = ppq;
                                for ppq in ppq_values {
                                    ui.selectable_value(&mut new_value, ppq, format!("{}", ppq));
                                }
                                
                                if new_value != old_value {
                                    project_manager.change_ppq(new_value);
                                    self.custom_ppq_field.set_value(new_value);
                                }
                            });

                        if self.custom_ppq_field.show("Or custom value", ui, None).lost_focus() {
                            if ppq != self.custom_ppq_field.value() {
                                project_manager.change_ppq(self.custom_ppq_field.value());
                            }
                        }
                    });

                    ui.horizontal(|ui| {
                        if ui.button("Close").clicked() {
                            self.is_showing = false;
                        }
                    });
                });
            //});
    }
}