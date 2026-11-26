#!/usr/bin/env node
import { randomUUID } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, realpathSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import path from 'node:path';

function requireText(object, keys, label) {
  for (const key of keys) {
    if (typeof object?.[key] !== 'string' || !object[key].trim()) {
      throw new Error(`${label}.${key} must be a non-empty string`);
    }
  }
}

function readSourceLines(filename) {
  const bytes = readFileSync(filename);
  let source;
  try {
    source = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  } catch {
    throw new Error(`Expected UTF-8 source in ${filename}`);
  }
  const lines = source.split(/\r\n|\n|\r/);
  if (lines.length > 1 && lines.at(-1) === '') lines.pop();
  return lines;
}

function collectTour(root, tour, output) {
  requireText(tour, ['project', 'title', 'summary'], 'tour');
  if (!Array.isArray(tour.steps) || tour.steps.length === 0) {
    throw new Error('tour.steps must contain at least one step');
  }
  const ids = new Set();
  const files = new Map();
  const steps = tour.steps.map((step, index) => {
    const label = `steps[${index}]`;
    requireText(step, ['id', 'title', 'symbol', 'file', 'explanation'], label);
    if (ids.has(step.id)) throw new Error(`${label}: duplicate step id "${step.id}"`);
    ids.add(step.id);
    if (!Array.isArray(step.next) || step.next.some(id => typeof id !== 'string' || !id.trim())) {
      throw new Error(`${label}.next must be an array of step ids`);
    }
    if (path.isAbsolute(step.file)) throw new Error(`${label}.file must be relative to the repository root`);
    const filename = realpathSync(path.resolve(root, step.file));
    const relative = path.relative(root, filename);
    if (relative === '..' || relative.startsWith('..' + path.sep) || path.isAbsolute(relative)) {
      throw new Error(`${label}.file is outside the repository: ${step.file}`);
    }
    if (path.relative(filename, output) === '') throw new Error(`Output would overwrite source: ${step.file}`);
    const file = relative.split(path.sep).join('/');
    if (!files.has(file)) files.set(file, { path: file, lines: readSourceLines(filename) });
    const length = files.get(file).lines.length;
    if (!Number.isInteger(step.start) || !Number.isInteger(step.end) ||
        step.start < 1 || step.end < step.start || step.end > length) {
      throw new Error(`${label} "${step.id}": expected an inclusive range within ${file}:1–${length}, got ${step.start}–${step.end}`);
    }
    return { ...step, file };
  });
  for (const step of steps) {
    for (const id of step.next) {
      if (!ids.has(id)) throw new Error(`Step "${step.id}" links to unknown step "${id}"`);
    }
  }
  return { ...tour, steps, files: [...files.values()] };
}

function render(tour) {
  const asset = name => readFileSync(new URL('../assets/' + name, import.meta.url), 'utf8');
  const json = JSON.stringify(tour).replaceAll('<', '\\u003c');
  const slots = {
    STYLE: `<style>${asset('viewer.css')}</style>`,
    DATA: `<script id="tour-data" type="application/json">${json}</script>`,
    SCRIPT: `<script>${asset('viewer.js')}</script>`,
  };
  return asset('viewer.html').replace(/<!--TOUR_(STYLE|DATA|SCRIPT)-->/g, (_, slot) => slots[slot]);
}

function build(rootArg, tourArg, outputArg) {
  const root = realpathSync(rootArg);
  const tourFile = realpathSync(tourArg);
  const output = path.resolve(outputArg);
  const destination = existsSync(output) ? realpathSync(output) : output;
  if (path.relative(tourFile, destination) === '') throw new Error('Output would overwrite the tour manifest');
  const tour = JSON.parse(readFileSync(tourFile, 'utf8').replace(/^\uFEFF/, ''));
  const html = render(collectTour(root, tour, destination));
  const directory = path.dirname(output);
  mkdirSync(directory, { recursive: true });
  const temporary = path.join(directory, '.source-walkthrough-' + randomUUID() + '.tmp');
  try {
    writeFileSync(temporary, html, { flag: 'wx' });
    renameSync(temporary, output);
  } finally {
    rmSync(temporary, { force: true });
  }
  console.log(output);
}

try {
  const args = process.argv.slice(2);
  if (args.length !== 3) throw new Error('Usage: node build.mjs <repo-root> <tour.json> <output.html>');
  build(...args);
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
