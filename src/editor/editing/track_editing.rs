#![warn(unused)]
use std::{cell::RefCell, collections::VecDeque, rc::Rc, sync::{Arc, Mutex, RwLock}};

use eframe::egui::{self, Key, Ui};

use crate::{
    app::{
        main_window::{EditorTool, EditorToolSettings},
        view_settings::ViewSettings
    }, 
    editor::{
        actions::{EditorAction, EditorActions},
        editing::{
            SharedClipboard,
            SharedSelectedNotes,
            note_editing::note_sequence_funcs::{
                extract,
                extract_and_remap_ids,
                merge_notes,
                merge_notes_and_return_ids
            }
        },
        navigation::{
            PianoRollNavigation,
            TrackViewNavigation
        },
        playhead::Playhead,
        project::{
            project_manager::ProjectManager
        }, 
        util::{
            MIDITick,
            SignedMIDITick,
            get_notes_in_range
        }
    },
    midi::{
        events::{
            channel_event::ChannelEvent,
            note::Note
        },
        midi_track::MIDITrack
    }
};

pub mod track_flags {
    pub const TRACK_EDIT_FLAGS_NONE: u16 = 0x0;
    pub const TRACK_EDIT_MOUSE_OVER_UI: u16 = 0x1;
    pub const TRACK_EDIT_MOUSE_DOWN_ON_UI: u16 = 0x2;
    pub const TRACK_EDIT_SELECTION_MOVE: u16 = 0x4;
    pub const TRACK_EDIT_ANY_DIALOG_OPEN: u16 = 0x8;

    pub const TRACK_EDIT_SHIFT_DOWN: u16 = 0x100;
}

use track_flags::*;

#[derive(Default)]
struct TrackEditMouseInfo {
    mouse_pos: (f32, f32),
    mouse_midi_track_pos: (MIDITick, u16),
    last_mouse_click_pos: (MIDITick, u16)
}

// because im a lazy mf, i put note track editing as a separate file
#[derive(Default)]
pub struct TrackEditing {
    project_manager: Arc<RwLock<ProjectManager>>,
    // project_manager: Arc<RwLock<ProjectManager>>,
    view_settings: Arc<Mutex<ViewSettings>>,
    shared_selected_note_ids: Arc<RwLock<SharedSelectedNotes>>,

    editor_tool: Rc<RefCell<EditorToolSettings>>,
    editor_actions: Rc<RefCell<EditorActions>>,
    pr_nav: Arc<Mutex<PianoRollNavigation>>,
    nav: Arc<Mutex<TrackViewNavigation>>,
    mouse_info: TrackEditMouseInfo,

    right_clicked_track: u16,
    flags: u16,
    
    pub selection_range: (MIDITick, MIDITick, u16, u16),
    draw_select_box: bool,
    // true after user makes a selection in track view
    pub has_selection: bool,

    ghost_notes: Arc<Mutex<Vec<(u16, Vec<Note>)>>>,

    pub ppq: u16,

    // notes clipboard
    shared_clipboard: Arc<RwLock<SharedClipboard>>,
    playhead: Rc<RefCell<Playhead>>
}

impl TrackEditing {
    pub fn new(
        project_manager: &Arc<RwLock<ProjectManager>>,
        editor_tool: &Rc<RefCell<EditorToolSettings>>,
        editor_actions: &Rc<RefCell<EditorActions>>,

        pr_nav: &Arc<Mutex<PianoRollNavigation>>,
        nav: &Arc<Mutex<TrackViewNavigation>>,

        view_settings: &Arc<Mutex<ViewSettings>>,
        shared_clipboard: &Arc<RwLock<SharedClipboard>>,
        shared_selected_note_ids: &Arc<RwLock<SharedSelectedNotes>>,
        playhead: &Rc<RefCell<Playhead>>,
    ) -> Self {
        Self {
            project_manager: project_manager.clone(),
            // ppq,
            editor_tool: editor_tool.clone(),
            editor_actions: editor_actions.clone(),

            pr_nav: pr_nav.clone(),
            nav: nav.clone(),
            mouse_info: Default::default(),

            view_settings: view_settings.clone(),
            flags: TRACK_EDIT_FLAGS_NONE,
            right_clicked_track: 0,
            ppq: 960,
            selection_range: (0, 0, 0, 0),

            ghost_notes: Arc::new(Mutex::new(Vec::new())),

            draw_select_box: false,
            has_selection: false,
            shared_clipboard: shared_clipboard.clone(),
            shared_selected_note_ids: shared_selected_note_ids.clone(),
            playhead: playhead.clone()
        }
    }

    pub fn update(&mut self, ui: &mut Ui) {
        // convert from mouse pos to midi track pos
        let (mouse_x, mouse_y) = {
            let pos = ui.input(|i| i.pointer.hover_pos().unwrap_or_default());
            (pos.x, pos.y)
        };

        // self.mouse_info.mouse_midi_track_pos = get_mouse_track_view_pos(ui, &self.nav);
        self.mouse_info.mouse_pos = (mouse_x, mouse_y);
        self.mouse_info.mouse_midi_track_pos = self.screen_pos_to_midi_track_pos((mouse_x, mouse_y), ui);

        // if ui.input(|i| i.pointer.primary_pressed()) {
        //     self.mouse_info.last_mouse_click_pos = self.mouse_info.mouse_midi_track_pos;
        // }

        self.set_flag(TRACK_EDIT_SHIFT_DOWN, ui.input(|i| i.modifiers.shift));
    }

