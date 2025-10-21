// abstraction is NEEDED!!
use crate::{
    app::{
        custom_widgets::{NumberField, NumericField}, rendering::{data_view::DataViewRenderer, note_cull_helper::NoteCullHelper, RenderManager, RenderType, Renderer}, shared::{NoteColorIndexing, NoteColors}, ui::{dialog::Dialog, edtior_info::EditorInfo, main_menu_bar::{MainMenuBar, MenuItem}, manual::EditorManualDialog}, util::image_loader::ImageResources, view_settings::{VS_PianoRoll_DataViewState, VS_PianoRoll_OnionColoring, VS_PianoRoll_OnionState}}, 
        audio::{event_playback::PlaybackManager, kdmapi_engine::kdmapi::KDMAPI, midi_devices::MIDIDevices}, 
        editor::{
            edit_functions::EFChopDialog, editing::note_editing::note_edit_flags::NOTE_EDIT_MOUSE_OVER_UI, midi_bar_cacher::BarCacher, navigation::{TrackViewNavigation, GLOBAL_ZOOM_FACTOR}, playhead::Playhead, plugins::{plugin_andromeda_obj::AndromedaObj, plugin_dialog::PluginDialog, plugin_lua::PluginLua, PluginLoader}, settings::{editor_settings::{ESAudioSettings, ESGeneralSettings, ESSettingsWindow, Settings}, project_settings::ProjectSettings}, util::{get_mouse_midi_pos, path_rel_to_abs, MIDITick}}, midi::{events::meta_event::{MetaEvent, MetaEventType}, midi_file::MIDIEvent}};
use crate::editor::editing::{
    meta_editing::{MetaEditing, MetaEventInsertDialog},
    note_editing::{NoteEditing, note_edit_flags::*},
    track_editing::TrackEditing
};


use eframe::{
    egui::{self, Color32, PaintCallback, Pos2, Rect, RichText, Shape, Stroke, Ui, Vec2}, egui_glow::CallbackFn, glow::HasContext
};
use egui_double_slider::DoubleSlider;
use rayon::prelude::*;
use rounded_div::RoundedDiv;

use crate::{
    app::{
        view_settings::ViewSettings,
    },
    editor::{
        actions::{EditorActions},
        edit_functions::{EFStretchDialog, EditFunction, EditFunctions},
        navigation::PianoRollNavigation,
        project_data::ProjectData
    },
    midi::{
        midi_file::MIDIFileWriter,
    },
};
use eframe::glow;
use std::{
    cell::RefCell, collections::{HashMap}, rc::Rc, sync::{Arc, Mutex, RwLock}, time::Instant
};

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

#[derive(Clone, PartialEq)]
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
    pub snap_ratio: (u8, u16),
}

