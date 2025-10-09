/*use eframe::egui::Color32;

enum RichTextToken<'a> {
    Unformatted(&'a str),
    Bold(Vec<RichTextToken<'a>>),
    Italic(Vec<RichTextToken<'a>>),
    Underline(Vec<RichTextToken<'a>>),
    Code(Vec<RichTextToken<'a>>),
    Colored(Color32, Vec<RichTextToken<'a>>)
}

pub struct RichTextParser {}
impl RichTextParser {
    /// Takes in a [`str`] and returns an array of Rich text tokens.
    fn parse_str(text: &str) -> Vec<RichTextToken> {
        let mut tokens = Vec::new();
        let mut stack = Vec::new();
        let mut cursor = 0;

        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        
        while i < chars.len() {
            if chars[i] == '[' {
                if let Some(close) = chars[i..].iter().position(|&c| c == ']') {
                    let tag = &text[i+1..i+close];
                    if ["b", "i", "u"].contains(&tag) {
                        if cursor < i {
                            Self::push_text(&mut stack, &mut tokens, &text[cursor..i]);
                        }
                        stack.push((tag, Vec::new()));
                        i += close + 1;
                        cursor = i;
                        continue;
                    } else if tag.starts_with('/') {
                        let tname = &tag[1..];
                        if let Some((open_tag, children)) = stack.pop() {
                            if open_tag == tname {
                                if cursor < i {
                                    Self::push_text(&mut stack, &mut tokens, &text[cursor..i]);
                                }
                                let node = match open_tag {
                                    "b" => RichTextToken::Bold(children),
                                    "i" => RichTextToken::Italic(children),
                                    "u" => RichTextToken::Underline(children),
                                    _ => unreachable!()
                                };
                                if let Some((_, parent_children)) = stack.last_mut() {
                                    parent_children.push(node)
                                } else {
                                    tokens.push(node);
                                }
                                i += close + 1;
                                cursor = i;
                                continue;
                            }
                        }
                    }
                }
            }

            i += 1;
        }

        if cursor < text.len() {
            Self::push_text(&mut stack, &mut tokens, &text[cursor..]);
        }

        tokens
    }

    fn push_text<'a>(
        stack: &mut Vec<(&str, Vec<RichTextToken<'a>>)>,
        tokens: &mut Vec<RichTextToken<'a>>,
        text: &'a str,
    ) {
        if text.is_empty() {
            return;
        }
        let node = RichTextToken::Unformatted(text);
        if let Some((_, children)) = stack.last_mut() {
            children.push(node);
        } else {
            tokens.push(node);
        }
    }
}*/