use std::{cell::RefCell, rc::Rc};

use eframe::egui::{self, Ui};

use crate::app::{ui::{dialog::{Dialog, DialogAction, DialogActionButtons, flags::*}, dialog_manager::DialogManager}, util::image_loader::ImageResources};

#[derive(Default)]
pub struct DialogDrawer {
    dialog_manager: Rc<RefCell<DialogManager>>
}

impl DialogDrawer {
    pub fn init(&mut self, manager: &Rc<RefCell<DialogManager>>) {
        self.dialog_manager = manager.clone();
    }

    pub fn draw_all_dialogs(&mut self, ctx: &egui::Context, image_resources: &ImageResources) {
        let mut dialog_actions = Vec::new();
        
        // drawing dialogs logic
        {
            let mut dialog_manager = self.dialog_manager.borrow_mut();
            
            for dialog in dialog_manager.get_opened_dialogs() {
                let (action, btn_action) = self.draw_dialog(dialog, ctx, image_resources);
                
                if let Some(action) = action {
                    dialog_actions.push(action);
                }

                if let Some(btn_action) = btn_action {
                    dialog_actions.push(btn_action);
                }
            }
        }

        // process dialog actions
        self.handle_dialog_actions(ctx, dialog_actions);
    }

    fn draw_dialog(&self, dialog: &mut Box<dyn Dialog + 'static>, ctx: &egui::Context, image_resources: &ImageResources) -> (Option<DialogAction>, Option<DialogAction>) {
        let dialog_window_title = dialog.get_dialog_title();

        let mut action = None;
        let mut btn_action = None;
        egui::Window::new(dialog_window_title)
            .collapsible(!dialog.flag_enabled(DIALOG_NO_COLLAPSABLE))
            .resizable(!dialog.flag_enabled(DIALOG_NO_RESIZABLE))
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    action = dialog.draw(ui, image_resources);

                    if let Some(action_buttons) = dialog.get_action_buttons() {
                        btn_action = self.draw_action_buttons(dialog, ui, action_buttons);
                    }
                });
            });

        (action, btn_action)
    }

    /// Returns a bool that determines if a dialog closes after an action
    fn draw_action_buttons(&self, dialog: &mut Box<dyn Dialog + 'static>, ui: &mut Ui, action_buttons: DialogActionButtons) -> Option<DialogAction> {
        let mut action = None;
        
        ui.separator();
        ui.horizontal(|ui| {
            match action_buttons {
                DialogActionButtons::YesNo(mut yes_callback, mut no_callback) => {
                    if ui.button("Yes").clicked() { action = yes_callback(dialog); }
                    if ui.button("No").clicked() { action = no_callback(dialog); }
                },
                DialogActionButtons::Ok(mut ok_callback) => {
                    if ui.button("Ok").clicked() { action = ok_callback(dialog); }
                },
                DialogActionButtons::OkCancel(mut ok_callback, mut cancel_callback) => {
                    if ui.button("Ok").clicked() { action = ok_callback(dialog); }
                    if ui.button("Cancel").clicked() {action = cancel_callback(dialog); }
                },
                DialogActionButtons::ApplyClose(mut apply_callback, mut close_callback) => {
                    if ui.button("Apply").clicked() { action = apply_callback(dialog); }
                    if ui.button("Close").clicked() { action = close_callback(dialog); }
                }
            }
        });

        action
    }

    fn handle_dialog_actions(&mut self, ctx: &egui::Context, dialog_actions: Vec<DialogAction>) {
        let mut dialog_manager = self.dialog_manager.borrow_mut();
        for action in dialog_actions.into_iter() {
            match action {
                DialogAction::Open(dialog_id, args) => {
                    dialog_manager.open_dialog_by_name(&dialog_id, args);
                },
                DialogAction::Close(dialog_id) => {
                    dialog_manager.close_dialog(&dialog_id);
                },
                DialogAction::TerminateApp => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }
}