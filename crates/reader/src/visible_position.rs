use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisibleSourceRange {
    pub lid: String,
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadingTextPosition {
    pub start: u32,
    pub end: u32,
    pub text: String,
}

/// Device pagination reports canonical source, never display page numbers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisibleReadingPosition {
    pub lid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_position: Option<ReadingTextPosition>,
    pub ranges: Vec<VisibleSourceRange>,
}

impl Reader {
    fn validate_visible_position(&self, book: &Book, position: &VisibleReadingPosition) -> Result<usize, ToolError> {
        let invalid = || ToolError { error_code: "INVALID_RANGE".into(), category: "validation".into(), message: "可见读位必须是按原文顺序排列的连续范围，且锚点属于首段".into() };
        let first = position.ranges.first().ok_or_else(invalid)?;
        if first.lid != position.lid { return Err(invalid()); }
        let mut previous: Option<(usize, u32, u32)> = None;
        let mut first_index = 0;
        for range in &position.ranges {
            let index = self.leaf_lids.iter().position(|lid| lid == &range.lid).ok_or_else(invalid)?;
            let text = book.text(&range.lid, None)?;
            let units: Vec<u16> = text.encode_utf16().collect();
            if range.start >= range.end || range.end as usize > units.len()
                || String::from_utf16(&units[range.start as usize..range.end as usize]).is_err() { return Err(invalid()); }
            if let Some((last_index, last_end, last_length)) = previous {
                if !(index == last_index && range.start == last_end || index == last_index + 1 && last_end == last_length && range.start == 0) { return Err(invalid()); }
            } else {
                first_index = index;
                if let Some(anchor) = &position.text_position {
                    if anchor.start != range.start || anchor.end <= anchor.start || anchor.end > range.end
                        || String::from_utf16(&units[anchor.start as usize..anchor.end as usize]).ok().as_deref() != Some(anchor.text.as_str()) { return Err(invalid()); }
                }
            }
            previous = Some((index, range.end, units.len() as u32));
        }
        Ok(first_index)
    }

    pub fn visible_position(&self) -> Option<&VisibleReadingPosition> { self.visible_position.as_ref() }

    /// Uses the same asynchronous read ledger as navigation, limited to the
    /// confirmed visible leaves. This command does not change logical layout.
    pub fn confirm_visible_position(&mut self, book: &Book, store: &mut MemoryStore, position: VisibleReadingPosition, now: &str) -> Result<ViewportEffect, ToolError> {
        let index = self.validate_visible_position(book, &position)?;
        if store.private_storage_available() {
            let mut last = None;
            for range in &position.ranges {
                if last != Some(&range.lid) { store.enqueue_read(&book.base.book_id, &range.lid, now)?; last = Some(&range.lid); }
            }
        }
        self.top_idx = index;
        self.visible_position = Some(position);
        self.selection = None;
        self.revision += 1;
        Ok(ViewportEffect { ok: true, viewport: self.viewport() })
    }

    pub fn restore_visible_position(&mut self, book: &Book, position: &VisibleReadingPosition) {
        if let Ok(index) = self.validate_visible_position(book, position) {
            self.top_idx = index;
            self.visible_position = Some(position.clone());
        }
    }
}
