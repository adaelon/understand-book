// PB5-3 共享:把书路径载成续建各 CLI 需要的确定性派生(source/lidNodes/byLid/windows)。
// 零落盘、零 LLM；旧 EPUB 的规范正文从已有 base.json 恢复 LID，窗口按当前规则计算。
import type { SourceBlock } from "../../packages/core/src/segment";
import { loadBookSource } from "../../packages/core/src/book-source";
import { splitWindows, type Window } from "../../packages/core/src/window";
import type { LidNode } from "../../packages/core/src/generated/LidNode";

export interface LoadedBook {
  source: string;
  blocks: SourceBlock[];
  lidNodes: LidNode[];
  byLid: Map<string, LidNode>;
  windows: Window[];
}

/** 载入来源快照及其结构，确定性计算窗口。 */
export function loadBookWindows(book: string): LoadedBook {
  const { source, blocks, lidNodes } = loadBookSource(book);
  const byLid = new Map(lidNodes.map((n) => [n.lid, n]));
  const windows = splitWindows(lidNodes, source);
  return { source, blocks, lidNodes, byLid, windows };
}

/** 按 id 取窗口;不存在则报错列出合法 id 范围。 */
export function windowById(windows: Window[], id: number): Window {
  const w = windows.find((x) => x.id === id);
  if (!w) {
    const ids = windows.map((x) => x.id);
    throw new Error(`窗口 id=${id} 不存在(合法 id: ${ids[0]}..${ids[ids.length - 1]},共 ${ids.length} 窗)`);
  }
  return w;
}
