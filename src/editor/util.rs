#![warn(unused)]
use eframe::egui::Ui;

use crate::{editor::navigation::{PianoRollNavigation, TrackViewNavigation}, midi::events::{meta_event::{MetaEvent, MetaEventType}, note::Note}};
use std::{cmp::Ordering, collections::{HashMap}, path::PathBuf, sync::{Arc, Mutex}};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering as AtomicOrdering};

pub type MIDITick = u32;
pub type SignedMIDITick = i32;

pub trait AtomicMIDITick: Copy + Send + Sync + 'static {
    type Atomic: Send + Sync;

    fn new(val: Self) -> Self::Atomic;
    fn load(atom: &Self::Atomic, ord: AtomicOrdering) -> Self;
    fn store(atom: &Self::Atomic, val: Self, ord: AtomicOrdering);
}

impl AtomicMIDITick for u32 {
    type Atomic = AtomicU32;

    fn new(val: Self) -> Self::Atomic {
        AtomicU32::new(val)
    }

    fn load(atom: &Self::Atomic, ord: AtomicOrdering) -> Self {
        atom.load(ord)
    }

    fn store(atom: &Self::Atomic, val: Self, ord: AtomicOrdering) {
        atom.store(val, ord)
    }
}

impl AtomicMIDITick for u64 {
    type Atomic = AtomicU64;

    fn new(val: Self) -> Self::Atomic {
        AtomicU64::new(val)
    }

    fn load(atom: &Self::Atomic, ord: AtomicOrdering) -> Self {
        atom.load(ord)
    }

    fn store(atom: &Self::Atomic, val: Self, ord: AtomicOrdering) {
        atom.store(val, ord)
    }
}

pub type MIDITickAtomic = <MIDITick as AtomicMIDITick>::Atomic;

// binary searches within a given channel and track, returns an index
pub fn bin_search_notes(notes: &Vec<Note>, tick: MIDITick) -> usize {
    if notes.is_empty() { return 0; }
    
    let mut low = 0;
    let mut high = notes.len();

    if tick <= notes[low].start { return 0; }
    if tick >= notes[high - 1].start { return high; }

    while low < high {
        let mid = (low + high) / 2;
        if notes[mid].start <= tick {
            low = mid + 1;
        } else {
            high = mid;
        }
    }

    low
    /*if low == notes.len() {
        notes.len() - 1
    } else {
        low
    }*/
}

// used for getting the exact index of the nearest/last note
pub fn bin_search_notes_exact(notes: &Vec<Note>, tick: MIDITick) -> usize {
    if notes.is_empty() { return 0; }

    let mut low = 0;
    let mut high = notes.len() - 1;

    if tick <= notes[low].start { return 0; }
    if tick >= notes[high].start { return high; }

    while low < high {
        let mid = (low + high) / 2;
        if notes[mid].start < tick {
            low = mid + 1;
        } else  {
            high = mid;
        }
    }

    low
}

pub fn get_notes_in_range(notes: &Vec<Note>, min_tick: MIDITick, max_tick: MIDITick, min_key: u8, max_key: u8, include_ends: bool) -> Vec<usize> {
    let mut note_ids = Vec::new();
    // skip selecting entirely if sel_notes is blank
    if notes.is_empty() { return note_ids; }

    // skip selecting if the selection area isn't within the note tick range
    {
        let low_note = notes.first().unwrap();
        let high_note = notes.last().unwrap();

        if min_tick < low_note.start && max_tick < low_note.start { return note_ids; }
        if min_tick > high_note.start + high_note.length && max_tick > high_note.start + high_note.length { return note_ids; }
    }
    
    // TODO: replace this with binary search lol
    for (i, note) in notes.iter().enumerate() {
        if (note.start >= min_tick || (note.start + note.length > min_tick && include_ends)) && 
            note.start < max_tick &&
            note.key >= min_key && note.key <= max_key {
                note_ids.push(i);
            }
        if note.start > max_tick { break; } // we don't need to select further
    }

    note_ids
}

