// abstraction is NEEDED!!
use crate::{
    LAST_PANIC, app::{
        custom_widgets::{NumberField, NumericField}, rendering::{RenderManager, RenderType, Renderer, data_view::DataViewRenderer, note_cull_helper::NoteCullHelper, track_view::TrackViewRenderer}, shared::{NoteColorIndexing, NoteColors}, ui::{dialog::{Dialog, names::*}, dialog_drawer::DialogDrawer, dialog_manager::DialogManager, dialogs::{crash_dialog::CrashDialog, filter_channels::FilterChannelsDialog}, edtior_info::EditorInfo, main_menu_bar::{MainMenuBar, MenuItem}, manual::EditorManualDialog}, util::image_loader::ImageResources, view_settings::{VS_PianoRoll_DataViewState, VS_PianoRoll_OnionColoring, VS_PianoRoll_OnionState}}, audio::{event_playback::PlaybackManager, kdmapi_engine::kdmapi::KDMAPI, midi_audio_engine::MIDIAudioEngine, midi_devices::MIDIDevices, track_mixer::TrackMixer}, editor::{
            edit_functions::{EFChopDialog, EFGlueDialog}, editing::{SharedClipboard, SharedSelectedNotes, data_editing::{DataEditing, data_edit_flags::{DATA_EDIT_ANY_DIALOG_OPEN, DATA_EDIT_DRAW_EDIT_LINE, DATA_EDIT_MOUSE_OVER_UI}}, note_editing::note_edit_flags::NOTE_EDIT_MOUSE_OVER_UI, track_editing::track_flags::{TRACK_EDIT_ANY_DIALOG_OPEN, TRACK_EDIT_ERASING, TRACK_EDIT_MOUSE_OVER_UI}}, midi_bar_cacher::BarCacher, navigation::{GLOBAL_ZOOM_FACTOR, TrackViewNavigation}, playhead::Playhead, plugins::{PluginLoader, plugin_andromeda_obj::AndromedaObj, plugin_dialog::PluginDialog, plugin_error_dialog::PluginErrorDialog, plugin_lua::PluginLua}, project::{project_data, project_manager::ProjectManager}, settings::{editor_settings::{ESAudioEngineType, ESAudioSettings, ESGeneralSettings, ESSettingsWindow, PR_KEYBOARD_WIDTH, Settings}, project_settings::ProjectSettings}, util::{MIDITick, get_mouse_midi_pos, path_rel_to_abs}}, midi::{events::{meta_event::{MetaEvent, MetaEventType}, note}, midi_file::MIDIEvent}, util::{debugger::Debugger, send_discord_webhook_crash_message, system_stats::SystemStats, timer::Timer}};
use crate::editor::editing::{
    meta_editing::{MetaEditing, MetaEventInsertDialog},
    note_editing::{NoteEditing, note_edit_flags::*},
    track_editing::TrackEditing
};


use as_any::{AsAny, Downcast};
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
        project::project_data::ProjectData
    },
    midi::{
        midi_file::MIDIFileWriter,
    },
};
use eframe::glow;
use std::{
    cell::RefCell, collections::HashMap, fs, panic::{AssertUnwindSafe, catch_unwind}, path::{Path, PathBuf}, rc::Rc, sync::{Arc, LazyLock, Mutex, RwLock}, time::Instant
};

pub static PLUGIN_PATH: LazyLock<PathBuf> = LazyLock::new(|| path_rel_to_abs("./assets/plugins".into()));
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
    project_manager: Arc<RwLock<ProjectManager>>,
    // pub project_data: Rc<RefCell<ProjectData>>,
    bar_cacher: Arc<Mutex<BarCacher>>,
    gl: Option<Arc<glow::Context>>,
    // renderer: Option<Arc<Mutex<dyn Renderer + Send + Sync>>>,
    render_manager: Option<Arc<Mutex<RenderManager>>>,
    data_view_renderer: Option<Arc<Mutex<DataViewRenderer>>>,
    playback_manager: Option<Arc<Mutex<PlaybackManager>>>,
    pub note_editing: Arc<Mutex<NoteEditing>>,
    pub meta_editing: Arc<Mutex<MetaEditing>>,
    pub track_editing: Arc<Mutex<TrackEditing>>,
    pub data_editing: Arc<Mutex<DataEditing>>,
    track_mixer: Rc<RefCell<TrackMixer>>,

    // clipboard
    shared_clipboard: Arc<RwLock<SharedClipboard>>,
    shared_selected_notes: Arc<RwLock<SharedSelectedNotes>>,
    // other
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
    // dialogs: HashMap<&'static str, Box<dyn Dialog>>,
    dialog_manager: Rc<RefCell<DialogManager>>,
    dialog_drawer: DialogDrawer,

    // images
    image_resources: Option<ImageResources>,

    // plugin stuff
    plugin_loader: Option<PluginLoader>,

    // context menu stuff
    context_menu_shown: bool,

    // ==== OTHER ====
    last_playhead_frac: f32,
    last_is_playing: bool,
    sys_stats: SystemStats,
    timer: Timer,
    has_crashed: bool,
    crash_dlg_shown: bool
}

impl MainWindow {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut s = Self::default();

        s.midi_devices = Some(Arc::new(Mutex::new(
            MIDIDevices::new().unwrap()
        )));

        s.kdmapi = Some(Arc::new(Mutex::new({
            let mut kdmapi = KDMAPI::new();
            kdmapi.init();
            kdmapi
        })));

        // initialize settings
        s.settings = vec![
            Box::new(ESGeneralSettings::default()),
            Box::new(ESAudioSettings::default())
        ];