    fn screen_pos_to_midi_track_pos(&self, screen_pos: (f32, f32), ui: &mut Ui) -> (MIDITick, u16) {
        let rect = ui.min_rect();
        
        let screen_x_norm = (screen_pos.0 - rect.left()) / rect.width();
        let screen_y_norm = (screen_pos.1 - rect.top()) / rect.height();

        let nav = self.nav.lock().unwrap();
        let screen_x_tick = (screen_x_norm * nav.zoom_ticks_smoothed + nav.tick_pos_smoothed) as MIDITick;
        let screen_y_trck = (screen_y_norm * nav.zoom_tracks_smoothed + nav.track_pos_smoothed) as u16;

        (screen_x_tick, screen_y_trck)
    }

    fn midi_track_pos_to_screen_pos(&self, midi_track_pos: (MIDITick, u16), ui: &mut Ui) -> (f32, f32) {
        let rect = ui.min_rect();
        let nav = self.nav.lock().unwrap();

        let screen_x_norm = (midi_track_pos.0 as f32 - nav.tick_pos_smoothed) / nav.zoom_ticks_smoothed;
        let screen_y_norm = (midi_track_pos.1 as f32 - nav.track_pos_smoothed) / nav.zoom_tracks_smoothed;

        let screen_x = rect.left() + screen_x_norm * rect.width();
        let screen_y = rect.top() + screen_y_norm * rect.height();

        (screen_x, screen_y)
    }

    // ======== INPUT EVENTS ========
    pub fn on_mouse_down(&mut self) {
        if self.get_flag(TRACK_EDIT_MOUSE_OVER_UI) {
            self.enable_flag(TRACK_EDIT_MOUSE_DOWN_ON_UI);
            return;
        }

        let editor_tool = {
            let editor_tool = self.editor_tool.try_borrow().unwrap();
            editor_tool.get_tool().clone()
        };

        {
            let midi_pos = self.get_mouse_midi_pos_snapped();
            self.change_track(midi_pos.1);

            let mut playhead = self.playhead.borrow_mut();
            playhead.set_start(midi_pos.0);
        }

        match editor_tool {
            EditorTool::Pencil => {

            },
            EditorTool::Selector => {
                self.select_mouse_down();
            },
            EditorTool::Eraser => {

            }
        }
    }

    pub fn on_right_mouse_down(&mut self) {
        if self.get_flag(TRACK_EDIT_MOUSE_OVER_UI) { return; }

        self.right_clicked_track = self.get_mouse_track_pos();
    }

    pub fn on_mouse_move(&mut self) {
        if self.get_flag(TRACK_EDIT_MOUSE_DOWN_ON_UI) { return; }
        if self.get_flag(TRACK_EDIT_ANY_DIALOG_OPEN | TRACK_EDIT_MOUSE_OVER_UI) { return; }

        let editor_tool = {
            let editor_tool = self.editor_tool.try_borrow().unwrap();
            editor_tool.get_tool().clone()
        };

        match editor_tool {
            EditorTool::Pencil => {

            },
            EditorTool::Selector => {
                self.select_mouse_move();
            },
            EditorTool::Eraser => {

            }
        }
    }

    pub fn on_mouse_up(&mut self) {
        if self.get_flag(TRACK_EDIT_MOUSE_DOWN_ON_UI) {
            self.disable_flag(TRACK_EDIT_MOUSE_DOWN_ON_UI);
            return;
        }

        if self.get_flag(TRACK_EDIT_MOUSE_OVER_UI | TRACK_EDIT_ANY_DIALOG_OPEN) { return; }

        let editor_tool = {
            let editor_tool = self.editor_tool.try_borrow().unwrap();
            editor_tool.get_tool().clone()
        };

        match editor_tool {
            EditorTool::Pencil => {

            },
            EditorTool::Selector => {
                self.select_mouse_up();
            },
            EditorTool::Eraser => {

            }
        }
    }

    pub fn on_key_down(&mut self, ui: &mut Ui) {
        if self.get_flag(TRACK_EDIT_ANY_DIALOG_OPEN | TRACK_EDIT_MOUSE_OVER_UI) { return; }

        let curr_track = self.get_pianoroll_track();

        if ui.input(|i| i.key_pressed(egui::Key::ArrowUp) && i.modifiers.command) {
            if curr_track > 0 { self.change_track(curr_track - 1); }
        }

        if ui.input(|i| i.key_pressed(egui::Key::ArrowDown) && i.modifiers.command) {
            if curr_track < u16::MAX { self.change_track(curr_track + 1); }
        }

        if ui.input(|i| i.key_pressed(Key::Delete)) {
            self.delete_selection();
        }

        if ui.input(|i| i.events.iter().any(|ev| matches!(ev, egui::Event::Copy))) {
            println!("Copied");
            self.copy_notes();
        }

        if ui.input(|i| i.events.iter().any(|ev| matches!(ev, egui::Event::Cut))) {
            println!("Cut");
            self.cut_notes();
        }

        if ui.input(|i| i.events.iter().any(|ev| matches!(ev, egui::Event::Paste(_)))) {
            println!("Pasted");
            self.paste_notes(curr_track);
        }
    }

