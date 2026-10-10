// agent 答案的 Markdown + LaTeX 渲染。
// 安全:html:false ⇒ 转义 LLM 输出里的原始 HTML(防 XSS);KaTeX 输出由插件自身规则注入,可信。
// 数学:$...$ 行内 / $$...$$ 独立公式(@traptitech/markdown-it-katex,throwOnError 关闭=语法错时显红字不崩)。
// PDF/Markdown 清洗常把作者单位上标写成 `$ ^{1,2} $`;这种裸上标不是公式,应显示为文本右上标。
// 其它行内数学若写成 `$ x^2 $`,渲染前只对明显像 TeX 的单 `$...$` 做内侧 trim。
// $$...$$ display math 插件要求独占行;用户/agent 常把 $$ 和文字挤在同一行(note、answer),
// 插件当场跳过不渲染。renderMarkdown 前做一道 normalizeDisplayMath:把内联 $$ 前后补换行规整成独占行,
// 不动已独占行的 $$、不动 $...$ 行内公式。
import MarkdownIt from "markdown-it";
import katexPlugin from "@traptitech/markdown-it-katex";
import katex from "katex";

const md = new MarkdownIt({
  html: false, // 不放行原始 HTML(XSS 防线)
  linkify: true, // 裸 URL 自动成链
  breaks: true, // 单换行 → <br>,贴合聊天气泡
});
md.use(katexPlugin, { throwOnError: false });

const SUP_TOKEN_PREFIX = "@@UB_INLINE_SUP_";
const SUP_TOKEN_SUFFIX = "@@";

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function isEscapedDollar(src: string, index: number): boolean {
  let slashCount = 0;
  for (let i = index - 1; i >= 0 && src[i] === "\\"; i -= 1) slashCount += 1;
  return slashCount % 2 === 1;
}

