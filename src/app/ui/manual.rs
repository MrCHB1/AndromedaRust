use eframe::egui::{self, text::LayoutJob, Color32, FontId, RichText, Stroke, TextFormat, Ui};
use crate::app::{ui::dialog::{Dialog, DialogAction, DialogActionButtons, names::DIALOG_NAME_EDITOR_MANUAL}, util::image_loader::ImageResources};

#[derive(PartialEq, Eq)]
enum EditorManualSection {
    Welcome,
    Navigating
}

impl Default for EditorManualSection {
    fn default() -> Self {
        EditorManualSection::Welcome
    }
}

#[derive(Default)]
pub struct EditorManualDialog {
    showing: bool,
    manual_section: EditorManualSection,
}

impl EditorManualDialog {
    fn draw_welcome_tab(&self, ui: &mut Ui) {
        ui.label(RichText::new("Welcome to Andromeda.").size(20.0));
        ui.label("Welcome to andromeda, the most well-optimized MIDI editor there is out there.");
    }

    fn draw_navigating_tab(&self, ui: &mut Ui) {
        ui.label(RichText::new("Navigating").size(20.0));
        ui.label("If you are familiar with any MIDI editor such as FL Studio or Domino, navigation in Andromeda is pretty straightforward, however, do note that some things in this editor differ.");
        ui.label({
            let mut text = LayoutJob::default();
            text.append("To switch between the ", 0.0, 
                TextFormat { font_id: FontId::proportional(14.0), color: Color32::PLACEHOLDER, ..Default::default() });
            text.append("Piano Roll", 0.0,
                TextFormat { font_id: FontId::proportional(14.0), color: Color32::PLACEHOLDER, underline: Stroke::new(1.0, Color32::PLACEHOLDER), ..Default::default() });
            text.append(" and the ", 0.0, 
                TextFormat { font_id: FontId::proportional(14.0), color: Color32::PLACEHOLDER, ..Default::default() });
            text.append("Track View", 0.0,
                TextFormat { font_id: FontId::proportional(14.0), color: Color32::PLACEHOLDER, underline: Stroke::new(1.0, Color32::PLACEHOLDER), ..Default::default() });
            text.append(", press ", 0.0, 
                TextFormat { font_id: FontId::proportional(14.0), color: Color32::PLACEHOLDER, ..Default::default() });
            text.append("[TAB]", 0.0, 
                TextFormat { font_id: FontId::monospace(14.0), color: Color32::PLACEHOLDER, ..Default::default() });
            text.append(".", 0.0, 
                TextFormat { font_id: FontId::proportional(14.0), color: Color32::PLACEHOLDER, ..Default::default() });
            text
        });
    }
}

impl Dialog for EditorManualDialog {
    fn draw(&mut self, ui: &mut Ui, _: &ImageResources) -> Option<super::dialog::DialogAction> {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                if ui.selectable_label(self.manual_section == EditorManualSection::Welcome, "Welcome").clicked() {
                    self.manual_section = EditorManualSection::Welcome
                }

                if ui.selectable_label(self.manual_section == EditorManualSection::Navigating, "Navigating").clicked() {
                    self.manual_section = EditorManualSection::Navigating
                }
            });
            ui.separator();
            ui.vertical(|ui| {
                egui::ScrollArea::vertical()
                    .min_scrolled_height(1000.0)
                    .show(ui,  |ui| {
                        match self.manual_section {
                            EditorManualSection::Welcome => {
                                self.draw_welcome_tab(ui);
                            },
                            EditorManualSection::Navigating => {
                                self.draw_navigating_tab(ui);
                            }
                        }
                    })
            });
        });

        None
    }

    fn get_dialog_name(&self) -> &'static str {
        DIALOG_NAME_EDITOR_MANUAL
    }

    fn get_dialog_title(&self) -> String {
        "Andromeda Manual".into()
    }

    fn get_action_buttons(&self) -> Option<super::dialog::DialogActionButtons> {
        Some(
            DialogActionButtons::Ok(Box::new(|_| { Some(DialogAction::Close(DIALOG_NAME_EDITOR_MANUAL)) }))
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

    fn draw(&mut self, ctx: &egui::Context, _: &ImageResources) -> () {
        if !self.is_showing() { return; }
        egui::Window::new(RichText::new("Andromeda Manual").size(20.0))
            .collapsible(false)
            .default_width(1000.0)
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            if ui.selectable_label(self.manual_section == EditorManualSection::Welcome, "Welcome").clicked() {
                                self.manual_section = EditorManualSection::Welcome
                            }

                            if ui.selectable_label(self.manual_section == EditorManualSection::Navigating, "Navigating").clicked() {
                                self.manual_section = EditorManualSection::Navigating
                            }
                        });
                        ui.separator();
                        ui.vertical(|ui| {
                            egui::ScrollArea::vertical()
                                .min_scrolled_height(1000.0)
                                .show(ui,  |ui| {
                                    match self.manual_section {
                                        EditorManualSection::Welcome => {
                                            self.draw_welcome_tab(ui);
                                        },
                                        EditorManualSection::Navigating => {
                                            self.draw_navigating_tab(ui);
                                        }
                                    }
                                })
                        });
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Close").clicked() {
                            self.close();
                        }
                    });
                });
            });
    }
    */
}