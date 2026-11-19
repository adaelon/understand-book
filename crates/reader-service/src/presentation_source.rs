//! Exact source operations over an immutable presentation's entrypoint.
use runtime::presentation_author::PresentationTextEdit;
use serde_json::{json, Value};

pub(super) fn read(source: &str, offset: usize, length: Option<usize>) -> Result<Value, String> {
    let total = source.chars().count();
    if offset > total || length == Some(0) {
        return Err("Use offset within the file and a positive length".into());
    }
    let count = length.unwrap_or(total - offset).min(total - offset);
    let text: String = source.chars().skip(offset).take(count).collect();
    Ok(
        json!({"text":text,"offset":offset,"end_offset":offset+count,
        "total_characters":total,"chunk_characters":count,"next_offset":null}),
    )
}

pub(super) fn search(
    source: &str,
    query: &str,
    offset: usize,
    max_matches: Option<usize>,
) -> Result<Value, String> {
    let max_matches = max_matches.unwrap_or(20);
    if query.is_empty() || !(1..=50).contains(&max_matches) {
        return Err("search requires a nonempty query and max_matches between 1 and 50".into());
    }
    let chars: Vec<char> = source.chars().collect();
    if offset > chars.len() {
        return Err("Offset exceeds file length".into());
    }
    let start_byte = source
        .char_indices()
        .nth(offset)
        .map_or(source.len(), |(at, _)| at);
    let mut matches = Vec::new();
    let mut next_offset = None;
    for (relative, _) in source[start_byte..].match_indices(query) {
        let at = start_byte + relative;
        let char_offset = source[..at].chars().count();
        if matches.len() == max_matches {
            next_offset = Some(char_offset);
            break;
        }
        let context_start = char_offset.saturating_sub(120);
        let context_end = (char_offset + query.chars().count().min(200) + 120).min(chars.len());
        matches.push(json!({"offset":char_offset,"line":source[..at].bytes().filter(|b| *b == b'\n').count()+1,
            "context_offset":context_start,"context_end":context_end,
            "text":chars[context_start..context_end].iter().collect::<String>()}));
    }
    Ok(
        json!({"query":query,"offset":offset,"matches":matches,"next_offset":next_offset,"total_characters":chars.len()}),
    )
}

pub(super) fn patch(source: &str, edits: &[PresentationTextEdit]) -> Result<String, String> {
    if edits.is_empty() {
        return Err("patch requires at least one edit".into());
    }
    let mut updated = source.to_owned();
    for (index, edit) in edits.iter().enumerate() {
        if edit.old_text.is_empty() {
            return Err(format!("Edit {index}: old_text must not be empty"));
        }
        let Some(at) = updated.find(&edit.old_text) else {
            return Err(format!(
                "Edit {index}: old_text not found; read/search the selected base and retry"
            ));
        };
        if updated.rfind(&edit.old_text) != Some(at) {
            return Err(format!(
                "Edit {index}: old_text matches more than once; include more surrounding source"
            ));
        }
        updated.replace_range(at..at + edit.old_text.len(), &edit.new_text);
    }
    Ok(updated)
}