        s
    }

    fn on_gl_init(&mut self, ctx: &egui::Context) {
        self.editor_actions = Rc::new(RefCell::new(EditorActions::new(256)));
        self.editor_functions = Rc::new(RefCell::new(EditFunctions::default()));
        
        self.init_first_project();
        self.init_playback();
        
        self.init_navigation();
        self.init_view_settings();
        self.init_colors();
        // self.init_render_manager();

        self.init_note_editing();
        self.init_render_manager();

        let mut plugin_loader = PluginLoader::new(&PLUGIN_PATH);
        plugin_loader.load_all_plugins().unwrap();
        // plugin_loader.load_plugins(dir)
        // plugin_loader.load_plugins(&path_rel_to_abs("./assets/plugins".into())).unwrap();
        self.plugin_loader = Some(plugin_loader);
        self.load_images(ctx);
        self.init_main_menu();

        self.init_dialogs();

        self.timer.start();
    }

    fn init_first_project(&mut self) {
        let mut project_manager = ProjectManager::new();
        project_manager.new_empty_project();

        self.note_culler = Arc::new(Mutex::new(NoteCullHelper::new(project_manager.get_tracks())));
        let project_manager_arc = Arc::new(RwLock::new(project_manager));

        self.bar_cacher = Arc::new(Mutex::new(BarCacher::new(&project_manager_arc)));
        self.project_manager = project_manager_arc;
    }

    fn init_playback(&mut self) {
        let project_manager = self.project_manager.read().unwrap();
        //let project_data = project_manager.get_project_data();

        let audio_settings = self.settings[1].as_ref().as_any().downcast_ref::<ESAudioSettings>().unwrap();
        let midi_audio_engine = audio_settings.get_engine();

        let playback_manager = PlaybackManager::new(
            // self.kdmapi.as_ref().unwrap().clone()
            match midi_audio_engine {
                &ESAudioEngineType::KDMAPI => {
                    self.kdmapi.as_ref().unwrap().clone()
                },
                &ESAudioEngineType::MidiIO => {
                    self.midi_devices.as_ref().unwrap().clone()
                },
                &ESAudioEngineType::Prerendered => {
                    println!("[WARNING] Prerendered audio is not yet implemented, using KDMAPI instead");
                    self.kdmapi.as_ref().unwrap().clone()
                }
            },
            project_manager.get_tracks(),
            project_manager.get_metas(),
            //project_manager.get_channel_evs(),
            project_manager.get_tempo_map()
        );
        let playback_manager_arc = Arc::new(Mutex::new(playback_manager));
        let playhead = Playhead::new(0, &playback_manager_arc);
        let playhead_rc = Rc::new(RefCell::new(playhead));

        let track_mixer = TrackMixer::new(project_manager.get_tracks());

        self.playback_manager = Some(playback_manager_arc);
        self.playhead = playhead_rc;
        self.track_mixer = Rc::new(RefCell::new(track_mixer));
    }

    fn load_images(&mut self, ctx: &egui::Context) {
        let mut image_resources = ImageResources::new();
        let icon_names = [
            "logo",
            "logo_medium",
            "logo_small",
            "zoom_x_in",
            "zoom_x_out",
            "zoom_y_in",
            "zoom_y_out",
            "pencil",
            "select",
            "eraser",
            "copy",
            "cut",
            "paste",
            "undo",
            "redo"
        ];

        for name in icon_names {
            image_resources.preload_image(ctx, path_rel_to_abs(format!("./assets/icons/{}.png", name)).to_str().unwrap(), String::from(name));
        }
        self.image_resources = Some(image_resources);
    }

    fn init_dialogs(&mut self) {
        self.dialog_drawer.init(&self.dialog_manager);
        let mut dialog_manager = self.dialog_manager.borrow_mut();
        
        dialog_manager.register_dialog(DIALOG_NAME_EDITOR_MANUAL, Box::new(|| { Box::new(EditorManualDialog::default()) }));
        
        // ewww spaghetti code... i have no other way of doing this though
        {
            let note_editing = self.note_editing.clone();
            let editor_functions = self.editor_functions.clone();
            let editor_actions = self.editor_actions.clone();
            dialog_manager.register_dialog(DIALOG_NAME_EF_STRETCH, Box::new(move || {
                Box::new(EFStretchDialog::new(&note_editing, &editor_functions, &editor_actions))
            }));
        }

        {
            let note_editing = self.note_editing.clone();
            let editor_functions = self.editor_functions.clone();
            let editor_actions = self.editor_actions.clone();
            dialog_manager.register_dialog(DIALOG_NAME_EF_CHOP, Box::new(move || { Box::new(EFChopDialog::new(&note_editing, &editor_functions, &editor_actions)) }));
        }

        {
            let note_editing = self.note_editing.clone();
            let editor_functions = self.editor_functions.clone();
            let editor_actions = self.editor_actions.clone();
            dialog_manager.register_dialog(DIALOG_NAME_EF_GLUE, Box::new(move || { Box::new(EFGlueDialog::new(&note_editing, &editor_functions, &editor_actions)) }));
        }

        dialog_manager.register_dialog(DIALOG_NAME_EDITOR_INFO, Box::new(|| { Box::new(EditorInfo::default()) }));

        {
            let project_manager = self.project_manager.clone();
            dialog_manager.register_dialog(DIALOG_NAME_PROJECT_SETTINGS, Box::new(move || { Box::new(ProjectSettings::new(&project_manager)) }));
        }

        dialog_manager.register_dialog(DIALOG_NAME_INSERT_META, Box::new(|| { Box::new(MetaEventInsertDialog::default()) }));
        dialog_manager.register_dialog(DIALOG_NAME_PLUGIN_ERROR_DIALOG, Box::new(|| { Box::new(PluginErrorDialog::new()) }));
        
        // settings dialog
        {
            let midi_devices = self.midi_devices.as_ref().unwrap().clone();
            let kdmapi = self.kdmapi.as_ref().unwrap().clone();
            let playback_manager = self.playback_manager.as_ref().unwrap().clone();

            dialog_manager.register_dialog(DIALOG_NAME_EDITOR_SETTINGS, Box::new(move || { 
                let mut edit_settings_dialog = ESSettingsWindow::default();
                edit_settings_dialog.use_midi_devices(&midi_devices);
                edit_settings_dialog.use_kdmapi(&kdmapi);
                edit_settings_dialog.use_playback_manager(&playback_manager);
                Box::new(edit_settings_dialog)
            }));
        }

        {
            let note_editing = self.note_editing.clone();
            let editor_actions = self.editor_actions.clone();

            dialog_manager.register_dialog(DIALOG_NAME_PLUGIN_DIALOG, Box::new(move || {
                let mut plugin_dialog = PluginDialog::default();
                plugin_dialog.init(&editor_actions, &note_editing);
                Box::new(plugin_dialog)
            }));
        }

        dialog_manager.register_dialog(DIALOG_NAME_FILTER_CHANNELS, Box::new(move || {
            Box::new(FilterChannelsDialog::default())
        }));

        dialog_manager.register_dialog(DIALOG_NAME_CRASH, Box::new(move || {
            Box::new(CrashDialog::default())
        }))
    }

    fn import_midi_file(&mut self) {
        {
            // let mut project_data = self.project_data.try_borrow_mut().unwrap();
            let midi_fd = rfd::FileDialog::new().add_filter("MIDI Files", &["mid", "midi"]);
            if let Some(file) = midi_fd.pick_file() {
                let import_timer = Instant::now();
                
                let start = import_timer.elapsed().as_secs_f32();
                let mut project_manager = self.project_manager.write().unwrap();
                project_manager.import_from_midi_file(String::from(file.to_str().unwrap()));
                let end = import_timer.elapsed().as_secs_f32();

                println!("Imported MIDI in {}s", end - start);
                
                let ppq = project_manager.get_ppq();
                self.update_global_ppq(ppq);
            }
        }

        self.on_midi_loaded();
    }

    fn export_midi_file(&mut self) {
        let midi_fd = rfd::FileDialog::new().add_filter("MIDI Files", &["mid"]);
        if let Some(file) = midi_fd.save_file() {
            let export_timer = Instant::now();
            let start = export_timer.elapsed().as_secs_f32();

            let project_manager = self.project_manager.read().unwrap();
            let ppq = project_manager.get_ppq();

            // let notes = project_manager.get_notes().read().unwrap();
            let global_metas = project_manager.get_metas().read().unwrap();
            // let channel_evs = project_manager.get_channel_evs().read().unwrap();
            let tracks = project_manager.get_tracks().read().unwrap();

            // build tracks in parallel
            let per_track_chunks: Vec<Vec<MIDIEvent>> = tracks.par_iter()
                .map(|track| {
                    let (notes, ch_evs) = (track.get_notes(), track.get_channel_evs());
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

    fn on_midi_loaded(&mut self) {
        let mut playback_manager = self.playback_manager.as_mut().unwrap().lock().unwrap();
        
        if playback_manager.playing {
            playback_manager.toggle_playback();
            playback_manager.reset_events();
        }

        let mut editor_actions = self.editor_actions.borrow_mut();
        editor_actions.clear_actions();
    }

    fn update_global_ppq(&self, ppq: u16) {
        if let Some(playback_manager) = self.playback_manager.as_ref() {
            let mut playback_manager = playback_manager.lock().unwrap();
            playback_manager.ppq = ppq;

            let mut bar_cacher = self.bar_cacher.lock().unwrap();
            bar_cacher.clear_cache();

            let render_manager = self.render_manager.as_ref().unwrap();
            let mut render_manager = render_manager.lock().unwrap();
            render_manager.set_ppq(ppq);
        }

        if let Some(data_view_renderer) = self.data_view_renderer.as_ref() {
            let mut data_view_renderer = data_view_renderer.lock().unwrap();
            data_view_renderer.ppq = ppq;
        }

        {
            let mut note_editing = self.note_editing.lock().unwrap();
            let mut meta_editing = self.meta_editing.lock().unwrap();
            let mut track_editing = self.track_editing.lock().unwrap();

            note_editing.ppq = ppq;
            meta_editing.ppq = ppq;
            track_editing.ppq = ppq;
        }
    }

    fn get_ppq(&self) -> u16 {
        let project_manager = self.project_manager.read().unwrap();
        project_manager.get_ppq()
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
            let note_editing = &self.note_editing;
            let track_editing = &self.track_editing;

            render_manager.init_renderers(
                self.project_manager.clone(), 
                Some(gl.clone()), 
                self.nav.as_ref().unwrap().clone(), 
                self.track_view_nav.as_ref().unwrap().clone(), 
                view_settings.clone(), 
                playback_manager.clone(), 
                self.bar_cacher.clone(), 
                &self.note_colors, 
            &self.note_culler,
                &self.shared_selected_notes,

                note_editing,
                track_editing
            );

            self.data_view_renderer = Some(Arc::new(Mutex::new(unsafe {
                DataViewRenderer::new(
                    &self.project_manager,
                    &view_settings,
                    self.nav.as_ref().unwrap(),
                    &gl,
                    &playback_manager,
                    &self.bar_cacher,
                    &self.note_colors,
                    &self.note_culler,
                    &self.shared_selected_notes
                )
            })))
        }

        render_manager.switch_renderer(RenderType::PianoRoll);

        self.render_manager = Some(Arc::new(Mutex::new(render_manager)));
    }

    fn init_note_editing(&mut self) {
        {
            // let project_data = self.project_data.try_borrow().unwrap();
            let project_manager = self.project_manager.read().unwrap();

            // let notes = project_manager.get_notes();
            let tracks = project_manager.get_tracks();
            let metas = project_manager.get_metas();
            // let ch_evs = project_manager.get_channel_evs();
            let tempo_map = project_manager.get_tempo_map();

            let nav = self.nav.as_ref().unwrap();
            let editor_tool = &self.editor_tool;
            // self.note_editing = Arc::new(Mutex::new(NoteEditing::new(notes, nav, editor_tool, render_manager, self.data_view_renderer.as_ref().unwrap(), &self.editor_actions, &self.toolbar_settings)));
            let note_editing = NoteEditing::new(
                tracks,
                nav,
                editor_tool,
                &self.editor_actions,
                &self.toolbar_settings,
                &self.shared_clipboard,
                &self.shared_selected_notes
            );

            let track_editing = TrackEditing::new(
                &self.project_manager,
                &self.editor_tool,
                &self.editor_actions,
                &self.nav.as_ref().unwrap(),
                self.track_view_nav.as_ref().unwrap(),
                self.view_settings.as_ref().unwrap(),
                &self.shared_clipboard,
                &self.shared_selected_notes,
                &self.playhead
            );

            self.note_editing = Arc::new(Mutex::new(note_editing));
            self.meta_editing = Arc::new(Mutex::new(MetaEditing::new(metas, &self.bar_cacher, &self.editor_actions, tempo_map)));
            self.track_editing = Arc::new(Mutex::new(track_editing));
            self.data_editing = Arc::new(Mutex::new(DataEditing::new(tracks, self.view_settings.as_ref().unwrap(), &self.editor_tool, &self.editor_actions, self.nav.as_ref().unwrap())));
        }
    }

    fn init_main_menu(&mut self) {
        let image_resources = self.image_resources.as_ref().unwrap();
        let plugins = self.plugin_loader.as_ref().unwrap();

        let mut menu_bar = MainMenuBar::new();
        menu_bar.add_menu_image_action("logo_small", Box::new(|mw| {mw.show_dialog(DIALOG_NAME_EDITOR_INFO); }), image_resources);

        menu_bar.add_menu("File", vec![
            ("New Project".into(), MenuItem::MenuButton(Some(Box::new(|mw| { mw.make_new_project(); })))),
            ("Save Project (WIP)".into(), MenuItem::MenuButton(Some(Box::new(|mw| { mw.save_project(); })))),
            ("".into(), MenuItem::Separator),
            ("Import MIDI file".into(), MenuItem::MenuButton(Some(Box::new(|mw| { mw.import_midi_file(); })))),
            ("Export MIDI file".into(), MenuItem::MenuButton(Some(Box::new(|mw| { mw.export_midi_file(); })))),
        ]);
        menu_bar.add_menu("Edit", vec![
            ("Undo".into(), MenuItem::MenuButtonEnabled(Some(Box::new(|mw| { mw.undo(); })), Box::new(|mw| { mw.can_undo() }))),
            ("Redo".into(), MenuItem::MenuButtonEnabled(Some(Box::new(|mw| { mw.redo(); })), Box::new(|mw| { mw.can_redo() }))),
            ("".into(), MenuItem::Separator),
            ("Insert...".into(), MenuItem::SubMenu(vec![
                ("Time Signature".into(), MenuItem::MenuButton(Some(Box::new(|mw| { mw.insert_meta(MetaEventType::TimeSignature); })))),
                ("Tempo".into(), MenuItem::MenuButton(Some(Box::new(|mw| { mw.insert_meta(MetaEventType::Tempo); }))))
            ])),
            ("".into(), MenuItem::Separator),
            ("Copy".into(), MenuItem::MenuButtonEnabled(Some(Box::new(|mw| { mw.request_editing_copy(); })), Box::new(|mw| { mw.can_copy() }))),
            ("Cut".into(), MenuItem::MenuButtonEnabled(Some(Box::new(|mw| { mw.request_editing_cut(); })), Box::new(|mw| { mw.can_copy() }))),
            ("Paste".into(), MenuItem::MenuButtonEnabled(Some(Box::new(|mw| { mw.request_editing_paste(); })), Box::new(|mw| { mw.can_paste() }))),
            ("".into(), MenuItem::Separator),
            ("Select...".into(), MenuItem::SubMenu(vec![
                ("Filter Selection...".into(), MenuItem::SubMenu(vec![
                    ("Filter channnels".into(), MenuItem::MenuButtonEnabled(
                        Some(Box::new(|mw| {
                            mw.filter_selection_channels()
                        })), Box::new(|mw| {
                            let sel = mw.shared_selected_notes.read().unwrap();
                            sel.is_any_note_selected()
                        }))
                    )
                ]))
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
                ("Stretch selection...".into(), MenuItem::MenuButtonEnabled(
                    Some(Box::new(|mw| { mw.apply_function(EditFunction::Stretch(Vec::new(), 0.0)); })),
                    Box::new(|mw| {  
                        let shared_selected = mw.shared_selected_notes.read().unwrap();
                        shared_selected.is_any_note_selected()
                    })
                )),
                ("Chop selection...".into(), MenuItem::MenuButtonEnabled(
                    Some(Box::new(|mw| { mw.apply_function(EditFunction::Chop(Vec::new(), 0)); })),
                    Box::new(|mw| {  
                        let shared_selected = mw.shared_selected_notes.read().unwrap();
                        shared_selected.is_any_note_selected()
                    })
                )),
                ("Slice notes at playhead".into(), MenuItem::MenuButtonEnabled(
                    Some(Box::new(|mw| { 
                        let playhead_pos = {
                            let playhead = mw.playhead.borrow();
                            playhead.start_tick
                        };
                        mw.apply_function(EditFunction::SliceAtTick(Vec::new(), playhead_pos));
                    })),
                    Box::new(|mw| {
                        let shared_selected = mw.shared_selected_notes.read().unwrap();
                        shared_selected.is_any_note_selected()
                    })
                )),
                ("Glue notes...".into(), MenuItem::MenuButtonEnabled(
                    Some(Box::new(|mw| { mw.apply_function(EditFunction::Glue(Vec::new(), 0, false)); })),
                    Box::new(|mw| {  
                        let shared_selected = mw.shared_selected_notes.read().unwrap();
                        shared_selected.is_any_note_selected()
                    })
                )),
                ("Remove Overlaps".into(),  MenuItem::MenuButtonEnabled(
                    Some(Box::new(|mw| { mw.apply_function(EditFunction::RemoveOverlaps) })),
                    Box::new(|mw| {  
                        let shared_selected = mw.shared_selected_notes.read().unwrap();
                        shared_selected.is_any_note_selected()
                    })
                )),
                ("".into(), MenuItem::Separator),
                ("Fade In".into(), MenuItem::MenuButtonEnabled(
                    Some(Box::new(|mw| {
                        mw.apply_function(EditFunction::FadeNotes(false))
                    })),
                    Box::new(|mw| {
                        let shared_selected = mw.shared_selected_notes.read().unwrap();
                        shared_selected.is_any_note_selected()
                    })
                )),
                ("Fade Out".into(), MenuItem::MenuButtonEnabled(
                    Some(Box::new(|mw| {
                        mw.apply_function(EditFunction::FadeNotes(true))
                    })),
                    Box::new(|mw| {
                        let shared_selected = mw.shared_selected_notes.read().unwrap();
                        shared_selected.is_any_note_selected()
                    })
                )),
                ("".into(), MenuItem::Separator),
                ("Plugins".into(), MenuItem::SubMenu(vec![
                    ("Manipulate...".into(), MenuItem::SubMenu({
                        let mut manip_plugins_buttons = Vec::new();
                        for plugin in plugins.manip_plugins.iter() {
                            let plugin_name = {
                                let plugin = plugin.try_borrow().unwrap();
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
                                let plugin = plugin.try_borrow().unwrap();
                                plugin.plugin_name.clone()
                            };
                            
                            let plugin = plugin.clone();
                            gen_plugins_buttons.push((plugin_name, MenuItem::MenuButton(Some(Box::new(move |mw| { 
                                mw.run_plugin(plugin.clone());
                            })))));
                        }
                        gen_plugins_buttons
                    })),
                    ("".into(), MenuItem::Separator),
                    ("Reload all plugins".into(),
                        MenuItem::MenuButtonWithTooltop("Only reloads the plugins andromeda has loaded at startup (plugins that were added after startup are not added, therefore a restart is required for newly added plugins).".into(),
                            Some(Box::new(move |mw| {
                                let plugin_loader = mw.plugin_loader.as_mut().unwrap();
                                plugin_loader.reload_plugins();
                            }))
                        )
                    ),
                    ("Open plugin folder".into(),
                        MenuItem::MenuButton(
                            Some(Box::new(move |_| {
                                let path = "./assets/plugins/custom/";

                                let p = Path::new(&path);
                                if !p.exists() {
                                    if let Err(err) = fs::create_dir_all(p) {
                                        panic!("Failed to create directory {}: {}", path, err);
                                    }
                                }

                                if let Err(err) = opener::open(p) {
                                    panic!("Directory exists bt failed to open: {}", err);
                                }
                            }))
                        )
                    )
                ]))
            ]))
        ]);
        menu_bar.add_menu("Help", vec![
            ("Manual".into(), MenuItem::MenuButton(Some(Box::new(|mw| { mw.show_dialog(DIALOG_NAME_EDITOR_MANUAL); }))))
        ]);
        self.menu_bar = Some(Arc::new(RwLock::new(menu_bar)));
    }

    fn make_new_project(&mut self) {
        let is_empty = {
            let project_manager = self.project_manager.read().unwrap();
            project_manager.is_project_empty(false)
        };

        if !is_empty {
            self.show_override_popup = true;
            self.override_popup_msg =
                "Are you sure you want to start a new project?";
            self.override_popup_func =
                Some(Box::new(|main_window, _: &egui::Context| {
                    {
                        let mut project_manager = main_window.project_manager.write().unwrap();
                        println!("Clearning notes...");
                        project_manager.new_empty_project();
                    }

                    {
                        println!("Removing action history...");
                        let mut editor_actions = main_window.editor_actions.try_borrow_mut().unwrap();
                        editor_actions.clear_actions();
                    }

                    {
                        let mut playhead = main_window.playhead.try_borrow_mut().unwrap();
                        playhead.set_start(0);

                        if let Some(nav) = main_window.nav.as_mut() {
                            let mut nav = nav.lock().unwrap();
                            nav.tick_pos = 0.0;
                        }
                    }
                }));
        }
    }

    fn save_project(&mut self) {
        let mut project_manager = self.project_manager.write().unwrap();
        project_manager.save_project();
    }
    
    fn can_undo(&self) -> bool {
        let editor_actions = self.editor_actions.try_borrow().unwrap();
        editor_actions.get_can_undo()
    }

    fn undo(&self) {
        if !self.can_undo() { return; }

        let mut editor_actions = self.editor_actions.try_borrow_mut().unwrap();
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
        let editor_actions = self.editor_actions.try_borrow().unwrap();
        editor_actions.get_can_redo()
    }

    fn redo(&self) {
        if !self.can_redo() { return; }

        let mut editor_actions = self.editor_actions.try_borrow_mut().unwrap();
        if let Some(action) = editor_actions.redo_action().as_mut() {
            let mut note_editing = self.note_editing.lock().unwrap();
            let mut meta_editing = self.meta_editing.lock().unwrap();
            let mut track_editing = self.track_editing.lock().unwrap();
            note_editing.apply_action(action);
            meta_editing.apply_action(&action);
            track_editing.apply_action(action);
        }
    }

    fn can_copy(&self) -> bool {
        let shared_selected = self.shared_selected_notes.read().unwrap();
        shared_selected.is_any_note_selected()
    }

    fn can_paste(&self) -> bool {
        let shared_clipboard = self.shared_clipboard.read().unwrap();
        !shared_clipboard.is_clipboard_empty()        
    }

    fn request_editing_copy(&mut self) {
        let curr_track = self.get_current_track().unwrap();
        let mut note_editing = self.note_editing.lock().unwrap();
        note_editing.copy_notes(curr_track);
    }

    fn request_editing_cut(&mut self) {
        let curr_track = self.get_current_track().unwrap();
        let mut note_editing = self.note_editing.lock().unwrap();
        note_editing.cut_selected_notes(curr_track);
    }

    fn request_editing_paste(&mut self) {
        let curr_track = self.get_current_track().unwrap();
        let mut note_editing = self.note_editing.lock().unwrap();
        note_editing.paste_notes(curr_track);
    }

    fn filter_selection_channels(&mut self) {
        let mut dialog_manager = self.dialog_manager.borrow_mut();
        let shared_selected_notes = self.shared_selected_notes.clone();
        let note_editing = self.note_editing.clone();
        
        dialog_manager.open_dialog_by_name(DIALOG_NAME_FILTER_CHANNELS, vec![
            Box::new(shared_selected_notes),
            Box::new(note_editing)
        ]);
    }

    fn insert_meta(&mut self, meta_type: MetaEventType) {
        match meta_type {
            MetaEventType::TimeSignature | MetaEventType::Tempo => {
                let meta_editing = self.meta_editing.clone();
                let playhead_pos = {
                    let playhead = self.playhead.try_borrow().unwrap();
                    playhead.start_tick
                };

                // let meta_dialog = self.get_dialog_mut::<MetaEventInsertDialog>("InsertMetaDialog");
                let mut meta_dialog = MetaEventInsertDialog::default();
                meta_dialog.init_meta_dialog(meta_type, move |data| {
                    let mut meta_editing = meta_editing.try_lock().unwrap();
                    meta_editing.insert_meta_event(MetaEvent { tick: playhead_pos, event_type: meta_type, data });
                });
                // meta_dialog.show();
                let mut dialog_manager = self.dialog_manager.borrow_mut();
                dialog_manager.open_dialog(Box::new(meta_dialog), Vec::new());
            },
            _ => {

            }
        }
    }

    pub fn apply_function(&mut self, function_type: EditFunction) {
        match function_type {
            EditFunction::Stretch(_, _) => {
                self.show_note_properties_popup = false;
                self.note_properties_mouse_up_processed = false;

                // self.ef_stretch_dialog.show();
                self.show_dialog(DIALOG_NAME_EF_STRETCH);
                // self.tool_dialogs_any_open = true;
            },
            EditFunction::Chop(_, _) => {
                self.show_note_properties_popup = false;
                self.note_properties_mouse_up_processed = false;
                self.show_dialog(DIALOG_NAME_EF_CHOP);
            },
            EditFunction::Glue(_, _, _) => {
                self.show_note_properties_popup = false;
                self.note_properties_mouse_up_processed = false;
                self.show_dialog(DIALOG_NAME_EF_GLUE);
            }
            EditFunction::SliceAtTick(_, playhead_tick) => {
                //let project_manager = self.project_manager.read().unwrap();
                let note_editing = self.note_editing.lock().unwrap();
                let tracks = note_editing.get_tracks();
                let mut tracks = tracks.write().unwrap();
                // let mut notes = note_editing.get().write().unwrap();

                let curr_track = self.get_current_track().unwrap();
                let notes = tracks[curr_track as usize].get_notes_mut();

                // let mut sel_notes = note_editing.get_selected_note_ids().lock().unwrap();
                // let sel_notes_clone = sel_notes.clone();
                let mut sel_notes = note_editing.get_shared_selected_ids().write().unwrap();
                let sel_notes = sel_notes.get_selected_ids_mut(curr_track);
                let sel_notes_clone = sel_notes.clone();

                let mut editor_functions = self.editor_functions.borrow_mut();
                let mut editor_actions = self.editor_actions.borrow_mut();
                editor_functions.apply_function(
                    notes, 
                    sel_notes,
                    EditFunction::SliceAtTick(sel_notes_clone, playhead_tick),
                    curr_track,
                    &mut editor_actions
                );
            },
            EditFunction::FadeNotes(fade_out) => {
                let note_editing = self.note_editing.lock().unwrap();
                let tracks = note_editing.get_tracks();
                let mut tracks = tracks.write().unwrap();

                let curr_track = self.get_current_track().unwrap();
                let notes = tracks[curr_track as usize].get_notes_mut();

                let mut sel_notes = note_editing.get_shared_selected_ids().write().unwrap();
                let sel_notes = sel_notes.get_selected_ids_mut(curr_track);

                let mut editor_functions = self.editor_functions.borrow_mut();
                let mut editor_actions = self.editor_actions.borrow_mut();
                editor_functions.apply_function(
                    notes, 
                    sel_notes, 
                    EditFunction::FadeNotes(fade_out), 
                    curr_track, 
                    &mut editor_actions
                );
            },
            EditFunction::RemoveOverlaps => {
                let note_editing = self.note_editing.lock().unwrap();
                let tracks = note_editing.get_tracks();
                let mut tracks = tracks.write().unwrap();

                let curr_track = self.get_current_track().unwrap();
                let notes = tracks[curr_track as usize].get_notes_mut();

                let mut sel_notes = note_editing.get_shared_selected_ids().write().unwrap();
                let sel_notes = sel_notes.get_selected_ids_mut(curr_track);

                let mut editor_functions = self.editor_functions.borrow_mut();
                let mut editor_actions = self.editor_actions.borrow_mut();
                editor_functions.apply_function(
                    notes, 
                    sel_notes, 
                    EditFunction::RemoveOverlaps, 
                    curr_track, 
                    &mut editor_actions
                );
            }
            _ => {}
        }
    }

    pub fn run_plugin(&mut self, plugin: Rc<RefCell<PluginLua>>) {
        let lua = {
            let p = plugin.try_borrow().unwrap();
            p.lua.clone()
        };

        let track_idx = self.get_current_track().unwrap() as usize;
        lua.globals().set("curr_track", track_idx).unwrap();

        let andromeda_obj = AndromedaObj::new(&self.project_manager, &self.playhead);
        let andromeda_obj = lua.create_userdata(andromeda_obj).unwrap();
        lua.globals().set("andromeda", andromeda_obj).unwrap();

    
        let run_result = {
            // let plugin_dialog = self.get_dialog_mut::<PluginDialog>("PluginDialog");
            let mut plugin_dialog = PluginDialog::default();
            plugin_dialog.init(&self.editor_actions, &self.note_editing);
            plugin_dialog.curr_track = track_idx;

            match plugin_dialog.load_plugin_dialog(&plugin) {
                Ok(should_show_dialog) => {
                    if should_show_dialog {
                        let mut dialog_manager = self.dialog_manager.borrow_mut();
                        dialog_manager.open_dialog(Box::new(plugin_dialog), Vec::new());
                    }
                    else { plugin_dialog.run_plugin(); }
                    Ok(())
                },
                Err(lua_error) => {
                    Err(lua_error)
                }
            }
        };

        if let Err(lua_error) = run_result {
            println!("error");
            // err_dialog.init_dialog(&plugin, lua_error.to_string());
            let mut dialog_manager = self.dialog_manager.borrow_mut();
            let plugin_name = plugin.borrow().plugin_name.clone();
            dialog_manager.open_dialog(Box::new(PluginErrorDialog::new()), vec![
                Box::new(plugin_name),
                Box::new(lua_error.to_string())
            ]);
        }

        /*if let Err(lua_error) = run_result {
            println!("error");
            let err_dialog: &mut PluginErrorDialog = self.get_dialog_mut("PluginErrorDialog");
            err_dialog.init_dialog(&plugin, lua_error.to_string());
            self.show_dialog("PluginErrorDialog");
        }*/
    }

    fn reload_plugins(&mut self) {
        let plugins = self.plugin_loader.as_ref().unwrap();
        for plugin in plugins.gen_plugins.iter() {

        }
    }

    pub fn show_dialog(&mut self, name: &'static str) {
        let mut dialog_manager = self.dialog_manager.borrow_mut();
        dialog_manager.close_all_dialogs();
        dialog_manager.open_dialog_by_name(name, Vec::new());
    }

    /*pub fn get_dialog_mut<D: Dialog>(&mut self, name: &'static str) -> &mut D {
        self.dialogs.get_mut(&name)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<D>().unwrap()
    }*/

    pub fn is_any_dialog_shown(&self) -> bool {
        let dialog_manager = self.dialog_manager.borrow();
        dialog_manager.is_any_dialog_shown()
    }

    fn handle_main_inputs(&mut self, ctx: &egui::Context, ui: &mut Ui, mouse_over_ui: bool, any_window_opened: bool) {
        self.handle_key_inputs(ui);

        match {
            let render_manager = self.render_manager.as_ref().unwrap().lock().unwrap();
            *render_manager.get_render_type()
        } {
            RenderType::PianoRoll => {
                if !(mouse_over_ui || any_window_opened) { self.handle_pianoroll_navigation(ui); }
                self.handle_pianoroll_inputs(ctx, ui, mouse_over_ui, any_window_opened);
            }
            RenderType::TrackView => {
                if !(mouse_over_ui || any_window_opened) { self.handle_trackview_navigation(ui); }
                self.handle_trackview_inputs(ctx, ui, mouse_over_ui, any_window_opened);
            }
        }
    }

    fn handle_pianoroll_navigation(&mut self, ui: &mut Ui) {
        let scroll_delta = ui.input(|i| i.raw_scroll_delta).y;
        if scroll_delta.abs() <= 0.001 {
            return;
        }

        let (alt_down, _shift_down, ctrl_down) = ui.input(|i| (i.modifiers.alt, i.modifiers.shift, i.modifiers.ctrl));

        let nav = self.nav.as_mut().unwrap();
        let mut nav = nav.lock().unwrap();

        // scroll up/down (no modifiers applied)
        let move_by = scroll_delta;

        // alt_down = zoom
        // shift_down = horizontal movements
        let zoom_factor = 1.01f32.powf(scroll_delta);

        let mut render_manager = self.render_manager.as_mut().unwrap().lock().unwrap();
        if ctrl_down {
            if alt_down {
                nav.zoom_ticks_by(zoom_factor);
            } else {
                let ppq = {
                    let project_manager = self.project_manager.read().unwrap();
                    project_manager.get_ppq()
                };

                let mut new_tick_pos = nav.tick_pos
                    + 2.0 * move_by * (nav.zoom_ticks / ppq as f32);
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
    }

    fn handle_trackview_navigation(&mut self, ui: &mut Ui) {
        let scroll_delta = ui.input(|i| i.raw_scroll_delta).y;
        if scroll_delta.abs() <= 0.001 {
            return;
        }

        let (alt_down, _shift_down, ctrl_down) =
            ui.input(|i| (i.modifiers.alt, i.modifiers.shift, i.modifiers.ctrl));

        let track_view_nav = self.track_view_nav.as_mut().unwrap();
        let mut track_view_nav = track_view_nav.lock().unwrap();

        // scroll up/down (no modifiers applied)
        let move_by = scroll_delta;

        // alt_down = zoom
        // shift_down = horizontal movements
        let zoom_factor = 1.01f32.powf(scroll_delta);

        let mut render_manager = self.render_manager.as_mut().unwrap().lock().unwrap();
 
        if ctrl_down {
            if alt_down {
                track_view_nav.zoom_ticks_by(zoom_factor);
            } else {
                let ppq = {
                    let project_manager = self.project_manager.read().unwrap();
                    project_manager.get_ppq()
                };

                let mut new_tick_pos = track_view_nav.tick_pos
                    + 2.0 * move_by * (track_view_nav.zoom_ticks / ppq as f32);
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

    fn handle_key_inputs(&mut self, ui: &mut Ui) {
        let mut render_manager = self.render_manager.as_ref().unwrap().lock().unwrap();
        let render_type = *render_manager.get_render_type();

        if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
            ui.input_mut(|i| {
                i.consume_key(egui::Modifiers::default(), egui::Key::Tab);
            });

            match render_type {
                RenderType::PianoRoll => render_manager.switch_renderer(RenderType::TrackView),
                RenderType::TrackView => render_manager.switch_renderer(RenderType::PianoRoll),
            }
        }

        match render_type {
            RenderType::PianoRoll => {
                let mut note_editing = self.note_editing.lock().unwrap();
                note_editing.on_key_down(ui);
            },
            RenderType::TrackView => {
                let mut track_editing = self.track_editing.lock().unwrap();
                track_editing.on_key_down(ui);
            }
        }

        if ui.input(|i| i.key_pressed(egui::Key::Z) && i.modifiers.command) {
            self.undo();
        }

        if ui.input(|i| i.key_pressed(egui::Key::Y) && i.modifiers.command) {
            self.redo();
        }

        if ui.input(|i| i.key_pressed(egui::Key::Space)) {
            let playback_manager = self.playback_manager.as_ref().unwrap();
            let mut playback_manager = playback_manager.lock().unwrap();
            playback_manager.toggle_playback();
        }
    }

    fn handle_data_viewer_inputs(&mut self, ctx: &egui::Context, ui: &mut Ui, mouse_over_ui: bool, any_window_opened: bool) {
        let mut data_editing = self.data_editing.lock().unwrap();
        data_editing.set_flag(DATA_EDIT_MOUSE_OVER_UI, mouse_over_ui);
        data_editing.set_flag(DATA_EDIT_ANY_DIALOG_OPEN, any_window_opened);
        data_editing.update(ui);

        if ui.input(|i| i.pointer.primary_pressed()) {
            data_editing.on_mouse_down();
        }

        if ui.input(|i| i.pointer.primary_down()) {
            data_editing.on_mouse_move();
        }

        if ui.input(|i| i.pointer.primary_released()) {
            data_editing.on_mouse_up();
        }
    }

    fn handle_pianoroll_inputs(&mut self, ctx: &egui::Context, ui: &mut Ui, mouse_over_ui: bool, any_window_opened: bool) {
        let mut should_pan = false;

        let mut note_editing = self.note_editing.lock().unwrap();
        note_editing.set_flag(NOTE_EDIT_MOUSE_OVER_UI, mouse_over_ui);
        note_editing.set_flag(NOTE_EDIT_ANY_DIALOG_OPEN, any_window_opened);
        note_editing.update(ui);

        if ui.input(|i| i.pointer.primary_pressed()) {
            note_editing.on_mouse_down();
           
            if note_editing.get_flag(NOTE_EDIT_SYNTH_PLAY) {
                let nav = self.nav.as_ref().unwrap();
                let (mouse_midi_pos, _) = get_mouse_midi_pos(ui, nav);
                let tbs = self.toolbar_settings.try_borrow().unwrap();

                let playback_manager = self.playback_manager.as_ref().unwrap();
                let mut playback_manager = playback_manager.lock().unwrap();
                playback_manager.start_play_at_mouse(mouse_midi_pos.1, tbs.note_channel.value() as u8 - 1, tbs.note_velocity.value() as u8);
            }
        }

        if ui.input(|i| i.pointer.secondary_pressed()) {
            note_editing.on_right_mouse_down();
        }

        if ui.input(|i| i.pointer.primary_down()) {
            note_editing.on_mouse_move();

            if note_editing.get_flag(NOTE_EDIT_SYNTH_PLAY) {
                let nav = self.nav.as_ref().unwrap();
                let (mouse_midi_pos, _) = get_mouse_midi_pos(ui, nav);
                let tbs = self.toolbar_settings.try_borrow().unwrap();

                let playback_manager = self.playback_manager.as_ref().unwrap();
                let mut playback_manager = playback_manager.lock().unwrap();
                playback_manager.update_play_at_mouse(mouse_midi_pos.1, tbs.note_channel.value() as u8 - 1, tbs.note_velocity.value() as u8);
            }

            should_pan = !ui.ui_contains_pointer();
        }

        if ui.input(|i| i.pointer.primary_released()) {
            if note_editing.get_flag(NOTE_EDIT_SYNTH_PLAY) {
                let nav = self.nav.as_ref().unwrap();
                let (mouse_midi_pos, _) = get_mouse_midi_pos(ui, nav);
                let tbs = self.toolbar_settings.try_borrow().unwrap();

                let playback_manager = self.playback_manager.as_ref().unwrap();
                let mut playback_manager = playback_manager.lock().unwrap();
                playback_manager.stop_play_at_mouse(mouse_midi_pos.1, tbs.note_channel.value() as u8 - 1);
            }

            note_editing.on_mouse_up();
        }

        drop(note_editing);

        // if should_pan { self.pan_view_if_mouse_near_edge(ctx, ui); }
    }

    fn handle_trackview_inputs(&mut self, ctx: &egui::Context, ui: &mut Ui, mouse_over_ui: bool, any_window_opened: bool) {
        let mut track_editing = self.track_editing.lock().unwrap();
        track_editing.set_flag(TRACK_EDIT_MOUSE_OVER_UI, mouse_over_ui);
        track_editing.set_flag(TRACK_EDIT_ANY_DIALOG_OPEN, any_window_opened);
        track_editing.update(ui);

        if ui.input(|i| i.pointer.primary_pressed()) {
            track_editing.on_mouse_down();
        }

        if ui.input(|i| i.pointer.secondary_pressed()) {
            track_editing.on_right_mouse_down();
        }

        if ui.input(|i| i.pointer.primary_down()) {
            track_editing.on_mouse_move();
        }

        if ui.input(|i| i.pointer.primary_released()) {
            track_editing.on_mouse_up();
        }
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

    fn pan_view_if_mouse_near_edge(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        let rect = ui.min_rect();
        if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
            if let Some(nav) = &self.nav {
                let mut nav = nav.lock().unwrap();

                let pan_bottom = rect.bottom() - 200.0 - mouse_pos.y < 0.0;
                let pan_top = mouse_pos.y - 200.0 < 0.0;

                if pan_bottom {
                    let pan_fac = -(rect.bottom() - 200.0 - mouse_pos.y) / 200.0 * 2.0;
                    nav.key_pos -= 0.25 * pan_fac;
                    if nav.key_pos < 0.0 {
                        nav.key_pos = 0.0;
                    }
                    ctx.request_repaint();
                }

                if pan_top {
                    let pan_fac = -(mouse_pos.y - 200.0) / 200.0 * 2.0;
                    nav.key_pos += 0.25 * pan_fac;
                    if nav.key_pos > 128.0 - nav.zoom_keys {
                        nav.key_pos = 128.0 - nav.zoom_keys;
                    }
                    ctx.request_repaint();
                }
            }
        }
    }

    fn curr_view_zoom_in_by(&mut self, x_fac: Option<f32>, y_fac: Option<f32>) {
        if x_fac.is_none() && y_fac.is_none() || self.nav.is_none() || self.track_view_nav.is_none() { return; }

        let rt = {
            let rm = self.render_manager.as_ref().unwrap();
            let rm: std::sync::MutexGuard<'_, RenderManager> = rm.lock().unwrap();
            let rt = rm.get_render_type();
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
        let rt = {
            let render_manager = self.render_manager.as_ref().unwrap();
            let rt = render_manager.lock().unwrap();
            let rt = rt.get_render_type();
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
            let playhead = self.playhead.try_borrow().unwrap();
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
                let rt = render_manager.lock().unwrap();
                let rt = rt.get_render_type();
                //let rt = rt.read().unwrap();
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
            let rt = render_manager.lock().unwrap();
            let rt = rt.get_render_type();
            *rt
        };

        {
            let mut playhead = self.playhead.try_borrow_mut().unwrap();
            playhead.set_start(tick);
        }

        if rt == RenderType::TrackView {
            let mut nav = self.nav.as_ref().unwrap().lock().unwrap();
            nav.tick_pos = (tick.saturating_sub(960)) as f32;
        }
    }

    fn handle_cursor_icon(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        let render_type = {
            let render_manager = self.render_manager.as_ref().unwrap();
            let rt = render_manager.lock().unwrap();
            let rt = rt.get_render_type();
            *rt
        };

        match render_type {
            RenderType::PianoRoll => { self.update_cursor(ctx, ui); }
            RenderType::TrackView => {
                ctx.set_cursor_icon(egui::CursorIcon::Default);
            }
        }
    }

    fn update_smoothed_values(&mut self, ctx: &egui::Context) {
        let nav = self.nav.as_ref().unwrap();
        let track_nav = self.track_view_nav.as_ref().unwrap();
        
        let mut nav = nav.lock().unwrap();
        let mut track_nav = track_nav.lock().unwrap();
        if nav.smoothed_values_needs_update() || track_nav.smoothed_values_needs_update() {
            let dt = self.timer.get_delta_time();
            nav.update_smoothed_values(dt);
            track_nav.update_smoothed_values(dt);
            ctx.request_repaint();
        }
    }

    fn draw(&mut self, ctx: &egui::Context, ui: &mut Ui, mouse_over_ui: bool, any_window_opened: bool) {
        // skip all this if gl or renderer isnt ready yet
        if self.gl.is_none() || self.render_manager.is_none() || self.nav.is_none() {
            return;
        }

        let available_size = ui.available_size();
        let (rect, _response) = ui.allocate_exact_size(available_size, egui::Sense::hover());
    
        self.handle_main_inputs(ctx, ui, mouse_over_ui, any_window_opened);
        self.handle_cursor_icon(ctx, ui);
        /*if !any_window_opened && ui.ui_contains_pointer() {
            self.update_editing_ui(ui);
            self.handle_input(ctx, ui);
            self.handle_cursor_icon(ctx, ui);
        }*/

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

    fn draw_select_box(&mut self, ui: &mut Ui, callback: PaintCallback) {
        let render_type = {
            let render_manager = self.render_manager.as_ref().unwrap();
            let rt = render_manager.lock().unwrap();
            let rt = rt.get_render_type();
            *rt
        };

        ui.painter().add(callback);

        let (draw, is_eraser, has_selected_in_trackview, (tl, br)) = match render_type {
            RenderType::PianoRoll => {
                let note_editing = self.note_editing.lock().unwrap();

                if note_editing.get_can_draw_selection_box() {
                    let (tl, br) = note_editing.get_selection_range_ui(ui);
                    
                    let is_eraser = {
                        let editor_tool = self.editor_tool.try_borrow().unwrap();
                        editor_tool.get_tool() == EditorTool::Eraser
                    };
                    (true, is_eraser, false, (tl, br))
                } else {
                    (false, false, false, ((0.,0.),(0.,0.)))
                }
            },
            RenderType::TrackView => {
                let track_editing = self.track_editing.lock().unwrap();

                let (tl, br) = track_editing.get_selection_range_ui(ui);
                (track_editing.get_can_draw_selection_box() || track_editing.has_selection, track_editing.get_flag(TRACK_EDIT_ERASING), track_editing.has_selection, (tl, br))
            }
        };

        if !draw { return; }

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
        
        if has_selected_in_trackview {
            ui.painter().rect_stroke(
                rect,
                0.0,
                Stroke {
                    width: 1.0, color: Color32::WHITE
                },
                egui::StrokeKind::Middle
            );
        } else {
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

    fn draw_playhead_line(&mut self, rect: Rect, ui: &mut Ui) {
        
        let mut playhead_pos = {
            let autoscroll = {
                let view_settings = self.view_settings.as_ref().unwrap().lock().unwrap();
                view_settings.pr_autoscroll
            };

            let playhead_pos = {
                if autoscroll {
                    let playhead = self.playhead.borrow();
                    
                    playhead.start_tick as f32 - if self.is_on_track_view() {
                        let nav = self.track_view_nav.as_ref().unwrap().lock().unwrap();
                        nav.tick_pos_smoothed
                    } else {
                        let nav = self.nav.as_ref().unwrap().lock().unwrap();
                        nav.tick_pos_smoothed
                    }

                } else {
                    self.get_playhead_pos(true)
                }
            };

            let (min_tick, max_tick) = self.get_view_tick_range();
            let is_playing = self.is_playing();

            let mut playhead_pos = playhead_pos / (max_tick as f32 - min_tick as f32);

            if is_playing && autoscroll {
                if is_playing != self.last_is_playing {
                    self.last_playhead_frac = playhead_pos;
                } else {
                    playhead_pos = self.last_playhead_frac;
                }
            }

            self.last_is_playing = self.is_playing();
            playhead_pos
        };

        // playhead line
        if playhead_pos > 0.0 && playhead_pos < 1.0 {
            let keyboard_width = PR_KEYBOARD_WIDTH / rect.width();
            if !self.is_on_track_view() { playhead_pos = playhead_pos * (1.0 - keyboard_width) + keyboard_width; }
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

    fn is_playing(&self) -> bool {
        let playback_manager = self.playback_manager.as_ref().unwrap().lock().unwrap();
        playback_manager.playing
    }

    fn is_on_track_view(&self) -> bool {
        let render_manager = self.render_manager.as_ref().unwrap().lock().unwrap();
        render_manager.get_render_type() == &RenderType::TrackView
    }

    fn allocate_for_keyboard(&self, ui: &mut Ui) {
        if self.is_on_track_view() { return; }
        ui.allocate_exact_size([PR_KEYBOARD_WIDTH, 1.0].into(), egui::Sense::hover());
    }

    fn draw_ui(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {

        let any_window_opened = self.is_any_dialog_shown();

        // initialize gl if not initialized already
        if self.gl.is_none() {
            if let Some(gl) = frame.gl() {
                self.gl = Some(gl.clone());
                self.on_gl_init(ctx);
            }
        }

        if let Some(playback_manager) = self.playback_manager.as_ref() {
            let playback_manager = playback_manager.lock().unwrap();
            if playback_manager.playing {
                ctx.request_repaint();
            }
        };

        // i have no idea where to put this statement lol
        {
            let mut project_manager = self.project_manager.write().unwrap();
            if project_manager.ppq_changed {
                let ppq = project_manager.get_ppq();
                self.update_global_ppq(ppq);
                project_manager.ppq_changed = false;
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

            self.draw_process_stats(ctx, ui);

            // editor tool stuff
            self.draw_editor_tools(ctx);
            self.draw_playback_buttons(ctx);

            // draw side bar buttons
            egui::SidePanel::right("editor_side_controls")
                .exact_width(40.0)
                .resizable(false)
                .show(ctx, |ui| {
                    self.draw_sidebar(ctx, ui);
                    self.mouse_over_ui |= ui.ui_contains_pointer();
                });

            // Meta event viewer on the left
            self.draw_meta_event_view(ctx, ui);
            self.mouse_over_ui |= ctx.is_pointer_over_area();

            self.draw_playhead_ui(ctx);

            {
                let (dataview_state, _dataview_size) = if let Some(view_settings) = self.view_settings.as_ref() {
                    let vs = view_settings.lock().unwrap();
                    (vs.pr_dataview_state, vs.pr_dataview_size)
                } else {
                    (VS_PianoRoll_DataViewState::Hidden, 200.0)
                };

                self.draw_scroll_navigation(ctx, ui);

                if dataview_state != VS_PianoRoll_DataViewState::Hidden && !self.is_on_track_view() {
                    egui::TopBottomPanel::bottom("data_viewer")
                        // .resizable(true)
                        // .default_height(_dataview_size)
                        .show(ctx, |ui| {
                            // if let Some(view_settings) = self.view_settings.as_mut() {
                            //     let mut vs = view_settings.lock().unwrap();
                            //     vs.pr_dataview_size = ui.min_rect().height();
                            // };
                            
                            ui.horizontal(|ui| {
                                self.allocate_for_keyboard(ui);

                                ui.vertical(|ui| {
                                    let mut mouse_over_ui = false;

                                    ui.horizontal(|ui| {
                                        ui.label("Property");
                                        egui::ComboBox::from_label("")
                                            .selected_text(dataview_state.to_string())
                                            .show_ui(ui, |ui| {
                                                let view_settings = self.view_settings.as_mut().unwrap();
                                                let mut view_settings = view_settings.lock().unwrap();

                                                ui.selectable_value(&mut view_settings.pr_dataview_state, VS_PianoRoll_DataViewState::NoteVelocities, "Velocity");
                                                ui.selectable_value(&mut view_settings.pr_dataview_state, VS_PianoRoll_DataViewState::PitchBend, "Pitch Bend");
                                            });
                                    });

                                    mouse_over_ui |= ui.ui_contains_pointer();
                                    self.draw_data_viewer(ctx, ui, mouse_over_ui, any_window_opened);
                                });

                                self.mouse_over_ui |= ui.ui_contains_pointer();
                            });

                            self.mouse_over_ui |= ui.ui_contains_pointer();
                    });
                }
            }

            // piano roll / track view rendering
            egui::CentralPanel::default().show(ctx, |ui| {
                self.draw(ctx, ui, self.mouse_over_ui, any_window_opened);
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
            self.dialog_drawer.draw_all_dialogs(ctx, img_resources);
        }
    }

    fn draw_editor_tools(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("editor_bar_top").show(ctx, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                ui.spacing_mut().button_padding = egui::vec2(1.0, 1.0);

                {
                    let mut editor_tool = self.editor_tool.try_borrow_mut().unwrap();

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
                        // let mut editor_tool = self.editor_tool.try_borrow_mut().unwrap();
                        editor_tool.switch_tool(EditorTool::Eraser);
                        self.is_waiting_for_no_ui_hover = false;
                    }

                    if ui.add({
                        let images = self.image_resources.as_ref().unwrap();
                        egui::ImageButton::new(&*images.get_image_handle(String::from("select"))).selected(editor_tool.curr_tool == EditorTool::Selector)
                    }).clicked() {
                        // let mut editor_tool = self.editor_tool.try_borrow_mut().unwrap();
                        editor_tool.switch_tool(EditorTool::Selector);
                        self.is_waiting_for_no_ui_hover = false;
                    }
                }

                ui.separator();
                ui.menu_button("Note Snap", |ui| {
                    {
                        let mut editor_tool = self.editor_tool.try_borrow_mut().unwrap();
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
                    let mut tbs = self.toolbar_settings.try_borrow_mut().unwrap();
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
                    {
                        let mut project_manager = self.project_manager.write().unwrap();
                        project_manager.get_project_data_mut().validate_tracks(track_to_change_to);
                    }
                    // let mut project_data = self.project_data.try_borrow_mut().unwrap();
                    // project_data.validate_tracks(track_to_change_to);
                    
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

                self.mouse_over_ui |= ui.ui_contains_pointer();
            });
            //ui.separator();
            self.mouse_over_ui |= ui.ui_contains_pointer();
        });
    }

    fn draw_playback_buttons(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("playback_buttons").show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            ui.spacing_mut().button_padding = egui::vec2(1.0, 1.0);

            ui.horizontal(|ui| {
                {
                    let ppq = self.get_ppq() as MIDITick;

                    if ui.button("<-").clicked() {
                        let mut playback_manager = self.playback_manager.as_mut().unwrap().lock().unwrap();
                        let ticks = playback_manager.get_playback_ticks().saturating_sub(ppq);
                        // lmao this has to be the worst way to navigate
                        let last_play_state = playback_manager.playing;
                        if last_play_state { playback_manager.toggle_playback(); }
                        playback_manager.navigate_to(ticks);
                        if last_play_state { playback_manager.toggle_playback(); }
                    }

                    if ui.button(if self.is_playing() { "||" } else { "|>" }).clicked() {
                        let mut playback_manager = self.playback_manager.as_mut().unwrap().lock().unwrap();
                        playback_manager.toggle_playback();
                    }
                    
                    if ui.button("->").clicked() {
                        let mut playback_manager = self.playback_manager.as_mut().unwrap().lock().unwrap();
                        let ticks = playback_manager.get_playback_ticks().saturating_add(ppq);
                        let last_play_state = playback_manager.playing;
                        if last_play_state { playback_manager.toggle_playback(); }
                        playback_manager.navigate_to(ticks);
                        if last_play_state { playback_manager.toggle_playback(); }
                    }
                }
                ui.separator();
                ui.label("Autoscroll");

                let mut view_settings = self.view_settings.as_mut().unwrap().lock().unwrap();
                ui.checkbox(&mut view_settings.pr_autoscroll, "");
            });

            self.mouse_over_ui |= ui.ui_contains_pointer();
        });
    }

    fn draw_playhead_ui(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("Playhead").show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                let (min_tick, max_tick) = self.get_view_tick_range();
                self.allocate_for_keyboard(ui);
                ui.style_mut().spacing.slider_width = ui.available_width();

                let mut playhead_time = {
                    let view_settings = self.view_settings.as_ref().unwrap().lock().unwrap();
                    if view_settings.pr_autoscroll {
                        let playhead = self.playhead.borrow();
                        playhead.start_tick
                    } else {
                        self.get_playhead_pos(false) as MIDITick
                    }
                };

                if ui.add(
                    egui::Slider::new(&mut playhead_time, min_tick..=max_tick)
                    .show_value(false)
                    .clamping(egui::SliderClamping::Never)
                ).changed() {
                    let min_snap_length = {
                        let editor_tool = self.editor_tool.try_borrow().unwrap();
                        let snap_ratio = editor_tool.snap_ratio;
                        if snap_ratio.0 == 0 { 1 }
                        else {
                            let ppq = {
                                let project_manager = self.project_manager.read().unwrap();
                                project_manager.get_ppq() as MIDITick
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
    }

    fn draw_sidebar(&mut self, _ctx: &egui::Context, ui: &mut Ui) {
        
        let images = self.image_resources.as_ref().unwrap();
        
        {
            let mut note_editing = self.note_editing.lock().unwrap();
            let mut track_editing = self.track_editing.lock().unwrap();

            let track = note_editing.get_current_track();
            
            let has_selected = {
                let shared_sel = self.shared_selected_notes.read().unwrap();
                shared_sel.is_any_note_selected()
            };

            if ui.add_enabled(has_selected, {
                egui::ImageButton::new(&*images.get_image_handle(String::from("copy")))
            }).clicked() {
                let is_track_view = self.is_on_track_view();
                if is_track_view {
                    (*track_editing).copy_notes();
                } else {
                    (*note_editing).copy_notes(track);
                }
            }

            if ui.add_enabled(has_selected, {
                // let images = self.image_resources.as_ref().unwrap();
                egui::ImageButton::new(&*images.get_image_handle(String::from("cut")))
            }).clicked() {
                let is_track_view = self.is_on_track_view();
                if is_track_view {
                    (*track_editing).cut_notes();
                } else {
                    (*note_editing).cut_selected_notes(track);
                }
                
            }

            if ui.add_enabled(self.can_paste(), {
                // let images = self.image_resources.as_ref().unwrap();
                egui::ImageButton::new(&*images.get_image_handle(String::from("paste")))
            }).clicked() {
                let is_track_view = self.is_on_track_view();
                if is_track_view {
                    (*track_editing).paste_notes(track);
                } else {
                    (*note_editing).paste_notes(track);
                }
            }
        }

        ui.separator();

        {
            if ui.add_enabled(self.can_undo(), {
                egui::ImageButton::new(&*images.get_image_handle(String::from("undo")))
            }).clicked() {  
                self.undo();
            }

            if ui.add_enabled(self.can_redo(), {
                egui::ImageButton::new(&*images.get_image_handle(String::from("redo")))
            }).clicked() {
                self.redo();
            }
        }
    }

    fn draw_scroll_navigation(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        let mut is_track = false;
        egui::TopBottomPanel::bottom("scroll_navigation").show(ctx, |ui| {
            ui.style_mut().spacing.slider_width = ui.available_width();

            let latest_note_start = {
                let notes = self.note_editing.lock().unwrap();
                notes.latest_note_start
            };

            let rt = {
                let render_manager = self.render_manager.as_ref().unwrap();
                let rt = render_manager.lock().unwrap();
                let rt = rt.get_render_type();
                //let rt = rt.read().unwrap();
                *rt
            };

            let (mut tick_pos, mut tick_end) = match rt {
                RenderType::PianoRoll => {
                    let nav = self.nav.as_ref().unwrap();
                    let nav = nav.lock().unwrap();

                    let tick_pos = nav.tick_pos;
                    let tick_end = nav.tick_pos + nav.zoom_ticks;

                    is_track = false;
                    (tick_pos, tick_end)
                },
                RenderType::TrackView => {
                    let nav = self.track_view_nav.as_ref().unwrap();
                    let nav = nav.lock().unwrap();

                    let tick_pos = nav.tick_pos;
                    let tick_end = nav.tick_pos + nav.zoom_ticks;

                    is_track = true;
                    (tick_pos, tick_end)
                }
            };

            if ui.add(
                DoubleSlider::new(&mut tick_pos, &mut tick_end, 0.0..=(latest_note_start as f32 + if is_track { 960.0 * 32.0 } else { 0.0 }))
                .width(ui.available_width())
                .separation_distance(480.0)
            ).changed() {
                let mut rend = self.render_manager.as_ref().unwrap().lock().unwrap();
                match rt {
                    RenderType::PianoRoll => {
                        let mut nav = self.nav.as_mut().unwrap().lock().unwrap();
                        nav.change_tick_pos(tick_pos, |time| {
                            rend.get_active_renderer().lock().unwrap().time_changed(time as u64);
                        });

                        nav.zoom_ticks = tick_end - tick_pos;
                    },
                    RenderType::TrackView => {
                        let mut nav = self.track_view_nav.as_mut().unwrap().lock().unwrap();
                        nav.change_tick_pos(tick_pos, |time| {
                            rend.get_active_renderer().lock().unwrap().time_changed(time as u64);
                        });

                        nav.zoom_ticks = tick_end - tick_pos;
                    }
                }
            }

            self.mouse_over_ui |= ui.ui_contains_pointer();
        });

        if is_track {
            egui::SidePanel::right("scroll_nav_vertical").default_width(10.0).resizable(false).show(ctx, |ui| {
                ui.style_mut().spacing.slider_width = ui.available_height();

                let nav = self.track_view_nav.as_mut().unwrap();

                let mut nav = nav.lock().unwrap();
                let mut track_pos = nav.track_pos;
                // let mut track_end = nav.track_pos + nav.zoom_tracks;
                
                if ui.add(
                egui::Slider::new(&mut track_pos, ({
                        let track_editing = self.track_editing.lock().unwrap();
                        track_editing.get_used_track_count() + 10
                    } as f32)..=0.0).vertical().show_value(false)
                ).changed() {
                    nav.track_pos = track_pos;
                }

                self.mouse_over_ui |= ui.ui_contains_pointer();
            });
        }
    }

    fn draw_data_viewer(&mut self, ctx: &egui::Context, ui: &mut Ui, mouse_over_ui: bool, any_window_opened: bool) {
        let available_width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2 { x: available_width, y: 200.0 }, egui::Sense::hover());

        if self.gl.is_none() || self.data_view_renderer.is_none() { return; }

        if ui.ui_contains_pointer() {
            self.handle_data_viewer_inputs(ctx, ui, mouse_over_ui, any_window_opened);
        }

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
        self.draw_data_view_edit_line(ctx, ui);
        self.mouse_over_ui |= mouse_over_ui;
    }

    fn draw_data_view_edit_line(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        let data_editing = self.data_editing.lock().unwrap();
        if !data_editing.get_flag(DATA_EDIT_DRAW_EDIT_LINE) { drop(data_editing); return; }

        let (point_1, point_2) = data_editing.get_data_view_line_points();
        ui.painter().line_segment([
            point_1.into(),
            point_2.into()
        ], Stroke::new(1.0, Color32::WHITE));
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
            let render_manager = self.render_manager.as_ref().unwrap();
            let rt = render_manager.lock().unwrap();
            let rt = rt.get_render_type();
            *rt
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

            ui.separator();

            if ui.button("Decompose Track").on_hover_text("Separates all channels in this track.").clicked() {
                let mut track_editing = self.track_editing.lock().unwrap();
                let right_clicked_track = track_editing.get_right_clicked_track();
                track_editing.decompose_track(right_clicked_track, true);
                should_close = true;
            }

            if should_close {
                ui.close_menu();
            }

            self.mouse_over_ui |= ui.ui_contains_pointer();
        });
    }

    fn draw_process_stats(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        // cpu usage, ram usage, other stats
        egui::TopBottomPanel::bottom("editor_stats").show(ctx, |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
                self.sys_stats.update();
                
                let (cpu, ram, ram_pers) = (
                    self.sys_stats.cpu_usage,
                    &self.sys_stats.memory_usage,
                    self.sys_stats.memory_pers
                );

                let cpu_str = format!("CPU: {cpu:.2}%");

                let cpu_label = if cpu >= 90.0 {
                    RichText::color(cpu_str.into(), Color32::RED)
                } else if cpu >= 50.0 {
                    RichText::color(cpu_str.into(), Color32::YELLOW)
                } else {
                    RichText::new(cpu_str)
                };

                ui.label(cpu_label);
                ui.separator();

                let ram_str = format!("RAM: {} ({:.1}%)", ram.to_string(), ram_pers);

                let ram_label = if ram_pers >= 90.0 {
                    RichText::color(ram_str.into(), Color32::RED)
                } else if ram_pers >= 50.0 {
                    RichText::color(ram_str.into(), Color32::YELLOW)
                } else {
                    RichText::new(ram_str)
                };

                ui.label(ram_label);
            });
        });
    }
}

impl eframe::App for MainWindow {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if !self.has_crashed {
            let result = catch_unwind(AssertUnwindSafe(|| {
                self.draw_ui(ctx, frame);
            }));

            if let Err(_) = result {
                self.has_crashed = true;
            }
        } else {
            if !self.crash_dlg_shown {
                self.show_dialog(DIALOG_NAME_CRASH);
                self.crash_dlg_shown = true;
            }

            let img_resources = self.image_resources.as_ref().unwrap();
            self.dialog_drawer.draw_all_dialogs(ctx, img_resources);
        }
    }
}