impl Default for EditorToolSettings {
    fn default() -> Self {
        Self {
            curr_tool: Default::default(),
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
}

pub struct ToolBarSettings {
    pub note_gate: NumericField<MIDITick>,
    pub note_velocity: NumericField<u8>,
    pub note_channel: NumericField<u8>,
}

impl Default for ToolBarSettings {
    fn default() -> Self {
        Self {
            note_gate: NumericField::new(960, Some(1), Some(u16::MAX.into())),
            note_velocity: NumericField::new(100, Some(1), Some(127)),
            note_channel: NumericField::new(1, Some(1), Some(16))
        }
    }
}

#[derive(Default)]
pub struct MainWindow {
    pub project_data: Rc<RefCell<ProjectData>>,
    bar_cacher: Arc<Mutex<BarCacher>>,
    gl: Option<Arc<glow::Context>>,
    // renderer: Option<Arc<Mutex<dyn Renderer + Send + Sync>>>,
    render_manager: Option<Arc<Mutex<RenderManager>>>,
    data_view_renderer: Option<Arc<Mutex<DataViewRenderer>>>,
    playback_manager: Option<Arc<Mutex<PlaybackManager>>>,
    pub note_editing: Arc<Mutex<NoteEditing>>,
    pub meta_editing: Arc<Mutex<MetaEditing>>,
    pub track_editing: Arc<Mutex<TrackEditing>>,
    nav: Option<Arc<Mutex<PianoRollNavigation>>>,
    track_view_nav: Option<Arc<Mutex<TrackViewNavigation>>>,
    view_settings: Option<Arc<Mutex<ViewSettings>>>,
    playhead: Rc<RefCell<Playhead>>,
    note_colors: Arc<Mutex<NoteColors>>,

    mouse_over_ui: bool,
    editor_tool: Rc<RefCell<EditorToolSettings>>,

    settings: Vec<Box<dyn Settings>>,
    note_culler: Arc<Mutex<NoteCullHelper>>,

    // ghost note index zero is reserved for the pencil note
    // ghost_notes: Arc<Mutex<Vec<GhostNote>>>,

    pub editor_actions: Rc<RefCell<EditorActions>>,
    pub editor_functions: Rc<RefCell<EditFunctions>>,

    // for the top toolbar
    toolbar_settings: Rc<RefCell<ToolBarSettings>>,

    // if mouse gets released while over ui
    is_waiting_for_no_ui_hover: bool,

    // other
    // override popup settings
    show_override_popup: bool,
    override_popup_msg: &'static str,
    override_popup_func: Option<Box<dyn Fn(&mut MainWindow, &egui::Context) -> ()>>, // hacky

    // note properties popup
    show_note_properties_popup: bool,
    // note_properties_popup_note_id: usize, // the id the popup is referring to
    note_properties_mouse_up_processed: bool, // to compensate for unprocessed mouse up events after the dialog opens

    last_click_time: f64,

    midi_devices: Option<Arc<Mutex<MIDIDevices>>>,
    kdmapi: Option<Arc<Mutex<KDMAPI>>>,

    // meta_ev_insert_dialog: MetaEventInsertDialog,

    // the ui stuff
    menu_bar: Option<Arc<RwLock<MainMenuBar>>>,
    dialogs: HashMap<&'static str, Box<dyn Dialog>>,

    // images
    image_resources: Option<ImageResources>,

    // plugin stuff
    plugin_loader: Option<PluginLoader>,

    // context menu stuff
    context_menu_shown: bool
}

impl MainWindow {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut s = Self::default();

        // s.load_colors("./assets/shaders/textures/notes.png");

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

        {
            let project_data = s.project_data.borrow();
            let playback_manager = Arc::new(Mutex::new(
                PlaybackManager::new(
                    s.kdmapi.as_ref().unwrap().clone(),
                    &project_data.notes,
                    &project_data.global_metas,
                    &project_data.channel_events,
                    &project_data.tempo_map
                )
            ));
            // s.settings_window.use_midi_devices(midi_devices.clone());
            // s.settings_window.use_playback_manager(playback_manager.clone());
            s.playhead = Rc::new(RefCell::new(Playhead::new(0, &playback_manager)));
            s.playback_manager = Some(playback_manager);
        }

        // s.project_settings = ProjectSettings::new(&s.project_data);
        /*s.ghost_notes = Arc::new(Mutex::new(vec![GhostNote {
            ..Default::default()
        }]));*/
        // s.project_data.lock().unwrap().new_empty_project();
        {
            let mut project_data = s.project_data.borrow_mut();
            project_data.new_empty_project();

            s.bar_cacher = Arc::new(Mutex::new(BarCacher::new(960, 
                &project_data.global_metas
            )));
            s.note_culler = Arc::new(Mutex::new(NoteCullHelper::new(&project_data.notes)));
        }
        s.editor_actions = Rc::new(RefCell::new(EditorActions::new(256)));
        s.editor_functions = Rc::new(RefCell::new(EditFunctions::default()));

        {
            let mut plugin_loader = PluginLoader::new();
            plugin_loader.load_plugins(&path_rel_to_abs("./assets/plugins".into())).unwrap();
            s.plugin_loader = Some(plugin_loader);
        }
        s
    }

    fn load_images(&mut self, ctx: &egui::Context) {
        let mut image_resources = ImageResources::new();
        let icon_names = ["logo", "logo_medium", "logo_small", "zoom_x_in", "zoom_x_out", "zoom_y_in", "zoom_y_out", "pencil", "select", "eraser"];
        for name in icon_names {
            image_resources.preload_image(ctx, path_rel_to_abs(format!("./assets/icons/{}.png", name)).to_str().unwrap(), String::from(name));
        }
        self.image_resources = Some(image_resources);
    }

    fn init_dialogs(&mut self) {
        let mut dialogs_hashmap: HashMap<_, Box<dyn Dialog + 'static>> = HashMap::new();
        dialogs_hashmap.insert("EditorManualDialog", Box::new(EditorManualDialog::default()));
        dialogs_hashmap.insert("StretchDialog", Box::new(EFStretchDialog::new(&self.note_editing, &self.editor_functions, &self.editor_actions)));
        dialogs_hashmap.insert("ChopDialog", Box::new(EFChopDialog::new(&self.note_editing, &self.editor_functions, &self.editor_actions)));
        dialogs_hashmap.insert("EditorInfo", Box::new(EditorInfo::default()));
        dialogs_hashmap.insert("ProjectSettings", Box::new(ProjectSettings::new(&self.project_data)));
        dialogs_hashmap.insert("InsertMetaDialog", Box::new(MetaEventInsertDialog::default()));
        // dialogs_hashmap.insert("PluginDialog", Box::new())

        // settings dialog
        let mut edit_settings_dialog = ESSettingsWindow::default();
        if let Some(midi_devices) = self.midi_devices.as_ref() {
            edit_settings_dialog.use_midi_devices(midi_devices);
        }
        if let Some(playback_manager) = self.playback_manager.as_ref() {
            edit_settings_dialog.use_playback_manager(playback_manager);
        }
        dialogs_hashmap.insert("EditorSettings", Box::new(edit_settings_dialog));

        // lua plugins dialog
        let mut plugin_dialog = PluginDialog::default();
        plugin_dialog.init(&self.editor_actions, &self.note_editing);
        dialogs_hashmap.insert("PluginDialog", Box::new(plugin_dialog));

        self.dialogs = dialogs_hashmap;
    }

    fn import_midi_file(&mut self) {
        {
            let mut project_data = self.project_data.borrow_mut();
            let midi_fd = rfd::FileDialog::new().add_filter("MIDI Files", &["mid", "midi"]);
            if let Some(file) = midi_fd.pick_file() {
                let import_timer = Instant::now();
                let start = import_timer.elapsed().as_secs_f32();
                project_data.import_from_midi_file(String::from(file.to_str().unwrap()));
                let end = import_timer.elapsed().as_secs_f32();
                println!("Imported MIDI in {}s", end - start);
            }
        }

        self.update_global_ppq();
    }

    fn export_midi_file(&mut self) {
        let midi_fd = rfd::FileDialog::new().add_filter("MIDI Files", &["mid"]);
        if let Some(file) = midi_fd.save_file() {
            let export_timer = Instant::now();
            let start = export_timer.elapsed().as_secs_f32();

            let project_data = self.project_data.borrow();

            // let mut midi_writer = MIDIFileWriter::new(project_data.project_info.ppq);
            let notes = project_data.notes.read().unwrap();
            let global_metas = project_data.global_metas.read().unwrap();
            let channel_evs = project_data.channel_events.read().unwrap();

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
            let end = export_timer.elapsed().as_secs_f32();
            println!("Exported MIDI in {}s", end - start);
        }
    }

    fn update_global_ppq(&mut self) {
        let ppq = {
            let project_data = self.project_data.borrow();
            project_data.project_info.ppq
        };

        if let Some(playback_manager) = self.playback_manager.as_mut() {
            let mut playback_manager = playback_manager.lock().unwrap();
            playback_manager.ppq = ppq;

            let mut bar_cacher = self.bar_cacher.lock().unwrap();
            bar_cacher.ppq = ppq;
            bar_cacher.clear_cache();

            let render_manager = self.render_manager.as_ref().unwrap();
            let mut render_manager = render_manager.lock().unwrap();
            render_manager.set_ppq(ppq);
        }

        if let Some(data_view_renderer) = self.data_view_renderer.as_ref() {
            let mut data_view_renderer = data_view_renderer.lock().unwrap();
            data_view_renderer.ppq = ppq;
        }
    }

    fn init_view_settings(&mut self) {
        let mut view_settings = ViewSettings::default();
        view_settings.pr_curr_track.on_change = Some(Box::new(|| {

        }));
        self.view_settings = Some(Arc::new(Mutex::new(view_settings)));
    }

    fn init_navigation(&mut self) {
        self.nav = Some(
            Arc::new(
                Mutex::new(
                    PianoRollNavigation::new()
                )
            )
        );

        self.track_view_nav = Some(
            Arc::new(
                Mutex::new(
                    TrackViewNavigation::new()
                )
            )
        );
    }

    fn init_colors(&mut self) {
        let note_colors = NoteColors::new(self.gl.as_ref().unwrap());
        self.note_colors = Arc::new(Mutex::new(note_colors));
        self.load_colors("./assets/shaders/textures/notes.png");
    }

    fn load_colors(&mut self, path: &'static str) {
        let mut note_colors = self.note_colors.lock().unwrap();
        note_colors.load_from_image(path);
    }

    fn init_render_manager(&mut self) {
        let mut render_manager = RenderManager::default();

        let view_settings = self.view_settings.as_ref().unwrap();
        let gl = self.gl.as_ref().unwrap();

        if let Some(playback_manager) = self.playback_manager.as_ref() {
            render_manager.init_renderers(
                self.project_data.clone(), 
                Some(gl.clone()), 
                self.nav.as_ref().unwrap().clone(), 
                self.track_view_nav.as_ref().unwrap().clone(), 
                view_settings.clone(), 
                playback_manager.clone(), 
                self.bar_cacher.clone(), 
                &self.note_colors, 
            &self.note_culler
            );

            self.data_view_renderer = Some(Arc::new(Mutex::new(unsafe {
                DataViewRenderer::new(
                    &self.project_data,
                    &view_settings,
                    self.nav.as_ref().unwrap(),
                    &gl,
                    &playback_manager,
                    &self.bar_cacher,
                    &self.note_colors,
                    &self.note_culler
                )
            })))
        }

        render_manager.switch_renderer(RenderType::PianoRoll);
        self.render_manager = Some(Arc::new(Mutex::new(render_manager)));
    }

    fn init_gl(&mut self) {
        self.init_navigation();

        // init view settings
        self.init_view_settings();

        // init colors
        self.init_colors();

        // init render manager
        self.init_render_manager();
    }

    fn init_note_editing(&mut self) {
        {
            let project_data = self.project_data.borrow();
            // let notes = &project_data.notes;
            let metas = &project_data.global_metas;

            let nav = self.nav.as_ref().unwrap();
            let editor_tool = &self.editor_tool;
            let render_manager = self.render_manager.as_ref().unwrap();
            // self.note_editing = Arc::new(Mutex::new(NoteEditing::new(notes, nav, editor_tool, render_manager, self.data_view_renderer.as_ref().unwrap(), &self.editor_actions, &self.toolbar_settings)));
            self.note_editing = Arc::new(Mutex::new(NoteEditing::new(&self.project_data, nav, editor_tool, &self.editor_actions, &self.toolbar_settings, render_manager, self.data_view_renderer.as_ref().unwrap())));
            self.meta_editing = Arc::new(Mutex::new(MetaEditing::new(metas, &self.bar_cacher, &self.editor_actions, &project_data.tempo_map)));
            self.track_editing = Arc::new(Mutex::new(TrackEditing::new(&self.project_data, &self.editor_tool, &self.editor_actions, &self.nav.as_ref().unwrap(), self.track_view_nav.as_ref().unwrap(), self.view_settings.as_ref().unwrap())))
        }

        self.init_dialogs();
    }

    fn init_main_menu(&mut self) {
        let image_resources = self.image_resources.as_ref().unwrap();
        let plugins = self.plugin_loader.as_ref().unwrap();

        let mut menu_bar = MainMenuBar::new();
        menu_bar.add_menu_image_action("logo_small", Box::new(|mw| {mw.show_dialog("EditorInfo"); }), image_resources);

        menu_bar.add_menu("File", vec![
            ("New Project".into(), MenuItem::MenuButton(Some(Box::new(|mw| { mw.make_new_project(); })))),
            ("Import MIDI file".into(), MenuItem::MenuButton(Some(Box::new(|mw| { mw.import_midi_file(); })))),
            ("Export MIDI file".into(), MenuItem::MenuButton(Some(Box::new(|mw| { mw.export_midi_file(); }))))
        ]);
        menu_bar.add_menu("Edit", vec![
            ("Undo".into(), MenuItem::MenuButtonEnabled(Some(Box::new(|mw| { mw.undo(); })), Box::new(|mw| { mw.can_undo() }))),
            ("Redo".into(), MenuItem::MenuButtonEnabled(Some(Box::new(|mw| { mw.redo(); })), Box::new(|mw| { mw.can_redo() }))),
            ("".into(), MenuItem::Separator),
            ("Insert...".into(), MenuItem::SubMenu(vec![
                ("Time Signature".into(), MenuItem::MenuButton(Some(Box::new(|mw| { mw.insert_meta(MetaEventType::TimeSignature); })))),
                ("Tempo".into(), MenuItem::MenuButton(Some(Box::new(|mw| { mw.insert_meta(MetaEventType::Tempo); }))))
            ]))
        ]);
        menu_bar.add_menu("Options", vec![
            ("Preferences...".into(), MenuItem::MenuButton(Some(Box::new(|mw| { mw.show_dialog("EditorSettings"); }))))
        ]);
        menu_bar.add_menu("Project", vec![
            ("Project settings...".into(), MenuItem::MenuButton(Some(Box::new(|mw| { mw.show_dialog("ProjectSettings"); }))))
        ]);
        menu_bar.add_menu("Tools", vec![
            ("Editing".into(), MenuItem::SubMenu(vec![
                /*("Flip X (Tick-wise)".into(), MenuItem::MenuButtonEnabled(
                    Some(Box::new(|mw| { mw.apply_function(EditFunction::FlipX(Vec::new())); })),
                    Box::new(|mw| {  
                        let note_editing = mw.note_editing.lock().unwrap();
                        note_editing.is_any_note_selected()
                    })
                )),
                ("Flip Y (Key-wise)".into(), MenuItem::MenuButtonEnabled(
                    Some(Box::new(|mw| { mw.apply_function(EditFunction::FlipY(Vec::new())); })),
                    Box::new(|mw| {  
                        let note_editing = mw.note_editing.lock().unwrap();
                        note_editing.is_any_note_selected()
                    })
                )),
                ("".into(), MenuItem::Separator),
                ("Stretch selection...".into(), MenuItem::MenuButtonEnabled(
                    Some(Box::new(|mw| { mw.apply_function(EditFunction::Stretch(Vec::new(), 0.0)); })),
                    Box::new(|mw| {  
                        let note_editing = mw.note_editing.lock().unwrap();
                        note_editing.is_any_note_selected()
                    })
                )),
                ("Chop selection...".into(), MenuItem::MenuButtonEnabled(
                    Some(Box::new(|mw| { mw.apply_function(EditFunction::Chop(Vec::new(), 0)); })),
                    Box::new(|mw| {  
                        let note_editing = mw.note_editing.lock().unwrap();
                        note_editing.is_any_note_selected()
                    })
                )),
                ("".into(), MenuItem::Separator),*/
                ("Plugins".into(), MenuItem::SubMenu(vec![
                    ("Manipulate...".into(), MenuItem::SubMenu({
                        let mut manip_plugins_buttons = Vec::new();
                        for plugin in plugins.manip_plugins.iter() {
                            let plugin_name = {
                                let plugin = plugin.borrow();
                                plugin.plugin_name.clone()
                            };
                            
                            let plugin = plugin.clone();
                            manip_plugins_buttons.push((plugin_name, MenuItem::MenuButton(Some(Box::new(move |mw| { 
                                mw.run_plugin(plugin.clone());
                            })))));
                        }
                        manip_plugins_buttons
                    })),
                    ("Generate...".into(), MenuItem::SubMenu({
                        let mut gen_plugins_buttons = Vec::new();
                        for plugin in plugins.gen_plugins.iter() {
                            let plugin_name = {
                                let plugin = plugin.borrow();
                                plugin.plugin_name.clone()
                            };
                            
                            let plugin = plugin.clone();
                            gen_plugins_buttons.push((plugin_name, MenuItem::MenuButton(Some(Box::new(move |mw| { 
                                mw.run_plugin(plugin.clone());
                            })))));
                        }
                        gen_plugins_buttons
                    })),
                ]))
            ]))
        ]);
        menu_bar.add_menu("Help", vec![
            ("Manual".into(), MenuItem::MenuButton(Some(Box::new(|mw| { mw.show_dialog("EditorManualDialog"); }))))
        ]);
        self.menu_bar = Some(Arc::new(RwLock::new(menu_bar)));
    }

    fn make_new_project(&mut self) {
        let is_empty = {
            let project_data = self.project_data.borrow();
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
                        let mut project_data = main_window.project_data.borrow_mut();
                        println!("Clearning notes...");
                        project_data.new_empty_project();
                    }

                    {
                        println!("Removing action history...");
                        let mut editor_actions = main_window.editor_actions.borrow_mut();
                        editor_actions.clear_actions();
                    }

                    {
                        let mut playhead = main_window.playhead.borrow_mut();
                        playhead.set_start(0);

                        if let Some(nav) = main_window.nav.as_mut() {
                            let mut nav = nav.lock().unwrap();
                            nav.tick_pos = 0.0;
                        }
                    }
                }));
        }
    }
    
    fn can_undo(&self) -> bool {
        let editor_actions = self.editor_actions.borrow();
        editor_actions.get_can_undo()
    }

    fn undo(&mut self) {
        if !self.can_undo() { return; }

        let mut editor_actions = self.editor_actions.borrow_mut();
        if let Some(action) = editor_actions.undo_action().as_mut() {
            let mut note_editing = self.note_editing.lock().unwrap();
            let mut meta_editing = self.meta_editing.lock().unwrap();
            let mut track_editing = self.track_editing.lock().unwrap();
            note_editing.apply_action(action);
            meta_editing.apply_action(&action);
            track_editing.apply_action(action);
        }
    }

    fn can_redo(&self) -> bool {
        let editor_actions = self.editor_actions.borrow();
        editor_actions.get_can_redo()
    }

    fn redo(&mut self) {
        if !self.can_redo() { return; }

        let mut editor_actions = self.editor_actions.borrow_mut();
        if let Some(action) = editor_actions.redo_action().as_mut() {
            let mut note_editing = self.note_editing.lock().unwrap();
            let mut meta_editing = self.meta_editing.lock().unwrap();
            let mut track_editing = self.track_editing.lock().unwrap();
            note_editing.apply_action(action);
            meta_editing.apply_action(&action);
            track_editing.apply_action(action);
        }
    }

    fn insert_meta(&mut self, meta_type: MetaEventType) {
        match meta_type {
            MetaEventType::TimeSignature | MetaEventType::Tempo => {
                let meta_editing = self.meta_editing.clone();
                let playhead_pos = {
                    let playhead = self.playhead.borrow();
                    playhead.start_tick
                };

                let meta_dialog = self.get_dialog_mut::<MetaEventInsertDialog>("InsertMetaDialog");
                meta_dialog.init_meta_dialog(meta_type, move |data| {
                    let mut meta_editing = meta_editing.lock().unwrap();
                    meta_editing.insert_meta_event(MetaEvent { tick: playhead_pos, event_type: meta_type, data });
                });
                meta_dialog.show();
            },
            _ => {

            }
        }
    }

    pub fn apply_function(&mut self, function_type: EditFunction) {
        /*match function_type {
            EditFunction::FlipX(_) => {
                let note_editing = self.note_editing.lock().unwrap();
                let notes = note_editing.get_notes();
                let sel_notes = note_editing.get_selected_note_ids();

                let mut notes = notes.write().unwrap();
                let mut sel_notes = sel_notes.lock().unwrap();
                let sel_notes_clone = sel_notes.clone();

                if let Some(curr_track) = self.get_current_track() {
                    let mut editor_actions = self.editor_actions.borrow_mut();

                    let mut editor_functions = self.editor_functions.borrow_mut();
                    editor_functions.apply_function(
                        &mut notes[curr_track as usize],
                        &mut sel_notes,
                        EditFunction::FlipX(sel_notes_clone),
                        curr_track,
                        &mut editor_actions
                    );
                }
            },
            EditFunction::FlipY(_) => {
                let note_editing = self.note_editing.lock().unwrap();
                let notes = note_editing.get_notes();
                let sel_notes = note_editing.get_selected_note_ids();

                let mut notes = notes.write().unwrap();
                let mut sel_notes = sel_notes.lock().unwrap();
                let sel_notes_clone = sel_notes.clone();

                if let Some(curr_track) = self.get_current_track() {
                    let mut editor_actions = self.editor_actions.borrow_mut();

                    let mut editor_functions = self.editor_functions.borrow_mut();
                    editor_functions.apply_function(
                        &mut notes[curr_track as usize],
                        &mut sel_notes,
                        EditFunction::FlipY(sel_notes_clone),
                        curr_track,
                        &mut editor_actions
                    );
                }
            },
            EditFunction::Stretch(_, _) => {
                self.show_note_properties_popup = false;
                self.note_properties_mouse_up_processed = false;

                // self.ef_stretch_dialog.show();
                self.show_dialog("StretchDialog");
                // self.tool_dialogs_any_open = true;
            },
            EditFunction::Chop(_, _) => {
                self.show_note_properties_popup = false;
                self.note_properties_mouse_up_processed = false;
                self.show_dialog("ChopDialog");
            }
            // _ => todo!("Implement other functions")
        }*/
    }

    pub fn run_plugin(&mut self, plugin: Rc<RefCell<PluginLua>>) {
        let lua = {
            let p = plugin.borrow();
            p.lua.clone()
        };

        let track_idx = self.get_current_track().unwrap() as usize;
        lua.globals().set("curr_track", track_idx).unwrap();

        let andromeda_obj = AndromedaObj::new(&self.project_data, &self.playhead);
        let andromeda_obj = lua.create_userdata(andromeda_obj).unwrap();
        lua.globals().set("andromeda", andromeda_obj).unwrap();

        let plugin_dialog = self.get_dialog_mut::<PluginDialog>("PluginDialog");
        plugin_dialog.curr_track = track_idx;
        
        match plugin_dialog.load_plugin_dialog(&plugin) {
            Ok(should_show_dialog) => {
                if should_show_dialog { plugin_dialog.show(); }
                else { plugin_dialog.run_plugin(); }
            },
            Err(lua_error) => {
                let plugin = plugin.borrow();
                println!("[PluginError] (While running {}): \n{}", plugin.plugin_name, lua_error);
            }
        }
    }


    pub fn show_dialog(&mut self, name: &'static str) {
        // close any opened dialog
        for dialog in self.dialogs.values_mut() {
            dialog.close();
        }

        if let Some(dialog) = self.dialogs.get_mut(&name) {
            (*dialog).show();
        }
    }

    pub fn get_dialog_mut<D: Dialog>(&mut self, name: &'static str) -> &mut D {
        self.dialogs.get_mut(&name)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<D>().unwrap()
    }

    pub fn is_any_dialog_shown(&self) -> bool {
        for dialog in self.dialogs.values() {
            if dialog.is_showing() { return true; }
        }
        return false;
    }

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

        let render_type = {
            let rt = render_manager.get_render_type();
            let rt = rt.read().unwrap();
            *rt
        };
 
        match render_type {
            RenderType::PianoRoll => {
                if ctrl_down {
                    if alt_down {
                        nav.zoom_ticks_by(zoom_factor);
                    } else {
                        let project_data = self.project_data.borrow();
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
                        nav.zoom_keys_by(zoom_factor);
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
                        track_view_nav.zoom_ticks_by(zoom_factor);
                    } else {
                        let project_data = self.project_data.borrow();
                        let mut new_tick_pos = track_view_nav.tick_pos
                            + 2.0 * move_by * (track_view_nav.zoom_ticks / project_data.project_info.ppq as f32);
                        if new_tick_pos < 0.0 {
                            new_tick_pos = 0.0;
                        }

                        track_view_nav.tick_pos = new_tick_pos;

                        let rend = render_manager.get_active_renderer();
                        track_view_nav.change_tick_pos(new_tick_pos, |time| {
                            rend.lock().unwrap().time_changed(time as u64)
                        });
                    }
                } else {
                    if alt_down {
                        track_view_nav.zoom_tracks_by(zoom_factor);
                    } else {
                        let mut new_track_pos = track_view_nav.track_pos + if move_by > 0.0 { -1.0 } else { 1.0 };
                        if new_track_pos < 0.0 { new_track_pos = 0.0; }
                        track_view_nav.track_pos = new_track_pos;
                    }
                }
            }
        }
    }

    fn handle_mouse_down(&mut self, _ctx: &egui::Context, ui: &mut Ui) {
        if self.nav.is_none() || self.track_view_nav.is_none() || self.render_manager.is_none() { return; }
        if self.mouse_over_ui { return; }

        let curr_view = {
            let render_manager = self.render_manager.as_ref().unwrap();
            let render_manager = render_manager.lock().unwrap();

            let rt = render_manager.get_render_type();
            let rt = rt.read().unwrap();
            *rt
        };

        match curr_view {
            RenderType::PianoRoll => {
                let mut note_editing = self.note_editing.lock().unwrap();
                note_editing.on_mouse_down();
                
                // if changing length of notes, don't play
                // if note_editing.get_flags() & NOTE_EDIT_LENGTH_CHANGE != 0 {
                //     return;
                // }

                if let Some(playback_manager) = self.playback_manager.as_ref() {
                    let nav = self.nav.as_ref().unwrap();
                    let (mouse_midi_pos, _) = get_mouse_midi_pos(ui, nav);

                    let tbs = self.toolbar_settings.borrow();
                    let mut playback = playback_manager.lock().unwrap();
                    playback.start_play_at_mouse(mouse_midi_pos.1, tbs.note_channel.value() as u8 - 1, tbs.note_velocity.value() as u8);
                }
            },
            RenderType::TrackView => {
                let mut track_editing = self.track_editing.lock().unwrap();
                track_editing.on_mouse_down();
            }
        }
    }

    /// Handles events when the right mouse button has been pressed
    fn handle_right_mouse_down(&mut self, _ctx: &egui::Context, _ui: &mut Ui) {
        if self.nav.is_none() || self.track_view_nav.is_none() || self.render_manager.is_none() { return; }
        if self.mouse_over_ui { return; }

        let curr_view = {
            let render_manager = self.render_manager.as_ref().unwrap();
            let render_manager = render_manager.lock().unwrap();

            let rt = render_manager.get_render_type();
            let rt = rt.read().unwrap();
            *rt
        };

        match curr_view {
            RenderType::PianoRoll => {
                let mut note_editing = self.note_editing.lock().unwrap();
                note_editing.on_right_mouse_down();
            },
            RenderType::TrackView => {
                let mut track_editing = self.track_editing.lock().unwrap();
                track_editing.on_right_mouse_down();
            }
        }
    }

    /// Handles double clicks in the editor. This doesn't run if the mouse is over any UI element.
    fn handle_mouse_double_down(&mut self, _ctx: &egui::Context, _ui: &mut Ui) {

    }

    fn handle_mouse_move(&mut self, _ctx: &egui::Context, ui: &mut Ui) {
        if self.nav.is_none() { return; }

        let curr_view = {
            let render_manager = self.render_manager.as_ref().unwrap();
            let render_manager = render_manager.lock().unwrap();

            let rt = render_manager.get_render_type();
            let rt = rt.read().unwrap();
            *rt
        };

        match curr_view {
            RenderType::PianoRoll => {
                if let Some(playback_manager) = self.playback_manager.as_ref() {
                    let nav = self.nav.as_ref().unwrap();
                    let (mouse_midi_pos, _) = get_mouse_midi_pos(ui, nav);

                    let tbs = self.toolbar_settings.borrow();
                    let mut playback = playback_manager.lock().unwrap();
                    playback.update_play_at_mouse(mouse_midi_pos.1, tbs.note_channel.value() as u8 - 1, tbs.note_velocity.value() as u8);
                }

                let mut note_editing = self.note_editing.lock().unwrap();
                note_editing.on_mouse_move();
            },
            RenderType::TrackView => {
                let mut track_editing = self.track_editing.lock().unwrap();
                track_editing.on_mouse_move();
            }
        }
    }

    fn handle_mouse_up(&mut self, _ctx: &egui::Context, ui: &mut Ui) {
        if self.nav.is_none() { return; }

        let curr_view = {
            let render_manager = self.render_manager.as_ref().unwrap();
            let render_manager = render_manager.lock().unwrap();

            let rt = render_manager.get_render_type();
            let rt = rt.read().unwrap();
            *rt
        };

        match curr_view {
            RenderType::PianoRoll => {
                if let Some(playback_manager) = self.playback_manager.as_ref() {
                    let nav = self.nav.as_ref().unwrap();
                    let (mouse_midi_pos, _) = get_mouse_midi_pos(ui, nav);

                    let tbs = self.toolbar_settings.borrow();
                    let mut playback = playback_manager.lock().unwrap();
                    playback.stop_play_at_mouse(mouse_midi_pos.1, tbs.note_channel.value() as u8 - 1);
                }

                let mut note_editing = self.note_editing.lock().unwrap();
                note_editing.on_mouse_up();
            },
            RenderType::TrackView => {
                let mut track_editing = self.track_editing.lock().unwrap();
                track_editing.on_mouse_up();
            }
        }
    }

    fn register_key_downs(&mut self, _ctx: &egui::Context, ui: &mut Ui) {
        let ctrl_down = ui.input(|i| i.modifiers.ctrl);

        // switch renderer 
        if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
            if let Some(renderer) = &self.render_manager {
                let mut renderer = renderer.lock().unwrap();

                let rt = {
                    let render_type = renderer.get_render_type();
                    let rt = render_type.try_read().unwrap();
                    *rt
                };

                match rt {
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

        {
            if ctrl_down {
                let mut editor_actions = self.editor_actions.borrow_mut();
                let mut action = {
                    if ui.input(|i| i.key_pressed(egui::Key::Z)) {
                        let undo_action = {
                            editor_actions.undo_action()
                        };
                        undo_action
                    } else if ui.input(|i| i.key_pressed(egui::Key::Y)) {
                        let redo_action = {
                            editor_actions.redo_action()
                        };
                        redo_action
                    } else { 
                        None
                    }
                };

                if let Some(action) = action.as_mut() {
                    let mut note_editing = self.note_editing.lock().unwrap();
                    let mut meta_editing = self.meta_editing.lock().unwrap();
                    let mut track_editing = self.track_editing.lock().unwrap();

                    note_editing.apply_action(action);
                    meta_editing.apply_action(&action);
                    track_editing.apply_action(action);
                }
            }
        }

        if ui.input(|i| i.key_pressed(egui::Key::Space)) {
            if let Some(playback_manager) = self.playback_manager.as_ref() {
                let mut playback_manager = playback_manager.lock().unwrap();
                playback_manager.toggle_playback();
            }
        }

        // shift updates
        // {
        //     let mut note_editing = self.note_editing.lock().unwrap();
        //     note_editing.set_key_shift_flag(ui.input(|i| i.modifiers.shift));
        // }
    }

    pub fn get_current_track(&self) -> Option<u16> {
        if let Some(nav) = &self.nav {
            let nav = nav.lock().unwrap();
            Some(nav.curr_track)
        } else {
            None
        }
    }

    fn update_cursor(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        let note_editing = self.note_editing.lock().unwrap();
        note_editing.update_cursor(ctx, ui);
    }

    /*fn pan_view_if_mouse_near_edge(&mut self, ctx: &egui::Context, ui: &mut Ui) {
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
    }*/

    fn curr_view_zoom_in_by(&mut self, x_fac: Option<f32>, y_fac: Option<f32>) {
        if x_fac.is_none() && y_fac.is_none() || self.nav.is_none() || self.track_view_nav.is_none() { return; }

        let rt = {
            let rm = self.render_manager.as_ref().unwrap();
            let rm: std::sync::MutexGuard<'_, RenderManager> = rm.lock().unwrap();
            let rt = rm.get_render_type();
            let rt = rt.read().unwrap();
            *rt
        };

        match rt {
            RenderType::PianoRoll => {
                let nav = self.nav.as_mut().unwrap();
                let mut nav = nav.lock().unwrap();
                if let Some(x_fac) = x_fac {
                    nav.zoom_ticks_by(x_fac);
                }
                if let Some(y_fac) = y_fac {
                    nav.zoom_keys_by(y_fac);
                }
            },
            RenderType::TrackView => {
                let nav = self.track_view_nav.as_mut().unwrap();
                let mut nav = nav.lock().unwrap();
                if let Some(x_fac) = x_fac {
                    nav.zoom_ticks_by(x_fac);
                }
                if let Some(y_fac) = y_fac {
                    nav.zoom_tracks_by(y_fac);
                }
            }
        }
    }

    fn get_view_tick_range(&self) -> (MIDITick, MIDITick) {
        let render_manager = self.render_manager.as_ref().unwrap();
        let rt = {
            let rt = render_manager.lock().unwrap().get_render_type();
            let rt = rt.read().unwrap();
            *rt
        };
        match rt {
            RenderType::PianoRoll => {
                if let Some(nav) = self.nav.as_ref() {
                    let nav = nav.lock().unwrap();
                    (nav.tick_pos_smoothed as MIDITick, (nav.tick_pos_smoothed + nav.zoom_ticks_smoothed) as MIDITick)
                } else {
                    (0, 1920)
                }
            },
            RenderType::TrackView => {
                if let Some(nav) = self.track_view_nav.as_ref() {
                    let nav = nav.lock().unwrap();
                    (nav.tick_pos_smoothed as MIDITick, (nav.tick_pos_smoothed + nav.zoom_ticks_smoothed) as MIDITick)
                } else {
                    (0, 1920)
                }
            }
        }
    }

    fn get_playhead_pos(&self, to_window: bool) -> f32 {
        let mut playhead_line_pos = {
            let playhead = self.playhead.borrow();
            playhead.start_tick
        } as f32;

        if let Some(playback_manager) = self.playback_manager.as_ref() {
            let playback_manager = playback_manager.lock().unwrap();
            if playback_manager.playing {
                playhead_line_pos = playback_manager.get_playback_ticks() as f32;
            }
        }

        let tick_pos_smoothed = {
            let rt = {
                let render_manager = self.render_manager.as_ref().unwrap();
                let rt = render_manager.lock().unwrap().get_render_type();
                let rt = rt.read().unwrap();
                *rt
            };

            match rt {
                RenderType::PianoRoll => {
                    let nav = self.nav.as_ref().unwrap();
                    let nav = nav.lock().unwrap();
                    nav.tick_pos_smoothed
                },
                RenderType::TrackView => {
                    let nav = self.track_view_nav.as_ref().unwrap();
                    let nav = nav.lock().unwrap();
                    nav.tick_pos_smoothed
                }
            }
        };

        if to_window { playhead_line_pos - tick_pos_smoothed }
        else { playhead_line_pos }
    }

    /// This will also set the piano roll tick position if user is currently in track view
    fn set_playhead_pos(&mut self, tick: MIDITick) {
        let rt = {
            let render_manager = self.render_manager.as_ref().unwrap();
            let rt = render_manager.lock().unwrap().get_render_type();
            let rt = rt.read().unwrap();
            *rt
        };

        {
            let mut playhead = self.playhead.borrow_mut();
            playhead.set_start(tick);
        }

        if rt == RenderType::TrackView {
            let mut nav = self.nav.as_ref().unwrap().lock().unwrap();
            nav.tick_pos = (tick.saturating_sub(960)) as f32;
        }
    }

    fn handle_input(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        let render_type = {
            let render_manager = self.render_manager.as_ref().unwrap().lock().unwrap();
            *(render_manager.get_render_type().read().unwrap())
        };

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

            self.handle_mouse_down(ctx, ui);

            if is_double_click {
                match render_type {
                    RenderType::PianoRoll => { self.handle_mouse_double_down(ctx, ui) }
                    RenderType::TrackView => {}
                }
            }
        }

        if ui.input(|i| i.pointer.secondary_pressed()) {
            self.handle_right_mouse_down(ctx, ui);
        }

        if ui.input(|i| i.pointer.is_moving()) {
            match render_type {
                RenderType::PianoRoll => { self.handle_mouse_move(ctx, ui) }
                RenderType::TrackView => {}
            }
        }

        if ui.input(|i| i.pointer.primary_released()) {
            match render_type {
                RenderType::PianoRoll => { self.handle_mouse_up(ctx, ui) }
                RenderType::TrackView => {}
            }
        }

        self.register_key_downs(ctx, ui);
    }

    fn handle_cursor_icon(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        let render_type = {
            let render_manager = self.render_manager.as_ref().unwrap().lock().unwrap();
            *(render_manager.get_render_type().read().unwrap())
        };

        match render_type {
            RenderType::PianoRoll => { self.update_cursor(ctx, ui); }
            RenderType::TrackView => {
                ctx.set_cursor_icon(egui::CursorIcon::Default);
            }
        }
    }

    fn update_editing_ui(&mut self, ui: &mut Ui) {
        let mut note_editing = self.note_editing.lock().unwrap();
        let mut track_editing = self.track_editing.lock().unwrap();

        note_editing.update_from_ui(ui);
        // note_editing.set_mouse_over_ui(self.mouse_over_ui);
        note_editing.set_flag(NOTE_EDIT_MOUSE_OVER_UI, self.mouse_over_ui);

        track_editing.update_from_ui(ui);
        track_editing.set_mouse_over_ui(self.mouse_over_ui);
    }

    fn draw_select_box(&mut self, ui: &mut Ui, callback: PaintCallback) {
        ui.painter().add(callback);
        {
            let note_editing = self.note_editing.lock().unwrap();

            if note_editing.get_can_draw_selection_box() {
                let (tl, br) = note_editing.get_selection_range_ui(ui);
                let is_eraser = {
                    let editor_tool = self.editor_tool.borrow();
                    editor_tool.get_tool() == EditorTool::Eraser
                };

                // let is_eraser = note_editing.is_eraser_active();

                let rect = egui::Rect::from_min_max(
                    egui::Pos2 { x: tl.0, y: tl.1 },
                    egui::Pos2 { x: br.0, y: br.1 },
                );

                // Draw selection box with stylish semi-transparent fill and border
                let (fill_color, stroke_color) = if is_eraser {
                    (Color32::from_rgba_unmultiplied(255, 50, 50, 40), Color32::from_rgb(255, 80, 80))
                } else {
                    (Color32::from_rgba_unmultiplied(100, 150, 255, 30), Color32::from_rgb(120, 180, 255))
                };
                
                ui.painter().rect(
                    rect,
                    2.0,
                    fill_color,
                    Stroke {
                        width: 1.5,
                        color: stroke_color,
                    },
                    egui::StrokeKind::Middle,
                );
            }
        }
    }

    fn draw_playhead_line(&mut self, rect: Rect, ui: &mut Ui) {
        let mut playhead_pos = {
            let playhead_pos = self.get_playhead_pos(true);
            let (min_tick, max_tick) = self.get_view_tick_range();
            playhead_pos / (max_tick as f32 - min_tick as f32)
        };

        // playhead line
        if playhead_pos > 0.0 && playhead_pos < 1.0 {
            playhead_pos = playhead_pos * rect.width() + rect.left();
            ui.painter().add(
                Shape::line_segment(
                    [
                        Pos2 { x: playhead_pos, y: rect.min.y },
                        Pos2 { x: playhead_pos, y: rect.max.y }
                    ],
                    Stroke::new(1.0, Color32::WHITE)
                )
            );
        }
    }

    fn update_smoothed_values(&mut self, ctx: &egui::Context) {
        let nav = self.nav.as_ref().unwrap();
        let track_nav = self.track_view_nav.as_ref().unwrap();
        
        let mut nav = nav.lock().unwrap();
        let mut track_nav = track_nav.lock().unwrap();
        if nav.smoothed_values_needs_update() || track_nav.smoothed_values_needs_update() {
            nav.update_smoothed_values();
            track_nav.update_smoothed_values();
            ctx.request_repaint();
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
            self.update_editing_ui(ui);
            self.handle_input(ctx, ui);
            self.handle_cursor_icon(ctx, ui);
        }

        let gl = self.gl.as_ref().unwrap();
        let renderer = self.render_manager.as_ref().unwrap();

        let callback = egui::PaintCallback {
            rect,
            callback: Arc::new(CallbackFn::new({
                
                let gl = Arc::clone(&gl);
                let renderer = Arc::clone(&renderer);

                move |info, _| unsafe {
                    let vp = info.viewport_in_pixels();
                    gl.enable(glow::SCISSOR_TEST);
                    gl.scissor(vp.left_px, vp.from_bottom_px, vp.width_px, vp.height_px);
                    // gl.viewport(vp.left_px, vp.from_bottom_px, vp.width_px, vp.height_px);
                    gl.clear(glow::COLOR_BUFFER_BIT);
                    gl.clear_color(0.0, 0.0, 0.0, 1.0);
                    {
                        let mut render = renderer.lock().unwrap();
                        let mut rnd = render.get_active_renderer().lock().unwrap();
                        (*rnd).window_size(Vec2 { x: vp.width_px as f32, y: vp.height_px as f32 });
                        (*rnd).draw();
                    }
                    gl.disable(glow::SCISSOR_TEST);
                }
            })),
        };

        self.draw_select_box(ui, callback);
        self.draw_playhead_line(rect, ui);
        self.draw_context_menu(ui);

        // update smoothed values
        self.update_smoothed_values(ctx);
    }

    fn draw_data_viewer(&mut self, _ctx: &egui::Context, ui: &mut Ui, _any_window_opened: bool) {
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
                        (*render).window_size(Vec2 { x: vp.width_px as f32, y: vp.height_px as f32 });
                        (*render).draw();
                    }
                    gl.disable(glow::SCISSOR_TEST);
                }
            }))
        };

        ui.painter().add(callback);
    }

    fn draw_meta_event_view(&mut self, ctx: &egui::Context, _ui: &mut Ui) {
        if let Some(view_settings) = self.view_settings.as_ref() {
            let view_settings = view_settings.lock().unwrap();
            if !view_settings.show_meta_events {
                drop(view_settings);
                return;
            }
        }

        egui::SidePanel::left("meta_viewer").width_range(20.0..=250.0)
            .resizable(false)
            .show(ctx, |ui|{ 
                ui.vertical_centered(|ui| {
                    ui.label("Meta Events");
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.allocate_at_least([ui.available_width(), 0.0].into(), egui::Sense::hover());
                        egui::Grid::new("meta_event_grid")
                            .striped(true)
                            .show(ui, |ui| {
                                let meta_editing = self.meta_editing.lock().unwrap();
                                let meta_evs = meta_editing.get_metas();
                                let meta_evs = meta_evs.read().unwrap();
                                
                                for meta in meta_evs.iter() {
                                    /*if highlight {
                                        let rect = egui::Rect::from_min_size(
                                            row_rect.min,
                                            egui::vec2(175.0, 20.0),
                                        );
                                        ui.painter().rect_filled(rect, 0.0, ui.visuals().selection.bg_fill);
                                    }*/

                                    ui.label(meta.tick.to_string());
                                    ui.label(meta.event_type.to_string());
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label(meta.get_value_string()) });
                                    
                                    ui.end_row();
                                }
                            });
                    });
                });
        });
    }

    fn draw_context_menu(&mut self, ui: &mut Ui) {
        let render_type = {
            let render_manager = self.render_manager.as_ref().unwrap().lock().unwrap();
            *(render_manager.get_render_type().read().unwrap())
        };

        match render_type {
            RenderType::TrackView => self.draw_trackview_context_menu(ui),
            RenderType::PianoRoll => {}
        };
    }

    fn draw_pianoroll_context_menu(&mut self, ui: &mut Ui) {

    }

    fn draw_trackview_context_menu(&mut self, ui: &mut Ui) {
        ui.response().context_menu(|ui| {
            let mut should_close = false;
            if ui.button("Insert track above").clicked() {
                let mut track_editing = self.track_editing.lock().unwrap();
                let right_clicked_track = track_editing.get_right_clicked_track();
                track_editing.insert_track(right_clicked_track);
                should_close = true;
            }

            if ui.button("Insert track below").clicked() {
                let mut track_editing = self.track_editing.lock().unwrap();
                let right_clicked_track = track_editing.get_right_clicked_track();
                track_editing.insert_track(right_clicked_track + 1);
                should_close = true;
            }

            ui.separator();

            if ui.button("Move track up").clicked() {
                let mut track_editing = self.track_editing.lock().unwrap();
                let right_clicked_track = track_editing.get_right_clicked_track();
                if right_clicked_track == 0 { /* do nothing */ }
                else { track_editing.swap_tracks(right_clicked_track, right_clicked_track - 1); }
                should_close = true;
            }

            if ui.button("Move track down").clicked() {
                let mut track_editing = self.track_editing.lock().unwrap();
                let right_clicked_track = track_editing.get_right_clicked_track();
                if right_clicked_track == track_editing.get_used_track_count() - 1 {
                    track_editing.insert_track(right_clicked_track);
                } else {
                    track_editing.swap_tracks(right_clicked_track, right_clicked_track + 1);
                }
                should_close = true;
            }

            ui.separator();

            if ui.button("Remove Track").clicked() {
                let mut track_editing = self.track_editing.lock().unwrap();
                track_editing.remove_right_clicked_track();
                should_close = true;
            }

            self.mouse_over_ui |= ui.ui_contains_pointer();
            if should_close {
                ui.close_menu();
            }
        });
    }
}