    // ======== TOOL MOUSE EVENTS ========

    fn select_mouse_down(&mut self) {
        let shift_down = self.get_flag(TRACK_EDIT_SHIFT_DOWN);

        let mouse_over_selection = self.is_mouse_over_select_area();

        if !mouse_over_selection {
            // make a new selection
            if !shift_down { self.deselect_all(); }
            self.init_selection_box(self.mouse_info.mouse_midi_track_pos);
        } else {
            // move the selection
            self.selected_notes_to_ghost_notes();

            self.mouse_info.last_mouse_click_pos = self.get_mouse_midi_pos_snapped();
            println!("{:?}", self.mouse_info.last_mouse_click_pos);
        }

        self.set_flag(TRACK_EDIT_SELECTION_MOVE, mouse_over_selection);
    }

    fn select_mouse_move(&mut self) {
        if self.get_flag(TRACK_EDIT_SELECTION_MOVE) {

        } else {
            self.update_selection_box(self.mouse_info.mouse_midi_track_pos);
        }
    }

    fn select_mouse_up(&mut self) {
        // self.has_selection = true;
        // self.draw_select_box = false;

        // let (min_tick, max_tick, min_track, max_track) = self.get_selection_range();
        if self.get_flag(TRACK_EDIT_SELECTION_MOVE) {
            // apply note movement
            self.apply_ghost_notes();
        } else {
            let shift_down = self.get_flag(TRACK_EDIT_SHIFT_DOWN);
            // select notes
            if !shift_down { self.deselect_all() };
            
            self.draw_select_box = false;

            let region = self.get_selection_range();

            if let Some(selected_ids_with_track) = self.get_note_ids_in_region(region) {
                self.has_selection = true;
                let mut shared_selected = self.shared_selected_note_ids.write().unwrap();

                let mut num_selected = 0;
                // select notes in each track, one by one
                for (track, ids) in selected_ids_with_track {
                    num_selected += ids.len();
                    if shift_down {
                        (*shared_selected).add_selected_to_track(ids, track);
                    } else {
                        (*shared_selected).set_selected_in_track(ids, track);
                    }
                }

                println!("Selected {num_selected} notes in track view.");
            } else {
                // no notes were selected at all, existing selection cleared already
                println!("Selected no notes in track view.");
            }
        }
    }

    // ======== SELECTION BOX ========

    fn init_selection_box(&mut self, start_pos: (MIDITick, u16)) {
        let snapped_tick = self.snap_tick(start_pos.0 as SignedMIDITick) as MIDITick;
        self.selection_range = (snapped_tick, snapped_tick, start_pos.1, start_pos.1);
        self.draw_select_box = true;
    }

    fn update_selection_box(&mut self, new_pos: (MIDITick, u16)) {
        self.selection_range.1 = self.snap_tick(new_pos.0 as SignedMIDITick) as MIDITick;
        self.selection_range.3 = new_pos.1;
    }

    fn get_selection_range(&self) -> (MIDITick, MIDITick, u16, u16) {
        let (min_tick, max_tick) = {
            if self.selection_range.0 > self.selection_range.1 {
                (self.selection_range.1, self.selection_range.0)
            } else {
                (self.selection_range.0, self.selection_range.1)
            }
        };

        let (min_track, max_track) = {
            if self.selection_range.2 > self.selection_range.3 {
                (self.selection_range.3, self.selection_range.2 + 1)
            } else {
                (self.selection_range.2, self.selection_range.3 + 1)
            }
        };

        (min_tick, max_tick, min_track, max_track)
    }

    #[inline(always)]
    pub fn get_can_draw_selection_box(&self) -> bool {
        self.draw_select_box
    }

    pub fn get_selection_range_ui(&self, ui: &mut Ui) -> ((f32, f32), (f32, f32)) {
        let (min_tick, max_tick, min_track, max_track) = self.get_selection_range();

        let tl = self.midi_track_pos_to_screen_pos((min_tick, min_track), ui);
        let br = self.midi_track_pos_to_screen_pos((max_tick, max_track), ui);

        (tl, br)
    }

    pub fn is_mouse_over_select_area(&self) -> bool {
        if !self.has_selection { return false; }
        
        let (min_tick, max_tick, min_track, max_track) = self.get_selection_range();
        let (mouse_x, mouse_y) = self.mouse_info.mouse_midi_track_pos;
        
        mouse_x > min_tick && mouse_x < max_tick && mouse_y >= min_track && mouse_y < max_track
    }

    // ======== SELECTION HELPER FUNCTIONS ========

    /// Gets all Note IDs within a region. Returns (start track, Vec<Vec<usize>>)
    fn get_note_ids_in_region(&self, region: (MIDITick, MIDITick, u16, u16)) -> Option<Vec<(u16, Vec<usize>)>> {
        let (min_tick, max_tick, min_track, max_track) = region;
        if min_tick == max_tick { return None; }

        let mut all_ids = Vec::with_capacity((max_track - min_track + 1) as usize);

        let project_manager = self.project_manager.read().unwrap();
        let tracks = project_manager.get_tracks().read().unwrap();
        
        // no notes can be selected if the min track is higher than what's actually present
        if min_track >= tracks.len() as u16 { return None; }

        for trk in min_track..max_track {
            if trk >= tracks.len() as u16 { break; }

            let track = tracks[trk as usize].get_notes();
            if track.is_empty() { continue; }

            let ids = get_notes_in_range(&track, min_tick, max_tick, 0, 127, true);
            if !ids.is_empty() { all_ids.push((trk, ids)) };
        }

        if all_ids.is_empty() { return None; }

        Some(all_ids)
    }

