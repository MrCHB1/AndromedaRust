use eframe::egui::{self, RichText, Ui};
use num_traits::Num;

use crate::{app::custom_widgets::{EditField, IntegerField}, editor::{midi_bar_cacher::BarCacher, util::MIDITick}, midi::events::meta_event::{MetaEvent, MetaEventType}};

use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct MetaEditing {
    bar_cacher: Arc<Mutex<BarCacher>>,
    global_metas: Arc<Mutex<Vec<MetaEvent>>>,
}

impl MetaEditing {
    pub fn new(global_metas: &Arc<Mutex<Vec<MetaEvent>>>, bar_cacher: &Arc<Mutex<BarCacher>>) -> Self {
        Self {
            bar_cacher: bar_cacher.clone(),
            global_metas: global_metas.clone()
        }
    }

    fn bin_search_metas(&self, tick_pos: MIDITick) -> usize {
        let metas = self.global_metas.lock().unwrap();
        if metas.is_empty() { return 0; }

        let mut low = 0;
        let mut high = metas.len();

        while low < high {
            let mid = (low + high) / 2;
            if metas[mid].tick <= tick_pos {
                low = mid + 1;
            } else {
                high = mid;
            }
        }

        low
    }

    pub fn insert_meta_event(&mut self, meta_event: MetaEvent) {
        let tick = meta_event.tick;
        let insert_idx = self.bin_search_metas(tick);
        
        {
            let mut metas = self.global_metas.lock().unwrap();
            metas.insert(insert_idx, meta_event);

            println!("{:?}", metas.iter().map(|m| (m.event_type, &m.data)).collect::<Vec<_>>());
        }

        {
            let mut bar_cacher = self.bar_cacher.lock().unwrap();
            bar_cacher.clear_cache(); // to regenerate bars
        }
    }
}



pub struct MetaEventInsertDialog {
    is_showing: bool,
    dialog_type: MetaEventType,

    fields: Vec<(&'static str, Box<dyn EditField<i32>>)>,

    meta_created: Option<Box<dyn Fn(Vec<u8>)>>
}

impl Default for MetaEventInsertDialog {
    fn default() -> Self {
        Self {
            is_showing: false,
            dialog_type: MetaEventType::Lyric,
            fields: Vec::new(),
            meta_created: None
        }
    }
}

impl MetaEventInsertDialog {
    pub fn show(&mut self, show_for: MetaEventType, on_meta_created: impl Fn(Vec<u8>) + 'static) {
        self.dialog_type = show_for;

        match show_for {
            MetaEventType::TimeSignature => {
                self.fields = vec![
                    ("Numerator", Box::new(IntegerField::new(4, Some(1), Some(12)))),
                    ("Denominator (Power of 2)", Box::new(IntegerField::new(2, Some(0), Some(4)))),
                ];
                self.meta_created = Some(Box::new(on_meta_created));
                self.is_showing = true;
            },
            _ => {}
        }
    }

    pub fn draw(&mut self, ctx: &egui::Context) -> bool {
        if !self.is_showing { return false; }

        egui::Window::new(RichText::new(format!("Insert {}", self.dialog_type.to_string())).size(15.0))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    for (label, field) in self.fields.iter_mut() {
                        field.show(label, ui, None);
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Insert").clicked() {
                            let mut data = Vec::new();

                            match self.dialog_type {
                                MetaEventType::TimeSignature => {
                                    data = self.fields.drain(..)
                                        .map(|(_, field)| {
                                            field.value() as u8
                                        }).collect::<Vec<_>>();
                                    println!("{:?}", data);
                                }
                                _ => {}
                            }

                            if !data.is_empty() {
                                if let Some(meta_created) = self.meta_created.take() {
                                    meta_created(data);
                                }
                            }
                            
                            self.is_showing = false;
                        }

                        if ui.button("Cancel").clicked() {
                            self.fields.clear();
                            self.is_showing = false;
                        }
                    });
                });
            });

        return self.is_showing;
    }
}