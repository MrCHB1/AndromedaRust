// abstraction is NEEDED!!
use crate::{app::{custom_widgets::{EditField, IntegerField}, rendering::{data_view::DataViewRenderer, RenderManager, RenderType, Renderer}, shared::NoteColors, view_settings::{VS_PianoRoll_DataViewState, VS_PianoRoll_OnionState}}, audio::{event_playback::PlaybackManager, kdmapi_engine::kdmapi::KDMAPI, midi_devices::MIDIDevices}, editor::{edit_functions::EFChopDialog, meta_editing::{MetaEditing, MetaEventInsertDialog}, midi_bar_cacher::BarCacher, navigation::TrackViewNavigation, note_editing::NoteEditing, playhead::Playhead, settings::{editor_settings::{ESAudioSettings, ESGeneralSettings, ESSettingsWindow, Settings}, project_settings::ProjectSettings}, util::MIDITick}, midi::{events::meta_event::{MetaEvent, MetaEventType}, midi_file::MIDIEvent}};

use eframe::{
    egui::{self, Color32, RichText, Stroke, Ui, Vec2},
    egui_glow::CallbackFn,
    glow::HasContext,
};
use egui_extras::StripBuilder;
use rayon::prelude::*;
use rounded_div::RoundedDiv;

use crate::{
    app::{
        rendering::piano_roll::{PianoRollRenderer},
        view_settings::ViewSettings,
    },
    editor::{
        actions::{EditorAction, EditorActions}, edit_functions::{EFStretchDialog, EditFunction, EditFunctions}, navigation::PianoRollNavigation, project_data::ProjectData, util::{bin_search_notes, bin_search_notes_exact, decode_note_group, find_note_at, get_absolute_max_tick_from_ids, get_min_max_ticks_in_selection, get_notes_in_range, move_element}
    },
    midi::{
        events::note::Note,
        midi_file::MIDIFileWriter,
    },
};
use eframe::glow;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex}, time::Instant,
};

const TOOL_FLAGS_NONE: u8 = 0x0;
const TOOL_PENCIL_DRAGGING: u8 = 0x1;
const TOOL_PENCIL_LENGTH_CHANGE: u8 = 0x2;
const TOOL_PENCIL_OVER_NOTE: u8 = 0x4;
const TOOL_PENCIL_ALL_FLAGS_EXCEPT_MULTIEDIT: u8 = 0b111;
const TOOL_PENCIL_MULTIEDIT: u8 = 0x10;

const TOOL_ERASER_ENABLE: u8 = 0x1;

const EDITOR_DEBUG: bool = true;

const SNAP_MAPPINGS: [((u8, u16), &str); 14] = [
    ((0, 0), "No snap"),
    ((1, 1), "Semibreve (1)"),
    ((1, 2), "Minim (1/2)"),
    ((1, 3), "Triplet (1/3)"),
    ((3, 4), "Dotted Minim (3/4)"),
    ((1, 4), "Crotchet (1/4)"),
    ((1, 6), "Minim Triplet (1/6)"),
    ((5, 8), "Dotted Crotchet (5/8)"),
    ((1, 8), "Quaver (1/8)"),
    ((1, 12), "Crotchet Triplet (1/12)"),
    ((1, 16), "Semiquaver (1/16)"),
    ((1, 32), "Demisemiquaver (1/32)"),
    ((1, 64), "Hemidemisemiquaver (1/64)"),
    ((1, 128), "Semiemidemisemiquaver (1/128)"),
];

#[derive(Clone)]
pub enum EditorTool {
    // Pencil(drag_offset)
    Pencil,
    Eraser,
    Selector,
}

impl Default for EditorTool {
    fn default() -> Self {
        EditorTool::Pencil
    }
}

pub struct EditorToolSettings {
    curr_tool: EditorTool,
    flags: u8,
    pub snap_ratio: (u8, u16),
}

impl Default for EditorToolSettings {
    fn default() -> Self {
        Self {
            curr_tool: Default::default(),
            flags: TOOL_FLAGS_NONE,
            snap_ratio: (1, 4),
        }
    }
}

impl EditorToolSettings {
    /*pub fn new() -> Self {
        Default::default()
    }*/

    pub fn switch_tool(&mut self, new_tool: EditorTool) {
        self.curr_tool = new_tool;
    }

    pub fn get_tool(&self) -> EditorTool {
        self.curr_tool.clone()
    }

    pub fn reset_flags(&mut self) {
        self.flags = TOOL_FLAGS_NONE;
    }
}

pub struct ToolBarSettings {
    pub note_gate: IntegerField,
    pub note_velocity: IntegerField,
}

impl Default for ToolBarSettings {
    fn default() -> Self {
        Self {
            note_gate: IntegerField::new(960, Some(1), Some(u16::MAX.into())),
            note_velocity: IntegerField::new(100, Some(1), Some(127)),
        }
    }
}


#[derive(Default)]
pub struct MainWindow {
    pub project_data: Arc<Mutex<ProjectData>>,
    bar_cacher: Arc<Mutex<BarCacher>>,
    gl: Option<Arc<glow::Context>>,
    // renderer: Option<Arc<Mutex<dyn Renderer + Send + Sync>>>,
    render_manager: Option<Arc<Mutex<RenderManager>>>,
    data_view_renderer: Option<Arc<Mutex<DataViewRenderer>>>,
    playback_manager: Option<Arc<Mutex<PlaybackManager>>>,
    note_editing: Arc<Mutex<NoteEditing>>,
    meta_editing: Arc<Mutex<MetaEditing>>,
    nav: Option<Arc<Mutex<PianoRollNavigation>>>,
    track_view_nav: Option<Arc<Mutex<TrackViewNavigation>>>,
    view_settings: Option<Arc<Mutex<ViewSettings>>>,
    playhead: Playhead,
    note_colors: Arc<NoteColors>,

    mouse_over_ui: bool,
    editor_tool: Arc<Mutex<EditorToolSettings>>,
    
    project_settings: ProjectSettings,
    settings: Vec<Box<dyn Settings>>,

    // used for all tools
    tool_mouse_down: bool,

    // ghost note index zero is reserved for the pencil note
    // ghost_notes: Arc<Mutex<Vec<GhostNote>>>,

    pub editor_actions: Arc<Mutex<EditorActions>>,
    pub editor_functions: EditFunctions,

    ef_stretch_dialog: EFStretchDialog,
    ef_chop_dialog: EFChopDialog,

    is_dragging_notes: bool,
    // old_drag_ticks: i64,
    // old_drag_keys: i16,
    // drag_ticks: i64,
    // drag_keys: i16,
    old_note_lengths: Vec<u32>,

    temp_del_notes: VecDeque<Note>,
    // only use for *immediate* note modifications!
    temp_modifying_note_ids: Vec<usize>,
    temp_note_positions: Vec<(u32, u8)>,
    pub temp_selected_notes: Arc<Mutex<Vec<usize>>>,
    drag_offset: i32,

    // for the top toolbar
    toolbar_settings: Arc<Mutex<ToolBarSettings>>,
    // start tick, end tick, start key, end key
    selection_range: (u32, u32, u8, u8),

    // if mouse gets released while over ui
    is_waiting_for_no_ui_hover: bool,
    dragged_from_ui: bool,
    draw_select_box: bool,

    // other
    // override popup settings
    show_override_popup: bool,
    override_popup_msg: &'static str,
    override_popup_func: Option<Box<dyn Fn(&mut MainWindow, &egui::Context) -> ()>>, // hacky

    // note properties popup
    show_note_properties_popup: bool,
    note_properties_popup_note_id: usize, // the id the popup is referring to
    note_properties_mouse_up_processed: bool, // to compensate for unprocessed mouse up events after the dialog opens

    last_click_time: f64,
    last_clicked_note_id: Option<usize>,

    // tool dialogs popups
    tool_dialogs_any_open: bool,
    midi_devices: Option<Arc<Mutex<MIDIDevices>>>,
    kdmapi: Option<Arc<Mutex<KDMAPI>>>,
    settings_window: ESSettingsWindow,

    last_midi_ev_key: u8,
    meta_ev_insert_dialog: MetaEventInsertDialog,
}

impl MainWindow {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut s = Self::default();

        // initialize settings
        s.settings = vec![
            Box::new(ESGeneralSettings::default()),
            Box::new(ESAudioSettings::default())
        ];
        s.midi_devices = Some(Arc::new(Mutex::new(
            MIDIDevices::new().unwrap()
        )));

        s.kdmapi = Some(Arc::new(Mutex::new(
            KDMAPI::new()
        )));

        if let Some(midi_devices) = s.midi_devices.as_ref() {
            let project_data = s.project_data.lock().unwrap();
            let playback_manager = Arc::new(Mutex::new(
                PlaybackManager::new(
                    s.kdmapi.as_ref().unwrap().clone(),
                    project_data.notes.clone(),
                    project_data.global_metas.clone(),
                    project_data.channel_events.clone()
                )
            ));
            s.settings_window.use_midi_devices(midi_devices.clone());
            s.settings_window.use_playback_manager(playback_manager.clone());
            s.playhead = Playhead::new(0, &playback_manager);
            s.playback_manager = Some(playback_manager);
        }