    fn deselect_all(&mut self) {
        self.has_selection = false;

        let mut shared_selected = self.shared_selected_note_ids.write().unwrap();
        shared_selected.clear_selected();
    }

    fn delete_selection(&mut self) {
        {
            let project_manager = self.project_manager.read().unwrap();
            let mut tracks = project_manager.get_tracks().write().unwrap();

            let mut selected = self.shared_selected_note_ids.write().unwrap();

            let mut affected_tracks = Vec::new();
            let mut deleted_notes = Vec::new();
            let mut deleted_ids = Vec::new();
            for (trk, ids) in selected.take_selected_from_all() {
                let notes_ = (*tracks)[trk as usize].get_notes_mut();
                let notes = std::mem::take(notes_);
                
                let (deleted, kept) = extract(notes, &ids);
                *notes_ = kept;

                affected_tracks.push(trk);
                deleted_notes.push(deleted);
                deleted_ids.push(ids);
            }

            let mut editor_actions = self.editor_actions.borrow_mut();
            editor_actions.register_action(EditorAction::DeleteNotesMultiTrack(deleted_ids, Some(deleted_notes), affected_tracks));
        }

        // deselect
        self.deselect_all();
    }

    // ======== GHOST NOTE STUFF ========

    fn selected_notes_to_ghost_notes(&mut self) {
        let (_, _, min_track, max_track) = self.get_selection_range();

        let mut old_notes = {
            let project_manager = self.project_manager.read().unwrap();
            let mut tracks = project_manager.get_tracks().write().unwrap();
            
            let mut notes = Vec::with_capacity((max_track - min_track) as usize);
            for trk in min_track..max_track {
                if trk >= tracks.len() as u16 { break; }

                let notes_ = std::mem::take((*tracks)[trk as usize].get_notes_mut());
                notes.push((trk, notes_));
            }

            notes
        };

        let tmp_ghosts= {
            let mut tmp_ghosts = Vec::new();

            for (trk, notes) in old_notes.drain(..) {
                let (tg, new_notes) = {
                    let selected = self.shared_selected_note_ids.read().unwrap();
                    if let Some(selected) = selected.get_selected_ids_in_track(trk) {
                        extract(notes, selected)
                    } else {
                        (Vec::new(), notes)
                    }
                };
                
                tmp_ghosts.push((trk, tg));
                self.set_notes_in_track(trk, new_notes);
            }

            tmp_ghosts
        };

        {
            let mut shared_selected = self.shared_selected_note_ids.write().unwrap();
            shared_selected.clear_selected();
        }

        let mut ghost_notes = self.ghost_notes.lock().unwrap();
        *ghost_notes = tmp_ghosts;
    }

    fn apply_ghost_notes(&mut self) -> Vec<(u16, Vec<usize>)> {
        let mut ghost_notes = {
            let mut ghost_notes = self.ghost_notes.lock().unwrap();
            std::mem::take(&mut *ghost_notes)
        };

        let mut tracks = {
            let project_manager = self.project_manager.read().unwrap();
            let mut tracks = project_manager.get_tracks().write().unwrap();
            std::mem::take(&mut *tracks)
        };

        let mut trk_ids = Vec::with_capacity(ghost_notes.len());

        for (trk, notes) in ghost_notes.drain(..) {
            let track = (*tracks)[trk as usize].get_notes_mut();
            let old_notes = std::mem::take(track);
            let (merged, ids) = merge_notes_and_return_ids(old_notes, notes);

            *track = merged;
            
            {
                let mut selected = self.shared_selected_note_ids.write().unwrap();
                selected.set_selected_in_track(ids.clone(), trk);
            }

            trk_ids.push((trk, ids));
        }

        let project_manager = self.project_manager.read().unwrap();
        (*(project_manager.get_tracks().write().unwrap())) = tracks;
        
        trk_ids
    }

    // ======== HELPER FUNCTIONS ========

    #[inline(always)]
    pub fn get_mouse_track_pos(&self) -> u16 {
        self.mouse_info.mouse_midi_track_pos.1
        // self.curr_mouse_track_pos.1
    }

    fn get_pianoroll_track(&self) -> u16 {
        let nav = self.pr_nav.lock().unwrap();
        nav.curr_track
    }

    fn set_notes_in_track(&mut self, track: u16, notes_: Vec<Note>) {
        let project_manager = self.project_manager.read().unwrap();
        let mut tracks = project_manager.get_tracks().write().unwrap();
        let notes = (*tracks)[track as usize].get_notes_mut();
        *notes = notes_;
    }

    fn insert_notes_and_ch_evs(&mut self, track: u16, notes: Vec<Note>, ch_evs: Vec<ChannelEvent>) {
        let project_manager = self.project_manager.read().unwrap();
        
        let mut track_ = project_manager.get_tracks().write().unwrap();
        /*let (mut notes_, mut ch_evs_) = (
            let mut track = 
            
            // project_manager.get_notes().write().unwrap(),
            // project_manager.get_channel_evs().write().unwrap()
        );*/

        track_.insert(track as usize, MIDITrack::new(notes, ch_evs, Vec::new()));
        // notes_.insert(track as usize, notes);
        // ch_evs_.insert(track as usize, ch_evs);
    }