impl eframe::App for MainWindow {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let any_window_opened = self.is_any_dialog_shown();
        
        // load image resources
        if self.image_resources.is_none() {
            self.load_images(ctx);
        }
        
        // init menu stuff
        if self.menu_bar.is_none() {
            self.init_main_menu();
        }

        // initialize gl if not initialized already
        if self.gl.is_none() {
            if let Some(gl) = frame.gl() {
                self.gl = Some(gl.clone());
                self.init_gl();
                self.init_note_editing();
            }
        }

        let is_on_track_view = {
            if let Some(render_manager) = self.render_manager.as_ref() {
                let render_manager = render_manager.lock().unwrap();
                *render_manager.get_render_type().read().unwrap() == RenderType::TrackView
            } else {
                false
            }
        };

        if let Some(playback_manager) = self.playback_manager.as_ref() {
            let playback_manager = playback_manager.lock().unwrap();
            if playback_manager.playing {
                ctx.request_repaint();
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let menu_bar = {
                let mb = self.menu_bar.as_mut().unwrap();
                mb.clone()
            };

            // Menu Bar at top
            {
                let mut menu_bar = menu_bar.write().unwrap();
                menu_bar.draw_menu(self, ctx);
                self.mouse_over_ui |= ctx.is_pointer_over_area();
            }

            // editor tool stuff
            egui::TopBottomPanel::top("editor_bar_top").show(ctx, |ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                    ui.spacing_mut().button_padding = egui::vec2(1.0, 1.0);

                    {
                        let mut editor_tool = self.editor_tool.borrow_mut();

                        if ui.add({
                            let images = self.image_resources.as_ref().unwrap();
                            egui::ImageButton::new(&*images.get_image_handle(String::from("pencil"))).selected(editor_tool.curr_tool == EditorTool::Pencil)
                        }).clicked() {
                            editor_tool.switch_tool(EditorTool::Pencil);
                            self.is_waiting_for_no_ui_hover = false;
                        }

                        if ui.add({
                            let images = self.image_resources.as_ref().unwrap();
                            egui::ImageButton::new(&*images.get_image_handle(String::from("eraser"))).selected(editor_tool.curr_tool == EditorTool::Eraser)
                        }).clicked() {
                            // let mut editor_tool = self.editor_tool.borrow_mut();
                            editor_tool.switch_tool(EditorTool::Eraser);
                            self.is_waiting_for_no_ui_hover = false;
                        }

                        if ui.add({
                            let images = self.image_resources.as_ref().unwrap();
                            egui::ImageButton::new(&*images.get_image_handle(String::from("select"))).selected(editor_tool.curr_tool == EditorTool::Selector)
                        }).clicked() {
                            // let mut editor_tool = self.editor_tool.borrow_mut();
                            editor_tool.switch_tool(EditorTool::Selector);
                            self.is_waiting_for_no_ui_hover = false;
                        }
                    }

                    ui.separator();
                    ui.menu_button("Note Snap", |ui| {
                        {
                            let mut editor_tool = self.editor_tool.borrow_mut();
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
                    {
                        let mut tbs = self.toolbar_settings.borrow_mut();
                        tbs.note_gate.show("Gate", ui, Some(30.0));
                        tbs.note_velocity.show("Velo", ui, Some(30.0));
                        tbs.note_channel.show("Chan", ui, Some(30.0));
                    }

                    ui.separator();
                    
                    // workaround :/
                    let mut tracks_need_update = false;
                    let mut track_to_change_to = 0;

                    if let Some(vs) = self.view_settings.as_ref() {
                        let mut vs = vs.lock().unwrap();
                        ui.label("View Track");
                        egui::ComboBox::from_id_salt("onion_track")
                            .selected_text(format!("{}", vs.pr_onion_state.to_string()))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut vs.pr_onion_state, VS_PianoRoll_OnionState::NoOnion, "No tracks");
                                ui.selectable_value(&mut vs.pr_onion_state, VS_PianoRoll_OnionState::ViewAll, "All tracks");
                                ui.selectable_value(&mut vs.pr_onion_state, VS_PianoRoll_OnionState::ViewNext, "Next track");
                                ui.selectable_value(&mut vs.pr_onion_state, VS_PianoRoll_OnionState::ViewPrevious, "Previous track");
                                self.mouse_over_ui |= ui.ui_contains_pointer();
                            });
                        ui.label("Onion Color");
                        egui::ComboBox::from_id_salt("onion_coloring")
                            .selected_text(format!("{}", vs.pr_onion_coloring.to_string()))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut vs.pr_onion_coloring, VS_PianoRoll_OnionColoring::GrayedOut, "Grayed Out");
                                ui.selectable_value(&mut vs.pr_onion_coloring, VS_PianoRoll_OnionColoring::PartialColor, "Partial Color");
                                ui.selectable_value(&mut vs.pr_onion_coloring, VS_PianoRoll_OnionColoring::FullColor, "Full Color");
                                self.mouse_over_ui |= ui.ui_contains_pointer();
                            });
                        vs.pr_curr_track.show("Curr. Track", ui, Some(50.0));
                        if vs.pr_curr_track.changed() {
                            println!("Track changed");
                            tracks_need_update = true;
                            track_to_change_to = vs.pr_curr_track.value();
                        }
                        ui.separator();
                    }