        s.project_settings = ProjectSettings::new(&s.project_data);
        /*s.ghost_notes = Arc::new(Mutex::new(vec![GhostNote {
            ..Default::default()
        }]));*/
        s.project_data.lock().unwrap().new_empty_project();
        {
            let project_data = s.project_data.lock().unwrap();
            s.bar_cacher = Arc::new(Mutex::new(BarCacher::new(960, 
                &project_data.global_metas
            )));
        }
        s.editor_actions = Arc::new(Mutex::new(EditorActions::new(256)));
        s.note_colors = Arc::new(NoteColors::new());
        s
    }

    fn import_midi_file(&mut self) {
        let mut project_data = self.project_data.lock().unwrap();
        let midi_fd = rfd::FileDialog::new().add_filter("MIDI Files", &["mid", "midi"]);
        if let Some(file) = midi_fd.pick_file() {
            let import_timer = Instant::now();
            let start = import_timer.elapsed().as_secs_f32();
            project_data.import_from_midi_file(String::from(file.to_str().unwrap()));
            let end = import_timer.elapsed().as_secs_f32();
            println!("Imported MIDI in {}s", end - start);
        }

        if let Some(playback_manager) = self.playback_manager.as_mut() {
            let mut playback_manager = playback_manager.lock().unwrap();
            playback_manager.ppq = project_data.project_info.ppq;
        }

        {
            let mut bar_cacher = self.bar_cacher.lock().unwrap();
            bar_cacher.clear_cache();
        }
    }

    fn export_midi_file(&mut self) {
        let midi_fd = rfd::FileDialog::new().add_filter("MIDI Files", &["mid"]);
        if let Some(file) = midi_fd.save_file() {
            let project_data = self.project_data.lock().unwrap();

            // let mut midi_writer = MIDIFileWriter::new(project_data.project_info.ppq);
            let notes = project_data.notes.read().unwrap();
            let global_metas = project_data.global_metas.lock().unwrap();
            let channel_evs = project_data.channel_events.lock().unwrap();

            let ppq = project_data.project_info.ppq;

            // build tracks in parallel
            let per_track_chunks: Vec<Vec<MIDIEvent>> = notes.par_iter()
                .zip(channel_evs.par_iter())
                .map(|(notes, ch_evs)| {
                    let mut writer = MIDIFileWriter::new(ppq);
                    writer.new_track();
                    writer.add_notes_with_other_events(notes, ch_evs);
                    writer.end_track();
                    writer.into_single_track()
                })
                .collect();

            let mut midi_writer = MIDIFileWriter::new(ppq);
            midi_writer.flush_global_metas(&global_metas);
            for chunk in per_track_chunks {
                midi_writer.append_track(chunk);
            }

            midi_writer.write_midi(file.to_str().unwrap()).unwrap();
        }
    }

    fn init_gl(&mut self) {
        let gl = self.gl.as_ref().unwrap();

        let nav = Arc::new(Mutex::new(PianoRollNavigation::new()));
        let track_view_nav = Arc::new(Mutex::new(TrackViewNavigation::new()));

        let view_settings = Arc::new(Mutex::new(ViewSettings::default()));
        let render_manager: Arc<Mutex<RenderManager>> = Arc::new(Mutex::new(Default::default()));
        
        if let Some(playback_manager) = self.playback_manager.as_ref() {
            let mut render_manager = render_manager.lock().unwrap();
            render_manager.init_renderers(
                self.project_data.clone(),
                Some(gl.clone()),
                nav.clone(),
                track_view_nav.clone(),
                view_settings.clone(),
                playback_manager.clone(),
                self.bar_cacher.clone(),
                &self.note_colors
            );
        
            self.data_view_renderer = Some(Arc::new(Mutex::new(unsafe { DataViewRenderer::new(
                &self.project_data,
                &nav,
                &gl.clone(),
                &playback_manager,
                &self.bar_cacher
            )})));
        }

        {
            let mut render_manager = render_manager.lock().unwrap();
            render_manager.switch_renderer(RenderType::PianoRoll);
        }

        self.nav = Some(nav.clone());
        self.track_view_nav = Some(track_view_nav);
        self.view_settings = Some(view_settings);
        self.render_manager = Some(render_manager);
        
    }

    fn init_note_editing(&mut self) {
        let project_data = self.project_data.lock().unwrap();
        let notes = &project_data.notes;
        let metas = &project_data.global_metas;

        let nav = self.nav.as_ref().unwrap();
        let editor_tool = &self.editor_tool;
        let render_manager = self.render_manager.as_ref().unwrap();
        self.note_editing = Arc::new(Mutex::new(NoteEditing::new(notes, nav, editor_tool, render_manager, &self.editor_actions, &self.toolbar_settings)));
        self.meta_editing = Arc::new(Mutex::new(MetaEditing::new(metas, &self.bar_cacher)));
    }

    // allows the renderer to draw ghost notes
    /*fn show_ghost_notes(&mut self) {
        let render_manager = self.render_manager.as_mut().unwrap();
        let mut render_manager = render_manager.lock().unwrap();

        let curr_renderer = render_manager.get_active_renderer();
        curr_renderer.lock().unwrap().set_ghost_notes(self.ghost_notes.clone());
    }

    fn hide_ghost_notes(&mut self) {
        let render_manager = self.render_manager.as_mut().unwrap();
        let mut render_manager = render_manager.lock().unwrap();

        let curr_renderer = render_manager.get_active_renderer();
        curr_renderer.lock().unwrap().clear_ghost_notes();
    }*/

    fn handle_navigation(&mut self, _ctx: &egui::Context, ui: &mut Ui) {
        let scroll_delta = ui.input(|i| i.raw_scroll_delta).y;
        if scroll_delta.abs() <= 0.001 {
            return;
        }

        let (alt_down, _shift_down, ctrl_down) =
            ui.input(|i| (i.modifiers.alt, i.modifiers.shift, i.modifiers.ctrl));
        let nav = self.nav.as_mut().unwrap();
        let track_view_nav = self.track_view_nav.as_mut().unwrap();

        let mut nav = nav.lock().unwrap();
        let mut track_view_nav = track_view_nav.lock().unwrap();

        // scroll up/down (no modifiers applied)
        let move_by = scroll_delta;

        // alt_down = zoom
        // shift_down = horizontal movements
        let zoom_factor = 1.01f32.powf(scroll_delta);

        let mut render_manager = self.render_manager.as_mut().unwrap().lock().unwrap();
        
        match render_manager.get_render_type() {
            RenderType::PianoRoll => {
                if ctrl_down {
                    if alt_down {
                        nav.zoom_ticks *= zoom_factor;
                        if nav.zoom_ticks < 10.0 {
                            nav.zoom_ticks = 10.0;
                        }
                        if nav.zoom_ticks > 384000.0 {
                            nav.zoom_ticks = 384000.0;
                        }
                    } else {
                        let project_data = self.project_data.lock().unwrap();
                        let mut new_tick_pos = nav.tick_pos
                            + 2.0 * move_by * (nav.zoom_ticks / project_data.project_info.ppq as f32);
                        if new_tick_pos < 0.0 {
                            new_tick_pos = 0.0;
                        }

                        nav.tick_pos = new_tick_pos;

                        let rend = render_manager.get_active_renderer();
                        nav.change_tick_pos(new_tick_pos, |time| {
                            rend.lock().unwrap().time_changed(time as u64)
                        });
                    }
                } else {
                    if alt_down {
                        // zoom in/out key-wise
                        let view_top = nav.key_pos + nav.zoom_keys;

                        nav.zoom_keys *= zoom_factor;
                        if nav.zoom_keys < 12.0 {
                            nav.zoom_keys = 12.0;
                        }
                        if nav.zoom_keys > 128.0 {
                            nav.zoom_keys = 128.0;
                        }

                        let view_top_new = nav.key_pos + nav.zoom_keys;
                        let view_top_delta = view_top_new - view_top;
                        if view_top_new > 128.0 {
                            nav.key_pos -= view_top_delta;
                        }

                        // clamp key view
                        if nav.key_pos < 0.0 {
                            nav.key_pos = 0.0;
                        }
                    } else {
                        let mut new_key_pos = nav.key_pos + move_by * (nav.zoom_keys / 128.0);
                        if new_key_pos < 0.0 {
                            new_key_pos = 0.0;
                        }
                        if new_key_pos + nav.zoom_keys > 128.0 {
                            new_key_pos = 128.0 - nav.zoom_keys;
                        }
                        nav.key_pos = new_key_pos;
                    }
                }
            },
            RenderType::TrackView => {
                if ctrl_down {
                    if alt_down {
                        track_view_nav.zoom_ticks *= zoom_factor;
                        if track_view_nav.zoom_ticks < 38400.0 {
                            track_view_nav.zoom_ticks = 38400.0;
                        }
                        if track_view_nav.zoom_ticks > 384000.0 {
                            track_view_nav.zoom_ticks = 384000.0;
                        }
                    } else {
                        let project_data = self.project_data.lock().unwrap();
                        let mut new_tick_pos = track_view_nav.tick_pos
                            + 2.0 * move_by * (track_view_nav.zoom_ticks / project_data.project_info.ppq as f32);
                        if new_tick_pos < 0.0 {
                            new_tick_pos = 0.0;
                        }

                        track_view_nav.tick_pos = new_tick_pos;

                        let rend = render_manager.get_active_renderer();
                        nav.change_tick_pos(new_tick_pos, |time| {
                            rend.lock().unwrap().time_changed(time as u64)
                        });
                    }
                } else {
                    if alt_down {
                        let view_top = track_view_nav.zoom_tracks + track_view_nav.zoom_tracks;
                        track_view_nav.zoom_tracks *= zoom_factor;
                        if track_view_nav.zoom_tracks < 10.0 {
                            track_view_nav.zoom_tracks = 10.0;
                        }
                        if track_view_nav.zoom_tracks > 32.0 {
                            track_view_nav.zoom_tracks = 32.0;
                        }
                    } else {
                        let mut new_track_pos = track_view_nav.track_pos + if move_by > 0.0 { -1.0 } else { 1.0 };
                        if new_track_pos < 0.0 { new_track_pos = 0.0; }
                        track_view_nav.track_pos = new_track_pos;
                    }
                }
            }
        }
    }

    // returns (tick_pos, key) based on navigation settings
    fn get_mouse_midi_pos(&self, ui: &mut Ui) -> ((u32, u8), (u32, u8)) {
        let rect = ui.min_rect();
        if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
            let mut mouse_x = mouse_pos.x;
            let mut mouse_y = mouse_pos.y;

            // normalize mouse position to 0-1
            mouse_x = (mouse_x - rect.left()) / rect.width();
            mouse_y = 1.0 - (mouse_y - rect.top()) / rect.height();
            // mouse_y = (mouse_y * -1.0 + rect.top() + rect.height()) / rect.height();

            {
                let nav = self.nav.as_ref().unwrap();
                let nav = nav.lock().unwrap();
                // tick position: from normalized
                let mouse_tick_pos =
                    (mouse_x * nav.zoom_ticks_smoothed + nav.tick_pos_smoothed) as u32;
                let (mouse_key_pos_rounded, mouse_key_pos) = (
                    (mouse_y * nav.zoom_keys_smoothed + nav.key_pos_smoothed).round() as u8,
                    (mouse_y * nav.zoom_keys_smoothed + nav.key_pos_smoothed) as u8
                );
                return ((mouse_tick_pos, mouse_key_pos), (mouse_tick_pos, mouse_key_pos_rounded));
            }
        }

        return ((0, 0), (0, 0));
    }

    fn midi_pos_to_ui_pos(&self, ui: &mut Ui, tick_pos: u32, key_pos: u8) -> (f32, f32) {
        let rect = ui.min_rect();
        if let Some(nav) = &self.nav {
            let nav = nav.lock().unwrap();
            let mut ui_x = (tick_pos as f32 - nav.tick_pos_smoothed) / nav.zoom_ticks_smoothed;
            let mut ui_y = (key_pos as f32 - nav.key_pos_smoothed) / nav.zoom_keys_smoothed;

            ui_x = ui_x * rect.width() + rect.left();
            ui_y = (1.0 - ui_y) * rect.height() + rect.top();

            (ui_x, ui_y)
        } else {
            (0.0, 0.0)
        }
    }

    /*fn init_selection_box(&mut self, start_tick_pos: u32, start_key_pos: u8) {
        let snapped_tick = self.snap_tick(start_tick_pos as i32) as u32;
        self.selection_range.0 = snapped_tick;
        self.selection_range.1 = snapped_tick;
        self.selection_range.2 = start_key_pos;
        self.selection_range.3 = start_key_pos;

        self.draw_select_box = true;
    }

    fn update_selection_box(&mut self, new_tick_pos: u32, new_key_pos: u8) {
        self.selection_range.1 = self.snap_tick(new_tick_pos as i32) as u32;
        self.selection_range.3 = new_key_pos;
    }*/

    fn get_selection_range(&self) -> (u32, u32, u8, u8) {
        let (min_tick, max_tick) = {
            if self.selection_range.0 > self.selection_range.1 {
                (self.selection_range.1, self.selection_range.0)
            } else {
                (self.selection_range.0, self.selection_range.1)
            }
        };

        let (min_key, max_key) = {
            if self.selection_range.2 > self.selection_range.3 {
                (self.selection_range.3, self.selection_range.2)
            } else {
                (self.selection_range.2, self.selection_range.3)
            }
        };

        (min_tick, max_tick, min_key, max_key)
    }

    /*fn create_ghost_note_from(&self, note: &Note, id: usize) -> GhostNote {
        let mut ghost_note: GhostNote = Default::default();
        ghost_note.id = Some(id);
        let gn = ghost_note.get_note_mut();
        gn.start = note.start;
        gn.key = note.key;
        gn.length = note.length;
        gn.velocity = note.velocity;

        ghost_note
    }*/

    /// Prepares the editor for changing the length of notes.
    /// - [`notes`]: Notes from a specific track AND channel to prepare length change.
    /// - [`base_id`]: The ID of the note to calculate the drag_offset. This is usually the ID of a clicked note.
    /// - [`note_ids`]: The IDs of all the notes that should have their lengths changed.
    /// - [`change_offset`]: If using the mouse to change the length, use the mouse's tick position in the MIDI.
    /*fn setup_notes_for_length_change(
        &mut self,
        curr_track: u16,
        curr_channel: u8,
        base_id: usize,
        note_ids: Arc<Mutex<Vec<usize>>>,
        change_offset: i32,
    ) {
        let project_data = self.project_data.lock().unwrap();

        let note_ids = note_ids.lock().unwrap();

        let notes = project_data.notes.lock().unwrap();
        let notes = &notes[curr_track as usize][curr_channel as usize];

        for note_id in note_ids.iter() {
            let note = &notes[*note_id];
            self.temp_modifying_note_ids.push(*note_id);
            self.old_note_lengths.push(note.length);
        }

        self.drag_offset = (&notes[base_id]).start as i32 + change_offset;
        self.editor_tool.flags |= TOOL_PENCIL_LENGTH_CHANGE;
    }

    fn setup_notes_for_drag(
        &mut self,
        curr_track: u16,
        curr_channel: u8,
        base_id: usize,
        note_ids: Arc<Mutex<Vec<usize>>>,
        drag_offset: i32,
    ) {
        let project_data = self.project_data.lock().unwrap();

        let note_ids = note_ids.lock().unwrap();
        let mut notes = project_data.notes.lock().unwrap();
        let notes = &mut notes[curr_track as usize][curr_channel as usize];

        // move all selected notes to ghost notes
        {
            // let selected_ids = self.temp_selected_notes.lock().unwrap();
            let mut ghost_notes = self.ghost_notes.lock().unwrap();
            ghost_notes.clear();

            // set ghost notes' ids to be the selected notes
            for (i, note_id) in note_ids.iter().enumerate() {
                let curr_note = &notes[*note_id];
                let ghost_note = self.create_ghost_note_from(&curr_note, *note_id);
                ghost_notes.push(ghost_note);

                // save the selected notes' positions
                self.temp_note_positions
                    .push((curr_note.start, curr_note.key));

                // this is here for calculating the ghost note's offsets
                // we are temporarily repurposing temp_modifying_note_ids for that reason
                // ...but only do it if note_ids.len() > 1
                if note_ids.len() <= 1 {
                    continue;
                }

                if *note_id == base_id {
                    self.temp_modifying_note_ids = vec![i];
                }
            }
        }

        // compensate for the clicked note's offset from the mouse cursor
        self.drag_offset = (&notes[base_id]).start as i32 + drag_offset;

        // a little risky! assuming that notes is already sorted...
        let mut rem_offset = 0;
        {
            for note_id in note_ids.iter() {
                notes.remove(*note_id - rem_offset);
                rem_offset += 1;
            }
        }

        self.editor_tool.flags |= TOOL_PENCIL_DRAGGING;
        self.is_dragging_notes = true;
    }*/

    /*fn get_clicked_note_idx(
        &mut self,
        curr_track: u16,
        curr_channel: u8,
        midi_mouse_pos: (u32, u8),
        set_flags: bool
    ) -> Option<usize> {
        let project_data = self.project_data.lock().unwrap();
        let mut notes = project_data.notes.lock().unwrap();
        let curr_notes = &mut notes[curr_track as usize][curr_channel as usize];
        let idx = find_note_at(curr_notes, midi_mouse_pos.0, midi_mouse_pos.1);

        if idx.is_some() {
            let clicked_note = &curr_notes[idx.unwrap()];

            let tbs = &mut self.toolbar_settings;
            tbs.note_gate.update_value(clicked_note.length as i32);
            tbs.note_velocity.update_value(clicked_note.velocity as i32);

            if set_flags { self.editor_tool.flags |= TOOL_PENCIL_OVER_NOTE; }
            self.last_clicked_note_id = Some(idx.unwrap());
        }

        idx
    }*/

    fn is_cursor_at_note_end(
        &self,
        clicked_idx: usize,
        curr_track: u16,
        curr_channel: u8,
        midi_mouse_pos: (u32, u8),
    ) -> bool {
        if let Some(nav) = &self.nav {
            let project_data = self.project_data.lock().unwrap();
            let notes = project_data.notes.read().unwrap();
            let curr_notes = &notes[curr_track as usize];
            let clicked_note = &curr_notes[clicked_idx];

            let nav = nav.lock().unwrap();
            let dist = ((clicked_note.start + clicked_note.length) as i32 - midi_mouse_pos.0 as i32)
                as f32
                / nav.zoom_ticks_smoothed
                * 960.0;

            dist < 8.0
        } else {
            false
        }
    }

    fn handle_mouse_down_pr(&mut self, _ctx: &egui::Context, ui: &mut Ui) {
        let mut note_editing = self.note_editing.lock().unwrap();
        note_editing.on_mouse_down();
        /*if self.show_note_properties_popup || self.tool_dialogs_any_open { return; }
        // dont handle this if the mouse is hovering over any ui element
        //if self.mouse_over_ui || self.is_waiting_for_no_ui_hover { return; }
        let (midi_mouse_pos, midi_mouse_pos_rounded) = self.get_mouse_midi_pos(ui);

        if !self.mouse_over_ui {
            self.tool_mouse_down = true;
        }

        if let Some((curr_track, curr_channel)) = self.get_current_track_and_channel() {
            match self.editor_tool.curr_tool {
                EditorTool::Pencil => {
                    // reset flags first
                    self.editor_tool.flags = TOOL_FLAGS_NONE;

                    // based on flags, skip until next mouse up
                    if self.mouse_over_ui {
                        //if self.editor_tool.flags & (TOOL_PENCIL_DRAGGING | TOOL_PENCIL_LENGTH_CHANGE) != 0 {
                        //    self.is_waiting_for_no_ui_hover = true;
                        //}
                        self.is_waiting_for_no_ui_hover = true;
                        self.dragged_from_ui = true;
                        return;
                    }

                    if let Some(clicked_idx) =
                        self.get_clicked_note_idx(curr_track, curr_channel, midi_mouse_pos, true)
                    {
                        // if we're over a selected note and there's more than one note in the selection, we're modifying multiple notes
                        {
                            let mut sel = self.temp_selected_notes.lock().unwrap();
                            if !sel.is_empty() {
                                if sel.contains(&clicked_idx) && sel.len() > 1 {
                                    self.editor_tool.flags |= TOOL_PENCIL_MULTIEDIT;
                                } else {
                                    sel.clear();
                                }
                            }
                        }

                        let is_at_end = self.is_cursor_at_note_end(
                            clicked_idx,
                            curr_track,
                            curr_channel,
                            midi_mouse_pos,
                        );

                        if self.editor_tool.flags & TOOL_PENCIL_MULTIEDIT != 0 {
                            if is_at_end {
                                let selected_ids = self.temp_selected_notes.clone();
                                self.setup_notes_for_length_change(
                                    curr_track,
                                    curr_channel,
                                    clicked_idx,
                                    selected_ids,
                                    -(midi_mouse_pos.0 as i32),
                                );
                            } else {
                                let selected_ids = self.temp_selected_notes.clone();
                                self.setup_notes_for_drag(
                                    curr_track,
                                    curr_channel,
                                    clicked_idx,
                                    selected_ids,
                                    -(midi_mouse_pos.0 as i32),
                                );
                            }
                        } else {
                            if is_at_end {
                                self.setup_notes_for_length_change(
                                    curr_track,
                                    curr_channel,
                                    clicked_idx,
                                    Arc::new(Mutex::new(vec![clicked_idx])),
                                    -(midi_mouse_pos.0 as i32),
                                );
                            } else {
                                self.setup_notes_for_drag(
                                    curr_track,
                                    curr_channel,
                                    clicked_idx,
                                    Arc::new(Mutex::new(vec![clicked_idx])),
                                    -(midi_mouse_pos.0 as i32),
                                );
                            }
                        }
                    } else {
                        // no note clicked, clear selected note id array if any
                        {
                            let mut sel = self.temp_selected_notes.lock().unwrap();
                            sel.clear();
                        }

                        // return;
                    }

                    if self.editor_tool.flags & TOOL_PENCIL_ALL_FLAGS_EXCEPT_MULTIEDIT == 0
                        || self.editor_tool.flags & TOOL_PENCIL_DRAGGING != 0
                    {
                        self.show_ghost_notes();
                    }
                }
                EditorTool::Eraser => {
                    if self.mouse_over_ui { return; }
                    self.editor_tool.flags = TOOL_ERASER_ENABLE;

                    self.init_selection_box(midi_mouse_pos.0, midi_mouse_pos_rounded.1);
                }
                EditorTool::Selector => {
                    if self.mouse_over_ui { return; }

                    self.init_selection_box(midi_mouse_pos.0, midi_mouse_pos_rounded.1);
                }
            }
        }*/
    }

    /// Handles double clicks in the editor. This doesn't run if the mouse is over any UI element.
    fn handle_mouse_double_down_pr(&mut self, _ctx: &egui::Context, ui: &mut Ui) {
        if self.show_note_properties_popup || self.tool_dialogs_any_open { return; }
        if self.mouse_over_ui { return; }
        let (_midi_mouse_pos, _midi_mouse_pos_rounded) = self.get_mouse_midi_pos(ui);

        /*match self.editor_tool.curr_tool {
            EditorTool::Pencil => {
                // unless the user's crazy fast, this should return the note we just clicked
                let Some(clicked_note_idx) = self.last_clicked_note_id else {
                    println!("Not actually over a note");
                    return;
                };

                // println!("{}", clicked_note_idx);

                if ui.input(|i| i.modifiers.ctrl) {
                    self.tool_mouse_down = false;

                    self.show_note_properties_popup = true;
                    self.note_properties_popup_note_id = clicked_note_idx;
                    return;
                }
            },
            EditorTool::Eraser => {

            },
            EditorTool::Selector => {
                // erm.. what would double clicking with a selector do?
            },
        }*/
    }

    fn handle_mouse_move_pr(&mut self, _ctx: &egui::Context, ui: &mut Ui) {
        let mut note_editing = self.note_editing.lock().unwrap();
        note_editing.on_mouse_move();
        /*if self.show_note_properties_popup || self.tool_dialogs_any_open { return; }
        // if self.mouse_over_ui { return; }
        let (midi_mouse_pos, midi_mouse_pos_rounded) = self.get_mouse_midi_pos(ui);

        match self.editor_tool.curr_tool {
            EditorTool::Pencil => {
                if self.mouse_over_ui {
                    if self.is_waiting_for_no_ui_hover
                        && (self.editor_tool.flags & TOOL_PENCIL_DRAGGING != 0)
                    {
                        return;
                    }
                }

                let tbs = &self.toolbar_settings;
                if self.editor_tool.flags & TOOL_PENCIL_ALL_FLAGS_EXCEPT_MULTIEDIT == 0 {
                    let mut ghost_notes = self.ghost_notes.lock().unwrap();
                    if ghost_notes.is_empty() {
                        ghost_notes.push(Default::default());
                    }
                    let gn = ghost_notes[0].get_note_mut();

                    gn.start = {
                        let mut snapped = self.snap_tick(midi_mouse_pos.0 as i32);
                        if snapped < 0 {
                            snapped = 0;
                        }
                        snapped
                    } as u32;
                    gn.key = {
                        let mut key = midi_mouse_pos.1 as u8;
                        if key > 127 {
                            key = 127;
                        }
                        key
                    };
                    gn.length = tbs.note_gate.value() as u32;
                    gn.velocity = tbs.note_velocity.value() as u8;
                } else if self.editor_tool.flags & TOOL_PENCIL_DRAGGING != 0 {
                    if self.editor_tool.flags & TOOL_PENCIL_MULTIEDIT != 0 {
                        // multi-note edit
                        let mut ghost_notes = self.ghost_notes.lock().unwrap();

                        let (cn_start, cn_key) = {
                            let clicked_note = &ghost_notes[self.temp_modifying_note_ids[0]].note;
                            (clicked_note.start, clicked_note.key)
                        };

                        let base_start = {
                            let mut snapped =
                                self.snap_tick(midi_mouse_pos.0 as i32 + self.drag_offset as i32);
                            if snapped < 0 {
                                snapped = 0;
                            }
                            snapped
                        } as u32;

                        let base_key = midi_mouse_pos.1;

                        for ghost_note in ghost_notes.iter_mut() {
                            // use temp_note_positions for calculating the offset from the clicked note index - so all ghost notes don't end up on the same position
                            // drag
                            let (tick_d, key_d) = {
                                let tick_d = ghost_note.note.start as i32 - cn_start as i32;
                                let key_d = ghost_note.note.key as i32 - cn_key as i32;
                                (tick_d, key_d)
                            };

                            ghost_note.note.start = {
                                let mut new_start = base_start as i32 + tick_d;
                                if new_start < 0 {
                                    new_start = 0;
                                }
                                new_start
                            } as u32;

                            ghost_note.note.key = {
                                let mut new_key = base_key as i32 + key_d;
                                if new_key < 0 {
                                    new_key = 0;
                                }
                                if new_key > 127 {
                                    new_key = 127;
                                }
                                new_key
                            } as u8;
                        }
                    } else {
                        // single-note edit
                        let mut ghost_notes = self.ghost_notes.lock().unwrap();
                        if ghost_notes.is_empty() {
                            ghost_notes.push(Default::default());
                        }
                        let gn = ghost_notes[0].get_note_mut();

                        gn.start = {
                            let mut snapped =
                                self.snap_tick(midi_mouse_pos.0 as i32 + self.drag_offset as i32);
                            if snapped < 0 {
                                snapped = 0;
                            }
                            snapped
                        } as u32;
                        gn.key = {
                            let mut key = midi_mouse_pos.1 as u8;
                            if key > 127 {
                                key = 127;
                            }
                            key
                        };
                    }
                } else if self.editor_tool.flags & TOOL_PENCIL_LENGTH_CHANGE != 0 {
                    if let Some((curr_track, curr_channel)) = self.get_current_track_and_channel() {
                        let project_data = self.project_data.lock().unwrap();
                        let mut notes = project_data.notes.lock().unwrap();

                        let curr_notes = &mut notes[curr_track as usize][curr_channel as usize];
                        for (i, tmp_mod_id) in self.temp_modifying_note_ids.iter().enumerate() {
                            let note_id = *tmp_mod_id;
                            let old_length = self.old_note_lengths[i];

                            // get notes we're changing the length of
                            let curr_note = &mut curr_notes[note_id];

                            let new_note_end = self
                                .snap_tick(midi_mouse_pos.0 as i32 + self.drag_offset)
                                + old_length as i32;
                            let mut new_note_length = new_note_end - curr_note.start as i32;

                            let min_possible_length = {
                                let min_snap = self.get_min_snap_tick_length() as i32;
                                let end_modulo = new_note_end % min_snap;
                                if end_modulo == 0 {
                                    min_snap
                                } else {
                                    end_modulo
                                }
                            };
                            if new_note_length < min_possible_length as i32 {
                                new_note_length = min_possible_length;
                            }
                            curr_note.length = new_note_length as u32;
                        }
                    }
                } else {
                }

                if self.tool_mouse_down {
                    if let Some(midi_devices) = self.midi_devices.as_ref() {
                        let mut midi_devices = midi_devices.lock().unwrap();
                        
                        if self.last_midi_ev_key != midi_mouse_pos.1 {
                            midi_devices.send_event(&[0x80, self.last_midi_ev_key, 127]).unwrap();
                            midi_devices.send_event(&[0x90, midi_mouse_pos.1, 127]).unwrap();
                            //midi_devices.send_note_on(midi_mouse_pos.1, 127).unwrap();

                            self.last_midi_ev_key = midi_mouse_pos.1;
                        }
                    }
                }
            }

            EditorTool::Eraser => {
                if self.mouse_over_ui { return; }

                if self.draw_select_box {
                    self.update_selection_box(midi_mouse_pos.0, midi_mouse_pos_rounded.1);
                }
                /*self.drag_ticks = midi_mouse_pos.0 as i64;
                self.drag_keys = midi_mouse_pos.1 as i16;

                if self.drag_ticks != self.old_drag_ticks || self.drag_keys != self.old_drag_keys {
                    // actually erase stuff!
                }

                self.old_drag_ticks = self.drag_ticks;
                self.old_drag_keys = self.drag_keys;*/
            }

            EditorTool::Selector => {
                if self.mouse_over_ui {
                    return;
                }

                if self.draw_select_box {
                    self.update_selection_box(midi_mouse_pos.0, midi_mouse_pos_rounded.1);
                }
            }
        }*/
    }

    fn handle_mouse_up_pr(&mut self, _ctx: &egui::Context, _ui: &mut Ui) {
        let mut note_editing = self.note_editing.lock().unwrap();
        note_editing.on_mouse_up();
        /*if self.show_note_properties_popup && self.note_properties_mouse_up_processed || self.tool_dialogs_any_open { return; }
        /*if self.mouse_over_ui {
            // bug prevention!!! :D
            self.is_waiting_for_no_ui_hover = true;
            return;
        }*/
        match self.editor_tool.curr_tool {
            EditorTool::Pencil => {
                if self.mouse_over_ui {
                    self.is_waiting_for_no_ui_hover = true;
                    return;
                }

                if let Some((curr_track, curr_channel)) = self.get_current_track_and_channel() {
                    if self.editor_tool.flags & TOOL_PENCIL_ALL_FLAGS_EXCEPT_MULTIEDIT == 0 {
                        if !self.dragged_from_ui {
                            // prevent notes suddenly popping up if the user dragged from the ui
                            self.hide_ghost_notes();
                            self.apply_ghost_notes(EditorAction::PlaceNotes(
                                Default::default(),
                                Default::default(),
                            ));
                        }
                    } else if self.editor_tool.flags & TOOL_PENCIL_DRAGGING != 0 {
                        if self.editor_tool.flags & TOOL_PENCIL_MULTIEDIT != 0 {
                            // multiple notes
                            let mut midi_pos_changes = Vec::new();

                            {
                                let ghost_notes = self.ghost_notes.lock().unwrap();
                                for (i, ghost_note) in ghost_notes.iter().enumerate() {
                                    let (old_tick, old_key) = self.temp_note_positions[i];
                                    let (new_tick, new_key) = {
                                        let ghost_note = ghost_note.get_note();
                                        (ghost_note.start, ghost_note.key)
                                    };

                                    let midi_pos_change = (
                                        new_tick as i32 - old_tick as i32,
                                        new_key as i32 - old_key as i32,
                                    );

                                    midi_pos_changes.push(midi_pos_change);
                                }

                                self.temp_note_positions.clear();
                            }

                            self.hide_ghost_notes();
                            self.apply_ghost_notes(EditorAction::NotesMove(
                                Default::default(),
                                Default::default(),
                                midi_pos_changes,
                                Default::default(),
                            ));
                        } else {
                            // single note
                            let (old_tick, old_key) = self.temp_note_positions.pop().unwrap();
                            let (new_tick, new_key) = {
                                let ghost_notes = self.ghost_notes.lock().unwrap();
                                let ghost_note = ghost_notes[0].get_note();
                                (ghost_note.start, ghost_note.key)
                            };

                            let (tick_d, key_d) = (
                                new_tick as i32 - old_tick as i32,
                                new_key as i32 - old_key as i32,
                            );

                            self.hide_ghost_notes();
                            self.apply_ghost_notes(EditorAction::NotesMove(
                                Default::default(),
                                Default::default(),
                                vec![(tick_d, key_d)],
                                Default::default(),
                            ));
                        }
                        self.is_dragging_notes = false;
                    } else if self.editor_tool.flags & TOOL_PENCIL_LENGTH_CHANGE != 0 {
                        if self.editor_tool.flags & TOOL_PENCIL_MULTIEDIT != 0 {
                            let project_data = self.project_data.lock().unwrap();
                            let mut notes = project_data.notes.lock().unwrap();
                            let curr_notes = &mut notes[curr_track as usize][curr_channel as usize];

                            let mut length_diffs = Vec::new();
                            let mut valid_note_ids = Vec::new();
                            for (i, tmp_mod_id) in self.temp_modifying_note_ids.iter().enumerate() {
                                let note_id = *tmp_mod_id;
                                let old_length = self.old_note_lengths[i];

                                // get the note we're changing the length of
                                let curr_note = &mut curr_notes[note_id];

                                let length_diff = curr_note.length as i32 - old_length as i32;
                                if length_diff != 0 {
                                    length_diffs.push(length_diff);
                                    valid_note_ids.push(*tmp_mod_id);
                                }
                            }

                            self.temp_modifying_note_ids.clear();
                            self.old_note_lengths.clear();

                            if length_diffs.len() > 0 {
                                self.editor_actions
                                    .register_action(EditorAction::LengthChange(
                                        valid_note_ids,
                                        length_diffs,
                                        curr_track as u32 * 16 + curr_channel as u32,
                                    ));
                            }
                        } else {
                            let note_id = self.temp_modifying_note_ids.pop().unwrap();
                            let old_length = self.old_note_lengths.pop().unwrap();

                            // get the note we're changing the length of
                            let project_data = self.project_data.lock().unwrap();
                            let mut notes = project_data.notes.lock().unwrap();
                            let curr_notes = &mut notes[curr_track as usize][curr_channel as usize];
                            let curr_note = &mut curr_notes[note_id];

                            let length_diff = curr_note.length as i32 - old_length as i32;

                            // register an action if there was a change in length
                            if length_diff != 0 {
                                self.editor_actions
                                    .register_action(EditorAction::LengthChange(
                                        vec![note_id],
                                        vec![length_diff],
                                        curr_track as u32 * 16 + curr_channel as u32,
                                    ));
                            }
                            //let new_note_end = self.snap_tick(midi_mouse_pos.0 as i64 + self.drag_offset as i64) + old_length as i64;
                            //let mut new_note_length = new_note_end - curr_note.start as i64;
                            //if new_note_length < old_length as i64 { new_note_length = old_length as i64; }
                            //curr_note.length = new_note_length as u32;
                        }
                    }
                }

                self.dragged_from_ui = false;

                self.old_note_lengths.clear(); // just to be safe
                self.temp_modifying_note_ids.clear();

                if let Some(midi_devices) = self.midi_devices.as_ref() {
                    let mut midi_devices = midi_devices.lock().unwrap();
                    
                    midi_devices.send_event(&[0x80, self.last_midi_ev_key, 127]).unwrap();
                }
            }
            EditorTool::Eraser => {
                if self.mouse_over_ui { self.is_waiting_for_no_ui_hover = true; return; }
                self.editor_tool.flags = TOOL_FLAGS_NONE;

                self.draw_select_box = false;

                let (min_tick, max_tick, min_key, max_key) = self.get_selection_range();

                if let Some((curr_track, curr_channel)) = self.get_current_track_and_channel() {
                    let selected = {
                        let project_data = self.project_data.lock().unwrap();
                        let notes = project_data.notes.lock().unwrap();
                        let sel_notes = &notes[curr_track as usize][curr_channel as usize];

                        get_notes_in_range(sel_notes, min_tick, max_tick, min_key, max_key, true)
                    };

                    if !selected.is_empty() {
                        self.delete_notes(Arc::new(Mutex::new(selected)));
                    }
                }
            }
            EditorTool::Selector => {
                if self.mouse_over_ui {
                    self.is_waiting_for_no_ui_hover = true;
                    return;
                }
                // select the notes
                self.draw_select_box = false;

                let (min_tick, max_tick, min_key, max_key) = self.get_selection_range();

                if let Some((curr_track, curr_channel)) = self.get_current_track_and_channel() {
                    let project_data = self.project_data.lock().unwrap();
                    let notes = project_data.notes.lock().unwrap();
                    let sel_notes = &notes[curr_track as usize][curr_channel as usize];

                    // find all notes within the bounds of the selection box
                    let selected = get_notes_in_range(sel_notes, min_tick, max_tick, min_key, max_key, true);

                    if let Some(renderer) = &self.render_manager {
                        let mut rnd = renderer.lock().unwrap();
                        rnd.get_active_renderer().lock().unwrap().set_selected(self.temp_selected_notes.clone());
                        (*self.temp_selected_notes.lock().unwrap()) = selected.clone();
                    }

                    let mut selected_global = self.temp_selected_notes.lock().unwrap();
                    if selected.is_empty() && !selected_global.is_empty() {
                        // deselect all notes
                        // no need to clear selected_global, below line does that for us already with std::mem::take :D
                        self.editor_actions.register_action(EditorAction::Deselect(
                            std::mem::take(&mut selected_global),
                            curr_track as u32 * 16 + curr_channel as u32,
                        ));
                        println!("Deselecting all notes");
                    } else {
                        self.editor_actions.register_action(EditorAction::Select(
                            selected_global.clone(),
                            curr_track as u32 * 16 + curr_channel as u32,
                        ));
                    }
                }
            }
        }
        self.tool_mouse_down = false;
        if self.is_waiting_for_no_ui_hover {
            self.is_waiting_for_no_ui_hover = false;
        }
        self.editor_tool.flags = TOOL_FLAGS_NONE;

        if !self.note_properties_mouse_up_processed && self.show_note_properties_popup {
            self.note_properties_mouse_up_processed = true;
        }*/
    }

    fn register_key_downs(&mut self, _ctx: &egui::Context, ui: &mut Ui) {
        let ctrl_down = ui.input(|i| i.modifiers.ctrl);

        // switch renderer 
        if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
            if let Some(renderer) = &self.render_manager {
                let mut renderer = renderer.lock().unwrap();
                let render_type = renderer.get_render_type();
                match render_type {
                    RenderType::PianoRoll => {
                        renderer.switch_renderer(RenderType::TrackView);
                    },
                    RenderType::TrackView => {
                        renderer.switch_renderer(RenderType::PianoRoll);
                    }
                }
            }
        }

        {
            let mut note_editing = self.note_editing.lock().unwrap();
            note_editing.on_key_down(ui, ctrl_down);
        }

        if ui.input(|i| i.key_pressed(egui::Key::Space)) {
            if let Some(playback_manager) = self.playback_manager.as_ref() {
                let mut playback_manager = playback_manager.lock().unwrap();
                playback_manager.toggle_playback();
            }
        }

        // undo/redo test
        /*if ctrl_down {
            if ui.input(|i| i.key_pressed(egui::Key::Z)) {
                let mut editor_actions = self.editor_actions.lock().unwrap();
                if let Some(action) = editor_actions.undo_action() {
                    let mut note_editing = self.note_editing.lock().unwrap();
                    note_editing.apply_action(action);
                    //self.apply_action(action);
                }
            }
            if ui.input(|i| i.key_pressed(egui::Key::Y)) {
                let mut editor_actions = self.editor_actions.lock().unwrap();
                if let Some(action) = editor_actions.redo_action() {
                    let mut note_editing = self.note_editing.lock().unwrap();
                    note_editing.apply_action(action);
                    //self.apply_action(action);
                }
            }

            // duplicate selected notes
            if ui.input(|i| i.key_pressed(egui::Key::D)) {
                if let Some((curr_track, curr_channel)) = self.get_current_track_and_channel() {
                    let (sel_notes_dupe, min_tick, max_tick) = {
                        let project_data = self.project_data.lock().unwrap();
                        let notes = project_data.notes.lock().unwrap();
                        let sel_notes = self.temp_selected_notes.lock().unwrap();
                        if let Some((min_tick, max_tick)) = get_min_max_ticks_in_selection(&notes[curr_track as usize][curr_channel as usize], &sel_notes) {
                            (sel_notes.to_vec(), min_tick, max_tick)
                        } else {
                            return;
                        }
                    };

                    self.duplicate_notes(sel_notes_dupe, max_tick, curr_track as u32 * 16 + curr_channel as u32, curr_track as u32 * 16 + curr_channel as u32, true);
                }
            }
        }

        // delete all selected notes
        if ui.input(|i| i.key_pressed(egui::Key::Delete)) {
            if self.is_dragging_notes { return; }

            let num_sel = {
                let tmp_sel = self.temp_selected_notes.lock().unwrap();
                tmp_sel.len()
            };
            if num_sel == 0 { return; }

            self.delete_selected_notes();
        }*/
    }

    fn delete_notes(&mut self, sel_ids: Arc<Mutex<Vec<usize>>>) {
        let mut tmp_sel = sel_ids.lock().unwrap();
        let project_data = self.project_data.lock().unwrap();
        let mut notes = project_data.notes.write().unwrap();

        if let Some((curr_track, curr_chan)) = self.get_current_track_and_channel() {
            let notes = &mut notes[curr_track as usize];
            let mut applied_ids = Vec::new();

            for id_sel in tmp_sel.drain(..).rev() {
                let removed_note = (*notes).remove(id_sel);
                applied_ids.push(id_sel);
                self.temp_del_notes.push_back(removed_note);
            }

            let mut editor_actions = self.editor_actions.lock().unwrap();
            editor_actions.register_action(EditorAction::DeleteNotes(applied_ids, curr_track as u32 * 16 + curr_chan as u32));
        }
    }

    fn delete_selected_notes(&mut self) {
        self.delete_notes(self.temp_selected_notes.clone());
    }

    /*fn apply_action(&mut self, action: EditorAction) {
        match action {
            EditorAction::PlaceNotes(note_ids, note_group) => {
                let project_data = self.project_data.lock().unwrap();
                let mut notes = project_data.notes.lock().unwrap();
                
                let chan = (note_group & 0xF) as usize;
                let trk = (note_group >> 4) as usize;
                let selected_notes = &mut notes[trk][chan];
                for ids in note_ids.iter().rev() {
                    let recovered_note = self.temp_del_notes.pop_back().unwrap();
                    (*selected_notes).insert(*ids, recovered_note);
                }
            }
            EditorAction::DeleteNotes(note_ids, note_group) => {
                let project_data = self.project_data.lock().unwrap();
                let mut notes = project_data.notes.lock().unwrap();

                let chan = (note_group & 0xF) as usize;
                let trk = (note_group >> 4) as usize;
                let selected_notes = &mut notes[trk][chan];
                for ids in note_ids.iter() {
                    let removed_note = (*selected_notes).remove(*ids);
                    self.temp_del_notes.push_back(removed_note);
                }
                //selected_notes.remove(note_id as usize);
            }
            EditorAction::LengthChange(note_ids, length_deltas, note_group) => {
                let project_data = self.project_data.lock().unwrap();
                let mut notes = project_data.notes.lock().unwrap();

                let chan = (note_group & 0xF) as usize;
                let trk = (note_group >> 4) as usize;
                let selected_notes = &mut notes[trk][chan];
                for (i, ids) in note_ids.iter().enumerate() {
                    let length = (*selected_notes)[*ids].length as i32;
                    (*selected_notes)[*ids].length = (length + length_deltas[i]) as u32;
                }
            }
            // probably the most complicated action of the entire editor ngl
            EditorAction::NotesMove(note_ids, new_note_ids, midi_pos_delta, note_group) => {
                let project_data = self.project_data.lock().unwrap();
                let mut notes = project_data.notes.lock().unwrap();

                let chan = (note_group & 0xF) as usize;
                let trk = (note_group >> 4) as usize;
                let selected_notes = &mut notes[trk][chan];

                let mut global_selected_notes = self.temp_selected_notes.lock().unwrap();

                // unless something gets borked, we assume that we're moving the selected notes if the array isn't empty
                let is_moving_selected = !global_selected_notes.is_empty();

                let mut expected_ticks: Vec<u32> = Vec::new();

                // println!("{:?}", selected_notes.iter().map(|a| a.start).collect::<Vec<u32>>());

                for (i, ids) in note_ids.iter().enumerate() {
                    let start = (*selected_notes)[*ids].start as i32;
                    let key = (*selected_notes)[*ids].key as i32;
                    let (new_start, new_key) = {
                        let mut new_start = start + midi_pos_delta[i].0;
                        let mut new_key = key + midi_pos_delta[i].1;
                        if new_start < 0 {
                            new_start = 0;
                        }
                        if new_key < 0 {
                            new_key = 0;
                        }
                        if new_key > 127 {
                            new_key = 127;
                        }

                        (new_start as u32, new_key as u32)
                    };

                    (*selected_notes)[*ids].start = new_start as u32;
                    (*selected_notes)[*ids].key = new_key as u8;

                    //if *ids < new_note_ids[i] { move_element(selected_notes, *ids, new_note_ids[i]); }
                    //else if *ids > new_note_ids[i] { move_element(selected_notes, new_note_ids[i], *ids); }
                    expected_ticks.push(new_start);
                    
                    // remap ids if we are
                    if is_moving_selected {
                       println!("Remapping {0} to {1}", global_selected_notes[i], new_note_ids[i]);
                       global_selected_notes[i] = new_note_ids[i];
                    }
                }

                // process any backwards-moving elements
                for (from, to) in note_ids.iter().zip(new_note_ids.iter())
                    .filter(|(f, t)| f > t)
                    .collect::<Vec<_>>() {
                        move_element(selected_notes, *to, *from);
                    }

                // process forward-moving elements in reverse
                for (from, to) in note_ids.iter().zip(new_note_ids.iter())
                    .filter(|(f, t)| f < t)
                    .rev()
                    .collect::<Vec<_>>() {
                        move_element(selected_notes, *from, *to);
                    }

                // note order check
                for (i, &tick) in expected_ticks.iter().enumerate() {
                    let new_id = new_note_ids[i];
                    let note_start = (&selected_notes[new_id]).start;
                    assert_eq!(tick, note_start, "Expected tick {0} at {1}, instead found {2} | full ticks: {3:?}\n expected ticks: {4:?}", tick, new_id, note_start, selected_notes.iter().map(|a| a.start).collect::<Vec<u32>>(), expected_ticks);
                }
            }
            EditorAction::NotesMoveImmediate(note_ids, midi_pos_delta, note_group) => {
                let project_data = self.project_data.lock().unwrap();
                let mut notes = project_data.notes.lock().unwrap();

                let chan = (note_group & 0xF) as usize;
                let trk = (note_group >> 4) as usize;
                let selected_notes = &mut notes[trk][chan];

                for (i, ids) in note_ids.iter().enumerate() {
                    let start = (*selected_notes)[*ids].start as i32;
                    let key = (*selected_notes)[*ids].key as i32;
                    let (new_start, new_key) = {
                        let mut new_start = start + midi_pos_delta[i].0;
                        let mut new_key = key + midi_pos_delta[i].1;
                        if new_start < 0 {
                            new_start = 0;
                        }
                        if new_key < 0 {
                            new_key = 0;
                        }
                        if new_key > 127 {
                            new_key = 127;
                        }

                        (new_start as u32, new_key as u32)
                    };
                    (*selected_notes)[*ids].start = new_start as u32;
                    (*selected_notes)[*ids].key = new_key as u8;
                }
            }
            EditorAction::Select(note_ids, _) => {
                // let chan = (note_group & 0xF) as usize;
                // let trk = (note_group >> 4) as usize;
                let mut tmp_sel = self.temp_selected_notes.lock().unwrap();
                tmp_sel.clear();
                for ids in note_ids.iter() {
                    tmp_sel.push(*ids);
                }
            }
            EditorAction::Deselect(note_ids, _) => {
                // let chan = (note_group & 0xF) as usize;
                // let trk = (note_group >> 4) as usize;
                let mut tmp_sel = self.temp_selected_notes.lock().unwrap();
                for ids in note_ids.iter() {
                    if let Some(index) = tmp_sel.iter().position(|&id| id == *ids) {
                        tmp_sel.remove(index);
                    }
                }
            }
            EditorAction::Bulk(mut actions) => {
                let mut actions_taken = 0;
                while let Some(action) = actions.pop() {
                    self.apply_action(action);
                    actions_taken += 1;
                }
                println!("Actions taken in a bulk action: {}", actions_taken);
            }
            EditorAction::Duplicate(_, _, _, _) => {

            }
        }
    }*/

    pub fn get_current_track_and_channel(&self) -> Option<(u16, u8)> {
        if let Some(nav) = &self.nav {
            let nav = nav.lock().unwrap();
            Some((nav.curr_track, nav.curr_channel))
        } else {
            None
        }
    }

    /// Returns the IDs of newly duplicated notes. The IDs belong to [`note_group_dst`].
    fn duplicate_notes(&mut self, note_ids: Vec<usize>, paste_tick: MIDITick, track_src: u32, track_dst: u32, select_duplicate: bool) -> Vec<usize> {
        let project_data = self.project_data.lock().unwrap();
        let mut notes = project_data.notes.write().unwrap();

        // let (src_track, src_channel) = decode_note_group(note_group_src);
        // let (dst_track, dst_channel) = decode_note_group(note_group_dst);

        let (mut notes_src, mut notes_dst) =
            if track_src == track_dst {
                (&mut notes[track_src as usize], None)
            } else {
                let (low, high) = notes.split_at_mut(std::cmp::max(track_src, track_dst) as usize);
                if track_src < track_dst {
                    (&mut low[track_src as usize],
                        Some(&mut high[0]))
                } else {
                    (&mut high[0],
                        Some(&mut low[track_dst as usize]))
                }
            };

        let mut paste_ids = Vec::new();

        {
            // deselect all notes
            let mut sel_notes = self.temp_selected_notes.lock().unwrap();
            sel_notes.clear();
        }

        let mut unique_id_hash = HashMap::new();

        // bruh this is gross
        if notes_dst.is_none() {
            let dst = &mut notes_src;
            let first_tick = dst[note_ids[0]].start;
            for &id in note_ids.iter() {
                let note_copy = {
                    let note = &dst[id];
                    Note {
                        start: note.start - first_tick + paste_tick,
                        length: note.length,
                        key: note.key,
                        velocity: note.velocity,
                        channel: note.channel
                    }
                };

                let insert_idx = bin_search_notes(&dst, note_copy.start);
                let offset = unique_id_hash.entry(insert_idx).or_insert(0);
                let unique_id = insert_idx + *offset;
                paste_ids.push(unique_id);

                if select_duplicate { // select the duplicate notes
                    let mut sel_notes = self.temp_selected_notes.lock().unwrap();
                    sel_notes.push(unique_id);
                }

                dst.insert(insert_idx, note_copy);

                *offset += 1;
            }
        } else {
            let dst = notes_dst.take().unwrap();
            let first_tick = &notes_src[note_ids[0]].start;

            for &id in note_ids.iter() {
                let note_copy = {
                    let note = &notes_src[id];
                    Note {
                        start: note.start - first_tick + paste_tick,
                        length: note.length,
                        key: note.key,
                        velocity: note.velocity,
                        channel: note.channel()
                    }
                };

                let insert_idx = bin_search_notes(&dst, note_copy.start);
                let offset = unique_id_hash.entry(insert_idx).or_insert(0);
                let unique_id = insert_idx + *offset;
                paste_ids.push(unique_id);

                if select_duplicate { // select the duplicate notes
                    let mut sel_notes = self.temp_selected_notes.lock().unwrap();
                    sel_notes.push(unique_id);
                }

                dst.insert(insert_idx, note_copy);

                *offset += 1;
            }
        }

        // why did i do this? because the way i implemented stuff is kinda weird lol
        let pasted_ids = { let mut ids = paste_ids.clone(); ids.reverse(); ids };

        let mut editor_actions = self.editor_actions.lock().unwrap();
        editor_actions.register_action(EditorAction::Duplicate(pasted_ids, paste_tick, track_src as u32, track_dst as u32));
        paste_ids
    }

    // will move the ghost notes to the actual project notes.
    fn apply_ghost_notes(&mut self, action: EditorAction) {
        /*if let Some((curr_track, curr_channel)) = self.get_current_track_and_channel() {
            let project_data = self.project_data.lock().unwrap();
            let mut notes = project_data.notes.lock().unwrap();
            let mut ghost_notes = self.ghost_notes.lock().unwrap();
            let selected_notes = &mut notes[curr_track as usize][curr_channel as usize];

            let track_chan = curr_track as u32 * 16 + curr_channel as u32;

            // vec to store in editor_actions
            let (mut old_ids, mut new_note_ids): (Vec<usize>, Vec<usize>) =
                (Vec::new(), Vec::new());

            // if global_selected_notes isn't empty, we're moving the selected notes
            let is_moving_selected = {
                let temp_selected_notes = self.temp_selected_notes.lock().unwrap();
                temp_selected_notes.len() > 0
            };

            let mut id_compensation: HashMap<usize, usize> = HashMap::new();
            //let mut id_compensate = 0; // in case the next insert_idx isn't unique.. could happen when two of the same ghost notes go on the same tick

            for (i, gnote) in ghost_notes.iter().enumerate() {
                let note = gnote.get_note();
                let insert_idx = bin_search_notes(&selected_notes, note.start);
                let offset = id_compensation.entry(insert_idx).or_insert(0);
                let real_idx = insert_idx + *offset;

                // println!("old id: {0} | new id: {1}", gnote.id.unwrap_or(insert_idx), real_idx);
                /*if is_moving_selected {
                    let mut temp_selected_notes = self.temp_selected_notes.lock().unwrap();
                    (*temp_selected_notes)[i] = insert_idx + id_compensate;
                }*/

                old_ids.push(gnote.id.unwrap_or(insert_idx));
                new_note_ids.push(real_idx);
                (*selected_notes).insert(
                    insert_idx,
                    Note {
                        start: note.start,
                        length: note.length,
                        key: note.key,
                        velocity: note.velocity,
                    },
                );

                *offset += 1;

                if is_moving_selected {
                    let mut temp_selected_notes = self.temp_selected_notes.lock().unwrap();
                    (*temp_selected_notes)[i] = real_idx;
                }

                //last_insert_idx = insert_idx;
            }

            // then register action
            match action {
                EditorAction::PlaceNotes(_, _) => {
                    self.editor_actions
                        .register_action(EditorAction::PlaceNotes(new_note_ids, track_chan));
                }
                EditorAction::NotesMove(id_override, _, position_deltas, _) => {
                    // before registering, make sure we actually have moved the notes lol
                    let valid_register = {
                        let mut vreg = false;
                        for (tick, key) in position_deltas.iter() {
                            if *tick != 0 || *key != 0 { vreg = true; break; }
                        }
                        vreg
                    };

                    if valid_register {
                        self.editor_actions.register_action(EditorAction::NotesMove(
                            if id_override.len() > 0 {
                                id_override
                            } else {
                                old_ids
                            },
                            new_note_ids,
                            position_deltas,
                            track_chan,
                        ));
                    }
                }
                _ => {}
            }

            // experimental: clear ghost notes
            //(*self.ghost_notes.lock().unwrap()).clear();
            ghost_notes.clear();
            //if register_action { self.editor_actions.register_action(EditorAction::PlaceNotes(applied_note_ids, curr_track as u32 * 16 + curr_channel as u32)); }
        }*/
    }

    /*fn snap_tick(&self, tick: i32) -> i32 {
        let snap = self.get_min_snap_tick_length();
        if snap == 1 { return tick; }
        (tick as f32 / snap as f32).round() as i32 * (snap as i32)
    }

    fn get_min_snap_tick_length(&self) -> u16 {
        let project_data = self.project_data.lock().unwrap();
        let snap_ratio = self.editor_tool.snap_ratio;
        if snap_ratio.0 == 0 { return 1; }
        return ((project_data.project_info.ppq as u32 * 4 * snap_ratio.0 as u32)
            / snap_ratio.1 as u32) as u16;
    }*/

    fn update_cursor(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        let note_editing = self.note_editing.lock().unwrap();
        note_editing.update_cursor(ctx, ui);

        /*if self.mouse_over_ui || self.tool_dialogs_any_open {
            ctx.set_cursor_icon(egui::CursorIcon::Default);
            return;
        }
        let (midi_mouse_pos, _) = self.get_mouse_midi_pos(ui);

        match self.editor_tool.curr_tool {
            EditorTool::Pencil => {
                if let Some((curr_track, curr_channel)) = self.get_current_track_and_channel() {
                    let note_id = {
                        let project_data = self.project_data.lock().unwrap();
                        let notes = project_data.notes.lock().unwrap();

                        let notes = &notes[curr_track as usize][curr_channel as usize];
                        find_note_at(&notes, midi_mouse_pos.0, midi_mouse_pos.1)
                    };

                    if note_id.is_none() {
                        ctx.set_cursor_icon(egui::CursorIcon::Default);
                        return;
                    }

                    let is_at_note_end = self.is_cursor_at_note_end(
                        note_id.unwrap(),
                        curr_track,
                        curr_channel,
                        midi_mouse_pos,
                    );
                    if !is_at_note_end {
                        ctx.set_cursor_icon(egui::CursorIcon::Move);
                        return;
                    }

                    ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                }
            }
            EditorTool::Eraser => {
                ctx.set_cursor_icon(egui::CursorIcon::Default);
            }
            EditorTool::Selector => {
                ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
            }
        }*/
    }

    fn pan_view_if_mouse_near_edge(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        let rect = ui.min_rect();
        if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
            if let Some(nav) = &self.nav {
                let mut nav = nav.lock().unwrap();
                let pan_bottom = rect.bottom() - 100.0 - mouse_pos.y < 0.0;
                let pan_top = mouse_pos.y - 100.0 < 0.0;

                if pan_bottom {
                    nav.key_pos -= 0.25;
                    if nav.key_pos < 0.0 {
                        nav.key_pos = 0.0;
                    }
                    ctx.request_repaint();
                }

                if pan_top {
                    nav.key_pos += 0.25;
                    if nav.key_pos > 128.0 - nav.zoom_keys {
                        nav.key_pos = 128.0 - nav.zoom_keys;
                    }
                    ctx.request_repaint();
                }
            }
        }
    }

    fn draw(&mut self, ctx: &egui::Context, ui: &mut Ui, any_window_opened: bool) {
        let available_size = ui.available_size();
        let (rect, _response) = ui.allocate_exact_size(available_size, egui::Sense::hover());

        // skip all this if gl or renderer isnt ready yet
        if self.gl.is_none() || self.render_manager.is_none() || self.nav.is_none() {
            return;
        }
        
        if !any_window_opened {
            self.nav
                .as_ref()
                .unwrap()
                .lock()
                .unwrap()
                .update_smoothed_values();
            
            self.track_view_nav
                .as_ref()
                .unwrap()
                .lock()
                .unwrap()
                .update_smoothed_values();
            
            let render_type = {
                let render_manager = self.render_manager.as_ref().unwrap().lock().unwrap();
                *(render_manager.get_render_type())
            };

            {
                let mut note_editing = self.note_editing.lock().unwrap();
                note_editing.update_from_ui(ui);
                note_editing.set_mouse_over_ui(self.mouse_over_ui);
            }

            // mouse stuff
            if ui.input(|i| i.pointer.primary_pressed()) {
                let is_double_click = { 
                    let curr_time = ui.input(|i| i.time);
                    if curr_time - self.last_click_time < 0.25 {
                        self.last_click_time = 0.0;
                        true
                    } else { 
                        self.last_click_time = curr_time;
                        false
                    }
                };
                println!("{}", is_double_click);

                match render_type {
                    RenderType::PianoRoll => {
                        self.handle_mouse_down_pr(ctx, ui)
                    }
                    RenderType::TrackView => {}
                }

                if is_double_click {
                    match render_type {
                        RenderType::PianoRoll => { self.handle_mouse_double_down_pr(ctx, ui) }
                        RenderType::TrackView => {}
                    }
                }
            }

            if self.tool_mouse_down {
                self.pan_view_if_mouse_near_edge(ctx, ui);
            }

            if ui.input(|i| i.pointer.is_moving()) {
                match render_type {
                    RenderType::PianoRoll => { self.handle_mouse_move_pr(ctx, ui) }
                    RenderType::TrackView => {}
                }
            }

            if ui.input(|i| i.pointer.primary_released()) {
                match render_type {
                    RenderType::PianoRoll => { self.handle_mouse_up_pr(ctx, ui) }
                    RenderType::TrackView => {}
                }
            }

            self.register_key_downs(ctx, ui);

            match render_type {
                RenderType::PianoRoll => { self.update_cursor(ctx, ui); }
                RenderType::TrackView => {
                    ctx.set_cursor_icon(egui::CursorIcon::Default);
                }
            }
        }

        let gl = self.gl.as_ref().unwrap();
        let renderer = self.render_manager.as_ref().unwrap();

        let window_height = ctx.input(|i| i.screen_rect).size().y;

        let callback = egui::PaintCallback {
            rect,
            callback: Arc::new(CallbackFn::new({
                
                let gl = Arc::clone(&gl);
                let renderer = Arc::clone(&renderer);

                move |info, _| unsafe {
                    let vp = info.viewport_in_pixels();
                    gl.enable(glow::SCISSOR_TEST);
                    gl.scissor(vp.left_px, vp.from_bottom_px, vp.width_px, vp.height_px);
                    gl.clear(glow::COLOR_BUFFER_BIT);
                    gl.clear_color(0.0, 0.0, 0.0, 1.0);
                    {
                        let mut render = renderer.lock().unwrap();
                        let mut rnd = render.get_active_renderer().lock().unwrap();
                        (*rnd).window_size(rect.size());
                        (*rnd).draw();
                    }
                    gl.disable(glow::SCISSOR_TEST);
                }
            })),
        };

        //ctx.layer_painter(egui::LayerId::background()).add(callback);
        ui.painter().add(callback);

        {
            let note_editing = self.note_editing.lock().unwrap();

            if note_editing.get_can_draw_selection_box() {
                let (tl, br) = note_editing.get_selection_range_ui(ui);
                let is_eraser = note_editing.is_eraser_active();
                
                ui.painter().rect(
                    egui::Rect::from_min_max(
                        egui::Pos2 { x: tl.0, y: tl.1 },
                        egui::Pos2 { x: br.0, y: br.1 },
                    ),
                    0,
                    Color32::TRANSPARENT,
                    Stroke {
                        width: 2.0,
                        color: if is_eraser { Color32::RED } else { Color32::WHITE },
                    },
                    egui::StrokeKind::Middle,
                );
            }
        }
    }

    fn draw_data_viewer(&mut self, ctx: &egui::Context, ui: &mut Ui, any_window_opened: bool) {
        let available_width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2 { x: available_width, y: 200.0 }, egui::Sense::hover());

        if self.gl.is_none() || self.data_view_renderer.is_none() { return; }
        
        let gl = self.gl.as_ref().unwrap();
        let data_view_renderer = self.data_view_renderer.as_ref().unwrap();

        let callback = egui::PaintCallback {
            rect,
            callback: Arc::new(CallbackFn::new({
                
                let gl = Arc::clone(&gl);
                let renderer = Arc::clone(&data_view_renderer);

                move |info, _| unsafe {
                    let vp = info.viewport_in_pixels();
                    gl.enable(glow::SCISSOR_TEST);
                    gl.scissor(vp.left_px, vp.from_bottom_px, vp.width_px, vp.height_px);
                    gl.clear(glow::COLOR_BUFFER_BIT);
                    gl.clear_color(0.0, 0.0, 0.0, 1.0);
                    {
                        let mut render = renderer.lock().unwrap();    
                        (*render).window_size(rect.size());
                        (*render).draw();
                    }
                    gl.disable(glow::SCISSOR_TEST);
                }
            }))
        };

        ui.painter().add(callback);
    }
}

