use super::{TeachBackBookQuote, TeachBackPassage};
use std::collections::HashSet;

// Show the model an exact sentence from its existing passage when it miscopies a quote.
// Choosing a replacement still goes through the same exact-quote and range checks.
pub(super) fn missing_quote(quote: &TeachBackBookQuote, read: &[TeachBackPassage]) -> String {
    let mut message = format!(
        "Quote not found in the supplied UTF-16 range of {}: {:?}.",
        quote.lid, quote.quote
    );
    if let Some(other) = read
        .iter()
        .find(|p| p.lid != quote.lid && super::passage_has_quote(p, &quote.quote))
    {
        message.push_str(&format!(
            " These exact words were supplied under LID {}. If that is your source, keep the words and correct the LID.",
            other.lid
        ));
        return message;
    }
    let draft: Vec<char> = quote.quote.chars().collect();
    let fragments: HashSet<&[char]> = draft.windows(4).collect();
    let candidate = read
        .iter()
        .filter(|page| page.lid == quote.lid)
        .flat_map(|page| {
            let limit = page
                .source_range_utf16
                .end
                .saturating_sub(page.source_range_utf16.start);
            let mut units = 0;
            let end = page
                .text
                .char_indices()
                .find_map(|(byte, ch)| {
                    units += ch.len_utf16();
                    (units > limit).then_some(byte)
                })
                .unwrap_or(page.text.len());
            page.text[..end]
                .split_inclusive(['。', '！', '？', '!', '?', '\n'])
                .map(str::trim)
        })
        .filter_map(|sentence| {
            let chars: Vec<char> = sentence.chars().collect();
            let overlap = chars
                .windows(4)
                .filter(|part| fragments.contains(*part))
                .count();
            (overlap > 0).then_some((overlap, sentence))
        })
        .max_by_key(|(overlap, _)| *overlap);
    if let Some((_, sentence)) = candidate {
        message.push_str(&format!(
            " If it supports the relation, copy all or part of this exact excerpt from the same LID:\n{sentence}"
        ));
    }
    message
}