    fn insert_track_at(&mut self, track_idx: u16, track: MIDITrack) {
        let project_manager = self.project_manager.read().unwrap();
        
        let mut tracks = project_manager.get_tracks().write().unwrap();
        tracks.insert(track_idx as usize, track);
    }

    pub fn insert_track(&mut self, track: u16) {
        let track_count = self.get_used_track_count();
        if track >= track_count { return; }

        self.insert_notes_and_ch_evs(track, Vec::new(), Vec::new());

        let mut editor_actions = self.editor_actions.try_borrow_mut().unwrap();
        editor_actions.register_action(EditorAction::AddTrack(track, None, false));
    }

    pub fn remove_track(&mut self, track: u16) {
        let mut track_count = self.get_used_track_count();
        if track >= track_count { return; }

        {
            // let (removed_track_notes, removed_track_ch_evs) = self.remove_note_and_ch_evs(track);
            let removed_track = self.remove_track_at(track);
            track_count -= 1;

            let mut removed_first = false;
            if track_count == 0 {
                // always guarantee at least one track
                track_count = 1;
                removed_first = true;
            }
            
            let mut removed_track_queue = VecDeque::new();
            removed_track_queue.push_back(removed_track);
            // removed_track_queue.push_back((removed_track_notes, removed_track_ch_evs)); 

            let mut editor_actions = self.editor_actions.try_borrow_mut().unwrap();
            editor_actions.register_action(EditorAction::RemoveTrack(track, Some(removed_track_queue), removed_first));
        }

        {
            let pr_track = self.get_pianoroll_track();
            if pr_track > track {
                self.change_track(pr_track - 1);
            }
        }
    }

    pub fn remove_track_at(&mut self, track: u16) -> MIDITrack {
        let project_manager = self.project_manager.read().unwrap();
        let mut tracks = project_manager.get_tracks().write().unwrap();
        tracks.remove(track as usize)
    }

    /*pub fn remove_note_and_ch_evs(&mut self, track: u16) -> (Vec<Note>, Vec<ChannelEvent>) {
        let project_manager = self.project_manager.read().unwrap();
        
        let mut track_ = project_manager.get_tracks().write().unwrap();
        /*let (mut notes, mut ch_evs) = (
            project_manager.get_notes().write().unwrap(),
            project_manager.get_channel_evs().write().unwrap()
        );*/

