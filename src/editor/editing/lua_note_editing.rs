use std::{collections::HashMap, sync::{Arc, Mutex}};

use mlua::{Function, IntoLua, Lua, UserData};

use crate::{editor::{actions::{EditorAction, EditorActions}, editing::note_editing::{note_sequence_funcs::{extract_with, merge_notes_and_return_ids}, NoteEditing}, util::{get_min_max_keys_in_selection, get_min_max_ticks_in_selection, move_notes_to, MIDITick, SignedMIDITick}}, midi::events::note::{Note}};

impl UserData for Note {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("channel", |_, this| Ok(this.channel));
        fields.add_field_method_set("channel", |_, this, val: u8| {
            this.channel = val;
            Ok(())
        });

        fields.add_field_method_get("start", |_, this| Ok(this.start));
        fields.add_field_method_set("start", |_, this, val: MIDITick| {
            this.start = val;
            Ok(())
        });

        fields.add_field_method_get("length", |_, this| Ok(this.length));
        fields.add_field_method_set("length", |_, this, val: MIDITick| {
            this.length = val;
            Ok(())
        });

        fields.add_field_method_get("key", |_, this| Ok(this.key));
        fields.add_field_method_set("key", |_, this, val: u8| {
            this.key = val;
            Ok(())
        });

        fields.add_field_method_get("velocity", |_, this| Ok(this.velocity));
        fields.add_field_method_set("velocity", |_, this, val: u8| {
            this.velocity = val;
            Ok(())
        });
    }
}

pub struct LuaNoteEditing {
    pub note_editing: Arc<Mutex<NoteEditing>>,
    pub delta_note_pos: HashMap<usize, (SignedMIDITick, i16)>,
    pub delta_note_lengths: HashMap<usize, SignedMIDITick>,
    pub delta_note_channels: HashMap<usize, i8>,
    pub delta_note_velocities: HashMap<usize, i8>,

    pub notes_to_add: Vec<Note>
}

impl LuaNoteEditing {
    pub fn new(note_editing: Arc<Mutex<NoteEditing>>) -> Self {
        Self { 
            note_editing,
            delta_note_pos: HashMap::new(),
            delta_note_lengths: HashMap::new(),
            delta_note_channels: HashMap::new(),
            delta_note_velocities: HashMap::new(),
            notes_to_add: Vec::new(),
        }
    }

    /// Helper function for editing a note directly from lua.
    /// This also updates note deltas if needed.
    fn change_note_from_lua(&mut self, lua: &Lua, func: &Function, note: &mut Note) -> mlua::Result<()> {
        lua.scope(|scope| {
            let note_ref = scope.create_userdata_ref_mut(note)?;
            func.call::<()>(note_ref)?;
            Ok(())
        })?;
        Ok(())
    }

    /// Helper function for calling an immutable lua function
    fn call_lua_note_fn(&self, lua: &Lua, func: &Function, note: &Note) -> mlua::Result<()> {
        lua.scope(|scope| {
            let note_ref = scope.create_userdata_ref(note)?;
            func.call::<()>(note_ref)?;
            Ok(())
        })?;
        Ok(())
    }

    fn change_note_and_update_deltas(&mut self, lua: &Lua, func: &Function, note: &mut Note, id: usize) -> mlua::Result<()> {
        let old_start = note.start();
        let old_key = note.key();
        let old_length = note.length();
        let old_channel = note.channel();
        let old_velocity = note.velocity();

        self.change_note_from_lua(lua, func, note)?;

        let delta_start = note.start() as SignedMIDITick - old_start as SignedMIDITick;
        let delta_key = note.key() as i16 - old_key as i16;
        let delta_length = note.length() as SignedMIDITick - old_length as SignedMIDITick;
        let delta_channel = note.channel() as i8 - old_channel as i8;
        let delta_velocity = note.velocity() as i8 - old_velocity as i8;

        if delta_start != 0 || delta_key != 0 {
            let dt_pos = self.delta_note_pos.entry(id).or_default();
            *dt_pos = (delta_start, delta_key);
        }

        if delta_length != 0 {
            let dt_length = self.delta_note_lengths.entry(id).or_default();
            *dt_length = delta_length;
        }

        if delta_channel != 0 {
            let dt_channel = self.delta_note_channels.entry(id).or_default();
            *dt_channel = delta_channel;
        }

        if delta_velocity != 0 {
            let dt_velocity = self.delta_note_velocities.entry(id).or_default();
            *dt_velocity = delta_velocity;
        }

        Ok(())
    }