impl eframe::App for MainWindow {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let any_window_opened = self.settings_window.is_showing() || self.tool_dialogs_any_open;
        // initialize gl if not initialized already
        if self.gl.is_none() {
            if let Some(gl) = frame.gl() {
                self.gl = Some(gl.clone());
                self.init_gl();
                self.init_note_editing();
            }
        }

        if let Some(playback_manager) = self.playback_manager.as_ref() {
            let playback_manager = playback_manager.lock().unwrap();
            if playback_manager.playing {
                ctx.request_repaint();
            }
        }

        let is_on_track_view = {
            if let Some(render_manager) = self.render_manager.as_ref() {
                let render_manager = render_manager.lock().unwrap();
                *render_manager.get_render_type() == RenderType::TrackView
            } else {
                false
            }
        };

        egui::CentralPanel::default().show(ctx, |ui| {
            // Menu Bar at top
            egui::TopBottomPanel::top("menu_bar")
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("New Project").clicked() {
                            let is_empty = {
                                let project_data = self.project_data.lock().unwrap();
                                let notes = project_data.notes.read().unwrap();
                                notes.is_empty()
                            };

                            if !is_empty {
                                self.show_override_popup = true;
                                self.override_popup_msg =
                                    "Are you sure you want to start a new project?";
                                self.override_popup_func =
                                    Some(Box::new(|main_window, _: &egui::Context| {
                                        {
                                            let mut project_data = main_window.project_data.lock().unwrap();
                                            println!("Clearning notes...");
                                            project_data.new_empty_project();
                                        }

                                        {
                                            println!("Removing action history...");
                                            let mut editor_actions = main_window.editor_actions.lock().unwrap();
                                            editor_actions.clear_actions();
                                        }

                                        {
                                            let mut playhead = &mut main_window.playhead;
                                            playhead.set_start(0);
                                            if let Some(nav) = main_window.nav.as_mut() {
                                                let mut nav = nav.lock().unwrap();
                                                nav.tick_pos = 0.0;
                                            }
                                        }
                                    }));
                            }
                        }
                        if ui.button("Import MIDI file").clicked() {
                            self.import_midi_file();
                        }
                        if ui.button("Export as MIDI file").clicked() {
                            self.export_midi_file();
                        }
                        self.mouse_over_ui |= ui.ui_contains_pointer();
                    });
                    ui.menu_button("Edit", |ui| {
                        let mut editor_actions = self.editor_actions.lock().unwrap();
                        if ui.add_enabled(editor_actions.get_can_undo(), egui::Button::new("Undo")).clicked() {
                            if let Some(action) = editor_actions.undo_action() {
                                let mut note_editing = self.note_editing.lock().unwrap();
                                note_editing.apply_action(action);
                                //self.apply_action(action);
                            }
                        }
                        if ui.add_enabled(editor_actions.get_can_redo(), egui::Button::new("Redo")).clicked() {
                            if let Some(action) = editor_actions.redo_action() {
                                let mut note_editing = self.note_editing.lock().unwrap();
                                note_editing.apply_action(action);
                                //self.apply_action(action);
                            }
                        }
                        ui.separator();
                        ui.menu_button("Insert...", |ui| {
                            if ui.button("Time Signature").clicked() {
                                let meta_editing = self.meta_editing.clone();
                                let playhead_pos = self.playhead.start_tick;

                                self.meta_ev_insert_dialog.show(MetaEventType::TimeSignature, move |data| {
                                    let mut meta_editing = meta_editing.lock().unwrap();
                                    meta_editing.insert_meta_event(MetaEvent {
                                        tick: playhead_pos,
                                        event_type: MetaEventType::TimeSignature,
                                        data
                                    });
                                });
                            }
                        });
                        self.mouse_over_ui |= ui.ui_contains_pointer();
                    });
                    ui.menu_button("Options", |ui| {
                        if ui.button("Preferences").clicked() {
                            self.settings_window.show();
                        }
                        self.mouse_over_ui |= ui.ui_contains_pointer();
                    });
                    ui.menu_button("Project", |ui| {
                        if ui.button("Project settings...").clicked() {
                            self.project_settings.show();
                        }
                        self.mouse_over_ui |= ui.ui_contains_pointer();
                    });
                    ui.menu_button("Tools", |ui| {
                        let mut should_close = false;
                        ui.menu_button("Editing", |ui| {
                            let has_selected_notes = {
                                let note_editing = self.note_editing.lock().unwrap();
                                note_editing.is_any_note_selected()
                            };
                            
                            if ui.add_enabled(has_selected_notes, egui::Button::new("Flip X (Tick-wise)")).clicked() {
                                let note_editing = self.note_editing.lock().unwrap();
                                let notes = note_editing.get_notes();
                                let sel_notes = note_editing.get_selected_note_ids();

                                let mut notes = notes.write().unwrap();
                                let mut sel_notes = sel_notes.lock().unwrap();
                                let sel_notes_clone = sel_notes.clone();

                                if let Some((curr_track, curr_channel)) = self.get_current_track_and_channel() {
                                    let mut editor_actions = self.editor_actions.lock().unwrap();
                                    self.editor_functions.apply_function(
                                        &mut notes[curr_track as usize],
                                        &mut sel_notes,
                                        EditFunction::FlipX(sel_notes_clone),
                                        curr_track,
                                        curr_channel,
                                        &mut editor_actions
                                    );
                                }

                                should_close = true;
                            }

                            if ui.add_enabled(has_selected_notes, egui::Button::new("Flip Y (Key-wise)")).clicked() {
                                let mut sel_notes = self.temp_selected_notes.lock().unwrap();
                                let project_data = self.project_data.lock().unwrap();
                                let mut notes = project_data.notes.write().unwrap();
                                let sel_notes_copy = sel_notes.clone();

                                if let Some((curr_track, curr_channel)) = self.get_current_track_and_channel() {
                                    let mut editor_actions = self.editor_actions.lock().unwrap();
                                    self.editor_functions.apply_function(&mut notes[curr_track as usize], &mut sel_notes, EditFunction::FlipY(sel_notes_copy), curr_track, curr_channel, &mut editor_actions);
                                }
                                should_close = true;
                            }
                            ui.separator();
                            if ui.add_enabled(has_selected_notes, egui::Button::new("Stretch selection...")).clicked() {
                                self.show_note_properties_popup = false;
                                self.note_properties_mouse_up_processed = false;

                                self.ef_stretch_dialog.show();
                                self.tool_dialogs_any_open = true;
                                //let mut notes = self.project_data.notes.lock().unwrap();
                                //let sel_notes = self.temp_selected_notes.lock().unwrap();

                                /*if let Some((curr_track, curr_channel)) = self.get_current_track_and_channel() {
                                    self.editor_functions.apply_function(&mut notes[curr_track as usize][curr_channel as usize], EditFunction::Stretch(sel_notes.to_vec(), 0.5), curr_track, curr_channel, &mut self.editor_actions);
                                }*/

                                should_close = true;
                            }

                            self.mouse_over_ui |= ui.ui_contains_pointer();
                        });

                        ui.menu_button("Generate", |ui| {
                            let has_selected_notes = {
                                let sel_notes = self.temp_selected_notes.lock().unwrap();
                                !sel_notes.is_empty()
                            };

                            if ui.add_enabled(has_selected_notes, egui::Button::new("Chop notes...")).clicked() {
                                self.show_note_properties_popup = false;
                                self.note_properties_mouse_up_processed = false;

                                self.ef_chop_dialog.show();
                            }
                        });
                        if should_close {
                            ui.close_menu();
                        }
                    });
                    ui.menu_button("Help", |ui| {
                        
                    });
                });
                ui.separator();
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                    if ui.button("Pencil").clicked() {
                        let mut editor_tool = self.editor_tool.lock().unwrap();
                        editor_tool.switch_tool(EditorTool::Pencil);
                        self.is_waiting_for_no_ui_hover = false;
                    }
                    if ui.button("Eraser").clicked() {
                        let mut editor_tool = self.editor_tool.lock().unwrap();
                        editor_tool.switch_tool(EditorTool::Eraser);
                        self.is_waiting_for_no_ui_hover = false;
                    }
                    if ui.button("Select").clicked() {
                        let mut editor_tool = self.editor_tool.lock().unwrap();
                        editor_tool.switch_tool(EditorTool::Selector);
                        self.is_waiting_for_no_ui_hover = false;
                    }
                    ui.separator();
                    ui.menu_button("Note Snap", |ui| {
                        {
                            let mut editor_tool = self.editor_tool.lock().unwrap();
                            for (ratio, name) in SNAP_MAPPINGS {
                                if ui
                                    .checkbox(&mut (ratio == editor_tool.snap_ratio), name)
                                    .clicked()
                                {
                                    editor_tool.snap_ratio = ratio;
                                }
                            }
                        }
                        self.mouse_over_ui |= ui.ui_contains_pointer();
                    });
                    ui.separator();
                    // note gate and velocity
                    let mut tbs = self.toolbar_settings.lock().unwrap();
                    tbs.note_gate.show("Gate", ui, Some(30.0));
                    tbs.note_velocity.show("Velo", ui, Some(30.0));

                    ui.separator();
                    //let vs = &mut self.view_settings;
                    if let Some(vs) = self.view_settings.as_mut() {
                        let mut vs = vs.lock().unwrap();
                        egui::ComboBox::from_label("View Tracks")
                        .selected_text(format!("{}", vs.pr_onion_state.to_string()))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut vs.pr_onion_state, VS_PianoRoll_OnionState::NoOnion, "No tracks");
                            ui.selectable_value(&mut vs.pr_onion_state, VS_PianoRoll_OnionState::ViewAll, "All tracks");
                            ui.selectable_value(&mut vs.pr_onion_state, VS_PianoRoll_OnionState::ViewNext, "Next track");
                            ui.selectable_value(&mut vs.pr_onion_state, VS_PianoRoll_OnionState::ViewPrevious, "Previous track");
                            self.mouse_over_ui |= ui.ui_contains_pointer();
                        });
                    }
                    ui.separator();
                    //int_edit_field(ui, "Gate", &mut tbs.note_gate, 1, u16::MAX.into(), Some(30.0));
                    //int_edit_field(ui, "Velo", &mut tbs.note_velocity, 1, 127, Some(30.0));

                    self.mouse_over_ui |= ui.ui_contains_pointer();
                });
                //ui.separator();
                self.mouse_over_ui |= ui.ui_contains_pointer();
            });

            egui::TopBottomPanel::top("Playhead")
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    let (min_tick, max_tick) = {
                        if let Some(nav) = self.nav.as_ref() {
                            let nav = nav.lock().unwrap();
                            (nav.tick_pos_smoothed as MIDITick, (nav.tick_pos_smoothed + nav.zoom_ticks_smoothed) as MIDITick)
                        } else {
                            (0, 1920)
                        }
                    };

                    ui.style_mut().spacing.slider_width = ui.available_width();
                    let mut playhead_time = self.playhead.start_tick;
                    if ui.add(
                        egui::Slider::new(&mut playhead_time, min_tick..=max_tick)
                        .show_value(false)
                        .clamping(egui::SliderClamping::Never)
                    ).changed() {
                        let min_snap_length = {
                            let editor_tool = self.editor_tool.lock().unwrap();
                            let snap_ratio = editor_tool.snap_ratio;
                            if snap_ratio.0 == 0 { 1 }
                            else {
                                let ppq = {
                                    let project_data = self.project_data.lock().unwrap();
                                    project_data.project_info.ppq as MIDITick
                                };
                                (ppq * 4 * snap_ratio.0 as MIDITick) / snap_ratio.1 as MIDITick
                            }
                        };

                        let playhead_time = playhead_time.rounded_div(min_snap_length) * min_snap_length;
                        self.playhead.set_start(playhead_time);
                        println!("{}", playhead_time);
                    }
                });
                self.mouse_over_ui |= ui.ui_contains_pointer();
            });

            if !any_window_opened {
                self.handle_navigation(ctx, ui);
            }

            {
                let (dataview_state, dataview_size) = if let Some(view_settings) = self.view_settings.as_ref() {
                    let vs = view_settings.lock().unwrap();
                    (vs.pr_dataview_state, vs.pr_dataview_size)
                } else {
                    (VS_PianoRoll_DataViewState::Hidden, 0.25)
                };

                if dataview_state != VS_PianoRoll_DataViewState::Hidden && !is_on_track_view {
                    egui::TopBottomPanel::bottom("data_viewer").show(ctx, |ui| {
                        egui::ComboBox::from_label("Property")
                            .selected_text(dataview_state.to_string())
                            .show_ui(ui, |ui| {
                                let view_settings = self.view_settings.as_mut().unwrap();
                                let mut view_settings = view_settings.lock().unwrap();

                                ui.selectable_value(&mut view_settings.pr_dataview_state, VS_PianoRoll_DataViewState::NoteVelocities, "Velocity");
                            });

                        // TODO: draw data_viewer
                        self.draw_data_viewer(ctx, ui, any_window_opened);

                        self.mouse_over_ui |= ui.ui_contains_pointer();
                    });
                }
            }

            // piano roll / track view rendering
            egui::CentralPanel::default().show(ctx, |ui| {
                self.draw(ctx, ui, any_window_opened);
                self.mouse_over_ui = false;
            });
        });

        if self.show_override_popup {
            self.show_note_properties_popup = false;
            egui::Window::new(RichText::new("Confirmation").size(10.0))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(RichText::new(self.override_popup_msg).size(15.0));
                    ui.horizontal(|ui| {
                        if ui.button("Yup").clicked() {
                            if let Some(func) = self.override_popup_func.take() {
                                func(self, ctx)
                            }
                            self.show_override_popup = false;
                        }

                        if ui.button("Nah").clicked() {
                            self.show_override_popup = false;
                        }
                        self.mouse_over_ui |= ui.ui_contains_pointer();
                    });
                    self.mouse_over_ui |= ui.ui_contains_pointer();
                });
        }

        if self.show_note_properties_popup && self.note_properties_mouse_up_processed {
            // self.mouse_over_ui = true;
            egui::Window::new(RichText::new("Note properties").size(15.0))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    if let Some((curr_track, curr_channel)) = self.get_current_track_and_channel() {
                        let project_data = self.project_data.lock().unwrap();
                        let mut notes = project_data.notes.read().unwrap();
                        // let note = &mut notes[curr_track as usize][curr_channel as usize][self.last_clicked_note_id.unwrap()];

                        // self.toolbar_settings.note_gate.show("Note gate", ui, None);
                        // self.toolbar_settings.note_velocity.show("Note velocity", ui, None);

                        if ui.button("Confirm").clicked() {
                            self.show_note_properties_popup = false;
                            self.note_properties_mouse_up_processed = false;
                        }
                    }
                    self.mouse_over_ui |= ui.ui_contains_pointer();
                });
        }

        // the fun stuff: the tool dialogs :D
        if self.ef_stretch_dialog.is_shown {
            // stretch notes dialog
            egui::Window::new(RichText::new("Stretch Selection").size(15.0))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    //decimal_edit_field(ui, "Factor (x)", &mut self.ef_stretch_dialog.stretch_factor, 0.0, 1024.0, None);
                    self.ef_stretch_dialog.stretch_factor.show("Stretch factor (x):", ui, None);

                    ui.horizontal(|ui| {
                        if ui.button("Confirm").clicked() {
                            if let Some((curr_track, curr_channel)) = self.get_current_track_and_channel() {
                                let project_data = self.project_data.lock().unwrap();
                                let mut notes = project_data.notes.write().unwrap();
                                let notes = &mut notes[curr_track as usize];

                                let mut sel_notes = self.temp_selected_notes.lock().unwrap();
                                let sel_notes_copy = sel_notes.clone();

                                let mut editor_actions = self.editor_actions.lock().unwrap();
                                self.editor_functions.apply_function(notes, &mut sel_notes, EditFunction::Stretch(sel_notes_copy, self.ef_stretch_dialog.stretch_factor.value() as f32), curr_track, curr_channel, &mut editor_actions);
                            }
                            self.ef_stretch_dialog.close();
                            self.tool_dialogs_any_open = false;
                        }

                        if ui.button("Cancel").clicked() {
                            self.ef_stretch_dialog.close();
                            self.tool_dialogs_any_open = false;
                        }

                        self.mouse_over_ui |= ui.ui_contains_pointer();
                    });

                    self.mouse_over_ui |= ui.ui_contains_pointer();
                });
        }

        if self.ef_chop_dialog.is_shown {
            egui::Window::new(RichText::new("Chop notes").size(15.0))
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.checkbox(&mut self.ef_chop_dialog.use_tick_lens, "By tick");
                    if self.ef_chop_dialog.use_tick_lens {
                        self.ef_chop_dialog.target_tick_len.show("Tick duration", ui, None);
                        self.mouse_over_ui |= ui.ui_contains_pointer();
                    } else {
                        ui.label("Chop duration");
                        ui.separator();
                        for (i, &(ratio, name)) in SNAP_MAPPINGS.iter().enumerate() {
                            if ui
                                .checkbox(&mut (i == self.ef_chop_dialog.snap_id), name)
                                .clicked()
                            {
                                self.ef_chop_dialog.snap_id = i;
                            }
                        }
                        self.mouse_over_ui |= ui.ui_contains_pointer();
                    }

                    ui.horizontal(|ui| {
                        if ui.button("Confirm").clicked() {
                            if let Some((curr_track, curr_channel)) = self.get_current_track_and_channel() {
                                let project_data = self.project_data.lock().unwrap();
                                let mut notes = project_data.notes.write().unwrap();
                                let notes = &mut notes[curr_track as usize];

                                let mut sel_notes = self.temp_selected_notes.lock().unwrap();
                                let sel_notes_copy = sel_notes.clone();

                                let mut editor_actions = self.editor_actions.lock().unwrap();
                                self.editor_functions.apply_function(notes, &mut sel_notes, EditFunction::Chop(sel_notes_copy, self.ef_chop_dialog.use_tick_lens, self.ef_chop_dialog.snap_id, self.ef_chop_dialog.target_tick_len.value() as u32), curr_track, curr_channel, &mut editor_actions);
                            }
                            self.ef_chop_dialog.close();
                            self.tool_dialogs_any_open = false;
                        }

                        if ui.button("Cancel").clicked() {
                            self.ef_chop_dialog.close();
                            self.tool_dialogs_any_open = false;
                        }
                        self.mouse_over_ui |= ui.ui_contains_pointer();
                    });
                    self.mouse_over_ui |= ui.ui_contains_pointer();
                    // self.ef_chop_dialog.
                });
        }

        {
            let mut note_editing = self.note_editing.lock().unwrap();
            if self.settings_window.draw_window(ctx) ||
            self.project_settings.draw_window(ctx) ||
            self.meta_ev_insert_dialog.draw(ctx) {
                note_editing.set_any_dialogs_open(true);
            } else {
                note_editing.set_any_dialogs_open(false);
            }
        }

        /*while let Some(action) = self.funcs_after_render.pop() {
            action(self, ctx);
        }*/
    }
}
