// Evidence extraction for the actual cloud fixture. It never supplies a review
// verdict or changes a model request, answer, rate, or account attribution.
export function isDeliveryReview(record) {
  return (record.request?.messages ?? []).some(message =>
    (message.role === 'system' || message.role === 'developer')
    && typeof message.content === 'string'
    && message.content.startsWith('Teaching delivery acceptance review.'));
}

export function deliveryReviewEvidence(records) {
  return records.filter(isDeliveryReview).map(record => {
    const input = record.request.messages.find(message => message.role === 'user')?.content;
    const output = record.response?.choices?.[0]?.message?.content;
    let packet, verdict, parse_error;
    try { packet = JSON.parse(input); verdict = JSON.parse(output); }
    catch (error) { parse_error = error.message; }
    return {
      provider_ordinal: record.ordinal,
      started_at: record.started_at,
      elapsed_ms: record.elapsed_ms,
      usage: record.usage,
      proposed_delivery: packet?.proposed_delivery,
      frozen_input: packet?.frozen_input,
      verdict,
      ...(parse_error ? { parse_error, raw_response: output } : {}),
    };
  });
}

export function reviewDeliveryValidation(row, records) {
  const reviews=deliveryReviewEvidence(records), last=reviews.at(-1);
  const revisionContexts=records.flatMap(record=>{
    const state=[...(record.request?.messages??[])].reverse().find(message=>
      typeof message.content==='string'&&message.content.startsWith('runtime_state_snapshot.v1'))?.content;
    const fragment=state?.split('key=agent.exam_answer_revision\n')[1]?.split('\n\nkey=')[0];
    if(!fragment)return [];
    const raw=fragment.split('\n').find(line=>line.startsWith('{'));
    try{return [{provider_ordinal:record.ordinal,context:JSON.parse(raw)}];}
    catch{return [{provider_ordinal:record.ordinal,parse_error:true,fragment}];}
  });
  const events=(row.stream_observation?.events??[]).map(item=>item.event);
  const finished=events.filter(event=>event.type==='model.finished'&&event.payload?.name==='tutor_delivery_review');
  const visible=events.filter(event=>event.type==='answer.patch'&&event.payload?.view);
  // Compare prose after removing only the documented canonical source markers.
  // Full source parts remain in the raw turn and undergo HTTP source resolution.
  const approvedText=last?.proposed_delivery?.text;
  const checks={
    review_executed:reviews.length>0,
    review_packets_parse:reviews.every(review=>!review.parse_error),
    final_review_accepted:last?.verdict?.decision==='accept',
    approved_prose_is_delivered:typeof approvedText==='string'
      &&approvedText.replace(/\[\[source:[^\]]+\]\]/g,'').trim()===row.answer?.trim(),
    every_emitted_rejection_reaches_author:reviews.filter(review=>review.verdict?.decision==='revise').every(review=>
      revisionContexts.some(item=>item.provider_ordinal>review.provider_ordinal
        &&item.context?.proposed_delivery?.text===review.proposed_delivery?.text
        &&item.context?.review?.decision==='revise')),
    stream_observed:row.stream_observation?.http_status===200&&!row.stream_observation?.error,
    every_review_observed_in_run:finished.length===reviews.length,
    answer_withheld_until_last_review:finished.length>0&&visible.length>0
      &&visible.every(event=>event.seq>finished.at(-1).seq),
  };
  return {checks,all_passed:Object.values(checks).every(Boolean),reviews,revision_contexts:revisionContexts,
    rejected_draft_count:reviews.filter(review=>review.verdict?.decision==='revise').length,
    revision_feedback_observed:revisionContexts.length>0,
    finished_review_event_sequences:finished.map(event=>event.seq),visible_answer_event_sequences:visible.map(event=>event.seq),
    semantic_review:'required; acceptance by the model reviewer does not prove factual correctness'};
}

