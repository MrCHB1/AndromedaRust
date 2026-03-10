pub mod meta_sequence_funcs;

use eframe::egui;

use crate::{
    app::{
        custom_widgets::{NumberField, NumericField},
        ui::dialog::{
            Dialog, DialogAction, DialogActionButtons, flags::*, names::DIALOG_NAME_INSERT_META
        },
    },
    editor::{
        actions::{EditorAction, EditorActions}, editing::{meta_editing::meta_sequence_funcs::merge_metas, note_editing::note_sequence_funcs::{extract, extract_and_remap_ids}}, midi_bar_cacher::BarCacher, tempo_map::TempoMap, util::{MIDITick, tempo_as_bytes}
    },
    midi::events::meta_event::{MetaEvent, MetaEventType}, util::debugger::Debugger,
};

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
        
        /*{
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
        }*/

        {
            let mut metas = self.global_metas.write().unwrap();
            
            let insert_idx = match metas.binary_search_by_key(&tick, |meta| meta.tick) {
                    Ok(ins) | Err(ins) => ins
                };

            let replace_meta = if insert_idx < metas.len() {
                meta_event.tick == metas[insert_idx].tick && meta_event.event_type == metas[insert_idx].event_type
            } else {
                false
            };

            if replace_meta {
                // extract old meta
                // let old_meta = metas.remove(insert_idx);
                // metas.insert(insert_idx, meta_event);
                let old_meta = std::mem::replace(&mut metas[insert_idx], meta_event);

                let mut editor_actions = self.editor_actions.borrow_mut();
                editor_actions.register_action(EditorAction::Bulk(vec![
                    EditorAction::DeleteMeta(vec![insert_idx], Some(vec![old_meta])),
                    EditorAction::AddMeta(vec![insert_idx], None)
                ]));

                Debugger::log("Meta event replaced");
            } else {
                metas.insert(insert_idx, meta_event);

                let mut editor_actions = self.editor_actions.borrow_mut();
                editor_actions.register_action(EditorAction::AddMeta(vec![insert_idx], None));
            }
        }

        if meta_ev_type == MetaEventType::Tempo {
            let mut tempo_map = self.tempo_map.try_write().unwrap();
            tempo_map.rebuild_tempo_map();
        }

        self.regenerate_bars();
    }

    pub fn take_metas(&mut self) -> Vec<MetaEvent> {
        let mut metas = self.global_metas.write().unwrap();
        std::mem::take(&mut *metas)
    }

    pub fn set_metas(&mut self, metas: Vec<MetaEvent>) {
        let mut metas_ = self.global_metas.write().unwrap();
        *metas_ = metas;
    }

    pub fn apply_action(&mut self, action: &mut EditorAction) {
        match action {
            EditorAction::AddMeta(_, deleted_metas) => {
                assert!(deleted_metas.is_some(), "[ADD METAS] Something has gone wrong while undoing meta deletion.");
                {
                    let recovered_metas = deleted_metas.take().unwrap();
                    let old_metas = self.take_metas();

                    let merged = merge_metas(old_metas, recovered_metas);
                    self.set_metas(merged);
                }

                {
                    let mut tempo_map = self.tempo_map.write().unwrap();
                    tempo_map.rebuild_tempo_map();
                }

                self.regenerate_bars();
            },
            EditorAction::DeleteMeta(meta_ids, deleted_metas) => {
                {
                    let old_metas = self.take_metas();

                    let (deleted, new_metas) = extract(old_metas, &meta_ids);
                    self.set_metas(new_metas);

                    *deleted_metas = Some(deleted);
                }

                {
                    let mut tempo_map = self.tempo_map.write().unwrap();
                    tempo_map.rebuild_tempo_map();
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
            DialogActionButtons::OkCancel(
                Box::new(|dlg| {
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
                }),
                Box::new(|dlg| {
                    let dlg = dlg.as_any_mut().downcast_mut::<Self>().unwrap();
                    let dlg_name = dlg.get_dialog_name();
                    Some(DialogAction::Close(dlg_name))
                })
            )
        )
    }

    fn get_flags(&self) -> u16 {
        DIALOG_NO_COLLAPSABLE | DIALOG_NO_RESIZABLE
    }
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