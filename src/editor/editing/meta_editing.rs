use eframe::egui::{self, RichText};

use crate::{app::{custom_widgets::{NumberField, NumericField}, ui::dialog::{Dialog, DialogAction, DialogActionButtons, flags::*, names::DIALOG_NAME_INSERT_META}}, editor::{actions::{EditorAction, EditorActions}, midi_bar_cacher::BarCacher, tempo_map::TempoMap, util::{MIDITick, tempo_as_bytes}}, midi::events::meta_event::{MetaEvent, MetaEventType}};

use std::{cell::RefCell, collections::VecDeque, rc::Rc, sync::{Arc, Mutex, RwLock}};

#[derive(Default)]
pub struct MetaEditing {
    bar_cacher: Arc<Mutex<BarCacher>>,
    global_metas: Arc<RwLock<Vec<MetaEvent>>>,
    editor_actions: Rc<RefCell<EditorActions>>,

    tmp_del_metas: VecDeque<MetaEvent>,
    tempo_map: Arc<RwLock<TempoMap>>,
    pub ppq: u16,
}

impl MetaEditing {
    pub fn new(
        global_metas: &Arc<RwLock<Vec<MetaEvent>>>,
        bar_cacher: &Arc<Mutex<BarCacher>>,
        editor_actions: &Rc<RefCell<EditorActions>>,
        tempo_map: &Arc<RwLock<TempoMap>>,
    ) -> Self {
        Self {
            bar_cacher: bar_cacher.clone(),
            global_metas: global_metas.clone(),
            editor_actions: editor_actions.clone(),

            tmp_del_metas: VecDeque::new(),
            tempo_map: tempo_map.clone(),
            ppq: 960,
        }
    }

    fn bin_search_metas(&self, tick_pos: MIDITick) -> usize {
        let metas = self.global_metas.read().unwrap();
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
        // let insert_idx = self.bin_search_metas(tick);

        let meta_ev_type = meta_event.event_type;
        
        {
            let mut metas = self.global_metas.try_write().unwrap();
            let insert_idx = match metas.binary_search_by_key(&tick, |meta| meta.tick) {
                Ok(ins) | Err(ins) => ins
            };

            let replace_meta = if insert_idx < metas.len() {
                meta_event.tick == metas[insert_idx].tick && meta_event.event_type == metas[insert_idx].event_type
            } else {
                false
            };

            // println!("{:?}", metas.iter().map(|m| (m.event_type, &m.data)).collect::<Vec<_>>());
        
            if replace_meta {
                metas[insert_idx].data = meta_event.data;
                println!("Meta event replaced");
            } else {
                metas.insert(insert_idx, meta_event);

                let mut editor_actions = self.editor_actions.try_borrow_mut().unwrap();
                editor_actions.register_action(EditorAction::AddMeta(vec![insert_idx]));
            }
        }

        if meta_ev_type == MetaEventType::Tempo {
            let mut tempo_map = self.tempo_map.try_write().unwrap();
            tempo_map.rebuild_tempo_map();
        }

        self.regenerate_bars();
    }

    pub fn apply_action(&mut self, action: &EditorAction) {
        match action {
            EditorAction::AddMeta(meta_ids) => {
                // pop last deleted meta from deleted metas deque
                {
                    let mut metas = self.global_metas.write().unwrap();
                    for id in meta_ids.iter() {
                        let meta = self.tmp_del_metas.pop_back().unwrap();
                        metas.insert(*id, meta);
                    }
                }

                {
                    let mut tempo_map = self.tempo_map.write().unwrap();
                    tempo_map.rebuild_tempo_map();
                }

                self.regenerate_bars();
            },
            EditorAction::DeleteMeta(meta_ids) => {
                {
                    let mut metas = self.global_metas.write().unwrap();
                    
                    // remove meta, then push last
                    // iterate in reverse, prevent index invalidation
                    for id in meta_ids.iter().rev() {
                        let meta = metas.remove(*id);
                        self.tmp_del_metas.push_back(meta);
                    }

                    {
                        let mut tempo_map = self.tempo_map.write().unwrap();
                        tempo_map.rebuild_tempo_map();
                    }
                }

                self.regenerate_bars();
            },
            _ => {}
        }
    }

