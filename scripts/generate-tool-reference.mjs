import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync, rmSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { BUILD_EXECUTOR_TOOL_CONTRACTS_V3 } from '../packages/core/src/build-executor-tool-contract.ts';
import { BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3 } from '../packages/core/src/build-executor-connection-capability.ts';
import { createBuildExecutorMcpSession } from '../skills/build/build-executor-mcp.ts';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
export const OUTPUT_FILES = ['工具契约参考.md', 'tool-contracts.json'];
const SURFACES = [
  ['resident', 'Resident', '完整注册表；每轮实际可见集合由工具暴露计划按权限、证据、内容 profile 和激活状态选择。Tutor 另按当前状态投影。'],
  ['local_mcp', '本地 MCP', '本地工作区发布的 Book 与 Artifact 工具；Book query、synthesize 和 visitor guide 可调用模型。'],
  ['remote_mcp', '远程 MCP', '面向内测账号获准的不可变发布版本。book_list／mcp_usage 免费；每次成功内容调用消费 1 次，包含空结果、续页和重复请求。'],
  ['executor', '专用 Build Executor', '仅专用 understand_book_executor 使用。按 handoff → 输入批次 → GENERATE → 提交候选推进，引用与序号取当次工具返回值。'],
];
const json = value => JSON.stringify(value, null, 2);
const code = value => `\`\`\`json\n${json(value)}\n\`\`\``;
const cell = value => String(value ?? '—').replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('|', '&#124;').replaceAll('\n', '<br>');
const anchor = (surface, name) => `${surface}-${name.replaceAll('.', '-')}`;
const linkSource = source => {
  const [file] = source.split(':');
  return `[${source}](../${file})`;
};

// Discover through the same session entry used by MCP; no tools/call or material access.
export function executorDefinitions(privateRoot) {
  const session = createBuildExecutorMcpSession({
    bootstrap_version: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.version,
    protocol_generation: BUILD_EXECUTOR_BOOTSTRAP_CONTRACT_V3.session_protocol,
    session_private_root: privateRoot,
  });
  const response = session.handle_message({ jsonrpc: '2.0', id: 1, method: 'tools/list' });
  const tools = response.result.tools;
  assert.deepEqual(tools.map(t => t.name), BUILD_EXECUTOR_TOOL_CONTRACTS_V3.map(t => t.name));
  for (const [index, tool] of tools.entries()) {
    assert.deepEqual(tool.inputSchema, BUILD_EXECUTOR_TOOL_CONTRACTS_V3[index].input_schema);
    assert.equal(tool.description, BUILD_EXECUTOR_TOOL_CONTRACTS_V3[index].description);
  }
  return tools;
}

export function createReference(rust, tutorStates, executor) {
  const reference = { ...rust, executor, tutor_states: tutorStates };
  for (const [surface] of SURFACES) {
    assert.ok(reference[surface]?.length, `Missing ${surface} export`);
    const names = reference[surface].map(t => t.name);
    assert.equal(new Set(names).size, names.length, `Duplicate ${surface} tool`);
    reference[surface] = [...reference[surface]].sort((a, b) => a.name < b.name ? -1 : a.name > b.name ? 1 : 0);
  }
  assert.ok(Object.keys(tutorStates).length, 'Missing Tutor state export');
  return reference;
}

function resolve(schema, root) {
  if (schema?.$ref?.startsWith('#/')) {
    return schema.$ref.slice(2).split('/').reduce((value, key) => value?.[key.replaceAll('~1', '/').replaceAll('~0', '~')], root) ?? schema;
  }
  return schema;
}

function shape(schema) {
  if (typeof schema === 'boolean') return schema ? '任意 JSON 值' : '不允许值';
  const facts = [];
  for (const key of ['type', '$ref', 'const', 'enum', 'default', 'minimum', 'maximum', 'minLength', 'maxLength', 'minItems', 'maxItems', 'pattern', 'additionalProperties']) {
    if (key in schema) facts.push(`${key}=${JSON.stringify(schema[key])}`);
  }
  for (const key of ['anyOf', 'oneOf', 'allOf']) {
    if (schema[key]) facts.push(`${key}: ${schema[key].map(s => shape(s) || 'object').join(' / ')}`);
  }
  return facts.join('; ') || '见完整 schema';
}

