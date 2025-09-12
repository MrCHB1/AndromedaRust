use eframe::egui::{self, RichText, Ui};

use crate::{app::custom_widgets::{IntegerField, EditField}, audio::{event_playback::PlaybackManager, midi_devices::MIDIDevices}};
use std::sync::{Arc, Mutex};

pub struct ESGeneralSettings {
    // import settings
    import_discard_empty_tracks: bool,
    import_max_ppq_override: bool,
    import_max_ppq_override_value: IntegerField,
    import_remove_overlaps: bool,

    export_discard_empty_tracks: bool
}

impl Default for ESGeneralSettings {
    fn default() -> Self {
        Self {
            import_discard_empty_tracks: false,
            import_max_ppq_override: false,
            import_max_ppq_override_value: IntegerField::new(960, Some(96), Some(7680)),
            import_remove_overlaps: false,

            export_discard_empty_tracks: true
        }
    }
}

pub struct ESAudioSettings {
    port_in: usize,
    port_out: usize,

    // advanced settings
    event_pool_size: IntegerField
}

impl Default for ESAudioSettings {
    fn default() -> Self {
        Self {
            port_in: 0,
            port_out: 0,
            event_pool_size: IntegerField::new(100000, Some(100), Some(1000000))
        }
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
    playback_manager: Option<Arc<Mutex<PlaybackManager>>>
}

impl ESSettingsWindow {
    pub fn show(&mut self) {
        self.is_shown = true;
    }

    pub fn use_midi_devices(&mut self, devices: Arc<Mutex<MIDIDevices>>) {
        self.midi_devices = Some(devices);
    }

    pub fn use_playback_manager(&mut self, playback_manager: Arc<Mutex<PlaybackManager>>) {
        self.playback_manager = Some(playback_manager);
    }

    fn draw_general_tab(&mut self, ui: &mut Ui) {
        let mut general_settings = &mut self.general_settings;
        ui.label(RichText::new("MIDI Import").size(15.0));
        {
            ui.checkbox(&mut general_settings.import_discard_empty_tracks, "Discard empty tracks");
            ui.checkbox(&mut general_settings.import_max_ppq_override, "Keep PPQ at a Maximum").on_hover_text_at_pointer("If any imported MIDI's PPQ exceeds the specified PPQ, the MIDI will be quantized.");
            ui.add_enabled_ui(general_settings.import_max_ppq_override, |ui| {
                general_settings.import_max_ppq_override_value.show("Max PPQ", ui, None);
            });
            ui.checkbox(&mut general_settings.import_remove_overlaps, "Remove overlaps");
        }
        ui.separator();
        ui.label(RichText::new("MIDI Export").size(15.0));
        {
            ui.checkbox(&mut general_settings.export_discard_empty_tracks, "Discard empty tracks");
        }
    } 

    fn draw_audio_tab(&mut self, ui: &mut Ui) {
        if let Some(midi_devices) = self.midi_devices.as_ref() {
            let mut audio_settings = &mut self.audio_settings;
            ui.label(RichText::new("MIDI Input Devices").size(15.0));
            {
                let midi_in_names = {
                    let midi_devices = midi_devices.lock().unwrap();
                    midi_devices.get_midi_in_port_names().clone()
                };

                for (i, in_name) in midi_in_names.iter().enumerate() {
                    if ui.selectable_label(audio_settings.port_in == i, in_name).clicked() {
                        audio_settings.port_in = i;
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
                    if ui.selectable_label(audio_settings.port_out == i, out_name).clicked() {
                        audio_settings.port_out = i;
                        let mut midi_devices = midi_devices.lock().unwrap();
                        midi_devices.connect_out_port(i).unwrap()
                    }
                    //ui.label(in_name);
                }
            }
            ui.separator();
            ui.label(RichText::new("Advanced").size(15.0));
            {
                audio_settings.event_pool_size.show("MIDI Event pool size", ui, None);
                if audio_settings.event_pool_size.changed {
                    if let Some(playback_manager) = self.playback_manager.as_ref() {
                        let mut playback_manager = playback_manager.lock().unwrap();
                        playback_manager.set_event_pool_size(audio_settings.event_pool_size.value() as usize);
                    }
                }
            }
        }
    }

    pub fn draw_window(&mut self, ctx: &egui::Context) -> bool {
        if !self.is_shown { return false; }
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
        return true;
    }

    pub fn is_showing(&self) -> bool {
        self.is_shown
    }
}