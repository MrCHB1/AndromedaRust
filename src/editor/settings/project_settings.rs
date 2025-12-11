use eframe::egui::{self, RichText};

use crate::{app::ui::dialog::{Dialog, DialogAction, DialogActionButtons, flags::{DIALOG_NO_COLLAPSABLE, DIALOG_NO_RESIZABLE}, names::DIALOG_NAME_PROJECT_SETTINGS}, editor::{midi_bar_cacher::BarCacher, project::{project_data::ProjectData, project_manager::ProjectManager}}};
use core::f32;
use std::sync::{Arc, RwLock, Mutex};

pub struct ProjectSettings {
    pub project_manager: Arc<RwLock<ProjectManager>>,
    is_showing: bool,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            project_manager: Default::default(),
            is_showing: false
        }
    }
}

impl ProjectSettings {
    pub fn new(project_data: &Arc<RwLock<ProjectManager>>) -> Self {
        Self { 
            project_manager: project_data.clone(),
            is_showing: false
        }
    }
}

impl Dialog for ProjectSettings {
    fn draw(&mut self, ui: &mut egui::Ui, _: &crate::app::util::image_loader::ImageResources) -> Option<DialogAction> {
        let mut project_manager = self.project_manager.write().unwrap();

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
                    }
                });
        });

        None
    }

    fn get_dialog_name(&self) -> &'static str {
        DIALOG_NAME_PROJECT_SETTINGS
    }

    fn get_dialog_title(&self) -> String {
        "Project Information".into()
    }

    fn get_flags(&self) -> u16 {
        DIALOG_NO_COLLAPSABLE | DIALOG_NO_RESIZABLE
    }

    /*fn show(&mut self) {
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
                                }
                            });
                    });

                    ui.horizontal(|ui| {
                        if ui.button("Close").clicked() {
                            self.is_showing = false;
                        }
                    });
                });
            //});
    }*/

    fn get_action_buttons(&self) -> Option<DialogActionButtons> {
        Some(
            DialogActionButtons::Ok(
                Box::new(|dlg| {
                    let dlg_name = dlg.get_dialog_name();
                    Some(DialogAction::Close(dlg_name))
                })
            )
        )
    }
}