function inputSection(schema) {
  const lines = ['**输入参数**', '', '| 字段 | 必填 | 类型与约束 | 语义 |', '|---|---|---|---|'];
  for (const [name, field] of Object.entries(schema.properties ?? {})) {
    lines.push(`| \`${name}\` | ${(schema.required ?? []).includes(name) ? '是' : '按分支／可选'} | ${cell(shape(field))} | ${cell(field.description)} |`);
  }
  if (!Object.keys(schema.properties ?? {}).length) lines.push('| — | — | 无具名参数 | 见完整 schema |');
  if (schema.additionalProperties === false) lines.push('', '顶层输入拒绝未声明字段。');
  const branches = schema.anyOf ?? schema.oneOf;
  if (branches) {
    lines.push('', '**输入分支约束**（嵌套对象及互斥规则见完整 schema）', '', code(branches));
  }
  return lines.join('\n');
}

function examplesSection(schema, exampleAnchor) {
  const examples = schema.examples ?? [];
  return [`<a id="${exampleAnchor}"></a>`, '', '**发布示例**', '', examples.length ? code(examples) : '当前定义未发布输入示例。'].join('\n');
}

// Expose result coverage fields directly from schemas. The full schema remains in the JSON artifact.
function resultFields(schema) {
  const fields = new Map();
  const walk = (node, prefix, depth, refs) => {
    if (!node || typeof node !== 'object' || depth > 4) return;
    if (node.$ref) {
      if (refs.has(node.$ref)) return;
      refs = new Set([...refs, node.$ref]);
      node = resolve(node, schema);
    }
    for (const variant of [...(node.anyOf ?? []), ...(node.oneOf ?? []), ...(node.allOf ?? [])]) walk(variant, prefix, depth, refs);
    for (const [key, field] of Object.entries(node.properties ?? {})) {
      const location = prefix ? `${prefix}.${key}` : key;
      if (!prefix || /^(coverage|complete|truncated|matched_count|returned_count|source_range|requested_range|next_calls|continuation|warnings|availability|structure_availability|data_path|total_items|status|usage|charged_units|charged_from)$/.test(key)) {
        const descriptions = fields.get(location) ?? new Set();
        descriptions.add(`${shape(field)}${field.description ? ` — ${field.description}` : ''}`);
        fields.set(location, descriptions);
      }
      walk(field, location, depth + 1, refs);
    }
    // Array item details are available in the complete schema, not flattened as scalar fields.
  };
  walk(schema, '', 0, new Set());
  return [...fields].map(([name, values]) => `| \`${name}\` | ${cell([...values].join(' / '))} |`);
}

function sources(surface, tool) {
  if (surface === 'resident') return [tool.input_source, 'crates/runtime/src/tool_registry.rs:ToolRegistration::output_schema/annotations/registration_for'];
  if (surface === 'local_mcp') return ['crates/server/src/mcp.rs:tools_list_result', tool.name.startsWith('artifact_') ? 'crates/artifact-tools/src/lib.rs:artifact_list_input_schema/artifact_search_input_schema/artifact_read_input_schema' : 'crates/book-tool-contracts/src/lib.rs:input_schema', 'crates/server/src/mcp_output_schema.rs:local'];
  if (surface === 'remote_mcp') return ['crates/server/src/remote_mcp.rs:tools_list_result', 'crates/server/src/mcp_read.rs:tool_definitions', 'crates/server/src/mcp_output_schema.rs:remote/directory/usage_result'];
  return ['packages/core/src/build-executor-tool-contract.ts:BUILD_EXECUTOR_TOOL_CONTRACTS_V3', 'skills/build/build-executor-mcp.ts:createBuildExecutorMcpSession'];
}

