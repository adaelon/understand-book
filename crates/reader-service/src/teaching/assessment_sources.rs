//! Freeze only read material already bound to the selected teaching target.
use super::*;
use memory::assessment::{AssessmentSource, AssessmentVisual};

pub(super) fn freeze(
    contract: &AssessmentContract,
    target: Option<&TeachingTarget>,
    binding: &TeachingBinding,
    evidence: &[SourceBinding],
) -> Result<BTreeMap<String, AssessmentSource>, ToolError> {
    let mut sources = BTreeMap::new();
    for id in &contract.source_ref_ids {
        let source = evidence.iter().find(|source| source.source_ref_id == *id
            && source.book_id == binding.source_id
            && source.source_revision.as_deref() == Some(binding.source_revision.as_str())
            && source.valid_evidence())
            .ok_or_else(|| invalid("评分来源未在本轮读取并登记，或不属于当前资料版本"))?;
        let target = target.ok_or_else(|| invalid("媒体评分来源必须属于本次教学目标"))?;
        let (text, visual) = if let Some(music) = &source.music {
            if !target.music_bindings.iter().flatten().any(|saved| saved.target == music.target) {
                return Err(invalid("评分音乐来源不属于本次教学目标"));
            }
            (serde_json::to_string_pretty(&music.material).unwrap(), None)
        } else if let Some(media) = &source.media {
            if !target.media_bindings.iter().flatten().any(|saved| saved.target == media.target) {
                return Err(invalid("评分视频来源不属于本次教学目标"));
            }
            let visual = runtime::video_source::members(&media.target).iter()
                .any(|part| matches!(part, base_schema::EvidenceTarget::Visual { .. }))
                .then(|| AssessmentVisual { target: media.target.clone(), duration_ms: media.duration_ms });
            let text = serde_json::to_string_pretty(&json!({
                "transcript":media.transcript,"audio_reviews":media.audio_reviews
            })).unwrap();
            (text, visual)
        } else {
            return Err(invalid("文本评分请使用已完整读取的 source_lids"));
        };
        sources.insert(id.clone(), AssessmentSource { source_id: binding.source_id.clone(),
            source_revision: binding.source_revision.clone(), text, visual });
    }
    Ok(sources)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_assessment_freezes_music_material_and_rejects_unread_or_other_target_refs() {
        let binding=TeachingBinding {tutor_session_id:"tutor".into(),session_revision:1,control_revision:1,
            source_id:"music".into(),source_revision:"book-revision".into(),map_revision:None,chat_session_id:"chat".into(),turn_id:"turn".into()};
        let source:SourceBinding=serde_json::from_value(json!({"source_ref_id":"read-music","book_id":"music","publication_id":"original","source_revision":"book-revision",
            "label_snapshot":"Repeat","preview_snapshot":"Preview alone cannot grade the score",
            "music":{"target":{"publication":{"book_id":"music","publication_id":"original"},
                "audio":{"recording":{"recording_id":"recording","revision":"recording-revision"},"start_ms":1000,"end_ms":2000},
                "score":{"score":{"score_id":"score","revision":"score-revision"},"source_ids":["note1"]},
                "occurrence_ids":["repeat"],"event_ids":["return"],"provenance_ids":["analysis"]},
                "title":"Work","label":"Repeat","preview":"Theme returns","material":{"kind":"score","written_pitch":"C4"}}})).unwrap();
        let target=bind_target(json!({"learning_focus":"theme","expected_performance":"Compare returns","capability":"explanation",
            "source_bindings":[],"source_ref_ids":["read-music"]}),&binding,&[source.clone()]).unwrap();
        let contract:AssessmentContract=serde_json::from_value(json!({"rule":{"kind":"choice_set","choices":["A","B"],"correct":["A"]},
            "source_ref_ids":["read-music"],"source_sufficient":true,"feedback":"after_submit"})).unwrap();
        contract.validate().unwrap();
        let frozen=freeze(&contract,Some(&target),&binding,&[source.clone()]).unwrap();
        assert!(frozen["read-music"].text.contains("C4"));
        assert!(!frozen["read-music"].text.contains("Preview alone"));
        assert!(frozen["read-music"].visual.is_none());
        assert!(freeze(&contract,Some(&target),&binding,&[]).is_err());
        let mut other_source=source.clone();other_source.source_revision=Some("other-revision".into());
        assert!(freeze(&contract,Some(&target),&binding,&[other_source]).is_err());
        let mut other_target=target;other_target.music_bindings.as_mut().unwrap()[0].target.audio.start_ms=500.0;
        assert!(freeze(&contract,Some(&other_target),&binding,&[source]).is_err());
    }

    #[test]
    fn media_assessment_freezes_exact_visual_target_without_turning_preview_into_evidence() {
        let binding=TeachingBinding {tutor_session_id:"tutor".into(),session_revision:1,control_revision:1,
            source_id:"video".into(),source_revision:"book-revision".into(),map_revision:None,chat_session_id:"chat".into(),turn_id:"turn".into()};
        let source:SourceBinding=serde_json::from_value(json!({"source_ref_id":"read-frame","book_id":"video","publication_id":"original","source_revision":"book-revision",
            "label_snapshot":"Frame","preview_snapshot":"A description is not the frame",
            "media":{"target":{"kind":"visual","publication":{"book_id":"video","publication_id":"original"},
                "frames":[],"regions":[{"frame":{"video":{"media_id":"video","media_revision":"v1"},"actual_pts_ms":1000,"width":640,"height":480},"rect_normalized":[0.1,0.2,0.4,0.5]}]},
                "publication":{"book_id":"video","publication_id":"original"},"duration_ms":2000,"transcript":[],"translations":[],"observations":[]}})).unwrap();
        let target=bind_target(json!({"learning_focus":"slopes","expected_performance":"Compare A and B","capability":"explanation",
            "source_bindings":[],"source_ref_ids":["read-frame"]}),&binding,&[source.clone()]).unwrap();
        let contract:AssessmentContract=serde_json::from_value(json!({"rule":{"kind":"choice_set","choices":["A","B"],"correct":["A"]},
            "source_ref_ids":["read-frame"],"source_sufficient":true,"feedback":"after_submit"})).unwrap();
        let frozen=freeze(&contract,Some(&target),&binding,&[source.clone()]).unwrap();
        assert_eq!(frozen["read-frame"].visual.as_ref().unwrap().target,source.media.as_ref().unwrap().target);
        assert!(!frozen["read-frame"].text.contains("description"));
        assert!(freeze(&contract,Some(&target),&binding,&[]).is_err());
    }
}
