use super::*;
use super::mu5_tests::Fixture;

fn scoped_scene(f: &Fixture, goal: &str, attachment: &str) -> Value {
    let workspace = f.ok(&f.a, "POST", "/api/learning-workspaces", json!({
        "goal":{"text":goal}, "members":[f.x]
    }));
    let scene = f.ok(&f.a, "POST", "/api/workspaces", json!({
        "published_book_ref":f.x, "attachment_id":attachment,
        "learning_workspace_id":workspace["learning_workspace_id"]
    }));
    f.action(&f.a, &scene, attachment, "chat/new", json!({}))
}

fn profile(f: &Fixture, scene: &Value, attachment: &str) -> Value {
    f.action(&f.a, &f.get(&f.a, scene), attachment, "profile/memory", json!({}))["result"].clone()
}

fn apply(f: &Fixture, scene: &Value, attachment: &str, action: Value) -> Value {
    let revision = profile(f, scene, attachment)["status"]["document_revision"].as_u64().unwrap();
    f.action(&f.a, &f.get(&f.a, scene), attachment, "profile/memory/apply", profile_mutation(revision, action))["result"].clone()
}

#[test]
fn w3_profile_api_binds_local_facts_to_workspace_and_preserves_global_preferences() {
    let f = Fixture::new();
    let a = scoped_scene(&f, "学习深度学习", "learning-a");
    let b = scoped_scene(&f, "期末考试冲刺", "learning-b");
    let local = apply(&f, &a, "learning-a", json!({
        "kind":"remember", "operation_id":"workspace-local", "evidence_text":"本工作区需要逐步推导梯度",
        "fact":profile_fact_draft("book", "depth", "逐步推导梯度", "normal")
    }));
    assert_eq!(local["outcome"]["kind"], "remembered");
    let local_id = local["outcome"]["fact_ids"][0].as_str().unwrap().to_owned();
    let global = apply(&f, &a, "learning-a", json!({
        "kind":"remember", "operation_id":"stable-global", "evidence_text":"我一直喜欢先看简要概览",
        "fact":profile_fact_draft("global", "structure", "先看简要概览", "normal")
    }));
    let global_id = global["outcome"]["fact_ids"][0].as_str().unwrap().to_owned();
    let principal = f.access.auth.authenticate(&f.a, crate::multi_user_host::now()).unwrap();
    let context = f.access.context(principal, crate::multi_user_host::now()).unwrap();
    {
        let user = context.user.lock().unwrap();
        let fact = user.store.profile_facts().iter().find(|fact| fact.fact_id == local_id).unwrap();
        assert!(matches!(&fact.scope, ProfileScope::Workspace { learning_workspace_id, book_id }
            if Some(learning_workspace_id.as_str()) == a["learning_workspace_id"].as_str() && book_id == &f.x.book_id));
    }
    let view_a = profile(&f, &a, "learning-a");
    assert!(view_a["facts"].as_array().unwrap().iter().any(|fact| fact["fact_id"] == local_id));
    assert!(view_a["snapshot"].to_string().contains("逐步推导梯度"));
    let view_b = profile(&f, &b, "learning-b");
    assert_eq!(view_b["facts"].as_array().unwrap().len(), 1);
    assert_eq!(view_b["facts"][0]["fact_id"], global_id);
    assert!(view_b["snapshot"].to_string().contains("先看简要概览"));
    assert!(!view_b.to_string().contains("逐步推导梯度"));
    assert!(!view_b["evidence"].as_array().unwrap().iter().any(|e| e["fact_id"] == local_id));

    for (operation, action) in [
        ("cross-correct", json!({"kind":"correct", "fact_id":local_id, "payload_value":"另一工作区覆写", "evidence_text":"覆盖", "valid_until":null})),
        ("cross-forget", json!({"kind":"forget", "fact_id":local_id})),
    ] {
        let revision = profile(&f, &b, "learning-b")["status"]["document_revision"].as_u64().unwrap();
        let mut action = action;
        action["operation_id"] = json!(operation);
        let mut input = profile_mutation(revision, action);
        let stamp = Fixture::stamp(&f.get(&f.a, &b), "learning-b");
        input.as_object_mut().unwrap().extend(stamp.as_object().unwrap().clone());
        let (status, error) = f.call(&f.a, "POST", &format!("/api/workspaces/{}/profile/memory/apply", b["workspace_id"].as_str().unwrap()), input);
        assert_eq!(status, 400, "{error}");
        assert_eq!(error["error_code"], "INVALID_MEMORY_SCOPE");
    }

    // Returning to A must use A's scope, even after B populated the user's cache.
    let corrected = apply(&f, &a, "learning-a", json!({
        "kind":"correct", "operation_id":"local-correct", "fact_id":local_id,
        "payload_value":"先解释方向导数，再推导梯度", "evidence_text":"按新的顺序讲解", "valid_until":null
    }));
    assert_eq!(corrected["outcome"]["kind"], "corrected");
    let corrected_id = corrected["outcome"]["fact_ids"][0].as_str().unwrap().to_owned();
    assert!(profile(&f, &a, "learning-a")["snapshot"].to_string().contains("先解释方向导数，再推导梯度"));
    assert!(!profile(&f, &b, "learning-b").to_string().contains("先解释方向导数，再推导梯度"));
    let forgotten = apply(&f, &a, "learning-a", json!({
        "kind":"forget", "operation_id":"local-forget", "fact_id":corrected_id
    }));
    assert_eq!(forgotten["outcome"]["kind"], "forgotten");
    assert!(!profile(&f, &a, "learning-a")["snapshot"].to_string().contains("先解释方向导数，再推导梯度"));
    assert_eq!(profile(&f, &b, "learning-b")["facts"][0]["fact_id"], global_id);
}