export function renderReference(reference) {
  const lines = [
    '# 工具契约参考', '',
    '<!-- Generated by scripts/generate-tool-reference.mjs. Edit source contracts, then regenerate. -->', '',
    '生成命令：`pnpm docs:tools`；检查生成漂移：`pnpm docs:tools:check`。需要工作区 Node／tsx 和 Rust 构建环境；复现配置见 [TO13 实施记录](performance/tool-usability/to13/README.md)。', '',
    '本页从当前源码的实际注册表、MCP tools/list 与 Tutor 状态投影生成。完整输入／输出 JSON Schema、示例、路由卡和预算策略保存在同次生成的 [tool-contracts.json](tool-contracts.json)；每项下方给出 JSON Pointer，嵌套约束以该完整结构为准。', '',
    '维护共同 Book 参数／示例时编辑 `book-tool-contracts`；状态示例编辑 `tutor::spec/spec_for_context`；其他工具按各项来源符号修改。工具清单从原所有者枚举，新工具进入原目录后由生成命令收录。', '',
    '## 表面与能力边界', '', '| 表面 | 工具数 | 边界 |', '|---|---:|---|',
    ...SURFACES.map(([key, title, description]) => `| [${title}](#${key}) | ${reference[key].length} | ${description} |`), '',
    'Resident 结果 schema 描述 ToolResultEnvelope，Provider 函数 parameters 仍仅描述输入。ok／partial／error、预算省略与采样后回收的 null 按实际交付解释。定位候选与结构投影只提供位置；正文证据以实际读取和接纳范围为准。', '',
    'MCP 的 outputSchema 描述成功的 structuredContent；isError=true 的错误沿独立诊断协议处理。远程 complete 描述本次投影／分页，候选 truncated 与 matched_count 描述召回范围；sources 保留发布版本及实际来源范围，usage 以服务端回执为准。', '',
    'Executor 在 MCP text content 中交付版本化 session 响应，依 action.kind 推进；示例引用来自合成测试 session，使用时替换为当次返回值。提交候选的接纳与终态由代码回执决定。', '',
    '## 任务入口', '',
    '- 查找字面出现：从 [Resident 搜索示例](#resident-book-search_text-examples) 或 [远程搜索示例](#remote_mcp-book_search_text-examples) 选定范围，再按结果读取正文。远程先调用 [book_list](#remote_mcp-book_list) 取得可用发布引用。',
    '- 继续教学：先核对 [Tutor 状态](#tutor-states)，使用该状态实际发布的示例与引用。',
    '- 修改演示：沿 [Presentation 示例](#resident-presentation-author-examples) 读取既有对象、修改候选、预览并交付。',
    '- 保存读者要求的笔记：使用 [reader.note 示例](#resident-reader-note-examples)，依据实际保存回执判断成果。',
    '- 推进构建：从 [executor.open 示例](#executor-executor-open-examples) 开始，按工具返回的 next_request 和 GENERATE 继续。', '',
  ];
  for (const [surface, title] of SURFACES) {
    lines.push(`<a id="${surface}"></a>`, '', `## ${title}`, '', '| 工具 | 输入示例数 | 输出 schema |', '|---|---:|---|');
    for (const tool of reference[surface]) lines.push(`| [${tool.name}](#${anchor(surface, tool.name)}) | ${tool.inputSchema.examples?.length ?? 0} | ${tool.outputSchema ? '已发布' : '未发布'} |`);
    for (const [index, tool] of reference[surface].entries()) {
      const id = anchor(surface, tool.name);
      lines.push('', `<a id="${id}"></a>`, '', `### ${tool.name}`, '', tool.description, '',
        `**来源符号**：${sources(surface, tool).map(linkSource).join('；')}。`, '',
        `**完整契约**：[tool-contracts.json](tool-contracts.json)，JSON Pointer：\`/${surface}/${index}\`。`, '');
      if (tool.routing_card) {
        const card = tool.routing_card;
        lines.push(`**适用**：${card.use_when.join('；')}`, '', `**避免**：${card.avoid_when.join('；')}`, '',
          `**效果与前提**：effect=\`${card.effects}\`；preconditions=${card.preconditions.map(x => `\`${x}\``).join('、') || '无'}；relative_cost=\`${card.relative_cost}\`。`, '',
          `**交付策略**：\`${tool.output_policy.result_policy}\`，model_body 上限 ${tool.output_policy.max_model_body_bytes} 字节；并行策略 \`${tool.parallelism}\`。`, '');
      }
      lines.push('**发布注解**', '', tool.annotations ? code(tool.annotations) : '当前表面未发布 annotations。', '', inputSection(tool.inputSchema), '');
      if (tool.name === 'tutor.step') lines.push('本项为完整注册定义。有效调用请使用下方 [Tutor 状态投影](#tutor-states) 中对应状态的 schema／示例；此处基础示例并不保证在每个状态都可用。', '');
      lines.push(examplesSection(tool.inputSchema, `${id}-examples`), '', '**结果与覆盖**', '');
      if (tool.outputSchema) {
        lines.push('以下字段取自实际输出 schema；完整分支、嵌套类型及数组元素约束见本项完整契约。', '', '| 字段路径 | 类型／约束／语义 |', '|---|---|', ...resultFields(tool.outputSchema));
      } else lines.push('当前定义未发布 outputSchema；结果／效果按以上工具说明、路由元数据和原执行回执解释。');
    }
  }
  lines.push('', '<a id="tutor-states"></a>', '', '## Tutor 状态投影', '',
    `来源：${linkSource('crates/runtime/src/tutor.rs:spec_for_context')}；场景输入取既有 ${linkSource('crates/runtime/src/tool_contract_tests.rs:tutor_examples_follow_final_state_schema_and_provider_requests')}。每个场景的 context 与最终 schema 同时保存在完整契约；引用由当轮真实上下文提供。`, '',
    '| 场景 | status | 暴露的 operation | 示例数 |', '|---|---|---|---:|');
  for (const [name, state] of Object.entries(reference.tutor_states)) {
    lines.push(`| [${name}](#tutor-${name}) | ${state.context.status} | ${state.spec?.parameters.properties.operation.enum.join('、') ?? '不暴露 tutor.step'} | ${state.spec?.parameters.examples.length ?? 0} |`);
  }
  for (const [name, state] of Object.entries(reference.tutor_states)) {
    lines.push('', `<a id="tutor-${name}"></a>`, '', `### ${name}`, '', '**状态输入**', '', code(state.context), '', `完整投影：\`/tutor_states/${name}\`。`, '');
    if (state.spec) lines.push(state.spec.description, '', examplesSection(state.spec.parameters, `tutor-${name}-examples`));
    else lines.push('该状态不暴露 tutor.step。');
  }
  lines.push('', '## 已知限制', '',
    '- 表面目录代表当前源码定义，实际运行还受权限、材料能力、状态与工具暴露计划约束。新状态分支应扩展原 Tutor 契约场景，生成器继续消费这些场景。',
    '- 未发布示例、annotations 或 outputSchema 的条目已明确标注。它们的类型化扩展按实际消费需求推进。',
    '- reader.note 返回 EXISTING 时已有笔记仍有效，但现有 Goal reader_action 完成判断可能缺少创建 effect；见 [TO5 已知限制](performance/tool-usability/to5/README.md#已知限制)。',
    '- 本文验证契约与生成一致性；真实 Agent 选用、完成率和成本收益由 TO14 验收。', '');
  return lines.join('\n');
}

