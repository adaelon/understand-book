// Execution facts from saved EX2 requests/responses; content correctness is checked separately.
import fs from 'node:fs';
import path from 'node:path';

for (const directory of process.argv.slice(2)) {
  const files = fs.readdirSync(directory).sort();
  const read = name => JSON.parse(fs.readFileSync(path.join(directory, name), 'utf8'));
  const responses = files.filter(name => /^response-\d+\.json$/.test(name)).map(read);
  const requests = files.filter(name => /^request-\d+\.json$/.test(name)).map(read);
  const calls = responses.flatMap(response => response.tool_calls ?? []);
  const author = calls.filter(call => call.name === 'presentation.author').map(call => JSON.parse(call.arguments));
  const receipts = new Map();
  for (const request of requests) for (const message of request.messages ?? []) {
    if (message.role !== 'Tool') continue;
    let envelope;
    try { envelope = JSON.parse(message.content); } catch { continue; }
    if (envelope.model_body?.reading) receipts.set(message.tool_call_id, {
      status: envelope.status, truncated: envelope.truncated,
      candidate_id: envelope.model_body.candidate_id,
      environment: envelope.model_body.environment_name,
      reading: envelope.model_body.reading,
    });
  }
  const outcome = files.includes('outcome.json') ? read('outcome.json') : null;
  const summary = files.includes('summary.json') ? read('summary.json') : null;
  const report = {
    directory, summary, completed: !!outcome, hasView: files.includes('view.json'),
    incomplete: outcome?.incomplete ?? null, warning: outcome?.warning ?? null,
    tokens: responses.reduce((sum, response) => sum + (response.usage ?? 0), 0),
    unknownUsage: responses.filter(response => response.usage == null).length + Math.max(0, requests.length - responses.length),
    revisions: [...new Set(requests.flatMap(request => (request.instruction_assets ?? []).filter(asset => asset.asset_id === 'resident-agent.skill.presentation-method').map(asset => asset.revision)))],
    operations: Object.fromEntries(['write', 'preview', 'deliver'].map(operation => [operation, author.filter(call => call.operation === operation).length])),
    selectedReads: author.filter(call => call.operation === 'preview' && call.read_selector).length,
    modelVisibleReadings: [...receipts.values()],
  };
  fs.writeFileSync(path.join(directory, 'audit.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify({ ...report, modelVisibleReadings: report.modelVisibleReadings.length }, null, 2));
}
