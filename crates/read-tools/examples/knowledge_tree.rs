//! Inspect a real immutable material without starting a model or a reader session.
use read_tools::Book;
use serde_json::json;
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert!(
        args.len() == 3,
        "knowledge_tree <directory> <publication-id or -> <output-json>"
    );
    let book = Book::load(&args[0]).expect("read material");
    let publication = (args[1] != "-").then(|| base_schema::PublishedBookRef {
        book_id: book.base.book_id.clone(),
        publication_id: args[1].clone(),
    });
    let start = std::time::Instant::now();
    let tree = book
        .knowledge_index(std::path::Path::new(&args[0]), publication.as_ref())
        .expect("project knowledge");
    let elapsed = start.elapsed().as_millis();
    let page: Vec<_> = tree.nodes["root"]
        .children
        .iter()
        .take(12)
        .map(|id| &tree.nodes[id])
        .collect();
    let page_bytes = serde_json::to_vec(&page).unwrap().len();
    let resident_budget_bytes = book.resident_budget_bytes();
    let entries: std::collections::BTreeMap<_, _> = tree.nodes.keys().map(|id| (id, json!({
        "entry_point":tree.entry_point(id).map(|n| &n.id),
        "introduction":tree.introduction(id).map(|n|json!({"text":n.summary,"is_opening":n.id != *id}))
    }))).collect();
    let value = json!({"source_id":book.base.book_id,"modality":tree.modality,"projection_ms":elapsed,"node_count":tree.nodes.len(),
        "first_page_bytes":page_bytes,"resident_budget_bytes":resident_budget_bytes,"nodes":tree.nodes,"entries":entries});
    std::fs::write(&args[2], serde_json::to_vec(&value).unwrap()).unwrap();
    println!(
        "{}",
        json!({"source_id":book.base.book_id,"modality":tree.modality,"projection_ms":elapsed,"node_count":tree.nodes.len(),"first_page_bytes":page_bytes,"resident_budget_bytes":resident_budget_bytes})
    );
}
