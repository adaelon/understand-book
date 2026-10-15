import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { CODEX_EXECUTOR_TRANSPORT_PROFILE_V2, DSH_EXECUTOR_TRANSPORT_PROFILE_V3, CODEX_EXECUTOR_DELIVERY_BATCH_LIMIT_V1,
  serializeExecutorMcpToolResult, measureExecutorTransportResponse } from '../../packages/core/src/executor-transport.ts';
import { cases, output, mcpResult, measure, save } from './image-experiment.mjs';

const inputs = JSON.parse(readFileSync(join(output, 'inputs.json'), 'utf8'));
const rows = Object.fromEntries(Object.keys(cases).map(id => {
  const native = mcpResult(inputs, id);
  const legacy = serializeExecutorMcpToolResult(native);
  return [id, { native: measure(native),
    legacy: { serialized_mcp_result_bytes: Buffer.byteLength(legacy),
      outer_content_types: JSON.parse(legacy).content.map((c: {type: string}) => c.type),
      exceeds_codex_batch_bytes: Buffer.byteLength(legacy) > CODEX_EXECUTOR_DELIVERY_BATCH_LIMIT_V1.max_serialized_batch_bytes,
      codex_blocking_reasons: measureExecutorTransportResponse(native, '', CODEX_EXECUTOR_TRANSPORT_PROFILE_V2).blocking_reasons,
      dsh_blocking_reasons: measureExecutorTransportResponse(native, '', DSH_EXECUTOR_TRANSPORT_PROFILE_V3).blocking_reasons } }];
}));
save(join(output, 'capacity.json'), { codex_profile: CODEX_EXECUTOR_TRANSPORT_PROFILE_V2,
  dsh_profile: DSH_EXECUTOR_TRANSPORT_PROFILE_V3, codex_batch: CODEX_EXECUTOR_DELIVERY_BATCH_LIMIT_V1, rows });
console.log('Measured native image results and current production text envelopes for ' + Object.keys(rows).length + ' cases.');
