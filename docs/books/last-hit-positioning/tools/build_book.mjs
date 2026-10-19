import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const require = createRequire(path.resolve(root, '../../../packages/web/package.json'));
const MarkdownIt = require('markdown-it');
const katex = require('katex');

const core = (await fs.readFile(path.join(root, 'examples/upstream/last_hit_positioning_v3.py'), 'utf8')).replaceAll('\r\n', '\n');
const license = (await fs.readFile(path.join(root, 'examples/upstream/LICENSE'), 'utf8')).trim();
const model = core.slice(0, core.indexOf('\ndef point_text'));
if (!model.includes('def simulate_window') || model.length < 1000) throw new Error('Reference core selection is missing');
const demo = `

if __name__ == "__main__":
    g = solve_geometry(
        enemy=(0, 0), own_minion=(4, 0), target_minion=(3, 10),
        enemy_aa=6, self_aa=2, self_cast=8, skill_range=5,
        safety_margin=0.5, aa_mode="effective",
    )
    sim = simulate_window(
        g, self_ms=5, enemy_ms=3, reaction=0.2, windup=0.2,
        remaining_cooldown=1.2, dt=0.002,
    )
    assert sim.ok and sim.window_valid
    assert abs(sim.attack_time - 0.7) < 1e-9
    assert abs(sim.return_time - 1.0) < 1e-9
    print("W1:", round(sim.dist_ready, 6), sim.window_valid)
`;
const standalone = license.split(/\r?\n/).map(line => '# ' + line).join('\n') + '\n\n' + model + demo;
await fs.writeFile(path.join(root, 'examples/standalone_model.py'), standalone, 'utf8');
const tick = String.fromCharCode(96);
const appendixD = `# 附录 D 可独立运行的参考实现

下面的完整代码可复制到一个 Python 文件中运行，只使用标准库。它收录固定上游版本中从向量、区间、技能形状到短时模拟的连续定义，省去后面的控制台说明输出函数，并附上 W1 运行示例。相同代码也保存为 [standalone_model.py](../examples/standalone_model.py)。

几何与模拟定义取自 Deeki-Sora 的 ${tick}last_hit_positioning_v3.py${tick}，提交 ${tick}50db387dd1218f5a0578183bdfd031cdb9375573${tick}；许可声明已包含在代码顶部。本书新增文末的运行示例。变量和输出含义见附录 A、B。

运行后输出 ${tick}W1: 9.337409 True${tick}。改变目标坐标或速度时，文末针对 W1 的两项时间断言与成立断言也应随新场景调整；函数本身可直接用于其他参数。

\`\`\`python
${standalone.trim()}
\`\`\`
`;
await fs.writeFile(path.join(root, 'appendices/D-参考实现.md'), appendixD, 'utf8');
const appendixB = await fs.readFile(path.join(root, 'appendices/B-计算接口与案例.md'), 'utf8');
const quickstart = appendixB.match(/```python\n([\s\S]*?)\n```/);
if (!quickstart) throw new Error('Appendix B example is missing');
await fs.writeFile(path.join(root, 'examples/quickstart.py'), quickstart[1] + '\n', 'utf8');

const parts = [];
for (const directory of ['chapters', 'appendices']) {
    const names = (await fs.readdir(path.join(root, directory))).filter(name => name.endsWith('.md')).sort();
    for (const name of names) {
        let text = (await fs.readFile(path.join(root, directory, name), 'utf8')).replaceAll('\r\n', '\n');
        let fence = false;
        text = text.split('\n').map(line => {
            if (line.startsWith('```')) fence = !fence;
            return !fence && /^#{1,5} /.test(line) ? '#' + line : line;
        }).join('\n');
        parts.push(text.trim().replaceAll('](../', ']('));
    }
}
const title = '# 这一刀怎么补\n\n看懂对线站位与出手机会\n\n';
const manuscript = title + parts.join('\n\n') + '\n';
await fs.writeFile(path.join(root, 'book.md'), manuscript, 'utf8');

