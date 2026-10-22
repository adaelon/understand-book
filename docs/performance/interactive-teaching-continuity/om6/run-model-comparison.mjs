// Bounded teaching-only comparison, using the existing provider recorder.
// Run from the repository root. Replays use the original Tutor tools separately.
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { startProviderRecorder, measuredUsage } from '../../../../evals/semantic/provider-recorder.mjs';

const here = path.resolve(process.env.OM6_OUTPUT ?? path.dirname(fileURLToPath(import.meta.url)));
const root = process.cwd();
const names = ['speed', 'borrowing', 'open-design'];
const save = (file, value) => {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify(value, null, 2) + '\n');
};
const read = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const mode = process.argv[2] ?? 'prepare';
const sourcePaths = ['crates/runtime/src/tutor.rs', 'crates/runtime/src/tutor_writing_rules.md'];

if (mode === 'prepare') {
  for (const variant of ['before', 'after']) {
    const sources = sourcePaths.map(file => variant === 'before'
      ? execFileSync('git', ['show', `HEAD:${file}`], { encoding: 'utf8', windowsHide: true })
      : fs.readFileSync(path.join(root, file), 'utf8'));
    fs.mkdirSync(path.join(here, variant), { recursive: true });
    sourcePaths.forEach((file, index) => fs.writeFileSync(path.join(here, variant, path.basename(file)), sources[index]));
    const modules = ['COMMON', 'DESIGN', 'SELECT', 'CONTINUATION', 'UNDERSTANDING'].map(name => {
      const match = sources[0].match(new RegExp(`const ${name}: &str = "([\\s\\S]*?)";`));
      if (!match) throw new Error(`Missing original module ${name}`);
      return { name, text: match[1] };
    });
    for (const name of names) {
      const packet = read(path.join(here, 'contexts', `${name}.json`));
      const full = packet.tool_contract.parameters;
      const contract = { ...full, properties: Object.fromEntries(
        ['operation', 'operation_id', 'design', 'move'].map(key => [key, full.properties[key]])),
        examples: undefined };
      const task = [
        'Act as the Tutor for this single turn. The packet contains the actual prepared context, original source and saved evidence.',
        'Return one tutor.step design argument JSON object. The surrounding replay will accept the design through the actual tool, then provide its receipt in the next sampling for select.',
        'Use the real evidence references in the packet when adopting evidence. The design_example is only the field shape; choose its content for this request.',
        'Do not invent new learner replies, model tool results, source citation IDs or completed page actions. No tools are executed during this content-generation call.',
        'All learner-visible text is Chinese. Return JSON only.',
      ].join('\n');
      save(path.join(here, variant, name, 'input.json'), {
        protocol: 'om6.teaching-only.v1', variant, name, task, guidance: modules,
        writing_rules: sources[1], contract, packet: { ...packet, tool_contract: undefined },
      });
    }
  }
  save(path.join(here, 'comparison.json'), {
    head: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8', windowsHide: true }).trim(),
    comparison: 'HEAD Tutor guidance and writing rules versus current guidance; identical persisted scene/source/request/context and tool contract in each pair',
    context_boundary: 'Both arms receive the current real context. This isolates guidance behavior, not an end-to-end old-binary comparison or a causal estimate for the continuity projection.',
    cases: names, calls_planned: 12, stages: ['design', 'select'], max_tokens_per_call: 8192, temperature: 0,
    thinking: 'disabled',
    budget_basis: 'Existing bounded teaching replay: six turns, each with design then select; no repeat after an accepted case.',
  });
  console.log('Prepared three paired packets; no provider calls.');
} else if (mode === 'run' || mode === 'select') {
  process.loadEnvFile(path.join(root, '.env'));
  const base = process.env.OPENCODE_BASE_URL;
  const key = process.env.OPENCODE_API_KEY;
  const model = process.env.FLUID_LLM_MODEL;
  if (!base || !key || !model) throw new Error('Existing provider is not configured');
  const recorder = await startProviderRecorder(base);
  const rows = [];
  try {
    for (const name of names) for (const variant of ['before', 'after']) {
      const dir = path.join(here, variant, name);
      // Resume only missing calls; recorded failures are not silently retried.
      const providerFile = mode === 'select' ? 'provider.json' : 'design-provider.json';
      if (fs.existsSync(path.join(dir, providerFile))) continue;
      const input = read(path.join(dir, 'input.json'));
      const next = mode === 'select' ? read(path.join(dir, 'stage2-input.json')) : null;
      const task = next ? [
        'Continue as Tutor at the next sampling. The previous design operation has actually been accepted; its receipt and current prepared context are supplied.',
        'Return one tutor.step select argument JSON object (operation and move), following the supplied tool contract.',
        'move.delivery_text is the exact final public teaching text. Use the accepted design_ref and block_refs from the receipt.',
        'Do not invent learner replies, source citation IDs or completed page actions. All learner-visible text is Chinese. Return JSON only.',
      ].join('\n') : input.task;
      const request = {
        model, temperature: 0, max_tokens: 8192, stream: false, thinking: { type: 'disabled' },
        response_format: { type: 'json_object' },
        messages: [
          { role: 'system', content: task + '\n\n' + input.guidance.map(v => v.text).join('\n\n') + '\n\n' + input.writing_rules },
          { role: 'user', content: JSON.stringify(next
            ? { contract: input.contract, ...next.packet, tool_contract: undefined, accepted_receipt: next.receipt }
            : { contract: input.contract, ...input.packet }) },
        ],
      };
      save(path.join(dir, mode === 'select' ? 'request.json' : 'design-request.json'), request);
      const response = await fetch(`${recorder.url}/chat/completions`, {
        method: 'POST', headers: { 'content-type': 'application/json', authorization: `Bearer ${key}` },
        body: JSON.stringify(request),
      });
      const publicResponse = await response.json();
      const record = recorder.records.at(-1);
      save(path.join(dir, providerFile), record);
      const content = publicResponse.choices?.[0]?.message?.content;
      const finish = publicResponse.choices?.[0]?.finish_reason;
      const row = { name, variant, model, status: response.status, finish_reason: finish,
        elapsed_ms: record.elapsed_ms, usage: record.usage };
      rows.push(row);
      save(path.join(here, 'model-progress.json'), rows);
      console.log(JSON.stringify(row));
      if (!response.ok || finish !== 'stop') throw new Error(`${variant}/${name} did not complete; see provider.json`);
      let output;
      try { output = JSON.parse(content); }
      catch { throw new Error(`${variant}/${name} returned invalid JSON; see provider.json`); }
      if (next) {
        save(path.join(dir, 'select-output.json'), output);
        if (output.operation !== 'select' || typeof output.move?.delivery_text !== 'string') {
          throw new Error(`${variant}/${name} did not return the expected selection; see select-output.json`);
        }
        const first = read(path.join(dir, 'design-output.json'));
        const combined = { design: { operation: first.operation, operation_id: first.operation_id, design: first.design },
          select: output, answer: output.move.delivery_text };
        save(path.join(dir, 'output.json'), combined);
        fs.writeFileSync(path.join(dir, 'answer.md'), combined.answer + '\n');
      } else {
        save(path.join(dir, 'design-output.json'), output);
      }
    }
  } finally {
    await recorder.stop(5000);
    const all = [];
    for (const name of names) for (const variant of ['before','after']) {
      for (const fileName of ['design-provider.json','provider.json']) {
        const file = path.join(here, variant, name, fileName);
        if (fs.existsSync(file)) all.push({ name, variant, stage: fileName === 'provider.json' ? 'select' : 'design', ...read(file) });
      }
    }
    const failed = path.join(here, 'failed-default-thinking-speed', 'provider.json');
    if (fs.existsSync(failed)) all.push({ name: 'speed', variant: 'before-default-thinking-failed', ...read(failed) });
    for (const fileName of ['design-provider.json', 'provider.json']) {
      const initial = path.join(here, 'after-open-design-initial', fileName);
      if (fs.existsSync(initial)) all.push({ name: 'open-design', variant: 'after-initial-math-failure',
        stage: fileName === 'provider.json' ? 'select' : 'design', ...read(initial) });
    }
    save(path.join(here, 'model-usage.json'), {
      model, provider_origin: new URL(base).origin, calls: all.length,
      usage: measuredUsage(all), elapsed_ms: all.reduce((sum, row) => sum + row.elapsed_ms, 0),
      samples: all.map(({ name, variant, stage, status, elapsed_ms, usage, response }) =>
        ({ name, variant, stage, status, elapsed_ms, usage, finish_reasons: response?.choices?.map(c => c.finish_reason) })),
    });
  }
} else {
  throw new Error('Expected prepare, run or select');
}