pub fn find_note_at(notes: &Vec<Note>, tick_pos: MIDITick, key_pos: u8) -> Option<usize> {
    if notes.is_empty() { return None; }

    let mut low = 0;
    let mut high = notes.len();

    // early checks
    {
        let lower = &notes[low];
        let upper = &notes[high - 1];
        if tick_pos < lower.start { return None; }
        if tick_pos > upper.start + upper.length && tick_pos > lower.start + lower.length { return None; }
    }

    while low < high {
        let mid = (low + high) / 2;
        if notes[mid].start <= tick_pos {
            low = mid + 1;
        } else {
            high = mid;
        }
    }

    //if low == 0 { return None; }

    for (i, note) in notes[0..low].iter().enumerate().rev() {
        if note.key == key_pos && note.start <= tick_pos && note.start + note.length >= tick_pos {
            return Some(i);
        }
    }

    for (i, note) in notes[low..notes.len()].iter().enumerate() {
        if note.key == key_pos && note.start <= tick_pos && note.start + note.length >= tick_pos {
            return Some(i + low);
        }
    }
    /*let mut low = 0;
    let mut high = notes.len() - 1;
    
    // early checks
    {
        let lower = &notes[low];
        let upper = &notes[high];
        if tick_pos < lower.start { return None; } // tick pos is too low
        if tick_pos > upper.start && tick_pos > upper.start + upper.length { return None; } // tick pos is too high
        //if lower.start > tick_pos && lower.start + lower.length > tick_pos { return None; } // note is too late to search
        //if upper.start < tick_pos && upper.start + upper.length < tick_pos { return None; } // last note is too early to search
    }

    // let mut possible_overlaps: Vec<usize> = Vec::with_capacity(notes.len());

    // pass 1: tick-wise
    while low <= high {
        let mid = (low + high) / 2;
        let note = &notes[mid];

        // start AND end of note is lower than tick pos
        if note.start < tick_pos && note.start + note.length < tick_pos {
            low = mid + 1;
            continue;
        }

        // start AND end of note is higher
        if note.start > tick_pos && note.start + note.length > tick_pos {
            high = mid - 1;
            continue;
        }

        if note.start <= tick_pos && note.start + note.length >= tick_pos {
            if note.key == key_pos { return Some(mid); } // note found!
            println!("possible overlap at {}", mid);
            break;
        }
    }

    // pass 2: linear search
    for (i, note) in notes[low..].iter().enumerate() {
        if note.start > tick_pos { break; } // we searched too far, break early
        if note.key == key_pos && note.start + note.length >= tick_pos {
            return Some(i + low);
        }
    }*/

    None
}

pub fn get_min_max_keys_in_selection(notes: &Vec<Note>, ids: &Vec<usize>) -> Option<(u8, u8)> {
    if ids.is_empty() { None }
    else {
        let mut min_key = 127u8;
        let mut max_key = 0u8;
        for id in ids {
            let note = &notes[*id];
            if min_key >= note.key { min_key = note.key; }
            if max_key <= note.key { max_key = note.key; }
        }
        Some((min_key, max_key))
    }
}

pub fn get_min_max_ticks_in_selection(notes: &Vec<Note>, ids: &Vec<usize>) -> Option<(MIDITick, MIDITick)> {
    if ids.is_empty() { return None; }

    let min_tick = notes[ids[0]].start;
    let max_tick = get_absolute_max_tick_from_ids(notes, ids).unwrap();

    Some((min_tick, max_tick))
}

pub fn get_absolute_max_tick_from_ids(notes: &Vec<Note>, ids: &Vec<usize>) -> Option<MIDITick> {
    if ids.is_empty() { return None; }

    let mut max_tick = 0;
    for &id in ids.iter() {
        let note = &notes[id];
        if note.start() + note.length() >= max_tick {
            max_tick = note.start() + note.length();
        }
    }

    Some(max_tick)
    // let mut assumed_max_tick = notes[last_idx].start + notes[last_idx].length;

    // refine the assumed last tick starting from the end
    /*for id in ids.iter().rev() {
        let note = notes[*id];
        if note.start + note.length > assumed_max_tick { assumed_max_tick = note.start + note.length; }
        if note.start < notes[last_idx].start && note.start + note.length < notes[last_idx].start { return Some(assumed_max_tick); }
    }*/

    // return Some(assumed_max_tick);
}

