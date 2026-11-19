pub use reader_service::session_recap::*;
use crate::*;
pub(crate) fn network(
    access: &authorization::Authorization,
    context: &authorization::AuthorizedContext,
    query: &HashMap<String, String>,
    now: &str,
) -> Result<Recap, ToolError> {
    let (id, through) = request(query)?;
    let user = context.user.lock().unwrap();
    user.check_owner(context.user_id())?;
    let projection = snapshot(&user, id, through)?;
    let mut recap = project(&projection, now);
    let mut library = access.library.lock().unwrap();
    let mut materials = BTreeMap::new();
    resolve(&mut recap, &projection, &user, |_, reference| {
        let reference = reference?;
        materials
            .entry(reference.clone())
            .or_insert_with(|| {
                library
                    .load(context.user_id(), reference)
                    .ok()
                    .map(|p| (p.book.clone(), p.directory().to_path_buf()))
            })
            .clone()
    });
    Ok(recap)
}