const md = new MarkdownIt({html:true, linkify:false, typographer:false});
const toc = [];
let heading = 0;
md.renderer.rules.heading_open = (tokens, idx, options, env, self) => {
    const id = 'section-' + (++heading);
    tokens[idx].attrSet('id', id);
    if (tokens[idx].tag === 'h2') toc.push({id, title:tokens[idx+1].content});
    return self.renderToken(tokens, idx, options);
};
let mathCount = 0;
const renderedMath = parts.join('\n\n').replace(/\$\$([\s\S]*?)\$\$/g, (_, source) => {
    mathCount++;
    return katex.renderToString(source, {displayMode:true, output:'mathml', throwOnError:true});
}).replace(/(?<!\$)\$([^\n$]+)\$(?!\$)/g, (_,source) => {
    mathCount++;
    return katex.renderToString(source, {displayMode:false, output:'mathml', throwOnError:true});
});
const body = md.render(renderedMath);
const escape = value => md.utils.escapeHtml(value);
const html = `<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>这一刀怎么补 · 看懂对线站位与出手机会</title>
<style>
:root{color-scheme:light;--ink:#23364a;--muted:#64747a;--paper:#fbfaf6;--accent:#287e69}
*{box-sizing:border-box}body{margin:0;background:var(--paper);color:var(--ink);font-family:"Microsoft YaHei",sans-serif}
a{color:#276b7d;text-decoration-thickness:1px;text-underline-offset:4px}a:hover{color:#a34d40}
.cover{background:#20364b;color:#f7f3e7;padding:76px max(6vw,24px) 66px;position:relative;overflow:hidden}
.cover:after{content:"";position:absolute;width:370px;height:370px;border:1px solid #bf8d49;border-radius:50%;right:5%;top:-100px;opacity:.35;pointer-events:none}
.cover .eyebrow{font-size:14px;letter-spacing:5px;color:#b8d5ca}.cover h1{font-size:clamp(36px,5vw,64px);letter-spacing:3px;margin:20px 0 18px;position:relative;z-index:1}.cover .sub{font-size:23px;color:#d9e5e5}.cover p{max-width:720px;line-height:1.9;font-size:17px;color:#d2dddd}.cover .meta{font-size:14px;margin-top:30px;color:#b9ccc8}
.layout{display:grid;grid-template-columns:235px minmax(0,790px);gap:62px;max-width:1175px;margin:0 auto;padding:52px 28px 100px}
nav{position:sticky;top:24px;align-self:start;max-height:calc(100vh - 48px);overflow:auto;font-size:14px;line-height:1.7;padding-right:10px}nav summary{font-size:16px;font-weight:bold;margin-bottom:16px;color:var(--accent)}nav ul{list-style:none;padding:0;margin:0}nav li{margin:0 0 13px}nav a{color:var(--muted);text-decoration:none}nav a:hover{text-decoration:underline}
article{font-family:"Microsoft YaHei",sans-serif;font-size:18px;line-height:1.95;min-width:0}article p{margin:1.2em 0}article h2{font-size:30px;line-height:1.5;padding-top:34px;margin:50px 0 26px;border-top:2px solid #b7cabe;color:#243e4a;scroll-margin-top:25px}article h2:first-child{margin-top:0}article h3{font-size:22px;line-height:1.6;margin:42px 0 17px;color:#2b625e}article ul{padding-left:1.35em}article li{padding-left:.2em;margin:.5em 0}
article img{display:block;max-width:100%;height:auto;border:1px solid #e1e4dd;border-radius:6px;margin:26px auto 10px}article p:has(img){margin-bottom:10px}article p:has(img)+p{font-size:15px;line-height:1.85;color:var(--muted);margin-top:8px;margin-bottom:30px}table{border-collapse:collapse;display:block;max-width:100%;overflow-x:auto;font-size:14px;line-height:1.7;margin:28px 0}thead{background:#e9efea}th,td{padding:12px 14px;text-align:left;border-bottom:1px solid #d5dfd7;vertical-align:top}th{font-weight:600}tbody tr:nth-child(even){background:#f3f4ed}th:first-child,td:first-child{min-width:75px}
code{font-family:Consolas,"Microsoft YaHei",monospace;font-size:.88em;background:#edf0eb;padding:2px 5px;border-radius:3px;overflow-wrap:anywhere}pre{background:#edf0eb;padding:22px;border-radius:6px;overflow:auto;font-size:14px;line-height:1.7}pre code{padding:0;overflow-wrap:normal}.katex-display{display:block;margin:26px 0;max-width:100%;overflow:auto;padding:8px 0}.katex-display math{display:block;font-size:1.06em}.katex{line-height:1.5}blockquote{border-left:3px solid var(--accent);margin:24px 0;padding:0 20px;color:var(--muted)}.footer{max-width:790px;margin:0 auto 50px;padding:20px 28px;color:var(--muted);font-size:14px}
.katex:has(>math[display="block"]){display:block;max-width:100%;overflow-x:auto;padding:10px 0;margin:20px 0}.katex>math[display="block"]{min-width:max-content}
@media(max-width:900px){.layout{display:block;padding:32px 22px 65px;max-width:800px}nav{position:static;max-height:none;border-bottom:1px solid #cbd8ce;padding-bottom:20px;margin-bottom:30px}nav ul{columns:2;column-gap:26px}nav li{break-inside:avoid}article{font-size:17px}.cover{padding:42px 24px}.cover .sub{font-size:20px}}
@media(max-width:520px){nav ul{columns:1}article h2{font-size:26px}article h3{font-size:21px}article{font-size:16.5px;line-height:1.95}th,td{padding:9px 10px}pre{padding:14px;font-size:12px}.layout{padding-left:18px;padding-right:18px}.cover .meta{line-height:1.8}}
@media print{.cover{background:white;color:black;padding:30px 0}.cover p,.cover .sub,.cover .meta{color:#333}.cover:after,nav{display:none}.layout{display:block;padding:0;max-width:none}article{font-size:11pt;line-height:1.7}article h2{break-before:page}article img{max-height:235mm;width:auto;max-width:100%;break-inside:avoid}pre{white-space:pre-wrap}a{color:inherit;text-decoration:none}.footer{display:none}}
</style></head><body>
<header class="cover"><div class="eyebrow">对线观察与判断</div><h1>这一刀怎么补</h1><div class="sub">看懂对线站位与出手机会</div><p>把一段对线暂停，再慢慢播放。看清目标的位置、对手的动作，以及这一刀之后准备退向哪里。</p><div class="meta">七章正文 · 八幅图解 · 数学建模与代码附录</div></header>
<div class="layout"><nav aria-label="全书目录"><details open><summary>阅读目录</summary><ul>${toc.map(item=>`<li><a href="#${item.id}">${escape(item.title)}</a></li>`).join('')}</ul></details></nav><article>${body}</article></div>
<footer class="footer">内容依据、版本与许可见来源说明。<a href="book.md">完整 Markdown 书稿</a> · <a href="examples/book_cases.py">可复算案例</a></footer>
</body></html>`;
await fs.writeFile(path.join(root, 'book.html'), html, 'utf8');
console.log(JSON.stringify({parts:parts.length, mathExpressions:mathCount, chineseCharacters:(manuscript.match(/[\u3400-\u9fff]/g)||[]).length, markdownBytes:Buffer.byteLength(manuscript), headings:toc.length},null,2));
