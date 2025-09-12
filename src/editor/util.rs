use crate::midi::events::{meta_event::{MetaEvent, MetaEventType}, note::Note};
use std::{cmp::Ordering, collections::{HashMap, HashSet, VecDeque}};
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

    let last_idx = ids.len() - 1;
    let mut assumed_max_tick = notes[last_idx].start + notes[last_idx].length;

    // refine the assumed last tick starting from the end
    for id in ids.iter().rev() {
        let note = notes[*id];
        if note.start + note.length > assumed_max_tick { assumed_max_tick = note.start + note.length; }
        if note.start < notes[last_idx].start && note.start + note.length < notes[last_idx].start { return Some(assumed_max_tick); }
    }

    return Some(assumed_max_tick);
}

// helper function for moving/modifying the ticks of notes lol
pub fn manipulate_note_ticks(notes: &mut Vec<Note>, ids: &Vec<usize>, start_fn: impl Fn(MIDITick) -> MIDITick) -> (Vec<usize>, Vec<usize>, Vec<(SignedMIDITick, i16)>) {
    let mut updates: Vec<(usize, Note, SignedMIDITick, MIDITick)> = ids.iter().rev().map(|&id| {
        let mut note = notes.remove(id);
        let new_start = start_fn(note.start);
        let start_change = new_start as SignedMIDITick - note.start as SignedMIDITick;
        note.start = new_start;
        (id, note, start_change, new_start)
    }).collect();

    updates.sort_by_key(|&(_, _, _, new_start)| new_start);

    let mut ids_with_pos = Vec::new();

    let mut id_compensation = HashMap::new();
    for (i, (old_id, note, start_change, _)) in updates.into_iter().enumerate() {
        let insert_idx = bin_search_notes(notes, note.start);
        let offset = id_compensation.entry(insert_idx).or_insert(0);
        let real_idx = insert_idx + *offset;
        ids_with_pos.push(((old_id, real_idx), (start_change, 0)));
        notes.insert(insert_idx, note);
        *offset += 1;
    }

    ids_with_pos.sort_by_key(|(ids, _)| ids.1);
    let ((old_ids, new_ids), changed_positions) = ids_with_pos.into_iter().unzip();
    //let (old_ids, new_ids, changed_posiitons) = ids_with_pos.into_iter().unzip();
    
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