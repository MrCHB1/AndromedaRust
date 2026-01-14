#![warn(unused)]

use as_any::AsAny;
use eframe::egui::{self, RichText, Ui};

use crate::{app::{custom_widgets::{NumberField, NumericField}, ui::dialog::{Dialog, DialogAction, DialogActionButtons, flags::DIALOG_NO_COLLAPSABLE, names::DIALOG_NAME_EDITOR_SETTINGS}}, audio::{event_playback::PlaybackManager, kdmapi_engine::kdmapi::KDMAPI, midi_devices::MIDIDevices}};
use std::{collections::HashMap, sync::{Arc, Mutex}};
use std::any::Any;

pub const PR_KEYBOARD_WIDTH: f32 = 100.0;

pub trait Settings: Any {
    fn as_any(&self) -> &dyn Any;
}

pub struct ESGeneralSettings {
    // import settings
    import_discard_empty_tracks: bool,
    import_keep_empty_with_cc: bool,
    import_reassign_channels: bool,
    import_reassign_channel_10_as_11: bool,
    import_max_ppq_override: bool,
    import_max_ppq_override_value: NumericField<u16>,
    import_remove_overlaps: bool,

    export_discard_empty_tracks: bool
}

impl Default for ESGeneralSettings {
    fn default() -> Self {
        Self {
            import_discard_empty_tracks: false,
            import_keep_empty_with_cc: true,
            import_reassign_channels: false,
            import_reassign_channel_10_as_11: false,
            import_max_ppq_override: false,
            import_max_ppq_override_value: NumericField::new(960, Some(96), Some(7680)),
            import_remove_overlaps: false,

            export_discard_empty_tracks: true
        }
    }
}

impl Settings for ESGeneralSettings {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum ESAudioEngineType {
    MidiIO,
    KDMAPI,
    Prerendered
}

pub struct ESAudioSettings {
    md_engine: ESAudioEngineType,
    md_port_in: usize,
    md_port_out: usize,

    // advanced settings
    md_event_pool_size: NumericField<usize>
}

impl ESAudioSettings {
    pub fn get_engine(&self) -> &ESAudioEngineType {
        &self.md_engine
    }
}

impl Default for ESAudioSettings {
    fn default() -> Self {
        Self {
            md_engine: ESAudioEngineType::KDMAPI,
            md_port_in: 0,
            md_port_out: 0,
            md_event_pool_size: NumericField::new(4096, Some(100), Some(262144))
        }
    }
}

impl Settings for ESAudioSettings {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(PartialEq)]
enum ESCurrentSettings {
    General,
    Audio
}

impl Default for ESCurrentSettings {
    fn default() -> Self {
        ESCurrentSettings::General
    }
}

#[derive(Default)]
pub struct ESSettingsWindow {
    is_shown: bool,
    curr_settings: ESCurrentSettings,
    general_settings: ESGeneralSettings,
    audio_settings: ESAudioSettings,

    midi_devices: Option<Arc<Mutex<MIDIDevices>>>,
    kdmapi: Option<Arc<Mutex<KDMAPI>>>,
    playback_manager: Option<Arc<Mutex<PlaybackManager>>>
}

impl ESSettingsWindow {
    pub fn show(&mut self) {
        self.is_shown = true;
    }

    pub fn use_midi_devices(&mut self, devices: &Arc<Mutex<MIDIDevices>>) {
        self.midi_devices = Some(devices.clone());
    }

    pub fn use_kdmapi(&mut self, kdmapi: &Arc<Mutex<KDMAPI>>) {
        self.kdmapi = Some(kdmapi.clone());
    }

    pub fn use_playback_manager(&mut self, playback_manager: &Arc<Mutex<PlaybackManager>>) {
        self.playback_manager = Some(playback_manager.clone());
    }

