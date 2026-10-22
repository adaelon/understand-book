// Shared read/runtime regression fixture, using the user's optimization textbook excerpts.
fn optimization_math_book() -> read_tools::Book {
    let (base, source, index) = optimization_math_material();
    read_tools::Book::new(base, &source)
        .with_math_structure(Some(serde_json::from_value(index).unwrap()))
}

fn optimization_math_material() -> (base_schema::ReadOnlyBase, String, serde_json::Value) {
    use base_schema::{LidNode, NodeKind, Span};
    use serde_json::json;
    let original = include_str!("../../../packages/core/test/fixtures/optimization-context.md")
        .replace("\r\n", "\n");
    let paragraph = |prefix: &str| {
        original
            .split("\n\n")
            .find(|p| p.starts_with(prefix))
            .unwrap()
            .to_owned()
    };
    let theorem = paragraph("定理 5.3");
    let boundary = theorem.find("如果").unwrap();
    let pieces = vec![
        theorem[..boundary].to_owned(),
        theorem[boundary..].to_owned(),
        paragraph("$$\n\\nabla"),
        paragraph("证明. 任取"),
        paragraph("定理 6.5"),
        paragraph("$$\n2\\left"),
        paragraph("推论 6.2"),
        paragraph("#### 习题 6.3"),
        paragraph("利用定理6.5"),
    ];
    let source = pieces.join("\n\n");
    let mut base = base_schema::sample_base();
    base.book_id = "optimization-math".into();
    base.graph_nodes.clear();
    base.graph_edges.clear();
    base.lid_nodes = vec![LidNode {
        lid: "1".into(),
        path: vec![1],
        kind: NodeKind::Chapter,
        span: Span {
            start: 0,
            end: source.encode_utf16().count(),
        },
        children: (1..=pieces.len()).map(|n| format!("1.{n}")).collect(),
    }];
    let mut start = 0;
    for (i, piece) in pieces.iter().enumerate() {
        let end = start + piece.encode_utf16().count();
        base.lid_nodes.push(LidNode {
            lid: format!("1.{}", i + 1),
            path: vec![1, (i + 1) as u32],
            kind: if piece.starts_with("$$") {
                NodeKind::Formula
            } else {
                NodeKind::Paragraph
            },
            span: Span { start, end },
            children: vec![],
        });
        start = end + 2;
    }
    let index = json!({"version":"math_structure.v1","header":{
        "book_id":base.book_id,"book_version":"v1","profile_id":"technical_learning",
        "profile_version":"technical_learning_v0","core_schema_version":"core_v0","generated_at":"1970-01-01T00:00:00.000Z"
    },"items":[
        {"anchor_lid":"1.1","kind":"theorem","label":"定理 5.3","evidence_lids":["1.1","1.2","1.3"],
         "assumptions":[{"quote":"假设 f 在全空间  $R^n$ 可微.","evidence_lids":["1.1"]},
                        {"quote":"如果  $x^*$ 是一个局部极小点","evidence_lids":["1.2"]}],"references":[]},
        {"anchor_lid":"1.4","kind":"proof","evidence_lids":["1.4"],"assumptions":[],"references":[
            {"relation":"proves","target_kind":"theorem","target_label":"定理 5.3","scope":"local","quote":"证明.",
             "evidence_lids":["1.1","1.4"],"resolution":"resolved","target_lid":"1.1"}]},
        {"anchor_lid":"1.5","kind":"theorem","label":"定理 6.5","evidence_lids":["1.5","1.6"],
         "assumptions":[{"quote":"在假设6.1的条件下","evidence_lids":["1.5"]}],"references":[
            {"relation":"uses","target_kind":"assumption","target_label":"假设6.1","scope":"local","quote":"在假设6.1的条件下",
             "evidence_lids":["1.5"],"resolution":"missing"}]},
        {"anchor_lid":"1.7","kind":"corollary","label":"推论 6.2","evidence_lids":["1.7"],"assumptions":[],"references":[]},
        {"anchor_lid":"1.8","kind":"exercise","label":"习题 6.3","evidence_lids":["1.8","1.9"],"assumptions":[],"references":[
            {"relation":"uses","target_kind":"theorem","target_label":"定理6.5","scope":"local","quote":"利用定理6.5证明推论6.2.",
             "evidence_lids":["1.9"],"resolution":"resolved","target_lid":"1.5"},
            {"relation":"asks_to_prove","target_kind":"corollary","target_label":"推论6.2","scope":"local","quote":"利用定理6.5证明推论6.2.",
             "evidence_lids":["1.9"],"resolution":"resolved","target_lid":"1.7"}]}
    ]});
    (base, source, index)
}