// helper function for moving/modifying the ticks of notes lol
pub fn manipulate_note_ticks(notes: &mut Vec<Note>, ids: &Vec<usize>, start_fn: impl Fn(MIDITick) -> MIDITick) -> (Vec<usize>, Vec<usize>, Vec<(SignedMIDITick, i16)>) {
    let ids_with_delta_pos: Vec<(usize, (SignedMIDITick, i16))> = ids.iter().map(|&id| {
        let note = &mut notes[id];
        let old_start = note.start();
        *(note.start_mut()) = start_fn(note.start());
        let new_start = note.start();
        (id, (new_start as SignedMIDITick - old_start as SignedMIDITick, 0))
    }).collect();

    move_notes_to(notes, ids_with_delta_pos)
}

/// Moves notes by [`ids_with_delta_pos`]. Returns two [`Vec<usize>`]s for ID preservation (usually utilized in undoing/redoing).
/// This should only be called when notes have already moved but need to be re-sorted.
/// [`ids_with_delta_pos`] must already be sorted by IDs and have no duplicate IDs.
pub fn move_notes_to(notes: &mut Vec<Note>, ids_with_delta_pos: Vec<(usize, (SignedMIDITick, i16))>) -> (Vec<usize>, Vec<usize>, Vec<(SignedMIDITick, i16)>) {
    let total = notes.len();
    let moved_count = ids_with_delta_pos.len();

    let old_notes = std::mem::take(notes);

    let mut kept = Vec::with_capacity(total - moved_count);
    let mut moved = Vec::with_capacity(moved_count);

    let mut ids_iter = ids_with_delta_pos.iter();
    let mut next = ids_iter.next();

    for (idx, note) in old_notes.into_iter().enumerate() {
        if let Some(&(move_id, (start_delta, key_delta))) = next {
            if idx == move_id {
                moved.push((move_id, note, start_delta, key_delta));
                next = ids_iter.next();
                continue;
            }
        }

        kept.push(note);
    }

    moved.sort_unstable_by_key(|&(_, note, _, _)| note.start());
    
    let mut kept_iter = kept.into_iter().peekable();
    let mut moved_iter = moved.into_iter().peekable();

    let mut merged = Vec::with_capacity(total);
    let mut old_ids = Vec::with_capacity(moved_count);
    let mut new_ids = Vec::with_capacity(moved_count);
    let mut changed_positions = Vec::with_capacity(moved_count);

    let mut write_idx = 0;

    loop {
        match (kept_iter.peek(), moved_iter.peek()) {
            (Some(k), Some((_, mnote, _, _))) => {
                if k.start() <= mnote.start() {
                    merged.push(kept_iter.next().unwrap());
                    write_idx += 1;
                } else {
                    let (old_id, note, start_delta, key_delta) = moved_iter.next().unwrap();
                    old_ids.push(old_id);
                    new_ids.push(write_idx);
                    changed_positions.push((start_delta, key_delta));
                    merged.push(note);
                    write_idx += 1;
                }
            }
            (Some(_), None) => {
                merged.extend(kept_iter.by_ref());
                break;
            }
            (None, Some(_)) => {
                while let Some((old_id, note, start_delta, key_delta)) = moved_iter.next() {
                    old_ids.push(old_id);
                    new_ids.push(write_idx);
                    changed_positions.push((start_delta, key_delta));
                    merged.push(note);
                    write_idx += 1;
                }
                break;
            }
            (None, None) => { break; }
        }
    }

    *notes = merged;
    
    (old_ids, new_ids, changed_positions)
}

pub fn manipulate_note_lengths(notes: &mut Vec<Note>, ids: &Vec<usize>, length_fn: impl Fn(MIDITick) -> MIDITick) -> Vec<SignedMIDITick> {
    let mut changed_lengths = Vec::new();

    for id in ids.iter() {
        let note = &mut notes[*id];
        let old_length = note.length;
        let new_length = length_fn(old_length);

        changed_lengths.push(new_length as SignedMIDITick - old_length as SignedMIDITick);
        note.length = new_length;
    }

    changed_lengths
}