                    if tracks_need_update {
                        let mut project_data = self.project_data.borrow_mut();
                        project_data.validate_tracks(track_to_change_to);
                        
                        if let Some(nav) = self.nav.as_ref() {
                            let mut nav = nav.lock().unwrap();
                            nav.curr_track = track_to_change_to;
                        }
                    }

                    {
                        let mut colors = self.note_colors.lock().unwrap();
                        let color_indexing = colors.get_index_type_mut();
                        ui.label("Color notes by");
                        egui::ComboBox::from_id_salt("color_notes_by")
                            .selected_text(format!("{}", color_indexing.to_string()))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(color_indexing, NoteColorIndexing::Track, "Track");
                                ui.selectable_value(color_indexing, NoteColorIndexing::Channel, "Channel");
                                ui.selectable_value(color_indexing, NoteColorIndexing::ChannelTrack, "Track & Channel");
                                self.mouse_over_ui |= ui.ui_contains_pointer();
                            });
                        ui.separator();
                    }

                    // zoom buttons, temporarily text
                    if ui.add({
                        let images = self.image_resources.as_ref().unwrap();
                        egui::ImageButton::new(&*images.get_image_handle(String::from("zoom_x_in"))).frame(false)
                    }).clicked() {
                        self.curr_view_zoom_in_by(Some(1.0 / GLOBAL_ZOOM_FACTOR), None);
                    }

                    if ui.add({
                        let images = self.image_resources.as_ref().unwrap();
                        egui::ImageButton::new(&*images.get_image_handle(String::from("zoom_x_out"))).frame(false)
                    }).clicked() {
                        self.curr_view_zoom_in_by(Some(GLOBAL_ZOOM_FACTOR), None);
                    }

                    ui.separator();

                    if ui.add({
                        let images = self.image_resources.as_ref().unwrap();
                        egui::ImageButton::new(&*images.get_image_handle(String::from("zoom_y_in"))).frame(false)
                    }).clicked() {
                        self.curr_view_zoom_in_by(None, Some(1.0 / GLOBAL_ZOOM_FACTOR));
                    }

                    if ui.add({
                        let images = self.image_resources.as_ref().unwrap();
                        egui::ImageButton::new(&*images.get_image_handle(String::from("zoom_y_out"))).frame(false)
                    }).clicked() {
                        self.curr_view_zoom_in_by(None, Some(GLOBAL_ZOOM_FACTOR));
                    }

                    //int_edit_field(ui, "Gate", &mut tbs.note_gate, 1, u16::MAX.into(), Some(30.0));
                    //int_edit_field(ui, "Velo", &mut tbs.note_velocity, 1, 127, Some(30.0));

                    self.mouse_over_ui |= ui.ui_contains_pointer();
                });
                //ui.separator();
                self.mouse_over_ui |= ui.ui_contains_pointer();
            });

            // Meta event viewer on the left
            self.draw_meta_event_view(ctx, ui);
            self.mouse_over_ui |= ctx.is_pointer_over_area();

            egui::TopBottomPanel::top("Playhead").show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    let (min_tick, max_tick) = self.get_view_tick_range();

                    ui.style_mut().spacing.slider_width = ui.available_width();
                    // let mut playhead_time = self.playhead.start_tick;
                    let mut playhead_time = self.get_playhead_pos(false) as MIDITick;
                    if ui.add(
                        egui::Slider::new(&mut playhead_time, min_tick..=max_tick)
                        .show_value(false)
                        .clamping(egui::SliderClamping::Never)
                    ).changed() {
                        let min_snap_length = {
                            let editor_tool = self.editor_tool.borrow();
                            let snap_ratio = editor_tool.snap_ratio;
                            if snap_ratio.0 == 0 { 1 }
                            else {
                                let ppq = {
                                    let project_data = self.project_data.borrow();
                                    project_data.project_info.ppq as MIDITick
                                };
                                (ppq * 4 * snap_ratio.0 as MIDITick) / snap_ratio.1 as MIDITick
                            }
                        };

                        let playhead_time = playhead_time.rounded_div(min_snap_length) * min_snap_length;
                        self.set_playhead_pos(playhead_time);
                        println!("{}", playhead_time);
                    }
                });
                self.mouse_over_ui |= ui.ui_contains_pointer();
            });

            {
                let (dataview_state, _dataview_size) = if let Some(view_settings) = self.view_settings.as_ref() {
                    let vs = view_settings.lock().unwrap();
                    (vs.pr_dataview_state, vs.pr_dataview_size)
                } else {
                    (VS_PianoRoll_DataViewState::Hidden, 0.25)
                };

                egui::TopBottomPanel::bottom("scroll_navigation").show(ctx, |ui| {
                    ui.style_mut().spacing.slider_width = ui.available_width();

                    if let Some(nav) = self.nav.as_mut() {
                        let latest_note_start = {
                            let notes = self.note_editing.lock().unwrap();
                            notes.latest_note_start
                        };

                        let mut nav = nav.lock().unwrap();
                        let mut tick_pos = nav.tick_pos;
                        let mut tick_end = nav.tick_pos + nav.zoom_ticks;
                        if ui.add(
                            DoubleSlider::new(&mut tick_pos, &mut tick_end, 0.0..=(latest_note_start as f32))
                            .width(ui.available_width())
                            .separation_distance(480.0)
                        ).changed() {

                            {
                                let mut rend = self.render_manager.as_ref().unwrap().lock().unwrap();
                                nav.change_tick_pos(tick_pos, |time| {
                                    rend.get_active_renderer().lock().unwrap().time_changed(time as u64);
                                });
                            }
                            nav.zoom_ticks = tick_end - tick_pos;
                        }
                        // nav.tick_pos = tick_pos;
                        
                        /*if ui.add(egui::Slider::new(&mut nav.tick_pos, 0.0..=(latest_note_start as f32))).changed() {

                        }*/
                    }
                    self.mouse_over_ui |= ui.ui_contains_pointer();
                });

                if dataview_state != VS_PianoRoll_DataViewState::Hidden && !is_on_track_view {
                    egui::TopBottomPanel::bottom("data_viewer").show(ctx, |ui| {
                        egui::ComboBox::from_label("Property")
                            .selected_text(dataview_state.to_string())
                            .show_ui(ui, |ui| {
                                let view_settings = self.view_settings.as_mut().unwrap();
                                let mut view_settings = view_settings.lock().unwrap();

                                ui.selectable_value(&mut view_settings.pr_dataview_state, VS_PianoRoll_DataViewState::NoteVelocities, "Velocity");
                                ui.selectable_value(&mut view_settings.pr_dataview_state, VS_PianoRoll_DataViewState::PitchBend, "Pitch Bend");
                            });

                        // TODO: draw data_viewer
                        self.draw_data_viewer(ctx, ui, any_window_opened);

                        self.mouse_over_ui |= ui.ui_contains_pointer();
                    });
                }
            }

            // piano roll / track view rendering
            egui::CentralPanel::default().show(ctx, |ui| {
                if !(any_window_opened || self.mouse_over_ui) {
                    self.handle_navigation(ctx, ui);
                }

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

        {
            let img_resources = self.image_resources.as_ref().unwrap();
            for dialog in self.dialogs.values_mut() {
                dialog.draw(ctx, img_resources);
            }
        }
    }
}
