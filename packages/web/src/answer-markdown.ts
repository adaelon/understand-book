import { t as uiText } from './i18n';
import type { AgentAnswerPart, OuterOutcome } from './api';
import type { AgentAnswerSource } from './generated/AgentAnswerSource';
import { renderMarkdown } from './md';
import { sourceChipLabel } from './source-chip.js';
import { pitchName } from './music-example-player';

export type AnswerDisplayPart = { kind: "html"; html: string; markdown: string[]; blocks?: string[]; socratic?: boolean } | Extract<AgentAnswerPart, { kind: "presentation" }>;

/** Bind raw teaching citations only to sources already published with this answer. */
export function sourceMarkedParts(text: string, sources: AgentAnswerSource[]): AgentAnswerPart[] {
  const known = new Set(sources.map(source => source.source_ref_id));
  const parts: AgentAnswerPart[] = [];
  let cursor = 0;
  for (const match of text.matchAll(/\[\[source:([^\]]*)\]\]/g)) {
    parts.push({ kind: 'markdown', text: text.slice(cursor, match.index) });
    const id = match[1].trim();
    parts.push(known.has(id) ? { kind: 'sources', source_ref_ids: [id] } : { kind: 'markdown', text: uiText('（来源不可用）') });
    cursor = match.index! + match[0].length;
  }
  parts.push({ kind: 'markdown', text: text.slice(cursor) });
  return parts;
}

// Keep completed rendered blocks mounted while the active tail grows.
function draftMarkdownBlocks(html: string): string[] {
  const template = document.createElement("template");
  template.innerHTML = html;
  return Array.from(template.content.childNodes).flatMap(node => {
    if (node.nodeType === Node.ELEMENT_NODE) return [(node as Element).outerHTML];
    if (!node.textContent?.trim()) return [];
    const span = document.createElement("span"); span.textContent = node.textContent;
    return [span.outerHTML];
  });
}

const SOURCE_PLACEHOLDER = /\uE000UB_SOURCE_(\d+)\uE001/g;
const SOURCE_AFTER_TABLE_ROW = /^(\s*\|.*\|)[ \t]*(\uE000UB_SOURCE_\d+\uE001(?:[ \t]*\uE000UB_SOURCE_\d+\uE001)*)[ \t]*$/;
const SOURCE_ONLY_LINE = /^[ \t]*(\uE000UB_SOURCE_\d+\uE001(?:[ \t]*\uE000UB_SOURCE_\d+\uE001)*)[ \t]*$/;

function placeTableSourcesInCells(markdown: string): string {
  const lines = markdown.split(/\r?\n/);
  const putInLastCell = (row: string, tokens: string) => `${row.trimEnd().slice(0, -1)} ${tokens} |`;
  for (let index = 0; index < lines.length; index += 1) {
    const trailing = lines[index].match(SOURCE_AFTER_TABLE_ROW);
    if (trailing) {
      lines[index] = putInLastCell(trailing[1], trailing[2]);
      continue;
    }
    const standalone = lines[index].match(SOURCE_ONLY_LINE);
    if (standalone && index > 0 && /^\s*\|.*\|\s*$/.test(lines[index - 1])) {
      lines[index - 1] = putInLastCell(lines[index - 1], standalone[1]);
      lines.splice(index, 1);
      index -= 1;
    }
  }
  return lines.join("\n");
}

function inlineSourceButton(part: Extract<AgentAnswerPart, { kind: "sources" }>, index: number, sources: NonNullable<OuterOutcome["answer_view"]>["sources"], draft: boolean, canOpen: boolean): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = draft ? "agent-source-button draft-source" : "agent-source-button";
  button.dataset.answerSourceIndex = String(index);
  button.setAttribute('data-reader-selection-ignore', '');
  button.disabled = !draft && !canOpen;
  if (!draft) {
    const icon = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    icon.setAttribute("width", "14");
    icon.setAttribute("height", "14");
    icon.setAttribute("viewBox", "0 0 24 24");
    icon.setAttribute("fill", "none");
    icon.setAttribute("stroke", "currentColor");
    icon.setAttribute("stroke-width", "2");
    icon.setAttribute("stroke-linecap", "round");
    icon.setAttribute("stroke-linejoin", "round");
    icon.setAttribute("aria-hidden", "true");
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("d", "M12 7v14M3 18V5a2 2 0 0 1 2-2h4a3 3 0 0 1 3 3 3 3 0 0 1 3-3h4a2 2 0 0 1 2 2v13a1 1 0 0 1-1 1h-5a3 3 0 0 0-3 3 3 3 0 0 0-3-3H4a1 1 0 0 1-1-1Z");
    icon.append(path);
    button.append(icon);
  }
  const label = document.createElement("span");
  label.textContent = part.source_ref_ids.length === 1
    ? sourceChipLabel(sources, part.source_ref_ids[0], uiText('来源不可用'))
    : uiText('{0} 个来源', [part.source_ref_ids.length]);
  button.append(label);
  return button;
}