    pub fn get_metas(&self) -> Arc<RwLock<Vec<MetaEvent>>> {
        self.global_metas.clone()
    }

    fn regenerate_bars(&mut self) {
        let mut bar_cacher = self.bar_cacher.lock().unwrap();
        bar_cacher.clear_cache();
    }
}



pub struct MetaEventInsertDialog {
    is_showing: bool,
    dialog_type: MetaEventType,

    fields: Vec<(&'static str, Box<dyn NumberField>)>,

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

impl Dialog for MetaEventInsertDialog {
    fn draw(&mut self, ui: &mut egui::Ui, _: &crate::app::util::image_loader::ImageResources) -> Option<crate::app::ui::dialog::DialogAction> {
        for (label, field) in self.fields.iter_mut() {
            field.show(label, ui, None);
        }
        None
    }

    fn cleanup_dialog(&mut self) -> Result<(), &'static str> {
        self.fields.clear();
        Ok(())
    }

    fn get_dialog_name(&self) -> &'static str {
        DIALOG_NAME_INSERT_META
    }

    fn get_dialog_title(&self) -> String {
        format!("Insert {}", self.dialog_type.to_string())
    }

    fn get_action_buttons(&self) -> Option<crate::app::ui::dialog::DialogActionButtons> {
        Some(
            DialogActionButtons::Ok(Box::new(
                |dlg| {
                    let dlg = dlg.as_any_mut().downcast_mut::<Self>().unwrap();

                    let mut data = Vec::new();

                    match dlg.dialog_type {
                        MetaEventType::TimeSignature => {
                            data = vec![dlg.fields[0].1.as_u8(), dlg.fields[1].1.as_u8()];
                        },
                        MetaEventType::Tempo => {
                            data = tempo_as_bytes(dlg.fields[0].1.as_f32()).to_vec();
                        }
                        _ => {}
                    }

                    if !data.is_empty() {
                        if let Some(meta_created) = dlg.meta_created.take() {
                            meta_created(data);
                        }
                    }

                    let dlg_name = dlg.get_dialog_name();
                    Some(DialogAction::Close(dlg_name))
                }
            ))
        )
    }

    fn get_flags(&self) -> u16 {
        DIALOG_NO_COLLAPSABLE | DIALOG_NO_RESIZABLE
    }
    /*fn show(&mut self) -> () {
        self.is_showing = true;
    }

    fn close(&mut self) -> () {
        self.fields.clear();
        self.is_showing = false;
    }

    fn is_showing(&self) -> bool {
        self.is_showing
    }

    fn draw(&mut self, ctx: &egui::Context, _image_resources: &crate::app::util::image_loader::ImageResources) -> () {
        if !self.is_showing { return; }

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
                                    data = vec![self.fields[0].1.as_u8(), self.fields[1].1.as_u8()];
                                    println!("{:?}", data);
                                },
                                MetaEventType::Tempo => {
                                    data = tempo_as_bytes(self.fields[0].1.as_f32()).to_vec();
                                }
                                _ => {}
                            }

                            if !data.is_empty() {
                                if let Some(meta_created) = self.meta_created.take() {
                                    meta_created(data);
                                }
                            }
                            
                            self.close();
                        }

                        if ui.button("Cancel").clicked() {
                            self.close();
                        }
                    });
                });
            });
    }*/
}

impl MetaEventInsertDialog {
    pub fn init_meta_dialog(&mut self, meta_type: MetaEventType, on_meta_created: impl Fn(Vec<u8>) + 'static) {
        self.dialog_type = meta_type;

        match meta_type {
            MetaEventType::TimeSignature => {
                self.fields = vec![
                    ("Numerator", Box::new(NumericField::<u8>::new(4, Some(1), Some(12)))),
                    ("Denominator (Power of 2)", Box::new(NumericField::<u8>::new(2, Some(0), Some(4)))),
                ];
                self.meta_created = Some(Box::new(on_meta_created));
                self.is_showing = true;
            },
            MetaEventType::Tempo => {
                self.fields = vec![
                    ("Tempo", Box::new(NumericField::<f32>::new(120.0, Some(60000000.0 / (0xFFFFFF as f32)), Some(60000000.0 / 1.0))))
                ];
                self.meta_created = Some(Box::new(on_meta_created));
                self.is_showing = true;
            }
            _ => {}
        }
    }
}