export function billingEvidence(report, records) {
  const rows = report.admin_usage?.items ?? [];
  const runIds = report.cases.map(row => row.turn_id);
  const workspaceIds = report.cases.map(row => row.learning_workspace_id);
  const reviewRecords = records.filter(isDeliveryReview);
  const reviewCharges = rows.filter(row => row.purpose === 'tutor_delivery_review');
  const backgroundRecords=records.filter(record=>record.purpose==='background_profile_review');
  const foregroundRecords=records.filter(record=>record.purpose!=='background_profile_review');
  const runRows=rows.filter(row=>runIds.includes(row.run_ref));
  const backgroundRows=rows.filter(row=>row.purpose==='memory_review'&&row.run_ref===null);
  const usageTotal = rows.reduce((sum, row) => sum + (row.provider_usage?.total_tokens ?? 0), 0);
  const proxyTotal = records.reduce((sum, record) => sum + (record.usage?.total_tokens ?? 0), 0);
  const checks = {
    complete_ledger_page: report.admin_usage?.total === rows.length,
    one_ledger_entry_per_observed_call: rows.length === records.length && records.length > 0,
    unique_ledger_calls: new Set(rows.map(row => row.call_id)).size === rows.length,
    same_reader_account: rows.length > 0 && rows.every(row => row.user_id === 'A' && row.cost_owner === 'account'),
    foreground_same_accepted_runs: runRows.length===foregroundRecords.length
      &&runRows.every(row => row.parent_task_ref === row.run_ref),
    foreground_same_learning_workspaces: runRows.every(row => workspaceIds.includes(row.learning_workspace_id)),
    transport_scope_matches_ledger: rows.every(row => row.send_identity?.scope?.user_id === row.user_id
      &&(runIds.includes(row.run_ref)
        ?row.send_identity.scope.kind === 'reader_run'&&row.send_identity.scope.run_ref === row.run_ref
        :row.purpose==='memory_review'&&row.send_identity.scope.kind==='reader_task'
          &&row.send_identity.scope.task_ref===row.task_ref)
      && row.send_identity.call_id === row.call_id && row.send_identity.purpose === row.purpose),
    same_provider_and_model: rows.length > 0 && rows.every(row => !!row.send_identity?.provider && !!row.model)
      && new Set(rows.map(row => row.send_identity?.provider)).size === 1
      && new Set(rows.map(row => row.model)).size === 1,
    reservation_snapshot_present: rows.every(row => row.reservation_estimate
      && Number.isInteger(row.reserved_micro_cny) && row.reserved_micro_cny > 0),
    one_allowance_period: rows.length > 0 && rows.every(row => !!row.period_id)
      && new Set(rows.map(row => row.period_id)).size === 1,
    all_settled: rows.every(row => row.state === 'settled' && row.needs_reconciliation === false),
    provider_usage_present: rows.every(row => Number.isInteger(row.provider_usage?.total_tokens)),
    observed_and_billed_tokens_match: usageTotal === proxyTotal,
    delivery_review_calls_billed: reviewRecords.length === reviewCharges.length,
    delivery_review_tokens_match: reviewRecords.reduce((sum, r) => sum + (r.usage?.total_tokens ?? 0), 0)
      === reviewCharges.reduce((sum, r) => sum + (r.provider_usage?.total_tokens ?? 0), 0),
  };
  const foregroundChecks={
    calls_match:runRows.length===foregroundRecords.length&&runRows.length>0,
    same_run_and_workspace:checks.foreground_same_accepted_runs&&checks.foreground_same_learning_workspaces,
    all_settled:runRows.every(row=>row.state==='settled'&&row.needs_reconciliation===false),
    reported_tokens_match:runRows.reduce((sum,row)=>sum+(row.provider_usage?.total_tokens??0),0)
      ===foregroundRecords.reduce((sum,record)=>sum+(record.usage?.total_tokens??0),0),
    delivery_review_billed:checks.delivery_review_calls_billed&&checks.delivery_review_tokens_match,
  };
  return { checks, all_passed: Object.values(checks).every(Boolean), observed_calls: records.length,
    ledger_calls: rows.length, observed_tokens: proxyTotal, ledger_tokens: usageTotal,
    delivery_review_calls: reviewRecords.length, delivery_review_charge_ids: reviewCharges.map(row => row.call_id),
    foreground:{checks:foregroundChecks,all_passed:Object.values(foregroundChecks).every(Boolean),
      calls:foregroundRecords.length,tokens:foregroundRecords.reduce((sum,r)=>sum+(r.usage?.total_tokens??0),0)},
    background:{purpose:'memory_review',observed_calls:backgroundRecords.length,ledger_calls:backgroundRows.length,
      tokens:backgroundRecords.reduce((sum,r)=>sum+(r.usage?.total_tokens??0),0),
      ledger_states:backgroundRows.map(row=>row.state),
      attribution:'Existing account-level reader_task; related workspaces are references, not a nested answer-review Run.',
      settlement_observed:backgroundRows.length===backgroundRecords.length
        &&backgroundRows.every(row=>row.state==='settled'&&Number.isInteger(row.provider_usage?.total_tokens))},
    limitation: 'The isolated host uses synthetic admission rates. Matching usage and attribution do not establish actual supplier cost.' };
}