        (notes.remove(track as usize), ch_evs.remove(track as usize))
    }*/

    /// Decomposes a track with multiple channels into multiple tracks, each containing only one channel
    pub fn decompose_track(&mut self, track: u16, should_register: bool) {
        let (notes, ch_evs) = {
            let project_manager = self.project_manager.read().unwrap();
            let mut tracks = project_manager.get_tracks().write().unwrap();
            (
                std::mem::take(tracks[track as usize].get_notes_mut()),
                std::mem::take(tracks[track as usize].get_channel_evs_mut()),
            )
        };

        if notes.is_empty() && ch_evs.is_empty() {
            return;
        }

        // allocate exactly how many channels we actually have
        let mut decomposed: Vec<(Vec<Note>, Vec<ChannelEvent>)> = Vec::with_capacity(16);
        for _ in 0..16 {
            decomposed.push((Vec::new(), Vec::new()));
        }

        // assign notes
        for note in notes {
            let ch = note.channel() as usize;
            decomposed[ch].0.push(note);
        }

        // assign channel events
        for ev in ch_evs {
            let ch = ev.channel as usize;
            decomposed[ch].1.push(ev);
        }

        // keep only non-empty tracks
        let decomposed: Vec<(Vec<Note>, Vec<ChannelEvent>)> =
            decomposed.into_iter().filter(|t| !t.0.is_empty() || !t.1.is_empty()).collect();

        if decomposed.is_empty() {
            return;
        }

        let decomposed_count = decomposed.len();

        {
            let project_manager = self.project_manager.read().unwrap();
            let mut tracks = project_manager.get_tracks().write().unwrap();

            // preallocate space for new tracks instead of inserting repeatedly
            tracks.reserve(decomposed_count.saturating_sub(1));

            // replace base track first, then append additional tracks
            for (i, (notes_for_channel, evs_for_channel)) in decomposed.into_iter().enumerate() {
                if i == 0 {
                    let track_mut = &mut tracks[track as usize];
                    *track_mut.get_notes_mut() = notes_for_channel;
                    *track_mut.get_channel_evs_mut() = evs_for_channel;
                } else {
                    let mut new_track = MIDITrack::default();
                    *new_track.get_notes_mut() = notes_for_channel;
                    *new_track.get_channel_evs_mut() = evs_for_channel;
                    tracks.insert(track as usize + i, new_track); // only do insert for new tracks
                }
            }
        }

        if should_register {
            let mut editor_actions = self.editor_actions.borrow_mut();
            editor_actions.register_action(EditorAction::DecomposeTrack(track, decomposed_count as u16));
        }
    }

    pub fn append_empty_track(&mut self) {
        let project_manager = self.project_manager.read().unwrap();
        let mut tracks = project_manager.get_tracks().write().unwrap();
        tracks.push(MIDITrack::new_empty());
    }

    pub fn pop_track(&mut self) {
        let project_manager = self.project_manager.read().unwrap();
        let mut tracks = project_manager.get_tracks().write().unwrap();
        tracks.pop();
    }

    pub fn get_right_clicked_track(&self) -> u16 {
        self.right_clicked_track
    }

    pub fn remove_right_clicked_track(&mut self) {
        self.remove_track(self.right_clicked_track);
    }

    pub fn get_used_track_count(&self) -> u16 {
        let project_manager = self.project_manager.read().unwrap();
        let tracks = project_manager.get_tracks().read().unwrap();
        tracks.len() as u16
        // let notes = project_manager.get_notes().read().unwrap();
        // notes.len() as u16
    }

    pub fn change_track(&mut self, new_track: u16) {
        {
            let mut view_settings = self.view_settings.lock().unwrap();
            view_settings.pr_curr_track.set_value(new_track);
        }

        {
            //let mut project_data = self.project_data.try_borrow_mut().unwrap();
            let mut project_manager = self.project_manager.write().unwrap();
            project_manager.get_project_data_mut().validate_tracks(new_track);
        }

        {
            let mut nav = self.pr_nav.lock().unwrap();
            nav.curr_track = new_track;
        }
    }

    fn swap_tracks_and_register(&mut self, track_1: u16, track_2: u16, allow_register: bool) {
        let track_count = self.get_used_track_count();

        {
            let mut project_manager = self.project_manager.write().unwrap();
            let project_data = project_manager.get_project_data_mut();
            
            if track_1 >= track_count || track_2 >= track_count {
                project_data.validate_tracks(track_1.max(track_2));
            }

            let mut tracks = project_data.tracks.write().unwrap();
            tracks.swap(track_1 as usize, track_2 as usize);
        }

        if allow_register {
            let mut editor_actions = self.editor_actions.try_borrow_mut().unwrap();
            editor_actions.register_action(EditorAction::SwapTracks(track_1, track_2));
        }

        let curr_track = self.get_pianoroll_track();
        if curr_track == track_1 { self.change_track(track_2); return; }
        if curr_track == track_2 { self.change_track(track_1); return; }
    }

    pub fn swap_tracks(&mut self, track_1: u16, track_2: u16) {
        self.swap_tracks_and_register(track_1, track_2, true);
    }

    // ======== CLIPBOARD STUFF ========

    fn clone_notes(&self, track: u16, ids: &[usize]) -> Vec<Note> {
        let project_manager = self.project_manager.read().unwrap();
        let mut tracks = project_manager.get_tracks().write().unwrap();
        let notes = (*tracks)[track as usize].get_notes_mut();

        let copied = ids.iter().map(|&id| {
            let note = &notes[id];
            note.clone()
        }).collect();

        copied
    }
    
    fn prepare_clipboard(&mut self) {
        let mut shared_clipboard = self.shared_clipboard.write().unwrap();
        shared_clipboard.clear_clipboard();
        
        let clipboard_start = shared_clipboard.get_clipboard_start_tick();
        let playhead_tick = {
            let playhead = self.playhead.borrow();
            playhead.start_tick
        };
        let offset_from_playhead = clipboard_start as SignedMIDITick - playhead_tick as SignedMIDITick;
        shared_clipboard.offset_from_playhead = offset_from_playhead;
    }

    pub fn copy_notes(&mut self) {
        let active_tracks = {
            let selected = self.shared_selected_note_ids.read().unwrap();
            selected.get_active_selected_tracks()
        };

        if active_tracks.is_empty() { return; }

        self.prepare_clipboard();

        let selected = self.shared_selected_note_ids.read().unwrap();
        for track in active_tracks {
            let selected = selected.get_selected_ids_in_track(track);
            if selected.is_none() { continue; }
        
            let selected = selected.unwrap();
            let copied_notes = self.clone_notes(track, selected);

            let mut shared_clipboard = self.shared_clipboard.write().unwrap();
            shared_clipboard.move_notes_to_clipboard(copied_notes, track, false);
        }
    }

    pub fn cut_notes(&mut self) {
        let active_tracks = {
            let selected = self.shared_selected_note_ids.read().unwrap();
            selected.get_active_selected_tracks()
        };

        if active_tracks.is_empty() { return; }

        self.prepare_clipboard();

        let mut actions = Vec::new();

        for track in active_tracks {
            let (ids, cut_notes, retained_notes) = {
                let mut selected = self.shared_selected_note_ids.write().unwrap();
                let selected = selected.take_selected_from_track(track);
                if selected.is_empty() { continue; }

                let old_notes = {
                    let project_manager = self.project_manager.read().unwrap();
                    let mut tracks = project_manager.get_tracks().write().unwrap();
                    let notes = std::mem::take((*tracks)[track as usize].get_notes_mut());
                    notes
                };

                let (cut_notes, retained_notes) = extract(old_notes, &selected);
                
                let mut shared_clipboard = self.shared_clipboard.write().unwrap();
                shared_clipboard.move_notes_to_clipboard(cut_notes.clone(), track, false);

                (selected, cut_notes, retained_notes)
            };
            
            self.set_notes_in_track(track, retained_notes);
            actions.push(EditorAction::DeleteNotes(ids, Some(cut_notes), track));
        }

        let mut editor_actions = self.editor_actions.borrow_mut();
        editor_actions.register_action(EditorAction::Bulk(actions));
    }

    pub fn paste_notes(&mut self, base_track: u16) {
        let (mut copied_notes, offset_from_playhead) = {
            let shared_clipboard = self.shared_clipboard.read().unwrap();
            (shared_clipboard.get_notes_from_clipboard(), shared_clipboard.offset_from_playhead)
        };

        if copied_notes.is_empty() { return; }
        copied_notes.sort_by_key(|(trk, _)| *trk);

        let first_track = copied_notes[0].0;
        let num_tracks = self.get_used_track_count();

        let playhead_tick = {
            let playhead = self.playhead.borrow();
            playhead.start_tick
        };

        let mut track_actions = Vec::new();
        let mut actions = Vec::new();

        for (src_track, notes_vec) in copied_notes.into_iter() {
            let rel = src_track - first_track;
            let dest_track = base_track + rel;
            let dest_idx = dest_track as usize;

            if dest_track >= num_tracks {
                let project_manager = self.project_manager.write().unwrap();
                let mut tracks = project_manager.get_tracks().write().unwrap();
                tracks.push(MIDITrack::default());
                track_actions.push(EditorAction::AddTrack(dest_track as u16, None, false));
            }

            let old_notes = {
                let project_manager = self.project_manager.write().unwrap();
                let mut tracks = project_manager.get_tracks().write().unwrap();
                let notes = std::mem::take((*tracks)[dest_idx as usize].get_notes_mut());
                notes
            };

            let notes_vec = notes_vec.into_iter().map(|n| {
                let mut note = n;
                *(note.start_mut()) = (n.start() as SignedMIDITick + playhead_tick as SignedMIDITick + offset_from_playhead).max(0) as MIDITick;
                note
            }).collect();

            let (new_notes, new_ids) = merge_notes_and_return_ids(old_notes, notes_vec);
            self.set_notes_in_track(dest_track, new_notes);

            {
                let mut selected = self.shared_selected_note_ids.write().unwrap();
                selected.set_selected_in_track(new_ids.clone(), dest_track);
            }

            actions.push(EditorAction::PlaceNotes(new_ids, None, dest_track));
        }

        let mut editor_actions = self.editor_actions.borrow_mut();
        editor_actions.register_action(EditorAction::Bulk([track_actions, actions].concat()));
    }

    // ======== ACTIONS ========

    pub fn apply_action(&mut self, action: &mut EditorAction) {
        match action {
            EditorAction::AddTrack(track_insert, removed_tracks, _) => {
                assert!(removed_tracks.is_some(), "[ADD_TRACKS] Something has gone wrong while trying to add a track.");

                let mut recovered_track = removed_tracks.take().unwrap();
                let recovered_track = recovered_track.pop_front().unwrap();
                self.insert_track_at(*track_insert, recovered_track);

                // change current track if inserted before current track
                let curr_track = self.get_pianoroll_track();
                if *track_insert <= curr_track {
                    self.change_track(curr_track + 1);
                }
            },
            EditorAction::RemoveTrack(track_rem_idx, removed_tracks, _) => {
                let mut rem_track_queue = VecDeque::with_capacity(1);

                let removed_track = self.remove_track_at(*track_rem_idx);
                rem_track_queue.push_front(removed_track);

                *removed_tracks = Some(rem_track_queue);

                // change current track if removed before current track
                let curr_track = self.get_pianoroll_track();
                if *track_rem_idx < curr_track {
                    self.change_track(curr_track - 1);
                }
            },
            EditorAction::PlaceNotesMultiTrack(_, removed_notes, tracks) => {
                assert!(removed_notes.is_some(), "[PLACE NOTES] Something has gone wrong while trying to add back notes in multiple tracks.");
                
                let recovered_notes = removed_notes.take().unwrap();

                for (recovered_notes, track) in recovered_notes.into_iter().zip(tracks) {
                    let old_notes = {
                        let project_manager = self.project_manager.read().unwrap();
                        let mut tracks_ = project_manager.get_tracks().write().unwrap();

                        std::mem::take((*tracks_)[*track as usize].get_notes_mut())
                    };
                    let merged = merge_notes(old_notes, recovered_notes);
                    self.set_notes_in_track(*track, merged);
                }
            },
            EditorAction::DeleteNotesMultiTrack(note_ids, notes_deleted, tracks) => {
                let mut notes_deleted_ = Vec::new();

                for (note_ids, track) in note_ids.into_iter().zip(tracks) {
                    let old_notes = {
                        let project_manager = self.project_manager.read().unwrap();
                        let mut tracks_ = project_manager.get_tracks().write().unwrap();

                        std::mem::take((*tracks_)[*track as usize].get_notes_mut())
                    };
                    
                    let mut shared_selected = self.shared_selected_note_ids.write().unwrap();
                        let old_sel_ids = shared_selected.take_selected_from_track(*track);
                        let (deleted, new_notes, new_ids) = extract_and_remap_ids(old_notes, &note_ids, old_sel_ids);
                        shared_selected.set_selected_in_track(new_ids, *track);
                    drop(shared_selected);

                    self.set_notes_in_track(*track, new_notes);

                    notes_deleted_.push(deleted);
                }

                *notes_deleted = Some(notes_deleted_);
            },
            EditorAction::ComposeTrack(base_track, channel_count) => {
                let decomposed_tracks = {
                    let project_manager = self.project_manager.read().unwrap();
                    let mut tracks = project_manager.get_tracks().write().unwrap();

                    let mut decomp = Vec::new();
                    for i in 0..(*channel_count) {
                        let notes = std::mem::take(tracks[(*base_track + i) as usize].get_notes_mut());
                        decomp.push(notes);
                    }

                    decomp
                };

                let composed = decomposed_tracks
                    .into_iter()
                    .reduce(|a, b| merge_notes(a, b))
                    .unwrap_or_else(Vec::new);

                {
                    let project_manager = self.project_manager.read().unwrap();
                    let mut tracks = project_manager.get_tracks().write().unwrap();
                    let track = tracks[*base_track as usize].get_notes_mut();
                    *track = composed;

                    // remove empty tracks
                    for i in (1..*channel_count).rev() {
                        tracks.remove((*base_track + i) as usize);
                    }
                }
            },
            EditorAction::DecomposeTrack(base_track, _) => {
                self.decompose_track(*base_track, false);
            },
            EditorAction::SwapTracks(track_1, track_2) => {
                self.swap_tracks_and_register(*track_1, *track_2, false);
            },
            _ => {}
        }
    }

    // ======== MISC ========

    fn get_mouse_midi_pos_snapped(&self) -> (MIDITick, u16) {
        let snapped_tick = self.snap_tick(self.mouse_info.mouse_midi_track_pos.0 as SignedMIDITick) as MIDITick;
        (snapped_tick, self.mouse_info.mouse_midi_track_pos.1)
    }

    fn snap_tick(&self, tick: SignedMIDITick) -> SignedMIDITick {
        let snap = self.get_min_snap_tick_length() as SignedMIDITick;
        if snap == 1 { return tick; }

        let half = snap / 2;
        if tick >= 0 {
            ((tick + half) / snap) * snap
        } else {
            ((tick - half) / snap) * snap
        }
    }

    fn get_min_snap_tick_length(&self) -> MIDITick {
        let editor_tool = self.editor_tool.try_borrow().unwrap();
        let snap_ratio = editor_tool.snap_ratio;
        if snap_ratio.0 == 0 { return 1; }
        return (self.ppq as MIDITick * 4 * snap_ratio.0 as MIDITick)
            /  snap_ratio.1 as MIDITick;
    }

    // ======== FLAG HELPER FUNCTIONS ========

    #[inline(always)]
    pub fn set_flag(&mut self, flag: u16, value: bool) {
        self.flags = (self.flags & !flag) | ((-(value as i16) as u16) & flag);
    }

    #[inline(always)]
    pub fn get_flag(&self, flag: u16) -> bool {
        self.flags & flag != 0
    }

    #[inline(always)]
    pub fn enable_flag(&mut self, flag: u16) {
        self.set_flag(flag, true);
    }

    #[inline(always)]
    pub fn disable_flag(&mut self, flag: u16) {
        self.flags &= !flag;
    }

    /*#[inline(always)]
    pub fn set_mouse_over_ui(&mut self, mouse_over_ui: bool) {
        self.flags &= !TRACK_EDIT_MOUSE_OVER_UI;
        if mouse_over_ui { self.flags |= TRACK_EDIT_MOUSE_OVER_UI; }
    }

    pub fn update_from_ui(&mut self, ui: &mut Ui) {
        let mouse_track_pos = get_mouse_track_view_pos(ui, &self.nav);
        self.curr_mouse_track_pos = mouse_track_pos;
    }

    #[inline(always)]
    pub fn get_mouse_track_pos(&self) -> u16 {
        self.curr_mouse_track_pos.1
    }

    // helper functions
    fn get_pianoroll_track(&self) -> u16 {
        let nav = self.pr_nav.lock().unwrap();
        nav.curr_track
    }

    pub fn on_mouse_down(&mut self) {
        if self.flags & TRACK_EDIT_MOUSE_OVER_UI != 0 {
            self.flags |= TRACK_EDIT_MOUSE_DOWN_ON_UI;
            return;
        }

        let editor_tool = {
            let editor_tool = self.editor_tool.try_borrow().unwrap();
            editor_tool.get_tool().clone()
        };

        match editor_tool {
            EditorTool::Pencil => {
                let track_pos = self.get_mouse_track_pos();
                self.change_track(track_pos);
            },
            EditorTool::Selector => {

            },
            EditorTool::Eraser => {

            }
        }
    }

    pub fn on_right_mouse_down(&mut self) {
        if self.flags & TRACK_EDIT_MOUSE_OVER_UI != 0 {
            return;
        }

        self.right_clicked_track = self.get_mouse_track_pos();
    }

    pub fn on_mouse_move(&mut self) {

    }

    pub fn on_mouse_up(&mut self) {

    }

    pub fn on_key_down(&mut self, ui: &mut Ui) {
        let curr_track = self.get_pianoroll_track();

        if ui.input(|i| i.key_pressed(egui::Key::ArrowUp) && i.modifiers.command) {
            if curr_track > 0 { self.change_track(curr_track - 1); }
        }

        if ui.input(|i| i.key_pressed(egui::Key::ArrowDown) && i.modifiers.command) {
            if curr_track < u16::MAX { self.change_track(curr_track + 1); }
        }
    }*/
}