    pub fn range_as_lua_table<T: IntoLua + 'static>(lua: &Lua, min: T, max: T) -> mlua::Result<mlua::Table> {
        let table = lua.create_table()?;
        table.set("min", min)?;
        table.set("max", max)?;
        Ok(table)
    }

    pub fn apply_changes(self, track: u16, editor_actions: &mut EditorActions) {
        let note_editing = self.note_editing.clone();

        let mut bulk_actions = Vec::new();

        let dt_channels: Vec<(usize, i8)> = self.delta_note_channels.into_iter().map(|(k, v)| (k, v)).collect();
        let dt_velocities: Vec<(usize, i8)> = self.delta_note_velocities.into_iter().map(|(k, v)| (k, v)).collect();
        let mut dt_position: Vec<(usize, (SignedMIDITick, i16))> = self.delta_note_pos.into_iter().map(|(k, v)| (k, v)).collect();
        let dt_length: Vec<(usize, SignedMIDITick)> = self.delta_note_lengths.into_iter().map(|(k, v)| (k, v)).collect();
        let mut notes_to_add = self.notes_to_add;

        // 1. apply note channel changes
        if !dt_channels.is_empty() {
            let (ids, ch_change): (Vec<usize>, Vec<i8>) = dt_channels.into_iter().unzip();
            bulk_actions.push(EditorAction::ChannelChange(ids, ch_change, track));
        }

        if !dt_velocities.is_empty() {
            let (ids, vel_change): (Vec<usize>, Vec<i8>) = dt_velocities.into_iter().unzip();
            bulk_actions.push(EditorAction::VelocityChange(ids, vel_change, track));
        }

        if !dt_length.is_empty() {
            let (ids, len_change): (Vec<usize>, Vec<SignedMIDITick>) = dt_length.into_iter().unzip();
            bulk_actions.push(EditorAction::LengthChange(ids, len_change, track));
        }

        if !dt_position.is_empty() {
            dt_position.sort_by_key(|&(id, _)| id);
            let (ids, delta_pos): (Vec<usize>, Vec<_>) = dt_position.into_iter().unzip();

            let mut note_editing = note_editing.lock().unwrap();
            let old_notes = note_editing.take_notes_in_track(track);
            
            // group notes with delta while extracting them to prevent delta pos index invalidation when sorting by note start
            let (mut notes_with_delta, old_notes) = extract_with(old_notes, &ids, delta_pos); // O(n)
            notes_with_delta.sort_unstable_by_key(|(n, _)| n.start());
            let (notes_to_move, delta) = notes_with_delta.into_iter().unzip();
    
            let (merged, note_ids) = merge_notes_and_return_ids(old_notes, notes_to_move); // O(n+k)
            note_editing.set_notes_in_track(track, merged);

            bulk_actions.push(EditorAction::NotesMove(note_ids, delta, track, true));
        }

        if !notes_to_add.is_empty() {
            notes_to_add.sort_unstable_by_key(|&n| n.start());

            let mut note_editing = note_editing.lock().unwrap();

            let old_notes = note_editing.take_notes_in_track(track);
            let (merged, ids) = merge_notes_and_return_ids(old_notes, notes_to_add);
            
            note_editing.set_notes_in_track(track, merged);
            bulk_actions.push(EditorAction::PlaceNotes(ids, None, track));
        }

        if !bulk_actions.is_empty() { editor_actions.register_action(EditorAction::Bulk(bulk_actions)); }
    }
}

