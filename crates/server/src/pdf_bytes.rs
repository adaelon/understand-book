//! Single byte ranges for PDF.js, read directly from the authorized publication file.
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

pub(crate) struct PdfBytes {
    pub status: u16,
    pub body: Vec<u8>,
    pub content_range: Option<String>,
}

// Unsupported/malformed range syntax falls back to the full representation.
// A syntactically valid but unsatisfiable single range returns 416.
fn interval(header: &str, length: u64) -> Option<Result<(u64, u64), ()>> {
    let spec = header.strip_prefix("bytes=")?;
    let (first, last) = spec.split_once('-')?;
    let number = |text: &str| -> Option<u64> {
        if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        text.parse().ok()
    };
    if first.is_empty() {
        let suffix = number(last)?;
        return Some(if suffix == 0 || length == 0 {
            Err(())
        } else {
            Ok((length.saturating_sub(suffix), length))
        });
    }
    let begin = number(first)?;
    let end = if last.is_empty() {
        length
    } else {
        let last = number(last)?;
        if last < begin {
            return None;
        }
        last.saturating_add(1).min(length)
    };
    Some(if begin >= length {
        Err(())
    } else {
        Ok((begin, end))
    })
}

pub(crate) fn read(path: &Path, range: Option<&str>) -> io::Result<PdfBytes> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    let selected = range.and_then(|header| interval(header, length));
    let (status, begin, end, content_range) = match selected {
        Some(Ok((begin, end))) => (
            206,
            begin,
            end,
            Some(format!("bytes {begin}-{}/{length}", end - 1)),
        ),
        Some(Err(())) => {
            return Ok(PdfBytes {
                status: 416,
                body: vec![],
                content_range: Some(format!("bytes */{length}")),
            })
        }
        None => (200, 0, length, None),
    };
    file.seek(SeekFrom::Start(begin))?;
    let mut body = Vec::new();
    file.take(end - begin).read_to_end(&mut body)?;
    Ok(PdfBytes {
        status,
        body,
        content_range,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pdf_ranges_only_read_the_requested_file_slice() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), b"0123456789").unwrap();
        for (header, status, bytes) in [
            (Some("bytes=2-4"), 206, &b"234"[..]),
            (Some("bytes=8-99"), 206, &b"89"[..]),
            (Some("bytes=-99"), 206, &b"0123456789"[..]),
            (Some("bytes=99-"), 416, &b""[..]),
            (Some("bytes=-0"), 416, &b""[..]),
            (Some("bytes=0-1,4-5"), 200, &b"0123456789"[..]),
            (Some("garbage"), 200, &b"0123456789"[..]),
            (None, 200, &b"0123456789"[..]),
        ] {
            let reply = read(file.path(), header).unwrap();
            assert_eq!((reply.status, reply.body.as_slice()), (status, bytes));
        }
    }
}