    fn draw_general_tab(&mut self, ui: &mut Ui) {
        let general_settings = &mut self.general_settings;
        ui.label(RichText::new("MIDI Import").size(15.0));
        {
            // ===== track discarding =====
            ui.checkbox(&mut general_settings.import_discard_empty_tracks, "Discard empty tracks");
            ui.add_enabled_ui(general_settings.import_discard_empty_tracks, |ui | {
                ui.checkbox(&mut general_settings.import_keep_empty_with_cc, "Keep empty tracks containing non-note events");
            });

            // ===== channel reassignment =====
            ui.checkbox(&mut general_settings.import_reassign_channels, "Reassign channels");
            ui.checkbox(&mut general_settings.import_reassign_channel_10_as_11, "Reassign channel 10 to channel 11");

            // ===== ppq clamping =====
            ui.checkbox(&mut general_settings.import_max_ppq_override, "Keep PPQ at a Maximum").on_hover_text_at_pointer("If any imported MIDI's PPQ exceeds the specified PPQ, the MIDI will be quantized.");
            ui.add_enabled_ui(general_settings.import_max_ppq_override, |ui| {
                general_settings.import_max_ppq_override_value.show("Max PPQ", ui, None);
            });
            
            // ===== overlaps remover =====
            ui.checkbox(&mut general_settings.import_remove_overlaps, "Remove overlaps");
        }
        ui.separator();
        ui.label(RichText::new("MIDI Export").size(15.0));
        {
            ui.checkbox(&mut general_settings.export_discard_empty_tracks, "Discard empty tracks");
        }
    } 

    fn draw_audio_tab(&mut self, ui: &mut Ui) {
        let playback_manager = self.playback_manager.as_mut().unwrap();
        
        ui.horizontal(|ui| {
            let mut playback_manager = playback_manager.lock().unwrap();
            if ui.selectable_label(self.audio_settings.md_engine == ESAudioEngineType::MidiIO, "MIDI I/O").clicked() {
                self.audio_settings.md_engine = ESAudioEngineType::MidiIO;
                
                let midi_dev = self.midi_devices.as_ref().unwrap().clone();
                playback_manager.switch_device(midi_dev);
            }

            if ui.selectable_label(self.audio_settings.md_engine == ESAudioEngineType::KDMAPI, "KDMAPI").clicked() {
                self.audio_settings.md_engine = ESAudioEngineType::KDMAPI;

                let kdmapi = self.kdmapi.as_ref().unwrap().clone();
                playback_manager.switch_device(kdmapi);
            }

            ui.add_enabled_ui(false, |ui| {
                if ui.selectable_label(self.audio_settings.md_engine == ESAudioEngineType::Prerendered, "Prerendered Audio (Not Implemented)").clicked() {
                    self.audio_settings.md_engine = ESAudioEngineType::Prerendered;
                }
            });
            
        });

        ui.separator();

        match self.audio_settings.md_engine {
            ESAudioEngineType::MidiIO => {
                self.draw_audio_tab_midi_io(ui);
            },
            ESAudioEngineType::KDMAPI => {
                self.draw_audio_tab_kdmapi(ui);
            },
            ESAudioEngineType::Prerendered => {
                self.draw_audio_tab_kdmapi(ui);
            },
        }
    }

    fn draw_audio_tab_midi_io(&mut self, ui: &mut Ui) {
        if let Some(midi_devices) = self.midi_devices.as_ref() {
            let audio_settings = &mut self.audio_settings;
            ui.label(RichText::new("MIDI Input Devices").size(15.0));
            {
                let midi_in_names = {
                    let midi_devices = midi_devices.lock().unwrap();
                    midi_devices.get_midi_in_port_names().clone()
                };

                for (i, in_name) in midi_in_names.iter().enumerate() {
                    if ui.selectable_label(audio_settings.md_port_in == i, in_name).clicked() {
                        audio_settings.md_port_in = i;
                        let mut midi_devices = midi_devices.lock().unwrap();
                        midi_devices.connect_in_port(i).unwrap();
                    }
                    //ui.label(in_name);
                }
            }
            ui.separator();
            ui.label(RichText::new("MIDI Output Devices").size(15.0));
            {
                let midi_out_names = {
                    let midi_devices = midi_devices.lock().unwrap();
                    midi_devices.get_midi_out_port_names().clone()
                };

                for (i, out_name) in midi_out_names.iter().enumerate() {
                    if ui.selectable_label(audio_settings.md_port_out == i, out_name).clicked() {
                        audio_settings.md_port_out = i;
                        let mut midi_devices = midi_devices.lock().unwrap();
                        midi_devices.connect_out_port(i).unwrap()
                    }
                    //ui.label(in_name);
                }
            }
            ui.separator();
            ui.label(RichText::new("Advanced").size(15.0));
            {
                audio_settings.md_event_pool_size.show("MIDI Event pool size", ui, None);
                if audio_settings.md_event_pool_size.changed {
                    if let Some(playback_manager) = self.playback_manager.as_ref() {
                        let mut playback_manager = playback_manager.lock().unwrap();
                        playback_manager.set_event_pool_size(audio_settings.md_event_pool_size.value());
                    }
                }
            }
        }
    }