impl UserData for LuaNoteEditing {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("for_each_note", |lua, this, func: Function| {
            let curr_track: usize = lua.globals().get("curr_track")?;
            
            let notes = {
                let note_editing = this.note_editing.lock().unwrap();
                note_editing.get_notes().clone()
            };

            let mut notes = notes.write().unwrap();
            let track = &mut notes[curr_track];
            
            for (i, note) in track.iter_mut().enumerate() {
                this.change_note_and_update_deltas(lua, &func, note, i)?;
            }
            Ok(())
        });

        methods.add_method_mut("for_each_selected", |lua, this, func: Function| {
            let curr_track: usize = lua.globals().get("curr_track")?;
            
            let (notes, sel_ids) = {
                let note_editing = this.note_editing.lock().unwrap();
                (note_editing.get_notes().clone(), note_editing.get_selected_note_ids().clone())
            };

            let mut notes = notes.write().unwrap();
            let track = &mut notes[curr_track];

            let sel_ids = sel_ids.lock().unwrap();

            for &sel_id in sel_ids.iter() {
                let note = &mut track[sel_id];
                this.change_note_and_update_deltas(lua, &func, note, sel_id)?;
            }

            Ok(())
        });

        methods.add_method("iter_selected", |lua, this, func: Function| {
            let curr_track: usize = lua.globals().get("curr_track")?;

            let (notes, sel_ids) = {
                let note_editing = this.note_editing.lock().unwrap();
                (note_editing.get_notes().clone(), note_editing.get_selected_note_ids().clone())
            };

            let notes = notes.read().unwrap();
            let track = &notes[curr_track];

            let sel_ids = sel_ids.lock().unwrap();
            for &sel_id in sel_ids.iter() {
                let note = &track[sel_id];
                this.call_lua_note_fn(lua, &func, note)?;
            }

            Ok(())
        });

        // inclusive: if note lengths are considered
        methods.add_method::<_, _, Option<mlua::Table>>("get_selection_tick_range", |lua, this, inclusive: bool| {
            let curr_track: usize = lua.globals().get("curr_track")?;

            let (notes, sel_ids) = {
                let note_editing = this.note_editing.lock().unwrap();
                (note_editing.get_notes().clone(), note_editing.get_selected_note_ids().clone())
            };

            let sel_ids = sel_ids.lock().unwrap();
            if sel_ids.is_empty() { return Ok(None); }

            let notes = notes.read().unwrap();
            let track = &notes[curr_track];

            let (min_tick, max_tick) = if inclusive {
                get_min_max_ticks_in_selection(track, &sel_ids).unwrap()
            } else {
                let start_tick = track[sel_ids[0]].start();
                let end_tick = track[sel_ids[sel_ids.len() - 1]].start();
                (start_tick, end_tick)
            };

            let table = Self::range_as_lua_table(lua, min_tick, max_tick)?;

            Ok(Some(table))
        });

        methods.add_method::<_, _, Option<mlua::Table>>("get_selection_key_range", |lua, this, _: ()| {
            let curr_track: usize = lua.globals().get("curr_track")?;

            let (notes, sel_ids) = {
                let note_editing = this.note_editing.lock().unwrap();
                (note_editing.get_notes().clone(), note_editing.get_selected_note_ids().clone())
            };

            let sel_ids = sel_ids.lock().unwrap();
            if sel_ids.is_empty() { return Ok(None); }

            let notes = notes.read().unwrap();
            let track = &notes[curr_track];

            let (min_key, max_key) = get_min_max_keys_in_selection(track, &sel_ids).unwrap();

            let table = Self::range_as_lua_table(lua, min_key, max_key)?;

            Ok(Some(table))
        });

        methods.add_method_mut("create_note", |_, this, (start, length, channel, key, velocity): (MIDITick, MIDITick, u8, u8, u8)| {
            this.notes_to_add.push(Note {
                start,
                length,
                channel,
                key,
                velocity
            });
            Ok(())
        });
    }
}