pub fn move_element<T>(v: &mut Vec<T>, from: usize, to: usize) {
    match from.cmp(&to) {
        Ordering::Less => v[from..=to].rotate_left(1),
        Ordering::Greater => v[to..=from].rotate_right(1),
        Ordering::Equal => {}
    }
}

pub fn decode_note_group(note_group: u32) -> (u16, u8) {
    ((note_group >> 8) as u16, (note_group & 0xF) as u8)
}

pub fn mul_rgb(rgb: u32, val: f32) -> u32 {
    let r = ((rgb & 0xFF0000) >> 16) as f32 * val;
    let g = ((rgb & 0xFF00) >> 8) as f32 * val;
    let b = (rgb & 0xFF) as f32 * val;
    return (((r as u32) & 0xFF) << 16) | (((g as u32) & 0xFF) << 8) | ((b as u32) & 0xFF);
}

pub fn get_meta_next_tick(metas: &Vec<MetaEvent>, meta_type: MetaEventType, tick: MIDITick) -> Option<&MetaEvent> {
    for meta in metas.iter() {
        if meta.event_type == meta_type {
            if meta.tick < tick { continue; }
            return Some(meta);
        }
    }
    None
}

pub fn get_mouse_midi_pos(ui: &mut Ui, nav: &Arc<Mutex<PianoRollNavigation>>) -> ((MIDITick, u8), (MIDITick, u8)) {
    let rect = ui.min_rect();
    if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
        let (mut mouse_x, mut mouse_y) = (mouse_pos.x, mouse_pos.y);

        mouse_x = (mouse_x - rect.left()) / rect.width();
        mouse_y = 1.0 - (mouse_y - rect.top()) / rect.height();

        let nav = nav.lock().unwrap();

        let mouse_tick_pos = (mouse_x * nav.zoom_ticks_smoothed + nav.tick_pos_smoothed) as MIDITick;
        let (mouse_key_pos_rounded, mouse_key_pos) = (
            (mouse_y * nav.zoom_keys_smoothed + nav.key_pos_smoothed).round() as u8,
            (mouse_y * nav.zoom_keys_smoothed + nav.key_pos_smoothed) as u8
        );

        ((mouse_tick_pos, mouse_key_pos), (mouse_tick_pos, mouse_key_pos_rounded))
    } else {
        ((0, 0), (0, 0))
    }
}

pub fn get_mouse_track_view_pos(ui: &mut Ui, nav: &Arc<Mutex<TrackViewNavigation>>) -> (MIDITick, u16) {
    let rect = ui.min_rect();
    if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
        let (mut mouse_x, mut mouse_y) = (mouse_pos.x, mouse_pos.y);

        mouse_x = (mouse_x - rect.left()) / rect.width();
        mouse_y = (mouse_y - rect.top()) / rect.height();

        let nav = nav.lock().unwrap();

        let mouse_tick_pos = (mouse_x * nav.zoom_ticks_smoothed + nav.tick_pos_smoothed) as MIDITick;
        let mouse_track_pos = (mouse_y * nav.zoom_tracks_smoothed + nav.track_pos_smoothed) as u16;

        (mouse_tick_pos, mouse_track_pos)
    } else {
        (0, 0)
    }
}

pub fn path_rel_to_abs(path: String) -> PathBuf {
    std::path::absolute(path).unwrap()
}

pub fn tempo_as_bytes(tempo: f32) -> [u8; 3] {
    let tempo_conv = (60000000.0 / tempo) as u32;
    return [
        ((tempo_conv >> 16) & 0xFF) as u8,
        ((tempo_conv >> 8) & 0xFF) as u8,
        (tempo_conv & 0xFF) as u8
    ];
}

pub fn bytes_as_tempo(bytes: &[u8]) -> f32 {
    let bytes_conv = ((bytes[0] as u32) << 16) | ((bytes[1] as u32) << 8) | (bytes[2] as u32);
    return 60000000.0 / (bytes_conv as f32);
}