    fn draw_audio_tab_kdmapi(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("KDMAPI").size(15.0));
        ui.separator();
        ui.label("Open OmniMIDI to adjust settings.");
    }
}

impl Dialog for ESSettingsWindow {
    fn draw(&mut self, ui: &mut Ui, _: &crate::app::util::image_loader::ImageResources) -> Option<crate::app::ui::dialog::DialogAction> {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                if ui.selectable_label(self.curr_settings == ESCurrentSettings::General, "General").clicked() {
                    self.curr_settings = ESCurrentSettings::General
                }
                if ui.selectable_label(self.curr_settings == ESCurrentSettings::Audio, "Audio").clicked() {
                    self.curr_settings = ESCurrentSettings::Audio
                }
            });
            ui.separator();
            ui.vertical(|ui| {
                egui::ScrollArea::vertical()
                    .min_scrolled_height(800.0)
                    .show(ui, |ui| {
                        match self.curr_settings {
                            ESCurrentSettings::General => {
                                self.draw_general_tab(ui);
                            },
                            ESCurrentSettings::Audio => {
                                self.draw_audio_tab(ui);
                            },
                        }
                    })
            })
        });

        None
    }

    fn get_dialog_name(&self) -> &'static str {
        DIALOG_NAME_EDITOR_SETTINGS
    }

    fn get_dialog_title(&self) -> String {
        "Editor Settings".into()
    }

    fn get_flags(&self) -> u16 {
        DIALOG_NO_COLLAPSABLE
    }

    fn get_action_buttons(&self) -> Option<crate::app::ui::dialog::DialogActionButtons> {
        Some(
            DialogActionButtons::Ok(
                Box::new(|dlg| {
                    let dlg_name = dlg.get_dialog_name();
                    Some(DialogAction::Close(dlg_name))
                })
            )
        )
    }
    /*fn show(&mut self) -> () {
        self.is_shown = true
    }

    fn close(&mut self) -> () {
        self.is_shown = false
    }

    fn is_showing(&self) -> bool {
        self.is_shown
    }

    fn draw(&mut self, ctx: &egui::Context, _image_resources: &crate::app::util::image_loader::ImageResources) -> () {
        if !self.is_shown { return; }
        egui::Window::new(RichText::new("Editor Settings").size(10.0))
            .collapsible(false)
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            if ui.selectable_label(self.curr_settings == ESCurrentSettings::General, "General").clicked() {
                                self.curr_settings = ESCurrentSettings::General
                            }
                            if ui.selectable_label(self.curr_settings == ESCurrentSettings::Audio, "Audio").clicked() {
                                self.curr_settings = ESCurrentSettings::Audio
                            }
                        });
                        ui.separator();
                        ui.vertical(|ui| {
                            egui::ScrollArea::vertical()
                                .min_scrolled_height(800.0)
                                .show(ui, |ui| {
                                    match self.curr_settings {
                                        ESCurrentSettings::General => {
                                            self.draw_general_tab(ui);
                                        },
                                        ESCurrentSettings::Audio => {
                                            self.draw_audio_tab(ui);
                                        },
                                    }
                                })
                        })
                    });
                    ui.horizontal(|ui| {
                        if ui.button("Close").clicked() {
                            self.is_shown = false;
                        }
                    });
                })
            });
    }*/
}