function inlineMusicExample(example: import('./generated/MusicExample').MusicExample, index: number, draft: boolean): HTMLElement {
  const group = document.createElement('span');
  group.className = 'music-example-inline';
  const play = document.createElement('button');
  play.type = 'button'; play.className = 'music-example-play'; play.textContent = '▶';
  play.dataset.musicExampleIndex = String(index); play.disabled = draft;
  play.setAttribute('aria-label', uiText('试听') + ' ' + example.label);
  play.setAttribute('data-reader-selection-ignore', '');
  group.append(play);
  for (const token of example.label.split(/( → | \+ )/)) {
    const pitch = example.notes.find(note => {
      return token === pitchName(note.midi_pitch);
    });
    if (!pitch) {
      if (token === example.label) {
        const phrase = document.createElement('button');
        phrase.type = 'button'; phrase.className = 'music-example-note'; phrase.textContent = token;
        phrase.dataset.musicExampleIndex = String(index); phrase.disabled = draft;
        group.append(phrase);
      } else { group.append(document.createTextNode(token)); }
      continue;
    }
    const button = document.createElement('button');
    button.type = 'button'; button.className = 'music-example-note'; button.textContent = token;
    button.dataset.musicExampleIndex = String(index); button.dataset.musicPitch = String(pitch.midi_pitch); button.disabled = draft;
    button.setAttribute('aria-label', uiText('试听') + ' ' + token);
    group.append(button);
  }
  return group;
}

export function answerDisplayParts(parts: AgentAnswerPart[], sources: NonNullable<OuterOutcome["answer_view"]>["sources"], draft = false, canOpen = true, render: (source: string) => string = renderMarkdown): AnswerDisplayPart[] {
  const display: AnswerDisplayPart[] = [];
  let markdown = "";
  let markdownParts: string[] = [];
  const inlineSources = new Map<number, Extract<AgentAnswerPart, { kind: "sources" | "music_example" }>>();
  let hasMusic = false;
  const flush = () => {
    if (!markdown) return;
    if (inlineSources.size === 0) {
      display.push({ kind: "html", html: render(markdown), markdown: markdownParts });
      markdown = "";
      markdownParts = [];
      return;
    }
    const template = document.createElement("template");
    template.innerHTML = render(placeTableSourcesInCells(markdown));
    const walker = document.createTreeWalker(template.content, NodeFilter.SHOW_TEXT);
    const textNodes: Text[] = [];
    while (walker.nextNode()) textNodes.push(walker.currentNode as Text);
    for (const node of textNodes) {
      const content = node.textContent ?? "";
      SOURCE_PLACEHOLDER.lastIndex = 0;
      if (!SOURCE_PLACEHOLDER.test(content)) continue;
      SOURCE_PLACEHOLDER.lastIndex = 0;
      const replacement = document.createDocumentFragment();
      let cursor = 0;
      for (const match of content.matchAll(SOURCE_PLACEHOLDER)) {
        const index = Number(match[1]);
        const source = inlineSources.get(index);
        if (!source) continue;
        replacement.append(document.createTextNode(content.slice(cursor, match.index)));
        replacement.append(source.kind === "music_example" ? inlineMusicExample(source.example, index, draft) : inlineSourceButton(source, index, sources, draft, canOpen));
        cursor = match.index + match[0].length;
      }
      replacement.append(document.createTextNode(content.slice(cursor)));
      node.replaceWith(replacement);
    }
    display.push({ kind: "html", html: template.innerHTML, markdown: hasMusic ? [markdownParts.join('')] : markdownParts });
    markdown = "";
    markdownParts = [];
    inlineSources.clear();
    hasMusic = false;
  };
  parts.forEach((part, index) => {
    if (part.kind === "markdown") { markdown += part.text; markdownParts.push(part.text); }
    else if (part.kind === "socratic_question") {
      flush();
      display.push({ kind: "html", html: render(part.text), markdown: [part.text], socratic: true });
    }
    else if (part.kind === "sources") {
      markdown += `\uE000UB_SOURCE_${index}\uE001`;
      inlineSources.set(index, part);
    } else if (part.kind === "music_example") {
      markdown += `\uE000UB_SOURCE_${index}\uE001`;
      markdownParts.push(part.example.label);
      inlineSources.set(index, part);
      hasMusic = true;
    } else {
      flush();
      display.push(part);
    }
  });
  flush();
  return draft ? display.map(part => part.kind === 'html' ? { ...part, blocks: draftMarkdownBlocks(part.html) } : part) : display;
}

