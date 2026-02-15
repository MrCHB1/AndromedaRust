use std::sync::{Arc, Mutex, RwLock};

use crate::{app::ui::dialog::{Dialog, DialogAction, DialogActionButtons, names::DIALOG_NAME_FILTER_CHANNELS}, editor::editing::{SharedSelectedNotes, note_editing::NoteEditing}};

#[derive(Default)]
pub struct FilterChannelsDialog {
    channels_filter: [bool; 16],
    should_filter: bool,
    shared_selected_notes: Arc<RwLock<SharedSelectedNotes>>,
    note_editing: Arc<Mutex<NoteEditing>>,
}

impl Dialog for FilterChannelsDialog {
    fn init_dialog(&mut self, args: Vec<Box<dyn std::any::Any>>) -> Result<(), &'static str> {
        let shared_selected_notes = args[0].as_ref().downcast_ref::<Arc<RwLock<SharedSelectedNotes>>>().unwrap();
        let note_editing = args[1].as_ref().downcast_ref::<Arc<Mutex<NoteEditing>>>().unwrap();

        self.channels_filter = [false; 16];
        self.should_filter = false;
        self.shared_selected_notes = shared_selected_notes.clone();
        self.note_editing = note_editing.clone();

        Ok(())
    }

    fn draw(&mut self, ui: &mut eframe::egui::Ui, _: &crate::app::util::image_loader::ImageResources) -> Option<crate::app::ui::dialog::DialogAction> {
        // draw first 8 rows
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                for (chan, filter) in self.channels_filter[0..8].iter_mut().enumerate() {
                    ui.checkbox(filter, chan.to_string());
                }
            });
            ui.horizontal(|ui| {
                for (chan, filter) in self.channels_filter[8..16].iter_mut().enumerate() {
                    ui.checkbox(filter, (chan + 8).to_string());
                }
            });
        });

        None
    }

    fn get_dialog_name(&self) -> &'static str {
        DIALOG_NAME_FILTER_CHANNELS
    }

    fn get_dialog_title(&self) -> String {
        "Filter selection channels".into()
    }

    fn get_action_buttons(&self) -> Option<crate::app::ui::dialog::DialogActionButtons> {
        Some(DialogActionButtons::ApplyClose(
            Box::new(|dlg| {
                let dlg = dlg.as_any_mut().downcast_mut::<Self>().unwrap();

                dlg.should_filter = true;

                let mut shared_selected_notes = dlg.shared_selected_notes.write().unwrap();
                
                let note_editing = dlg.note_editing.lock().unwrap();
                let tracks = note_editing.get_tracks();
                let tracks = tracks.read().unwrap();

                let active_tracks = shared_selected_notes.get_active_selected_tracks();
                
                for &track in active_tracks.iter() {
                    let notes = (*tracks)[track as usize].get_notes();

                    let mut kept_ids = Vec::new();
                    for id in shared_selected_notes.get_selected_ids_in_track(track).unwrap() {
                        let channel = notes[*id].channel();
                        if dlg.channels_filter[channel as usize] {
                            kept_ids.push(*id);
                        }
                    }

                    shared_selected_notes.set_selected_in_track(kept_ids, track);
                }

                Some(DialogAction::Close(dlg.get_dialog_name()))
            }), 
            Box::new(|dlg| {
                Some(DialogAction::Close(dlg.get_dialog_name()))
            })
        ))
    }
}