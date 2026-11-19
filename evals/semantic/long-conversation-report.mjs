// Rebuild observations solely from already-completed samples. Do not call a model.
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { costCeiling, inputUsage } from './long-conversation-pair.mjs';

function parse(value) {
  if (typeof value !== 'string') return value;
  try {return JSON.parse(value);} catch {return null;}
}

export function samplingStatus(record) {
  if (!record || record.blocked || record.scripted || !record.reservation) return 'not_sent';
  if (record.status >= 400) return 'upstream_rejected';
  if (record.status >= 200 && record.status < 300 &&
      (inputUsage(record.usage) !== null || record.response?.choices?.length)) return 'sampled';
  return 'delivery_unknown';
}

export function actionEvidence(records) {
  const actions=[];
  const byId=new Map();
  for(const record of records) {
    const sampling=samplingStatus(record);
    for(const message of record.request?.messages ?? []) {
      if (message.role !== 'tool') continue;
      const action=byId.get(message.tool_call_id);
      if (!action || action.observed_in) continue;
      if (sampling !== 'sampled') {
        if (sampling === 'delivery_unknown') action.delivery_unknown_in ??= record.ordinal;
        continue;
      }
      const envelope=parse(message.content);
      action.observed_in=record.ordinal;
      action.status=envelope?.status ?? null;
      const body=envelope?.model_body ?? {};
      action.diagnostic=body.error_code ?? envelope?.receipt?.error_code ?? null;
      action.candidate=body.candidate_id ?? null;
      action.receipt_status=body.status ?? null;
    }
    if (sampling !== 'sampled') continue;
    for(const reply of record.response?.choices ?? []) {
      for(const call of reply.message?.tool_calls ?? []) {
        const args=parse(call.function.arguments);
        const action={call_id:call.id, request:record.ordinal, tool:call.function.name,
          operation:args?.operation ?? null, finish_reason:reply.finish_reason ?? null,
          complete_arguments:args !== null && typeof args==='object', status:null, observed_in:null};
        byId.set(call.id,action);
        actions.push(action);
      }
    }
  }
  return actions;
}

const read=(dir,filename) => {
  const file=path.join(dir,filename);
  return fs.existsSync(file) ? JSON.parse(fs.readFileSync(file,'utf8')) : null;
};

function summarize(dir) {
  const sample=read(dir,'sample.json'),records=read(dir,'provider.json');
  const outcome=read(dir,'outcome.json');
  const before=read(dir,'before-notes.json'),after=read(dir,'after-notes.json');
  const delivery=read(dir,'delivery.json');
  const actions=actionEvidence(records);
  const first_main_sampling=samplingStatus(records.find(r=>r.ordinal===sample.first_ordinal));
  const status = !outcome ? 'response_unavailable' : outcome.error_code || outcome.warning ||
    (outcome.incomplete ? 'incomplete' : 'complete');
  const errors=actions.filter(a=>a.status === 'error');
  const pages=delivery?.delivered ?? null;
  const rejected=errors.filter(a=>a.operation === 'deliver');
  const sends=records.filter(r=>r.reservation);
  const measured=sends.filter(r=>inputUsage(r.usage)!==null &&
    Number.isSafeInteger(r.usage?.completion_tokens ?? r.usage?.output_tokens));
  const pending=sends.filter(r=>!measured.includes(r));
  return {...sample, status, pages, first_main_sampling,host_observed_actions:outcome?.trace?.length ?? null,
    note_delta: Array.isArray(before) && Array.isArray(after) ? after.length-before.length : null,
    observed_tool_errors:errors.length, model_issued_tool_calls:actions.length,
    model_completed_tool_calls:actions.filter(a=>a.finish_reason==='tool_calls' && a.complete_arguments).length,
    tool_call_entries_in_truncated_responses:actions.filter(a=>a.finish_reason==='length').length,
    rejected_delivery_receipts_returned:rejected.length,
    delivered_after_rejected_delivery:rejected.length && pages !== null ? pages > 0 : null,
    runtime_no_progress:status === 'AGENT_NO_PROGRESS' || errors.some(a=>a.diagnostic === 'AGENT_NO_PROGRESS'),
    unknown_usage:pending.length, priced_reported_usage_micro_cny:measured.reduce((n,r)=>n+costCeiling(r),0),
    unknown_usage_reserved_micro_cny:pending.reduce((n,r)=>n+r.reservation,0),
    measured_input_tokens:measured.reduce((n,r)=>n+(inputUsage(r.usage) ?? 0),0),
    requests:records.map(r=>({ordinal:r.ordinal,purpose:r.purpose,input:inputUsage(r.usage),
      output:r.usage?.completion_tokens ?? r.usage?.output_tokens ?? null,
      reasoning:r.usage?.completion_tokens_details?.reasoning_tokens ?? null,
      finish_reason:r.response?.choices?.[0]?.finish_reason ?? null,
      status:r.status??null,stop:r.blocked??null,sampling:samplingStatus(r)})),
    actions,
  };
}

export function report(dir) {
  const run=read(dir,'run.json');
  const samples=run.samples.map(row=>({id:row.id,...summarize(path.join(dir,row.id))}));
  const comparisons=[];
  for (const repeat of [1,2]) for(const band of ['short','long']) {
    const pair=samples.filter(s=>s.repeat === repeat && s.band === band && s.first_main_sampling==='sampled');
    if (pair.length !== 2) continue;
    const [a,b]=pair;
    const requests=pair.map(s=>read(path.join(dir,s.id),'provider.json')
      .find(r=>r.ordinal===s.first_ordinal)?.request);
    const comparable=(fn)=>requests.every(Boolean) &&
      JSON.stringify(fn(requests[0]))===JSON.stringify(fn(requests[1]));
    const systemMessages=q=>q.messages.filter(m=>['system','developer'].includes(m.role));
    const withoutEventTimes=q=>systemMessages(q).map(m=>({...m,content:m.content
      .replace(/^revision=fnv1a64:[0-9a-f]{16}$/gm,'revision=[generated]')
      .replace(/"(updated_at|occurred_at)":"\d{13}"/g,'"$1":"[event time]"')}));
    comparisons.push({repeat,band,observed_band:pair.every(s=>s.band_status === 'matched'),
      equal_tool_names:Array.isArray(a.first_tools) && Array.isArray(b.first_tools) && JSON.stringify(a.first_tools) === JSON.stringify(b.first_tools),
      equal_tool_definitions:comparable(q=>q.tools),
      equal_sampling_limits:comparable(q=>[q.temperature,q.max_tokens ?? q.max_completion_tokens,q.tool_choice]),
      equal_system_messages:comparable(systemMessages),
      equal_system_except_event_times_and_revisions:comparable(withoutEventTimes),
      all_verbatim_rules_seen:pair.every(s=>s.verbatim_user_constraints && Object.values(s.verbatim_user_constraints).every(Boolean)),
      input_tokens:pair.map(s=>[s.model,s.first_input])});
  }
  const confirmed=samples.filter(s=>s.first_main_sampling==='sampled').length;
  return {samples,comparisons,confirmed_first_main_samples:confirmed,aborted_or_unstarted:(run.planned_samples ?? 8)-confirmed};
}

if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) {
  const dir=path.resolve(process.argv[2]);
  fs.writeFileSync(path.join(dir,'observations.json'),JSON.stringify(report(dir),null,2)+'\n');
}
