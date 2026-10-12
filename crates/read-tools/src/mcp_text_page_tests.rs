use super::*;

fn book(parts: &[&str]) -> Book {
    let mut start = 0;
    let nodes = parts
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let end = start + text.encode_utf16().count();
            let node = LidNode {
                lid: (i + 1).to_string(),
                path: vec![(i + 1) as u32],
                kind: NodeKind::Paragraph,
                span: Span { start, end },
                children: vec![],
            };
            start = end;
            node
        })
        .collect();
    Book::new(
        ReadOnlyBase {
            book_id: "pages".into(),
            lid_nodes: nodes,
            graph_nodes: vec![],
            graph_edges: vec![],
        },
        &parts.concat(),
    )
}

#[test]
fn mcp_text_page_reassembles_cross_lid_unicode_source_with_exact_coordinates() {
    let left = "甲".repeat(7_999) + "😀";
    let right = "乙🦀\n".repeat(6_000);
    let b = book(&["未选😀", &left, "\n\n", &right, "未选尾"]);
    let expected = b.text("2", Some("4")).unwrap();
    let source: Vec<_> = ["未选😀", &left, "\n\n", &right, "未选尾"]
        .concat()
        .encode_utf16()
        .collect();
    let mut next = None;
    let mut cursor = "未选😀".encode_utf16().count();
    let mut joined = String::new();
    let mut pages = 0;
    loop {
        let page = b.text_page("2", Some("4"), next).unwrap();
        assert_eq!(page.source_range.start, cursor);
        assert_eq!(
            page.text,
            String::from_utf16(&source[cursor..page.source_range.end]).unwrap()
        );
        assert!(page.text.chars().count() <= MCP_TEXT_PAGE_CHARS);
        if pages == 0 {
            assert_eq!(page.text, left);
            assert_eq!(page.source_range.end - cursor, 8_001);
        }
        cursor = page.source_range.end;
        joined.push_str(&page.text);
        pages += 1;
        match page.next_start_utf16 {
            Some(offset) => {
                assert_eq!(offset, cursor);
                next = Some(offset);
            }
            None => {
                assert_eq!(cursor, page.requested_range.end);
                break;
            }
        }
    }
    assert_eq!(joined, expected);
    assert!(pages >= 4);
}

#[test]
fn mcp_text_page_handles_exact_limit_empty_source_and_last_page() {
    for length in [0, 1, 7_999, 8_000, 8_001, 16_000] {
        let source = "😀".repeat(length);
        let b = book(&[&source]);
        let first = b.text_page("1", None, None).unwrap();
        assert_eq!(first.text.chars().count(), length.min(8_000));
        if length <= 8_000 {
            assert!(first.next_start_utf16.is_none());
            assert_eq!(first.text, source);
        } else {
            let last = b.text_page("1", None, first.next_start_utf16).unwrap();
            assert_eq!(first.text + &last.text, source);
            assert!(last.next_start_utf16.is_none());
        }
    }
}

#[test]
fn mcp_text_page_rejects_reversed_out_of_range_and_split_surrogate_positions() {
    let b = book(&["先", "😀后", "尾"]);
    for (lid, end, start) in [
        ("2", Some("1"), None),
        ("2", None, Some(0)),
        ("2", None, Some(2)),
        ("2", None, Some(4)),
        ("2", None, Some(usize::MAX)),
    ] {
        assert!(b.text_page(lid, end, start).is_err());
    }
    assert_eq!(
        b.text_page("missing", None, None).unwrap_err().error_code,
        "LID_NOT_FOUND"
    );
    assert_eq!(b.text_page("2", None, Some(3)).unwrap().text, "后");
}
