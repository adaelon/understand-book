// Export the two source distributions from their single authoritative working tree.
// Usage: node scripts/export-editions.mjs --out ../understand-book-editions
import { execFileSync } from 'node:child_process';
import { constants, copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

const sourceRoot = fileURLToPath(new URL('../', import.meta.url));
const rootFiles = new Set([
  '.env.example', '.gitattributes', '.gitignore', '.mcp.json', '.npmrc',
  '.agents/plugins/marketplace.json', '.codex/agents/understand-book-executor.toml',
  'AGENTS.md', 'CONTEXT.md', 'CONTRIBUTING.md', 'LICENSE', 'README.md',
  'Cargo.toml', 'Cargo.lock', 'package.json', 'pnpm-lock.yaml', 'pnpm-workspace.yaml', 'tsconfig.json',
]);
const commonCrates = new Set([
  'artifact-tools', 'base-schema', 'book-tool-contracts', 'desktop-support',
  'maintenance-host', 'memory', 'read-tools', 'reader', 'reader-service', 'runtime',
]);
const commonPackages = new Set(['code-mcp', 'core', 'dsh-plugin', 'i18n', 'observability', 'web']);
const commonTrees = new Set([
  '.claude-plugin', '.codex-plugin', 'agents', 'assets', 'evals',
  'fixtures', 'plugins', 'skills', 'vendor',
]);
const commonDocs = new Set([
  'docs/工具契约参考.md', 'docs/tool-contracts.json', 'docs/模型配置使用说明.md',
  'docs/个人学习CLI与MCP.md', 'docs/本机论文与教学Skill.md', 'docs/examples/read-empirical-paper/SKILL.md',
]);
const courseExamples = new Set([
  'ai-infra-resource-bounds.course.json', 'ai-infra-resource-bounds.decision.course.json',
  'ai-infra-resource-bounds.v1.course.json', 'ai-infra-resource-bounds.v2.course.json',
  'ai-infra-resource-objects.unit.course.json', 'ai-infra-selected-structure.json',
  'optimization-foundations.course.json', 'optimization-foundations.py', 'ai-infra-time-bounds.md',
]);
const commonScripts = new Set([
  'build-dsh-prebuild-release.mjs', 'build-inspection-scenes.mjs', 'build-linked-methods.mjs',
  'build-method-gallery.mjs', 'build-presentation-three.mjs', 'build-spatial-method.mjs',
  'code-mcp-tunnel.ps1', 'create-exam-document-fixture.py', 'eval-personal-notes.ts',
  'langsmith_read.py', 'measure-provider-concurrency.mjs', 'music-build.mjs',
  'prepare-local-embedding.mjs', 'presentation-animation-requirements.txt',
  'presentation-plot-requirements.txt', 'register-executor-agent.ps1',
  'start-book-mcp.cmd', 'start-build-executor-mcp.cmd', 'start-video.cmd',
  'sync-extractor-contracts.ts', 'test_langsmith_read.py', 'validate-agent-streaming.mjs',
  'validate-presentation-preview.mjs', 'validate-presentation-preview-stop.py',
  'verify-dsh-prebuild-release.mjs', 'verify-exam-document-conversion.ts',
  'verify-linked-methods.mjs', 'verify-local-agent-acp.py', 'verify-personal-connection.py', 'verify-observability-release.mjs',
  'verify-presentation-method-library.mjs', 'verify-presentation-scenes.mjs',
  'verify-spatial-method.mjs', 'workspace-semantic-search.ts',
]);
const localLinuxScripts = new Set([
  '49-understand-book-render.rules', 'check-board-preview.sh', 'nginx-reader.conf.example',
  'presentation-sandbox.json.example', 'reader.env.example', 'relocate-reader-paths.mjs',
  'relocate-reader-paths.test.mjs', 'smoke-reader.mjs', 'start-local-reader.sh',
  'test-presentation-readiness.py', 'understand-book.service', 'service-limits.200-users.json.example',
]);

function sourceFile(file) {
  // Git's ignored-file rules also apply to untracked sources. These generated/data
  // paths are excluded even when an older release happened to track them.
  if (/(^|\/)(node_modules|target|dist(?:-[^/]*)?|test-results|playwright-report|__pycache__)(\/|$)/.test(file)) return false;
  if (/^(docs\/(evidence|performance|screenshots|project-book|books|design)|evals\/[^/]+\/(?:[^/]*-)?results)(\/|$)/.test(file)) return false;
  if (/^apps\/[^/]+\/src-tauri\/(gen|binaries)(\/|$)/.test(file)) return false;
  if (/^apps\/[^/]+\/src-tauri\/resources\//.test(file) && !file.endsWith('/.gitkeep')) return false;
  if (/^apps\/desktop\/local-reader\//.test(file)) return false;
  if (/^examples\/[^/]+\/verification\//.test(file)) return false;
  const name = path.posix.basename(file);
  if ((name.startsWith('.env') || name.endsWith('.env')) && !name.endsWith('.example')) return false;
  if (/\.(private|local)\.(json|env|log|md)$/.test(name) || /^(SESSION_CHECKPOINT|checkpoint_)/.test(name)) return false;
  return true;
}

function commonFile(file) {
  if (rootFiles.has(file)) return true;
  if (commonDocs.has(file)) return true;
  if (file.startsWith('docs/examples/inquiry-course/')) return courseExamples.has(path.posix.basename(file));
  const [tree, name, script] = file.split('/');
  if (tree === 'examples') return name === 'quickstart' || name === 'gear-transmission'
    || file === 'examples/optimization-practice/questions.json';
  if (tree === 'crates') return commonCrates.has(name);
  if (tree === 'packages') return commonPackages.has(name);
  if (commonTrees.has(tree)) return true;
  if (tree !== 'scripts') return false;
  if (['desktop', 'plugin', 'music'].includes(name)) return true;
  if (name === 'linux') return localLinuxScripts.has(script);
  return commonScripts.has(name);
}

function editionFile(file, edition) {
  if (!sourceFile(file)) return false;
  if (commonFile(file)) return true;
  if (edition === 'public') {
    return file === 'materials/README.md'
      || /^(apps\/(local-desktop|local-debug)|crates\/local-desktop-host)\//.test(file);
  }
  return /^(crates\/server|apps\/(admin|cloud-reader|desktop)|scripts\/linux|docs)\//.test(file)
    || /^scripts\/(admin-|cloud-|mcp9-|verify-mcp9-agent|verify-course-reuse|model-trial-config|generate-tool-reference)/.test(file);
}

function json(file) {
  return JSON.parse(readFileSync(file, 'utf8').replace(/^\uFEFF/, ''));
}

function writeJson(file, value) {
  writeFileSync(file, JSON.stringify(value, null, 2) + '\n');
}

function requireFiles(files, edition) {
  const required = ['Cargo.toml', 'Cargo.lock', 'package.json', 'pnpm-lock.yaml',
    'packages/web/package.json', 'crates/reader-service/Cargo.toml',
    'skills/presentation/methods/catalog.json', 'scripts/desktop/build-reader-runtime.mjs',
    'crates/maintenance-host/Cargo.toml', 'scripts/plugin/build-book-mcp.mjs',
    'scripts/linux/service-limits.200-users.json.example'];
  required.push(...(edition === 'public'
    ? ['apps/local-desktop/package.json', 'apps/local-desktop/src-tauri/Cargo.toml', 'crates/local-desktop-host/Cargo.toml']
    : ['crates/server/Cargo.toml', 'apps/cloud-reader/package.json', 'apps/admin/package.json', 'apps/desktop/src-tauri/Cargo.toml']));
  const missing = required.filter(file => !files.includes(file));
  if (missing.length) throw new Error(`${edition} source boundary is not ready. Missing sources:\n${missing.join('\n')}`);
}

function projectPnpmLock(text, packages) {
  const section = text.match(/^importers:\r?\n([\s\S]*?)(?=^[^\s])/m);
  if (!section) throw new Error('pnpm lockfile importers could not be found');
  const headers = [...section[1].matchAll(/^  ([^\s][^\r\n]*?):(?: \{\})?\r?$/gm)];
  const retained = new Set(['.', ...packages]);
  const blocks = headers.flatMap((header, index) => {
    const importer = header[1].replace(/^['"]|['"]$/g, '');
    if (!retained.has(importer)) return [];
    retained.delete(importer);
    const block = section[1].slice(header.index, headers[index + 1]?.index);
    for (const match of block.matchAll(/^\s+version:\s*['"]?link:([^'"\r\n]+)/gm)) {
      const target = path.posix.normalize(path.posix.join(importer, match[1].trim()));
      if (!packages.includes(target)) throw new Error(`pnpm importer ${importer} links outside this edition: ${target}`);
    }
    const workspaceSpecs = [...block.matchAll(/^\s+specifier:\s*['"]?workspace:/gm)].length;
    const workspaceLinks = [...block.matchAll(/^\s+version:\s*['"]?link:/gm)].length;
    if (workspaceSpecs > workspaceLinks) throw new Error(`pnpm importer ${importer} has an unresolved workspace dependency`);
    return [block];
  });
  if (retained.size) throw new Error(`Source pnpm lockfile is missing importers: ${[...retained].join(', ')}`);
  // Preserve registry package/snapshot sections byte-for-byte; only prune importers.
  return text.replace(section[0], `importers:\n\n${blocks.join('')}`);
}

function writeWorkspace(directory, edition, files) {
  // Membership changes with the product, while dependencies retain their real
  // relative layout. The source lockfiles remain the starting resolution.
  const cargoMembers = files.filter(file => /^crates\/[^/]+\/Cargo.toml$/.test(file)
    || /^apps\/[^/]+\/src-tauri\/Cargo.toml$/.test(file)).map(file => file.slice(0, -'/Cargo.toml'.length));
  const cargoPath = path.join(directory, 'Cargo.toml');
  const cargo = readFileSync(cargoPath, 'utf8');
  if (!/^members\s*=\s*\[[\s\S]*?\]/m.test(cargo)) throw new Error('Root Cargo workspace members could not be found');
  writeFileSync(cargoPath, cargo.replace(/^members\s*=\s*\[[\s\S]*?\]/m,
    `members = [\n${cargoMembers.map(member => `  ${JSON.stringify(member)},`).join('\n')}\n]`));
  const packages = files.filter(file => /^(packages|apps)\/[^/]+\/package.json$/.test(file))
    .map(file => file.slice(0, -'/package.json'.length));
  writeFileSync(path.join(directory, 'pnpm-workspace.yaml'), `packages:\n${packages.map(member => `  - ${JSON.stringify(member)}`).join('\n')}\n`);
  const pnpmLock = path.join(directory, 'pnpm-lock.yaml');
  writeFileSync(pnpmLock, projectPnpmLock(readFileSync(pnpmLock, 'utf8'), packages));
  const rootPackagePath = path.join(directory, 'package.json');
  const rootPackage = json(rootPackagePath);
  rootPackage.name = edition === 'public' ? 'understand-book-local-source' : 'understand-book-cloud-source';
  if (edition === 'public') {
    rootPackage.scripts = Object.fromEntries(Object.entries(rootPackage.scripts).filter(([name]) =>
      ['test', 'build', 'build:book-mcp', 'gen-schema', 'demo:prepare', 'verify:extractor',
        'test:t7-executor-release', 'test:t7-codex-cli-release', 'test:maintenance-plugin'].includes(name)
        || name.startsWith('eval:')));
    const webPackagePath = path.join(directory, 'packages/web/package.json');
    const webPackage = json(webPackagePath);
    delete webPackage.scripts['build:multi'];
    writeJson(webPackagePath, webPackage);
  } else {
    rootPackage.scripts.build = 'pnpm --filter @understand-book/web build:multi && pnpm --filter @understand-book/admin build';
  }
  writeJson(rootPackagePath, rootPackage);
}

function normalizeCargoLock(directory, edition) {
  const dependencyVersions = text => new Set(text.split('[[package]]').slice(1).flatMap(entry => {
    const field = name => entry.match(new RegExp(`^${name} = "([^"]+)"`, 'm'))?.[1];
    return field('source') ? [`${field('name')} ${field('version')} ${field('source')}`] : [];
  }));
  const lock = path.join(directory, 'Cargo.lock');
  const original = dependencyVersions(readFileSync(lock, 'utf8'));
  // Resolve the reduced workspace against the copied lock and local Cargo cache.
  // No dependency installation, compilation or online resolution occurs here.
  const options = { cwd: directory, encoding: 'utf8', windowsHide: true, maxBuffer: 16 * 1024 * 1024 };
  const host = execFileSync('rustc', ['-vV'], options).match(/^host: (.+)$/m)?.[1].trim();
  if (!host) throw new Error('rustc did not report a native host target');
  const args = ['metadata', '--offline', '--format-version', '1', '--filter-platform', host];
  execFileSync('cargo', args, options);
  const added = [...dependencyVersions(readFileSync(lock, 'utf8'))].filter(value => !original.has(value));
  if (added.length) throw new Error(`Cargo changed dependency versions in ${directory}; inspect the preserved output:\n${added.join('\n')}`);
  const removedManifests = edition === 'public' ? ['crates/server/Cargo.toml', 'apps/desktop/src-tauri/Cargo.toml']
    : ['crates/local-desktop-host/Cargo.toml', 'apps/local-desktop/src-tauri/Cargo.toml', 'apps/local-debug/src-tauri/Cargo.toml'];
  const packages = readFileSync(lock, 'utf8').split('[[package]]').slice(1);
  for (const manifest of removedManifests) {
    const name = readFileSync(path.join(sourceRoot, manifest), 'utf8').match(/^name\s*=\s*"([^"]+)"/m)?.[1];
    if (packages.some(entry => !/^source =/m.test(entry) && entry.match(/^name = "([^"]+)"/m)?.[1] === name)) {
      throw new Error(`Cargo.lock still contains excluded workspace package ${name}`);
    }
  }
  execFileSync('cargo', [...args, '--locked'], options);
  return host;
}

function publicToolReference(text) {
  return text.replace(/\r\n/g, '\n')
    .replace(/<!-- Generated[^\n]+-->/, '<!-- Public projection of the source tool reference, produced by export-editions.mjs. -->')
    .replace(/^生成命令：.*$/m, '本页随源码快照提供；公共工具来源与完整结构见下文。版本和验证状态见 [EDITION.md](../EDITION.md)。')
    .replace(/<a id="remote_mcp"><\/a>[\s\S]*?(?=<a id="executor"><\/a>)/, '')
    .replace(/^\| \[远程 MCP\].*\n/m, '')
    .replace(/^MCP 的 outputSchema.*$/m, '本地 MCP 的 outputSchema 描述成功的 structuredContent；isError=true 的错误沿独立诊断协议处理。')
    .replace(/^- 查找字面出现：.*$/m, '- 查找字面出现：从 [Resident 搜索示例](#resident-book-search_text-examples) 选定范围，再按结果读取正文。')
    .replaceAll('crates/server/src/mcp.rs', 'crates/reader-service/src/mcp.rs')
    .replaceAll('crates/server/src/mcp_output_schema.rs', 'crates/reader-service/src/mcp_output_schema.rs');
}

function publicModelGuide(text) {
  return text.replace(/\r\n/g, '\n')
    .replace(/云端管理员进入[^\n]+/, '')
    .replace('云端排队任务和教学任务按原修订恢复。', '')
    .replace(/；云端管理员使用部署目录的同名文件[^。]*。/, '。')
    .replace(/实际服务、部署、型号及能力的验收范围见\[多协议实施方案的当前状态\][^。]+。/, '实际服务、部署、型号及能力需要分别验证，源码快照状态见 [EDITION.md](../EDITION.md)。')
    .replace(/^使用推理模型时，[^\n]+/m, '使用推理模型时，应为推理与最终结果共同留足输出空间，并按实际输出及费用预算设置。')
    .replace(/## 真实模型复验[\s\S]*?(?=## 已知限制)/, '')
    .replace(/^- 云端收费调用[^\n]+\n?/m, '');
}

function writeEditionDocs(directory, edition, source) {
  const put = (file, value) => writeFileSync(path.join(directory, file), value);
  const read = file => readFileSync(path.join(directory, file), 'utf8');
  const notice = '项目代码沿用 [MIT 许可证](LICENSE)。内置第三方资源保留各自许可：' +
    '[Three.js](assets/presentation/three-0.180.0/LICENSE)、[Konva](assets/presentation/konva-10.7.0/LICENSE)、' +
    '[齿轮示例 Three.js](examples/gear-transmission/vendor/THREE-LICENSE.txt)。\n';
  if (edition === 'public') {
    put('README.md', '# Understand Book 本地版\n\n' +
      '在自己的电脑上阅读材料、提问、学习和保存笔记。正式应用使用独立本地数据目录，已有材料可以直接阅读；AI 功能使用自己配置的模型服务。\n\n' +
      '## Quick Start\n\n' +
      '准备 Node.js、pnpm、Rust 和平台构建工具后，在此源码目录执行：\n\n' +
      '```powershell\npnpm install --frozen-lockfile\npnpm demo:prepare\npnpm build\ncargo run -p reader-service --bin reader -- --reader-only .understand-book/quickstart-demo\n```\n\n' +
      '服务启动后打开终端显示的本机地址。Windows 安装包和 macOS 构建入口见[本地应用说明](apps/local-desktop/README.md)，' +
      '调试壳见[Local Debug](apps/local-debug/README.md)。\n\n' +
      '## 使用与开发\n\n' +
      '- [个人学习 CLI 与 MCP](docs/个人学习CLI与MCP.md)\n' +
      '- [模型服务配置](docs/模型配置使用说明.md)\n' +
      '- [学习材料下载、来源与范围](materials/README.md)\n' +
      '- [构建插件与执行器](plugins/understand-book/README.md)\n' +
      '- [工具契约参考](docs/工具契约参考.md)\n' +
      '- [源码结构](CONTEXT.md)、[贡献与验证](CONTRIBUTING.md)\n' +
      '- [本次源码版本与构建说明](EDITION.md)\n\n' +
      '## 许可与材料\n\n' + notice + '\n' +
      '下载资料独立于源码分发。原书、论文、乐谱和录音保留各自来源与权利说明；代码许可证不代替材料许可。详见[材料来源](materials/README.md#材料来源)。\n');
    put('CONTEXT.md', '# 本地源码结构\n\n' +
      '- `packages/web`：共享 Reader、本地书库、模型设置与本地应用界面。\n' +
      '- `crates/reader-service`：单用户服务、Book MCP 和公共运行接口；`crates/runtime` 承担 Agent 与教学执行。\n' +
      '- `crates/memory`：本地学习记录；其他公共 crates 承担资料合同和读取。\n' +
      '- `crates/desktop-support`、`crates/local-desktop-host`：桌面持久化、数据导入和本地宿主。\n' +
      '- `apps/local-desktop`、`apps/local-debug`：正式与调试应用壳。\n' +
      '- `packages/core`、`skills`、`plugins`、`crates/maintenance-host`：资料准备、构建插件及维护 CLI。\n' +
      '- `scripts/plugin`、`scripts/desktop`：公共可执行工具和平台资源装配。\n\n' +
      '源码与数据分别管理，发布来源见 [EDITION.md](EDITION.md)，工具接口见[工具契约参考](docs/工具契约参考.md)。\n');
    put('CONTRIBUTING.md', '# 本地版功能交付\n\n' +
      '围绕受影响的用户路径完成实现、验证与记录。先确定用户可观察的结果，再选择能识别该路径失败的已有测试；失败时修正根因并复验。界面修改还需打开实际页面或应用操作并查看效果。\n\n' +
      '共享合同使用 `pnpm verify:extractor`；Core 使用 `pnpm --filter @understand-book/core test`；Reader 使用 `pnpm --filter @understand-book/web test`。' +
      'Rust 修改运行对应 crate 的定向测试。桌面资源和安装路径沿[本地应用验证入口](apps/local-desktop/README.md#验证入口)。\n\n' +
      '插件源码合同使用 `pnpm test:maintenance-plugin`；Book MCP 使用 `pnpm build:book-mcp` 构建。' +
      '完整测试入口为 `pnpm test`，产品构建入口见 [EDITION.md](EDITION.md)。\n\n' +
      '历史真实试验需要显式输入：`MCP_SOURCE_EVIDENCE` 指向 HTTP 来源证据，`BSR4_EVIDENCE_DIR` 指向主题回放目录，' +
      '`SR6_RANKING_INPUT` 指向已保存向量。未提供时对应历史回放显示跳过，普通原创样本继续运行；模型试验需要自己的服务、材料和预算。\n\n' +
      '提交结果应区分执行通过、语义通过、失败与未执行，写明受影响入口、证据和剩余问题。' +
      '原始材料、账号、凭据及本地运行证据留在数据目录；可公开的回归输入放在对应模块的 fixtures 中。\n');
    put('docs/工具契约参考.md', publicToolReference(read('docs/工具契约参考.md')));
    const contracts = json(path.join(directory, 'docs/tool-contracts.json'));
    delete contracts.remote_mcp;
    writeJson(path.join(directory, 'docs/tool-contracts.json'), contracts);
    put('docs/模型配置使用说明.md', publicModelGuide(read('docs/模型配置使用说明.md')));
    put('materials/README.md', read('materials/README.md')
      .replace('可编辑章节、题目来源和制作说明见[讲义目录](../examples/optimization-practice/course-book/README.md)。',
        '题目来源和制作说明随[已发布资料包](#下载)提供。'));
    put('apps/local-desktop/README.md', read('apps/local-desktop/README.md')
      .replaceAll('[双发行方案](../../docs/切片方案-开源本地应用与云端服务双发行.md)', '[源码版本与平台状态](../../EDITION.md)')
      .replace('实际证据和仍未验收的平台见双发行方案。', '本次快照的平台验证状态见 [EDITION.md](../../EDITION.md)。'));
  } else {
    put('README.md', '# Understand Book 云端服务\n\n' +
      '云服务扩展包含账号、额度、共享工作区、网站、运营后台与云客户端；公共 Reader 和构建运行时以同次源码快照保存在此目录。\n\n' +
      '构建命令、公共依赖来源与当前验证状态见 [EDITION.md](EDITION.md)。' +
      '依赖只使用此目录中的源码和锁文件。公共实现的修改应回到权威来源后重新导出。\n\n' +
      '- [Linux 云端发行与部署](docs/Linux多人阅读-MU11发布运行单.md)\n' +
      '- [云端学习与考试冲刺运行说明](docs/运行说明-云端学习与考试冲刺.md)\n' +
      '- [模型服务配置](docs/模型配置使用说明.md)\n' +
      '- [工具契约参考](docs/工具契约参考.md)\n' +
      '- [功能交付流程](CONTRIBUTING.md)\n\n' + notice);
    put('CONTRIBUTING.md', read('CONTRIBUTING.md').replace(/^\| 本地\/云端边界[^\r\n]+/m,
      '| 云端宿主、网站与客户端 | 云目录独立构建、共享行为与宿主测试、认证入口和既有数据恢复 | [云端构建与部署](docs/Linux多人阅读-MU11发布运行单.md)、[云客户端](apps/desktop/README.md)、[独立源码构建](EDITION.md) |'));
    put('apps/desktop/README.md', read('apps/desktop/README.md').replace(
      /^For the independent Windows developer application[^\r\n]+/m,
      'The independent local Reader/AI application and its developer shell are provided by the public source distribution. This directory builds the cloud client.'));
    for (const file of ['docs/切片方案-开源本地应用与云端服务双发行.md', 'docs/architecture.md']) {
      put(file, read(file).replace(/\[([^\]]+)\]\((\.\.\/apps\/local-(?:desktop|debug)\/README\.md(?:#[^)]*)?)\)/g,
        '$1（位于独立公共源码中的对应应用目录）'));
    }
    put('docs/双发行源码导出.md', '# 独立源码快照\n\n本目录已经是导出结果。来源版本、依赖升级方式与编译入口见 [EDITION.md](../EDITION.md)。重新导出在权威双发行工作树中执行。\n');
  }
  put('EDITION.md', instructions(edition, source));
}

function instructions(edition, source) {
  const common = `# ${edition === 'public' ? '本地应用公共源码' : '云端服务源码'}\n\n` +
    `这是双发行工作树的独立源码快照，基线提交为 \`${source.base_commit}\`，源状态为 \`${source.state}\`。` +
    '准确状态见 `edition-source.json`；工作树快照不是已经发布的公共版本。\n\n';
  if (edition === 'public') return common +
    '从本目录安装依赖并启动原创阅读样本：\n\n```powershell\npnpm install --frozen-lockfile\npnpm demo:prepare\npnpm build\ncargo run -p reader-service --bin reader -- --reader-only .understand-book/quickstart-demo\n```\n\n' +
    '正式 Windows 安装包使用 `pnpm --filter @understand-book/local-desktop package:windows`。' +
    'macOS 使用 `pnpm --filter @understand-book/local-desktop package:macos`，需在对应原生架构的 macOS 环境执行。' +
    '导出时核对了本版的依赖归属。应用和命令行的构建、安装、签名与公证结果由各平台的实际验证记录提供。[本地版说明](apps/local-desktop/README.md)\n\n' +
    '公共 Book MCP 可执行文件使用 `pnpm build:book-mcp` 构建；其输出目录与目标架构由公共插件构建入口确定。\n\n' +
    '本目录包含公共 Reader、模型运行时、本地壳、构建插件、格式资源脚本与测试。' +
    '云账号、控制库、网站和云客户端由云端目录持有。\n';
  return common +
    '公共核心源码按同一次导出平铺在原有目录，所有构建依赖都位于本目录。' +
    '这些公共文件是生成的依赖快照，修改应回到公共核心后重新导出。\n\n' +
    '在本目录验证网站、Admin 与云宿主：\n\n```powershell\npnpm install --frozen-lockfile\npnpm build\ncargo build --locked -p server --bin server\n```\n\n' +
    '正式 Linux 组装沿用 `bash scripts/linux/build-multi-reader.sh`，包括原语义检索配置、Web/Admin 资产校验和构建回执。' +
    '服务数据根、模型配置和部署步骤沿原运行说明配置；此快照不携带账号、运行书库或凭据。\n\n' +
    '云客户端继续使用 `apps/desktop` 的产品身份和安装命令。远程仓库与公共发布提交确定后，' +
    '由云端显式升级公共依赖；本目录不自动跟随其他目录的修改。\n';
}

export { editionFile, publicToolReference, publicModelGuide, projectPnpmLock };

function main() {
  const { values } = parseArgs({ options: { out: { type: 'string' }, help: { type: 'boolean', short: 'h' } } });
  if (values.help) {
    console.log('Usage: node scripts/export-editions.mjs --out <new-or-empty-directory>\nCreates independent public/ and cloud/ source trees. Does not build or publish them.');
    process.exit(0);
  }
  if (!values.out) throw new Error('Choose a new output directory with --out');
  const output = path.resolve(values.out);
  if (existsSync(output) && readdirSync(output).length) throw new Error(`Output must be empty; earlier sources are preserved: ${output}`);
  const git = args => execFileSync('git', args, { cwd: sourceRoot, encoding: 'utf8', windowsHide: true, maxBuffer: 32 * 1024 * 1024 });
  const candidates = [...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split('\0').filter(Boolean))]
    .filter(file => existsSync(path.join(sourceRoot, file)) && statSync(path.join(sourceRoot, file)).isFile()).sort();
  const source = {
    base_commit: git(['rev-parse', 'HEAD']).trim(),
    state: git(['status', '--porcelain', '--untracked-files=normal']).trim() ? 'working-tree' : 'clean',
    exported_at: new Date().toISOString(),
  };
  const selections = Object.fromEntries(['public', 'cloud'].map(edition => [edition, candidates.filter(file => editionFile(file, edition))]));
  for (const [edition, files] of Object.entries(selections)) requireFiles(files, edition);
  mkdirSync(output, { recursive: true });
  for (const [edition, files] of Object.entries(selections)) {
    const directory = path.join(output, edition);
    mkdirSync(directory);
    for (const file of files) {
      const destination = path.join(directory, file);
      mkdirSync(path.dirname(destination), { recursive: true });
      copyFileSync(path.join(sourceRoot, file), destination, constants.COPYFILE_EXCL);
    }
    writeWorkspace(directory, edition, files);
    const assemblyPlatform = normalizeCargoLock(directory, edition);
    writeJson(path.join(directory, 'edition-source.json'), {
      edition, source,
      public_dependency: edition === 'cloud' ? 'bundled source from this same export; no external working-tree dependency' : null,
      source_files: files.length,
      assembly_validation: 'cargo_metadata_offline_locked',
      assembly_platform: assemblyPlatform,
      derived_files: edition === 'public'
        ? ['Cargo.toml', 'Cargo.lock', 'pnpm-workspace.yaml', 'pnpm-lock.yaml', 'package.json', 'packages/web/package.json',
          'README.md', 'CONTEXT.md', 'CONTRIBUTING.md', 'docs/工具契约参考.md', 'docs/tool-contracts.json',
          'docs/模型配置使用说明.md', 'materials/README.md', 'apps/local-desktop/README.md', 'EDITION.md']
        : ['Cargo.toml', 'Cargo.lock', 'pnpm-workspace.yaml', 'pnpm-lock.yaml', 'package.json', 'README.md', 'CONTRIBUTING.md',
          'apps/desktop/README.md', 'docs/切片方案-开源本地应用与云端服务双发行.md', 'docs/architecture.md', 'docs/双发行源码导出.md', 'EDITION.md'],
      validation: 'not_run',
    });
    writeEditionDocs(directory, edition, source);
    console.log(`${edition}: ${files.length} source files -> ${directory}`);
  }
  console.log('Source export complete. Build and validate each directory independently; see EDITION.md.');
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
