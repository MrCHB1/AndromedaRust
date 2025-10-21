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

pub fn merge_notes_and_preserve_deltas(notes_1: Vec<Note>, notes_2: Vec<Note>, deltas: Vec<(SignedMIDITick, i16)>) -> (Vec<Note>, Vec<usize>, Vec<(SignedMIDITick, i16)>) {
    let mut notes_1_iter = notes_1.into_iter().peekable();
    let mut notes_2_iter = notes_2.into_iter().zip(deltas).peekable();

    let mut merged = Vec::with_capacity(notes_1_iter.size_hint().0 + notes_2_iter.size_hint().0);
    
    let mut deltas = Vec::with_capacity(notes_2_iter.size_hint().0);
    let mut new_ids = Vec::with_capacity(notes_2_iter.size_hint().0);
    let mut write_idx = 0;

    loop {
        match (notes_1_iter.peek(), notes_2_iter.peek()) {
            (Some(n1), Some((n2, _))) => {
                if n1.start() <= n2.start() {
                    let note = notes_1_iter.next().unwrap();
                    merged.push(note);
                } else {
                    let (note, delta) = notes_2_iter.next().unwrap();
                    merged.push(note);
                    deltas.push(delta);
                    new_ids.push(write_idx);
                }
                write_idx += 1;
            },
            (Some(_), None) => {
                merged.extend(notes_1_iter.by_ref());
                break;
            },
            (None, Some(_)) => {
                while let Some((note, delta)) = notes_2_iter.next() {
                    merged.push(note);
                    deltas.push(delta);
                    new_ids.push(write_idx);
                    write_idx += 1;
                }
                break;
            },
            (None, None) => { break; }
        }
    }

    (merged, new_ids, deltas)
}

/// Returns: 1) The notes that were extracted. 2) The original notes with the extracted notes removed.
pub fn extract_notes(src: Vec<Note>, ids: &Vec<usize>) -> (Vec<Note>, Vec<Note>) {
    let mut extracted = Vec::with_capacity(ids.len());
    let mut new_notes = Vec::with_capacity(src.len() - ids.len());

    let mut ids_idx = 0;
    for (id, note) in src.into_iter().enumerate() {
        if ids_idx < ids.len() && id == ids[ids_idx] {
            extracted.push(note);
            ids_idx += 1;
            continue;
        } else {
            new_notes.push(note);
        }
    }

    (extracted, new_notes)
}

pub fn extract_notes_and_remap_ids(src: Vec<Note>, ids: &Vec<usize>, ids_to_remap: Vec<usize>) -> (Vec<Note>, Vec<Note>, Vec<usize>) {
    let mut extracted = Vec::with_capacity(ids.len());
    let mut new_notes = Vec::with_capacity(src.len() - ids.len());
    let mut new_ids = Vec::with_capacity(ids_to_remap.len());

    let mut extract_idx = 0;

    for (id, note) in src.into_iter().enumerate() {
        if extract_idx < ids.len() && id == ids[extract_idx] {
            extracted.push(note);
            extract_idx += 1;
            continue;
        } else {
            new_notes.push(note);
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

    (extracted, new_notes, new_ids)
}

pub fn move_each_note_by(notes_with_ids: Vec<Note>, dt_pos: &Vec<(SignedMIDITick, i16)>) -> Vec<(Note, (SignedMIDITick, i16))> {
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