export function writeReference(reference, outputDir, check = false) {
  const rendered = [renderReference(reference), `${json(reference)}\n`];
  if (!check) mkdirSync(outputDir, { recursive: true });
  for (const [index, file] of OUTPUT_FILES.entries()) {
    const target = path.join(outputDir, file);
    if (check) {
      let actual;
      try { actual = readFileSync(target, 'utf8'); } catch (error) { if (error.code !== 'ENOENT') throw error; }
      if (actual?.replaceAll('\r\n', '\n') !== rendered[index]) throw new Error(`${file} is stale or missing; run pnpm docs:tools`);
    } else writeFileSync(target, rendered[index]);
  }
}

function cargoTest(args, env) {
  const result = spawnSync('cargo', ['--config', 'profile.test.package.runtime.codegen-units=16', 'test', ...args], { cwd: ROOT, env, stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`Contract export failed (${result.status})`);
}

export function generateReference(check = false) {
  const temp = mkdtempSync(path.join(tmpdir(), 'ub-tool-reference-'));
  try {
    const rustPath = path.join(temp, 'rust.json');
    const tutorPath = path.join(temp, 'tutor.json');
    const env = { ...process.env, TOOL_CONTRACT_SNAPSHOT: rustPath, TOOL_TUTOR_SNAPSHOT: tutorPath };
    cargoTest(['-p', 'server', '--lib', '--', 'remote_mcp::tests::mcp_contract::mcp_contract_export_tool_definitions', '--exact', '--ignored'], env);
    cargoTest(['-p', 'runtime', '--lib', '--', 'tool_registry::contract_tests::tutor_examples_follow_final_state_schema_and_provider_requests', '--exact'], env);
    const reference = createReference(JSON.parse(readFileSync(rustPath, 'utf8')), JSON.parse(readFileSync(tutorPath, 'utf8')), executorDefinitions(path.join(temp, 'executor')));
    writeReference(reference, path.join(ROOT, 'docs'), check);
    console.log(`${check ? 'Checked' : 'Generated'} ${OUTPUT_FILES.join(', ')}; ${SURFACES.map(([s]) => `${s}=${reference[s].length}`).join(', ')}; Tutor states=${Object.keys(reference.tutor_states).length}`);
  } finally {
    // Only the freshly created, isolated export directory is removed.
    assert.equal(path.dirname(temp), path.resolve(tmpdir()));
    rmSync(temp, { recursive: true, force: true });
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  if (args.some(arg => arg !== '--check') || args.length > 1) throw new Error('Usage: pnpm docs:tools [--check]');
  generateReference(args.includes('--check'));
}