function looksLikeTex(src: string): boolean {
  return /^(?:[\\^_{]|[A-Za-z]+\s*[_^]|[A-Za-z]+\\|.*\\[A-Za-z])/.test(src);
}

function extractInlineSuperscripts(src: string): { src: string; superscripts: string[] } {
  const superscripts: string[] = [];
  const next = src.replace(/\$\s*\^\{([^{}]+)\}\s*\$/g, (m, body: string, offset: number) => {
    if (isEscapedDollar(src, offset) || src[offset - 1] === "$" || src[offset + m.length] === "$") return m;
    const text = body.trim();
    if (!text || text.length > 64) return m;
    const token = `${SUP_TOKEN_PREFIX}${superscripts.length}${SUP_TOKEN_SUFFIX}`;
    superscripts.push(text);
    return token;
  });
  return { src: next, superscripts };
}

function restoreInlineSuperscripts(html: string, superscripts: string[]): string {
  return superscripts.reduce((acc, text, index) => {
    return acc.replaceAll(
      `${SUP_TOKEN_PREFIX}${index}${SUP_TOKEN_SUFFIX}`,
      `<sup class="inline-citation-sup">${escapeHtml(text)}</sup>`,
    );
  }, html);
}

/** trim `$ ... $` 的内侧空白,但只处理看起来像 TeX 的单美元 inline math。 */
function normalizeInlineMath(src: string): string {
  let out = "";
  let cursor = 0;
  while (cursor < src.length) {
    const open = src.indexOf("$", cursor);
    if (open < 0) return out + src.slice(cursor);
    if (isEscapedDollar(src, open) || src[open + 1] === "$") {
      out += src.slice(cursor, open + 1);
      cursor = open + 1;
      continue;
    }
    let close = open + 1;
    while (close < src.length) {
      close = src.indexOf("$", close);
      if (close < 0 || (!isEscapedDollar(src, close) && src[close + 1] !== "$")) break;
      close += 1;
    }
    if (close < 0) return out + src.slice(cursor);
    const raw = src.slice(open + 1, close);
    const trimmed = raw.trim();
    out += src.slice(cursor, open);
    out += trimmed && raw !== trimmed && looksLikeTex(trimmed) ? `$${trimmed}$` : src.slice(open, close + 1);
    cursor = close + 1;
  }
  return out;
}

/** 把内联 $$...$$ 规整成独占行,让 katex 插件识别为 display math。 */
function normalizeDisplayMath(src: string): string {
  return src.replace(/\$\$([^$]+)\$\$/g, (m, tex, offset, str) => {
    const before = offset > 0 ? str[offset - 1] : "\n";
    const afterEnd = offset + m.length;
    const after = afterEnd < str.length ? str[afterEnd] : "\n";
    const needBefore = before !== "\n" && before !== "\r";
    const needAfter = after !== "\n" && after !== "\r";
    return (needBefore ? "\n" : "") + "$$" + tex + "$$" + (needAfter ? "\n" : "");
  });
}

/** 把 agent 答案文本渲染成 HTML(供 v-html)。空串安全。 */
export function renderMarkdown(src: string | null | undefined): string {
  const extracted = extractInlineSuperscripts(normalizeDisplayMath(src ?? ""));
  return restoreInlineSuperscripts(md.render(normalizeInlineMath(extracted.src)), extracted.superscripts);
}
/** 渲染行内 Markdown/LaTeX,不包外层 <p>;用于 reader 段落正文。 */
export function renderInlineMarkdown(src: string | null | undefined): string {
  const extracted = extractInlineSuperscripts(src ?? "");
  return restoreInlineSuperscripts(md.renderInline(normalizeInlineMath(extracted.src)), extracted.superscripts);
}

function renderHtmlTableContent(node: Node): string {
  if (node.nodeType === Node.TEXT_NODE) return renderInlineMarkdown(node.textContent);
  if (!(node instanceof Element)) return "";
  const tag = node.tagName.toLowerCase();
  if (tag === "script" || tag === "style") return "";
  if (tag === "br") return "<br>";
  if (tag === "code") return `<code>${escapeHtml(node.textContent ?? "")}</code>`;
  const body = Array.from(node.childNodes).map(renderHtmlTableContent).join("");
  if (["strong", "b", "em", "i", "sup", "sub"].includes(tag)) return `<${tag}>${body}</${tag}>`;
  if (tag === "a") {
    const href = node.getAttribute("href") ?? "";
    if (href && md.validateLink(href)) return `<a href="${escapeHtml(md.normalizeLink(href))}">${body}</a>`;
  }
  return body;
}

function renderHtmlTableStructure(node: Element): string {
  const tag = node.tagName.toLowerCase();
  if (tag === "td" || tag === "th") {
    const spans = ["colspan", "rowspan"].map((name) => {
      const value = Number(node.getAttribute(name));
      return Number.isInteger(value) && value > 1 ? ` ${name}="${value}"` : "";
    }).join("");
    const align = (node as HTMLElement).style.textAlign || node.getAttribute("align");
    const alignment = align && ["left", "center", "right"].includes(align) ? ` style="text-align:${align}"` : "";
    return `<${tag}${spans}${alignment}>${Array.from(node.childNodes).map(renderHtmlTableContent).join("")}</${tag}>`;
  }
  if (tag === "caption") return `<caption>${Array.from(node.childNodes).map(renderHtmlTableContent).join("")}</caption>`;
  if (["table", "thead", "tbody", "tfoot", "tr"].includes(tag)) {
    return `<${tag}>${Array.from(node.children).map(renderHtmlTableStructure).join("")}</${tag}>`;
  }
  return "";
}

/** Display a table asset without changing its canonical text or source offsets. */
export function renderTableSource(src: string): string {
  const text = src.trim();
  if (/^<table(?:\s|>)/i.test(text)) {
    const table = new DOMParser().parseFromString(text, "text/html").querySelector("table");
    if (table) return renderHtmlTableStructure(table);
  }

  const markdown = renderMarkdown(src);
  if (/^\s*<table>/.test(markdown)) return markdown;

  // EPUB's canonical serialization has pipe rows but no header separator or th/td metadata.
  // Keep every row as data, and its cell text literal, rather than inventing a header or Markdown.
  const lines = text.split(/\r?\n/).map((line) => line.trim());
  if (lines.every((line) => line.startsWith("|") && line.endsWith("|") && line.length > 1)) {
    const rows = lines.map((line) => {
      const cells = line.slice(1, -1).split("|").map((cell) => `<td>${escapeHtml(cell.trim())}</td>`).join("");
      return `<tr>${cells}</tr>`;
    }).join("");
    return `<table><tbody>${rows}</tbody></table>`;
  }
  return `<pre class="table-source-fallback">${escapeHtml(src)}</pre>`;
}

/** Formula LID 可来自段内 $...$，也可来自独占块的 $$...$$ 或多行 $...$。 */
export function isDisplayFormulaSource(src: string): boolean {
  const text = src.trim();
  return (text.startsWith("$$") && text.endsWith("$$"))
    || /^\$\r?\n[\s\S]+\r?\n\$$/.test(text);
}

export function renderFormulaSource(src: string): string {
  if (!isDisplayFormulaSource(src)) return renderInlineMarkdown(src);
  const text = src.trim();
  const delimiter = text.startsWith("$$") ? "$$" : "$";
  const rendered = katex.renderToString(text.slice(delimiter.length, -delimiter.length), {
    displayMode: true,
    throwOnError: false,
  });
  return delimiter === "$" ? `<span data-formula-delimiter="single">${rendered}</span>` : rendered;
}
