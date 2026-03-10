use crate::midi::events::meta_event::MetaEvent;

pub fn merge_metas(metas_1: Vec<MetaEvent>, metas_2: Vec<MetaEvent>) -> Vec<MetaEvent> {
    let mut notes_1_iter = metas_1.into_iter().peekable();
    let mut notes_2_iter = metas_2.into_iter().peekable();

    let mut merged = Vec::with_capacity(notes_1_iter.size_hint().0 + notes_2_iter.size_hint().0);

    loop {
        match (notes_1_iter.peek(), notes_2_iter.peek()) {
            (Some(n1), Some(n2)) => {
                let note = if n1.tick <= n2.tick {
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