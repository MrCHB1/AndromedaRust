use crate::{editor::util::{MIDITick, SignedMIDITick}, midi::events::note::Note};

pub fn remove_note(src: &mut Vec<Note>, id: usize) -> Note {
    src.remove(id)
}

pub fn merge_notes(notes_1: Vec<Note>, notes_2: Vec<Note>) -> Vec<Note> {
    let mut notes_1_iter = notes_1.into_iter().peekable();
    let mut notes_2_iter = notes_2.into_iter().peekable();

    let mut merged = Vec::with_capacity(notes_1_iter.size_hint().0 + notes_2_iter.size_hint().0);

    loop {
        match (notes_1_iter.peek(), notes_2_iter.peek()) {
            (Some(n1), Some(n2)) => {
                let note = if n1.start() <= n2.start() {
                    notes_1_iter.next().unwrap()
                } else {
                    notes_2_iter.next().unwrap()
                };

                merged.push(note);
            }
            (Some(_), None) => {
                merged.extend(notes_1_iter.by_ref());
                break;
            }
            (None, Some(_)) => {
                merged.extend(notes_2_iter.by_ref());
                break;
            }
            (None, None) => { break; }
        }
    }

    merged
}

/// Like [`merge_notes`], but returns the indices of where each note in [`notes_2`] got inserted in [`notes_1`].
pub fn merge_notes_and_return_ids(notes_1: Vec<Note>, notes_2: Vec<Note>) -> (Vec<Note>, Vec<usize>) {
    let mut notes_1_iter = notes_1.into_iter().peekable();
    let mut notes_2_iter = notes_2.into_iter().peekable();

    let mut merged = Vec::with_capacity(notes_1_iter.size_hint().0 + notes_2_iter.size_hint().0);
    
    let mut ids = Vec::with_capacity(notes_2_iter.size_hint().0);
    let mut write_idx = 0;

    loop {
        match (notes_1_iter.peek(), notes_2_iter.peek()) {
            (Some(n1), Some(n2)) => {
                let note = if n1.start() <= n2.start() {
                    notes_1_iter.next().unwrap()
                } else {
                    ids.push(write_idx);
                    notes_2_iter.next().unwrap()
                };

                merged.push(note);
                write_idx += 1;
            },
            (Some(_), None) => {
                merged.extend(notes_1_iter.by_ref());
                break;
            },
            (None, Some(_)) => {
                while let Some(n2) = notes_2_iter.next() {
                    ids.push(write_idx);
                    merged.push(n2);
                    write_idx += 1;
                }
                break;
            },
            (None, None) => { break; }
        }
    }

    (merged, ids)
}

/// Returns: 1) The elements that were extracted. 2) The original array with the extracted elements removed.
pub fn extract<T>(src: Vec<T>, ids: &[usize]) -> (Vec<T>, Vec<T>) {
    let mut extracted = Vec::with_capacity(ids.len());
    let mut new_arr = Vec::with_capacity(src.len() - ids.len());

    let mut ids_idx = 0;
    for (id, elem) in src.into_iter().enumerate() {
        match ids.get(ids_idx) {
            Some(&maybe_id) if id == maybe_id => {
                extracted.push(elem);
                ids_idx += 1;
            }
            _ => new_arr.push(elem)
        }
    }

    (extracted, new_arr)
}

pub fn extract_with<T, U>(src: Vec<T>, ids: &[usize], arr: Vec<U>) -> (Vec<(T, U)>, Vec<T>) {
    assert_eq!(ids.len(), arr.len(), "ids and arr must have the same length");

    let mut extracted = Vec::with_capacity(ids.len());
    let mut new_arr = Vec::with_capacity(src.len() - ids.len());
    
    let mut ids_iter = ids.iter();
    let mut next_id = ids_iter.next();
    let mut arr_iter = arr.into_iter();

    for (i, elem) in src.into_iter().enumerate() {
        match next_id {
            Some(&target) if i == target => {
                let paired = arr_iter.next().unwrap();
                extracted.push((elem, paired));
                next_id = ids_iter.next();
            }
            _ => new_arr.push(elem)
        }
    }

    (extracted, new_arr)
}

pub fn extract_and_remap_ids<T>(src: Vec<T>, ids: &[usize], ids_to_remap: Vec<usize>) -> (Vec<T>, Vec<T>, Vec<usize>) {
    let mut extracted = Vec::with_capacity(ids.len());
    let mut new_arr = Vec::with_capacity(src.len() - ids.len());
    let mut new_ids = Vec::with_capacity(ids_to_remap.len());

    let mut extract_idx = 0;

    for (id, elem) in src.into_iter().enumerate() {
        if extract_idx < ids.len() && id == ids[extract_idx] {
            extracted.push(elem);
            extract_idx += 1;
            continue;
        } else {
            new_arr.push(elem);
        }
    }

    let mut extract_idx = 0;

    for &id in ids_to_remap.iter() {
        while extract_idx < ids.len() && ids[extract_idx] < id {
            extract_idx += 1;
        }
        if extract_idx < ids.len() && id == ids[extract_idx] {
            extract_idx += 1;
            continue;
        }
        new_ids.push(id - extract_idx);
    }

    (extracted, new_arr, new_ids)
}

pub fn move_each_note_by(notes_with_ids: Vec<Note>, dt_pos: &[(SignedMIDITick, i16)]) -> Vec<(Note, (SignedMIDITick, i16))> {
    let mut tmp = Vec::with_capacity(notes_with_ids.len());

    for (mut note, (dt_tick, dt_key)) in notes_with_ids.into_iter().zip(dt_pos) {
        let orig_start = note.start() as SignedMIDITick;
        let orig_key = note.key() as i16;

        let mut new_start = orig_start + dt_tick;
        if new_start < 0 { new_start = 0; }

        let mut new_key = orig_key + dt_key;
        if new_key < 0 { new_key = 0; }
        else if new_key > 127 { new_key = 127; }

        *(note.start_mut()) = new_start as MIDITick;
        *(note.key_mut()) = new_key as u8;

        let new_dt_tick = new_start - orig_start;
        let new_dt_key = new_key - orig_key;

        tmp.push((note, (new_dt_tick, new_dt_key)));
    }

    tmp.sort_by_key(|(n, _)| n.start());
    tmp
}

pub fn move_all_notes_by(notes: Vec<Note>, dt_pos: (SignedMIDITick, i16)) -> Vec<Note> {
    let (dt_tick, dt_key) = dt_pos;

    let mut tmp = Vec::with_capacity(notes.len());

    for mut note in notes.into_iter() {
        let orig_start = note.start() as SignedMIDITick;
        let orig_key = note.key() as i16;

        let mut new_start = orig_start + dt_tick;
        if new_start < 0 { new_start = 0; }

        let mut new_key = orig_key + dt_key;
        if new_key < 0 { new_key = 0; }
        else if new_key > 127 { new_key = 127; }

        (*note.start_mut()) = new_start as MIDITick;
        (*note.key_mut()) = new_key as u8;

        // let new_dt_tick = new_start - orig_start;
        // let new_dt_key = new_key - orig_key;

        tmp.push(note);
    }

    tmp.sort_by_key(|&n| n.start());
    tmp
}

pub fn get_first_from_ids<'a, T>(arr: &'a [T], ids: &[usize]) -> &'a T {
    &arr[ids[0]]
}