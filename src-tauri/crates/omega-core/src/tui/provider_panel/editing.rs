//! Text-buffer editing helpers (P5 split from provider_panel/logic.rs).

pub fn insert_char(buf: &mut String, cursor: &mut usize, c: char) {
    let pos = (*cursor).min(buf.len());
    buf.insert(pos, c);
    *cursor = pos + c.len_utf8();
}

pub fn backspace(buf: &mut String, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }
    let prev = buf[..*cursor]
        .char_indices()
        .last()
        .map(|(i, _)| i)
        .unwrap_or(0);
    buf.drain(prev..*cursor);
    *cursor = prev;
}

pub fn cursor_left(buf: &str, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }
    *cursor = buf[..*cursor]
        .char_indices()
        .last()
        .map(|(i, _)| i)
        .unwrap_or(0);
}

pub fn cursor_right(buf: &str, cursor: &mut usize) {
    if *cursor >= buf.len() {
        return;
    }
    if let Some((i, _)) = buf[*cursor..].char_indices().nth(1) {
        *cursor += i;
    } else {
        *cursor = buf.len();
    }
}
