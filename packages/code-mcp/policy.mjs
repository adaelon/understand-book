import path from 'node:path';

export const ROOTS = ['packages', 'crates', 'apps', 'agents', 'skills', 'plugins', 'scripts', 'docs'];
export const ROOT_FILES = [
  'README.md', 'CONTEXT.md', 'SESSION_CHECKPOINT.md', 'understand-book.md',
  '需求文档.md', '需求文档-V2.md', '需求文档-V3.md', '产品担忧.md', '理解.md',
  'agent交互书.md', 'todo.md', 'grill.md', 'grill-0727.md', 'grill-0729.md', 'grill-0731.md',
  'DESIGN-apple.md', 'DESIGN-claude.md', 'DESIGN-mintlify.md',
  'package.json', 'pnpm-workspace.yaml', 'pnpm-lock.yaml', 'tsconfig.json',
  'Cargo.toml', 'Cargo.lock', 'LICENSE', '.gitignore', '.gitattributes', '.mcp.json',
];
const EXTENSIONS = new Set([
  '.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs', '.rs', '.py', '.sh', '.ps1', '.cmd', '.bat',
  '.vue', '.css', '.scss', '.html', '.json', '.toml', '.yaml', '.yml', '.md', '.txt', '.sql', '.lock',
]);
const EXCLUDED_DIRS = new Set([
  'node_modules', 'target', 'dist', 'coverage', 'tmp', 'temp',
  'binaries', 'web-dist', 'webview', 'logs', 'results', 'quality-results', '__pycache__',
]);
export function normalizePath(value = '') {
  if (typeof value !== 'string' || value.includes('\0') || value.includes(':')) throw new Error('Use a repository-relative literal path.');
  const normalized = value.replaceAll('\\', '/').replace(/\/$/, '');
  if (normalized.startsWith('/') || normalized.split('/').some(p => p === '..' || p === '.')) {
    throw new Error('Use a repository-relative path without traversal.');
  }
  return normalized;
}

export function isApproved(file) {
  if (ROOT_FILES.includes(file)) return true;
  const parts = file.split('/');
  if (parts.length < 2 || !ROOTS.includes(parts[0])) return false;
  if (parts.some(p => p.startsWith('.') || /^tmp[-_]/i.test(p)
    || EXCLUDED_DIRS.has(p.toLowerCase()))) return false;
  if (/^(apps\/desktop\/src-tauri\/resources|docs\/performance\/[^/]+\/)/.test(file)) return false;
  const name = parts.at(-1).toLowerCase();
  if (/^(credentials?|secrets?|tokens?)([._-]|$)/.test(name) || /(?:^|[.-])env(?:[.-]|$)/.test(name)
    || /(?:^|[.-])private(?:[.-]|$)/.test(name) || /\.(?:local|log)\./.test(name)) return false;
  if (parts[0] === 'docs') return name.endsWith('.md');
  return EXTENSIONS.has(path.posix.extname(name)) || name === 'license' || name === 'dockerfile';
}

export function validateScope(value = '') {
  const scope = normalizePath(value);
  if (scope && !ROOTS.some(root => scope === root || scope.startsWith(root + '/')) && !ROOT_FILES.includes(scope)) {
    throw new Error('Path is outside the approved project scope.');
  }
  return scope;
}

export const inScope = (file, scope) => !scope || file === scope || file.startsWith(scope + '/');
