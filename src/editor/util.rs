use crate::midi::events::note::Note;
use std::{cmp::Ordering, collections::{HashMap, VecDeque}};

// binary searches within a given channel and track, returns an index
pub fn bin_search_notes(notes: &Vec<Note>, tick: u32) -> usize {
    let sel_notes = notes;
    if sel_notes.is_empty() { return 0; }
    
    let mut low = 0;
    let mut high = sel_notes.len();

    if tick < notes[low].start { return 0; }
    if tick > notes[high - 1].start { return high; }

    while low < high {
        let mid = (low + high) / 2;
        if sel_notes[mid].start < tick {
            low = mid + 1;
        } else {
            high = mid;
        }
    }

    if low == sel_notes.len() {
        sel_notes.len() - 1
    } else {
        low
    }
}

// used for getting the exact index of the nearest/last note
pub fn bin_search_notes_exact(notes: &Vec<Note>, tick: u32) -> usize {
    if notes.is_empty() { return 0; }

    let mut low = 0;
    let mut high = notes.len() - 1;

    if tick < notes[low].start { return 0; }
    if tick > notes[high].start { return high; }

    while low < high {
        let mid = (low + high) / 2;
        if notes[mid].start < tick {
            low = mid + 1;
        } else if notes[mid].start > tick {
            if mid == 0 { break; }
            high = mid - 1;
        } else {
            return mid;
        }
    }

    high
}

pub fn get_notes_in_range(notes: &Vec<Note>, min_tick: u32, max_tick: u32, min_key: u8, max_key: u8, include_ends: bool) -> Vec<usize> {
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

pub fn find_note_at(notes: &Vec<Note>, tick_pos: u32, key_pos: u8) -> Option<usize> {
    if notes.is_empty() { return None; }

    let mut low = 0;
    let mut high = notes.len() - 1;
    
    // early checks
    {
        let lower = &notes[low];
        let upper = &notes[high];
        if lower.start > tick_pos && lower.start + lower.length > tick_pos { return None; } // note is too late to search
        if upper.start < tick_pos && upper.start + upper.length < tick_pos { return None; } // last note is too early to search
    }

    // pass 1: tick-wise
    while low <= high {
        let mid = (low + high) / 2;
        let note = &notes[mid];

        // start AND end of note is lower than tick pos
        if note.start < tick_pos && note.start + note.length < tick_pos {
            low = mid + 1;
            continue;
        }

        // start AND end of note is high than tick pos
        if note.start > tick_pos && note.start + note.length > tick_pos {
            high = mid - 1;
            continue;
        }

        // start is lower than tick pos but end is high than tick pos
        if note.start <= tick_pos && note.start + note.length >= tick_pos {
            if note.key == key_pos { return Some(mid); } // note found!
            break; // break early to linearly search between low and high
        }
    }

    if low > high { return None; }

    // pass 2: regular linear search because order of note key isn't kept in mind
    for (i, note) in notes[low..=high].iter().enumerate() {
        if note.key == key_pos {
            if note.start > tick_pos && note.start + note.length > tick_pos { continue; }
            return Some(i + low);
        }
    }

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

pub fn get_min_max_ticks_in_selection(notes: &Vec<Note>, ids: &Vec<usize>) -> Option<(u32, u32)> {
    if ids.is_empty() { return None; }

    let min_tick = notes[ids[0]].start;
    let max_tick = get_absolute_max_tick_from_ids(notes, ids).unwrap();

    Some((min_tick, max_tick))
}

pub fn get_absolute_max_tick_from_ids(notes: &Vec<Note>, ids: &Vec<usize>) -> Option<u32> {
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
pub fn manipulate_note_ticks(notes: &mut Vec<Note>, ids: &Vec<usize>, start_fn: impl Fn(u32) -> u32) -> (Vec<usize>, Vec<usize>, Vec<(i32, i32)>) {
    let mut changed_positions = Vec::new();

    let mut id_updates = Vec::new();
    let (mut old_ids, mut new_ids) = (Vec::new(), Vec::new());

    for id in ids.iter() {
        let note = &mut notes[*id];
        let old_start = note.start;
        let new_start = start_fn(old_start);

        changed_positions.push((new_start as i32 - old_start as i32, 0));
        old_ids.push(*id);
        id_updates.push((*id, new_start));
    }

    let mut notes_to_move = VecDeque::new();
    let mut rem_offset = 0;
    for (i, id) in ids.iter().enumerate() {
        let mut note = notes.remove(id - rem_offset);
        rem_offset += 1;
        note.start = id_updates[i].1;
        notes_to_move.push_front(note);
    }

    let mut id_compensation: HashMap<usize, usize> = HashMap::new();
    for &(_, new_start) in id_updates.iter() {
        let insert_idx = bin_search_notes(notes, new_start);
        let offset = id_compensation.entry(insert_idx).or_insert(0);
        let real_idx = insert_idx + *offset;

        new_ids.push(real_idx);
        notes.insert(insert_idx, notes_to_move.pop_back().unwrap());
        *offset += 1;
    }

    (old_ids, new_ids, changed_positions)
}

pub fn manipulate_note_lengths(notes: &mut Vec<Note>, ids: &Vec<usize>, length_fn: impl Fn(u32) -> u32) -> Vec<i32> {
    let mut changed_lengths = Vec::new();

    for id in ids.iter() {
        let note = &mut notes[*id];
        let old_length = note.length;
        let new_length = length_fn(old_length);

        changed_lengths.push(new_length as i32 - old_length as i32);
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