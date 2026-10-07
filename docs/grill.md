> Tutor 设计已于 2026-09-29 收敛并获准落档。当前合同见 [Tutor 切片方案](切片方案-Tutor全局模式与教学闭环.md) 与 ADR-0141–0144；下文保留历史讨论，后续收敛记录优先于当时的候选和待议描述。T1–T13 均待实施。

 看最新的adr和切片方案-paper-first..,和我继续grill设计，每次你在问问题和说推荐方案之前先查看E:\allwork\download\agent\zotero-main\zotero-main\document-
  worker下它有没有相关设计


• 我会继续按 software-engineering 的 §0.5 Grill 来做：先读最新 ADR 和 paper-pdf-first-hybrid 切片方案；之后每次提问和给推荐前，先去你指定的 Zotero document-
  worker 目录查有没有对应设计。

• 本地切片方案已经明确了 clean-room 边界：Zotero 只作架构参考，不能搬源码、协议、bundle、模型或 fork 路径。我现在读 ADR-0062，并在 Zotero document-worker 里
  查它对 PDF 文本、几何、渲染区域的分层方式。

• ADR-0062 和切片方案一致：Markdown 保持语义/LID，PDF 只做视觉面，pdf_source_map 负责投影；Zotero 明确是 clean-room 参考。document-worker 的搜索结果很大，我
  会缩小到 PDF 文本、结构和渲染入口文件来提炼可借鉴的设计点。

• Zotero 相关设计点集中在 pdf/index.js（fulltext/char objects）、pdf/text.js（rect 到文字 range）、pdf/renderer.js（区域渲染）和 pdf/structure/
  structure.js（chars/objects 到结构块）。我会只读这些入口片段，不复制实现。

• 关键参考点：Zotero 把 char stream、word/line rect、selection rect、区域 render 分开，而且有“rect 反查最近字符/范围”的逻辑；这支持我们把 pdf_source_map 设
  计成构建期视觉投影，再把 PDF 点击/选择反解到 LID。接下来我读结构入口，确认是否有 page furniture/结构块边界设计可参考。

• 我已先查了 Zotero document-worker 相关设计：它内部是 chars -> words/lines -> rects/ranges -> renderArea 分层，选择/高亮也依赖“rect 反查字符 range”。这给我
  们的启发是：不要只抽 word bbox，否则 PH5 的 PDF 选择反解到 LID 会很被动。

  Grill 问题 1：PH1 的 extractor 内部产物要不要从一开始就包含 chars + words + lines + page，但对外只持久化 pdf_source_map 的 word/line/block regions？

  我的推荐：要。

  // build-internal, not public reader contract
  interface PdfTextGeometry {
    pages: Array<{
      pageIndex: number;
      pageSize: { width: number; height: number };
      chars: PdfChar[];
      words: PdfWord[];
      lines: PdfLine[];
    }>;
  }

  理由：PH2 对齐可以用 words；PH4 渲染 overlay 用 persisted regions；PH5 PDF selection/region click 需要 chars/lines 做最近 LID 反解。这样不扩大公开契约，也
  不引入 Zotero 代码或命名，只借它的分层思想。

  你是否同意把 PH1 从“word extraction adapter”微调为“text geometry adapter：内部 chars/words/lines，公开 map 仍保持简洁”？


› 同意


• 收到。我会把 PH1 设计锁成“内部 text geometry，公开 source map 简洁”的方向。下一处最影响后续实现的是 bbox 坐标系；我先查 Zotero 对 PDF rect 与 viewport
  rect 的转换边界，再给推荐。

• 我查了 Zotero 的相关设计：它持久/内部使用 PDF page-space rect，再在渲染时通过 pdf.js viewport 转成屏幕坐标；renderArea 也是先把 rect fit 到 page.view，再
  convertToViewportPoint。这说明坐标系最好不要跟当前 canvas zoom/viewport 绑定。

  Grill 问题 2：pdf_source_map.json 里的 bbox 坐标系是否固定为 PDF page user-space，而不是 CSS 像素或归一化坐标？

  我的推荐：固定为 PDF page user-space，并在 map 里显式写清楚坐标约定。

  interface PdfSourceMap {
    coordinate_system: {
      space: "pdf_user_space";
      origin: "bottom_left";
      unit: "pt";
    };
    pages: Array<{
      pageIndex: number;
      width: number;
      height: number;
    }>;
  }

  前端渲染时：

  pdfRect -> page.getViewport({ scale }) -> viewportRect -> overlay CSS rect

  理由：PDF user-space 是稳定构建产物；CSS 像素会随 zoom、DPR、旋转和 canvas scale 变化；归一化坐标看似通用，但会让调试和 pdf.js 裁剪/区域渲染更麻烦。Zotero
  的分层也支持“PDF rect 作为真值，viewport rect 作为渲染派生物”。

  你是否同意把 pdf_source_map 的 bbox 统一定义为 PDF user-space，并新增 pages[] + coordinate_system 元数据？


› 同意


• 收到。我会把 bbox 坐标系锁为 PDF user-space，并给 map 增加 pages[] 和 coordinate_system 元数据。下一个关键点是 mapping 粒度和失败降级；我先查 Zotero 在结
  构块、page furniture 和 degraded fallback 上怎么分层。

• 我查了 Zotero 相关实现：它的结构提取即使模型/结构推断失败，也会退回到“按 PDF lines 合并成 paragraph fallback blocks”，并记录 degraded/fallback；重复页眉页
  脚会标成 excluded，不是直接混入正文结构。这对我们有两个设计启发：source map 需要“成功粒度”和“失败原因”，而不是只有有/无 bbox。

  Grill 问题 3：pdf_source_map 是否要允许每个 LID 记录一个 best-effort 映射状态，而不是只记录成功 entries？

  我的推荐：要，每个 leaf LID 都应有 mapping record，成功和失败统一进 entries，否则前端和验收很难区分“没生成、没匹配、被排除、低置信度降级”。

  interface PdfSourceMapEntry {
    lid: string;
    source_span: { start: number; end: number };

    status: "word_mapped" | "line_fallback" | "block_fallback" | "unmapped" | "excluded";
    pdf_regions: PdfRegion[];
    
    alignment: {
      matched_chars: number;
      total_chars: number;
      confidence: number;
      method: "md_to_pdf_words_v1";
      reason?: "below_threshold" | "page_furniture" | "no_pdf_text" | "ambiguous_match" | "empty_source";
    };
  }

  验收指标也就更清楚：

  mapped_leaf_lid_ratio =
    count(status in word_mapped | line_fallback | block_fallback) / count(all leaf LIDs)

  word_mapped_ratio =
    count(status == word_mapped) / count(all leaf LIDs)

  这样 PH4 可以对 unmapped 明确 fallback 到 Markdown source preview，PH2/PH3 可以把失败原因写入 report，后续调参不会靠猜。

  你是否同意 pdf_source_map.entries 覆盖所有 leaf LID，并用 status + reason 表达映射质量和失败原因？


› 同意


• 继续。下一个影响实现复杂度的是对齐算法的搜索策略：是全局任意匹配，还是利用论文/PDF自然阅读顺序做单调前向匹配。我先查 Zotero 在文本抽取和结构处理中对顺序、
  归一化、offset 的处理方式。

• 我查到 Zotero 的相关处理有两个稳定思路：PDF char stream 保留阅读顺序，word 由 spaceAfter/lineBreakAfter/pageIndex 切分；文本会做 NFC/NFKC 归一化，并在
  DOM/结构文本里用 delta map 保留“归一化文本位置 -> 原始位置”的关系。它不是任意全局匹配，而是尽量维护顺序和 offset 可追踪性。

  Grill 问题 4：Markdown 到 PDF 的对齐是否应采用“单调前向匹配”，而不是每个 LID 在整篇 PDF 里全局搜索最佳匹配？

  我的推荐：采用单调前向匹配，允许有限回看窗口。

  for block in markdownBlocksInLidOrder:
      pdfWindow = pdfWords[cursor - lookback .. cursor + lookahead]
      match = fuzzy_match(normalize(block.text), normalize(pdfWindow))
      if match.confidence >= threshold:
          emit mapped region
          cursor = max(cursor, match.endWordIndex)
      else:
          emit unmapped/low-confidence reason

  建议默认：

  lookback_words = 80
  lookahead_words = dynamic, e.g. max(800, block_word_count * 8)

  理由：论文正文和 born-digital PDF 的自然阅读顺序大体一致；单调匹配能避免 repeated phrases、section headings、figure captions 把 LID 错配到远处。有限回看可
  以处理 Markdown 清洗时移动了标题/脚注、PDF 两栏换行、页眉页脚等小错位。

  我不建议 v1 做全局最佳匹配，因为它会提高“看似高置信但位置错”的风险，尤其是综述论文里重复术语和小标题很多。

  你是否同意 PH2 的核心算法锁为“单调前向 fuzzy alignment + 有限回看”，而不是全局搜索？


› 同意


• 同意点我已锁定：PH2 用单调前向 fuzzy alignment，加有限回看，避免重复短语导致远距离错配。下一个设计点是“页眉页脚/页码这类 PDF furniture”是否作为显式排除产
  物；我先查 Zotero 对 repeated page furniture 的边界。

• 我查了 Zotero 的相关设计：它会按页面边缘区域、重复文本、重复页比例识别 page furniture，并把这些块标成 flowClass = excluded；后续 outline、reference、math
  等处理会跳过 excluded。它不是简单删除，而是保留“这是被排除内容”的状态。

  Grill 问题 5：我们的 PH2/PH3 是否要显式生成 excluded_regions，用于记录 PDF 页眉、页脚、页码等被排除的视觉文本区域？

  我的推荐：要，但 v1 只做保守规则，不做复杂结构推断。

  interface PdfExcludedRegion {
    pageIndex: number;
    bbox: [number, number, number, number];
    text: string;
    reason: "repeated_page_furniture" | "page_number" | "outside_body_band";
    confidence: number;
  }

  默认策略：

  candidate if:
    region is in top/bottom 12% page band
    and normalized text repeats on >= max(3 pages, 25% pages)
    and text length between 8 and 160

  then:
    add to excluded_regions
    omit from PDF word stream used for Markdown alignment

  这样做的价值是：对齐算法不会被页眉、期刊名、页码干扰；同时 report 里能解释为什么某些 PDF 文本没参与对齐。它也和 ADR-0062 的“page furniture is excluded
  from semantic source and recorded in report”一致。

  你是否同意 PH2/PH3 加 excluded_regions，并在 alignment 前从 PDF match stream 中排除这些区域？


› 同意


• 我把 excluded_regions 也锁定为 PH2/PH3 的显式产物，并且 alignment 前从 match stream 排除。下一点是前端点击/选择 PDF 区域时，是否需要把 char/word geometry
  带到前端；我先查 Zotero 在 annotation position 与 rect/range 之间如何分工。

• 我查了 Zotero 这一块：它在导入/处理 annotation 时用 rects 反查 chars 得到 text/range/offset，然后用 pageIndex + offset + top 排序。对我们来说，Zotero 的设
  计支持“PDF 视觉位置可以反推出文本范围”，但我们的约束是 LID 才是 memory/citation anchor，所以不应该把完整 char geometry 变成前端公共依赖。

  Grill 问题 6：PH4/PH5 中，PDF 点击/框选是否只通过 pdf_source_map 的 LID regions 做反解，而不把完整 char/word geometry 下发给前端？

  我的推荐：v1 只用 pdf_source_map 反解到 LID，不下发完整 char/word geometry。

  function resolvePdfSelectionToLids(selectionRects, pageIndex, sourceMap):
      candidates = entries.filter(entry =>
          entry.status is mapped
          && entry.pdf_regions.any(r => r.pageIndex == pageIndex && intersects(r.bbox, selectionRects))
      )
      return rank_by_intersection_area_then_reading_order(candidates)

  规则：

  PDF click:
    point -> smallest/highest-confidence region containing point -> lid

  PDF drag selection:
    rects -> intersecting regions -> ordered unique LIDs
    save note/highlight anchored to first/representative LID
    optional display metadata stores selected pdf rects, but not as citation anchor

  原因：前端只需要“视觉区域 -> LID”；完整 char stream 会增大 artifact/loader 复杂度，并模糊 public contract。等以后要做 PDF 原生文本复制、精确跨 LID
  selection 文本时，再加一个内部 /book/pdf_text_geometry 调试/高级接口也不晚。

  你是否同意 v1 的 PDF selection/click 只依赖 pdf_source_map region intersection 反解到 LID，不下发完整 char/word geometry？


› 但是我们现在在highlight和quote source时确实需要精确跨 LID selection 文本


• 你这个约束会改变刚才的推荐：如果 highlight 和 Quote source 要保留精确跨 LID selection 文本，单靠 coarse LID regions 不够。我先按你的规则再查一次 Zotero 的
  跨 rect/跨页 selection 文本恢复设计，并对照我们当前前端 selection 实现。

• 你说得对：PDF-first 后如果仍要保留现有 highlight/Quote source 的质量，必须有“PDF selection -> 精确文本 + 跨 LID ranges”的确定性路径。Zotero 的做法是用
  rects 反查 chars 得到 range/text；我们这里要再多一步，把 PDF chars/words 映射回 book.text 的 LID offset。

• 修正第 6 点：v1 不能只用 coarse pdf_source_map region intersection。那只能定位 LID，不能满足精确跨 LID selection 文本。

  我的新推荐是：前端不下发完整 geometry，但后端/runtime 必须有一个构建期产物用于精确 selection resolve。

  interface PdfSelectionMap {
    version: "pdf_selection_map.v1";
    book_id: string;
    pages: PdfPageMeta[];
    words: Array<{
      pageIndex: number;
      bbox: [number, number, number, number];
      text: string;

      // 对齐结果：PDF word 对应到 Markdown/book.text 的哪段
      lid: string | null;
      source_range: { start: number; end: number } | null; // UTF-16 offset in that LID text
      confidence: number;
    }>;
  }

  前端流程：

  PDF drag selection
    -> collect pageIndex + selection rects from pdf.js text layer/canvas overlay
    -> POST /reader/pdf_selection.resolve { rects }
    -> backend intersects words/chars from pdf_selection_map
    -> returns:
       {
         quote_markdown,
         ranges: [{ lid, range: { start, end } }],
         representative_lid
       }

  这样和当前 HTML reader 的语义一致：现在 packages/web/src/selection.ts:11 负责还原 Markdown 文本，App.selectionRanges 负责 LID/offset；PDF reader 则由后端
  resolver 负责同一件事。Memory/highlight 仍然锚到 LID/range，不把 page:bbox 当 citation anchor。

  所以第 6 问改为：

  PH5 是否新增 pdf_selection_map.v1 和 /reader/pdf_selection.resolve，专门支持精确跨 LID highlight / Quote source，而 pdf_source_map 继续服务跳转和
  overlay？

  我的推荐：同意新增。pdf_source_map 是视觉投影；pdf_selection_map 是交互反解。两个职责分开，后续调试也清楚。


› 同意


• 同意点已更新：pdf_source_map 负责视觉 overlay/jump，pdf_selection_map + /reader/pdf_selection.resolve 负责 PDF 选区反解成 quote 和 LID ranges。下一步要定
  selection map 的精度：word 级还是 char 级；我先看当前 HTML reader 的 range 映射和 Zotero char range 的粒度。

• 当前 HTML reader 已经是 LID 内 UTF-16 精确 range：selectionRanges() 返回 { lid, start, end }，跨 LID 时用同一个 group id 保存多条 highlight。Zotero 对 PDF
  selection 也是 char-level：rects 先反查 anchorOffset/headOffset，再从 chars 生成 text 和 line rects。

  Grill 问题 7：pdf_selection_map 是否必须是 char-level，而不是 word-level？

  我的推荐：必须 char-level，word-level 只作为加速索引。

  interface PdfSelectionMap {
    version: "pdf_selection_map.v1";
    book_id: string;
    pages: PdfPageMeta[];
    chars: PdfMappedChar[];
    word_index: PdfMappedWord[]; // optional acceleration/debug
  }

  interface PdfMappedChar {
    pageIndex: number;
    bbox: [number, number, number, number]; // pdf user-space
    c: string;
    lineBreakAfter?: boolean;
    spaceAfter?: boolean;
    paragraphBreakAfter?: boolean;

    lid: string | null;
    source_offset: number | null; // UTF-16 offset in LID text
    confidence: number;
  }

  Resolver 输出保持现有前端语义：

  interface PdfSelectionResolveResponse {
    quote_markdown: string;
    ranges: Array<{ lid: string; range: { start: number; end: number } }>;
    representative_lid: string;
  }

  这样 PDF reader 的 highlight/Quote source 可以复用当前 memory API：每个 LID range 存一条，高亮跨 LID 时仍用 source_session_id group。word-level 不够，因为
  用户可能从半个词、公式符号、上标、标点开始选；至少 resolver 内部要能落到 char offset。

  你是否同意 pdf_selection_map 采用 char-level 主索引，word-level 只作为对齐/性能辅助？


› 同意


• 同意点已锁定：selection map 用 char-level 主索引，word-level 只做对齐/性能辅助。下一个关键点是归一化和 offset：PDF 抽取文本和 Markdown 原文会有 NFC、连字
  符、换行、空白差异；我先查 Zotero 的 delta map/归一化做法，再给建议。

• Zotero 的相关做法是：抽取/结构化时会对文本做 NFC/NFKC、空白折叠，并维护 delta map，把“归一化后的字符位置”映射回原始文本位置。我们当前项目的 LID span 明确
  是 Markdown UTF-16 code unit offset，所以 PDF selection 最终也必须回到这个坐标系，不能停在归一化文本坐标。

  Grill 问题 8：PH2 是否要把“归一化匹配坐标 -> 原始 Markdown UTF-16 offset”的 delta map 作为对齐算法的一等内部产物？

  我的推荐：要。否则 pdf_selection_map.chars[].source_offset 很容易在空白折叠、NFC、软换行连字符、Markdown 标记剥离后偏移。

  interface NormalizedSourceBlock {
    lid: string;
    raw_text: string;          // source.txt slice
    normalized_text: string;   // for matching only
    norm_to_raw: Array<{
      norm_offset: number;
      raw_offset: number;      // UTF-16 offset relative to LID text
    }>;
  }

  interface NormalizedPdfText {
    normalized_text: string;
    norm_to_pdf_char: Array<{
      norm_offset: number;
      pdf_char_index: number;
    }>;
  }

  对齐接受后：

  pdf char -> pdf normalized offset
           -> matched source normalized offset
           -> source raw UTF-16 offset inside LID
           -> pdf_selection_map.chars[].source_offset

  v1 规则建议：

  NFC normalize
  collapse whitespace runs to one space
  join PDF line wraps with space
  dehyphenate only when PDF line ends with "-" and Markdown normalized target has joined token
  never mutate source.txt or LidNode.span

  这让“匹配文本”和“证据锚点文本”分开：匹配可以宽松，最终保存的 highlight range 仍然是原始 Markdown/LID offset。

  你是否同意 PH2 增加 normalized-to-raw offset map，并把 pdf_selection_map 的 source_offset 定义为原始 LID 文本 UTF-16 offset？


› 同意


• 同意：对齐内部维护 normalized-to-raw offset map，source_offset 明确定义为原始 LID 文本 UTF-16 offset。下一个关键点是 PDF 文件如何进入后端/前端：是作为
  book artifact 服务，还是独立静态路径；我先查当前 server 的 asset/open book 路由和 Zotero worker 的 PDF 源边界。

• 当前项目已有 source_manifest 的 PDF sidecar 概念，但 server 只服务 /book/assets/*，还没有 PDF 文件路由。Zotero 的边界是 worker/renderer 接受 PDF buffer 或
  已打开的 pdfDocument；对我们来说，PDF 应当是 book artifact，由后端限定路径服务给 pdf.js。

• Grill 问题 9：PH4 的 PDF 文件是否应该作为 book artifact 由后端只读服务，而不是前端直接读任意本地路径？

  我的推荐：是，新增 book-scoped PDF manifest + route。

  现有 source_manifest.json 已有 original_pdf attachment 概念，可以扩展为 reader 可消费：

  {
    "attachments": [
      {
        "kind": "original_pdf",
        "path": "paper.pdf",
        "pdf_source_map": { "path": "pdf_source_map.json", "may_project_lid_to_pdf_region": true },
        "pdf_selection_map": { "path": "pdf_selection_map.json" }
      }
    ]
  }

  后端路由建议：

  GET /book/source_manifest
  GET /book/pdf/original        -> application/pdf
  GET /book/pdf_source_map      -> json
  GET /book/pdf_selection_map/status or resolve via POST only
  POST /reader/pdf_selection.resolve

  路径约束：

  only serve files under current book_dir
  reject traversal
  content-type application/pdf
  no arbitrary filesystem PDF path in frontend

  理由：和现有 /book/assets/... 的安全边界一致；pdf.js 前端拿 URL 渲染即可；selection resolve 仍在后端用 book artifacts 做确定性映射。这样也避免把用户本地绝
  对路径暴露到浏览器。

  你是否同意 PH4/PH5 采用 book-scoped PDF 路由，并让前端只通过 /api/book/pdf/original 加载当前书的 PDF？


› 同意


• 同意：PDF 作为当前 book 的只读 artifact，由后端 book-scoped route 服务给前端，不暴露本地任意路径。下一个设计点是 PH3 hybrid build 是否立即跑完整 paper
  pipeline，还是只生成 source/map artifacts；我先查当前 pass1-batch 和 source-manifest 已有边界。


• 本轮新增 Grill 规则：每次 Grill 得到用户确认后，进入下一个 Grill 问题之前，必须把本次 Grill 的过程、查证依据、推荐方案和用户答案 append 进 `grill.md`。同时，每次提问和给推荐方案之前，继续先查看 `E:\allwork\download\agent\zotero-main\zotero-main\document-worker` 下是否有相关设计。

• 我已先查：
  - 本项目：`pass1-batch.ts` 已能写 `source_manifest.json`，并支持 `--original-pdf` / `--pdf-source-map`；paper metadata、lexicon、discourse、Pass2、BookStructure 都是独立后续趟。
  - Zotero document-worker：PDF fulltext/structure/renderArea 是分层入口，不把“PDF 视觉处理”和“语义结构全管线”绑成一个不可拆命令。

  Grill 问题 10：PH3 hybrid build 是否只生成 Markdown base + PDF map artifacts，而不自动跑完整 paper pipeline？

  我的推荐：是，PH3 保持窄闭环。

  ```text
  PH3 hybrid build:
    paper.md + paper.pdf
    -> source.txt
    -> base.json
    -> source_manifest.json
    -> pdf_source_map.json
    -> pdf_selection_map.json
    -> alignment_report.json
    -> asset_manifest.json
  ```

  不在 PH3 自动跑：

  ```text
  paper_metadata.json
  paper_lexicon.json
  discourse_index.json
  pass2_audit.json
  book_structure.json
  ```

  理由：PDF-first 的核心风险是“Markdown LID ↔ PDF geometry/selection”能否稳定打通；把 metadata/lexicon/BookStructure 一起跑，会把失败来源混在 LLM 抽取、长程边、结构图和 PDF 对齐之间。已有 pipeline 已经支持这些 artifact 独立生成，PH3 只需要保证后续趟可以基于同一个 `base.json/source.txt/source_manifest.json` 继续运行。

  你是否同意把 PH3 锁为“hybrid foundation only”，完整 paper projections 作为后续独立验收链，不并入 PH3？


› 同意

**Locked result**: `pdf_selection_map` uses a manifest plus per-page shards. Runtime selection resolution loads only the pages touched by the PDF selection; `pdf_source_map.json` remains the lightweight whole-book LID-to-region index for frontend overlay and jump behavior.

---

## Grill Q15 - alignment report hard gates vs diagnostics

**Pre-check before asking/recommending**:
- Local plan checked: ADR-0062 and `docs/切片方案-paper-pdf-first-hybrid.md` already require deterministic alignment, `mapped_leaf_lid_ratio >= 0.80`, schema validation, valid bboxes, and clean-room Zotero boundaries.
- Zotero document-worker checked: progress callbacks are best-effort and do not affect extraction; structure extraction records degraded fallback reasons; structure index cache/materialization stats are diagnostic; EPUB page mapping uses coverage-style heuristics for mapping quality.

**Question**: Should `alignment_report.json` split acceptance metrics into `hard_gates` and `diagnostics`, instead of letting every statistic participate in build pass/fail?

**Recommendation**: Yes. Hard gates should only decide whether the hybrid build can safely enter the PDF-first reader; diagnostics should explain quality and guide tuning.

```ts
type AlignmentReport = {
  hard_gates: {
    schema_valid: boolean;
    freshness_valid: boolean;
    all_leaf_lids_have_entries: boolean;
    mapped_leaf_lid_ratio: number;
    mapped_region_valid_ratio: number;
    selection_map_valid: boolean;
    clean_room_valid: boolean;
  };
  diagnostics: {
    word_mapped_ratio: number;
    line_fallback_ratio: number;
    block_fallback_ratio: number;
    unmapped_ratio: number;
    excluded_region_count: number;
    mean_confidence: number;
    p50_confidence: number;
    p10_confidence: number;
    ambiguous_match_count: number;
    repeated_furniture_count: number;
    pages_with_low_mapping: Array<{ pageIndex: number; mapped_ratio: number }>;
  };
};
```

Hard gate examples:

```text
fail if:
  schema/freshness invalid
  any leaf LID has no entry record
  mapped_leaf_lid_ratio < 0.80
  any mapped bbox is outside page bounds
  selection_map manifest/page shard hash mismatch
  Zotero forbidden source/assets appear in implementation path
```

Diagnostic examples:

```text
word_mapped_ratio
fallback ratios
confidence percentiles
unmapped reasons
page furniture/excluded stats
per-page/per-section low mapping
ambiguous/repeated phrase counts
```

**User answer**: Agreed.

**Locked result**: `alignment_report.json` uses a two-layer acceptance model. A small set of non-negotiable hard gates determines build pass/fail; all other alignment statistics are diagnostics for explanation and tuning. Hard gates are centered on LID coverage, bbox/page validity, freshness consistency, selection-map integrity, and clean-room compliance.

---

## Grill Q16 - degraded fallback when PDF alignment hard gates fail

**Pre-check before asking/recommending**:
- Local plan checked: ADR-0062 and `docs/切片方案-paper-pdf-first-hybrid.md` currently say the whole build fails when `mapped_leaf_lid_ratio < 0.80`, but they do not yet separate Markdown semantic build validity from PDF-first capability validity.
- Zotero document-worker checked: invalid identity fields such as `sourceHash` are hard errors; progress/reporting is best-effort; PDF structure extraction records degraded fallback reasons instead of making every quality drop fatal; cache/stat counters are diagnostic.

**Question**: When PDF alignment hard gates fail, should PH3 fail the entire `paper.md --pdf paper.pdf` build with no openable book, or preserve the Markdown/LID semantic build while marking PDF-first capability unavailable?

**Recommendation**: Preserve the Markdown/LID semantic build, fail only the hybrid PDF capability, and make the command return non-zero by default.

```text
PH3 build flow:
  build source.txt/base.json from Markdown
  run PDF extraction/alignment
  if PDF hard gates pass:
      write pdf_source_map.json
      write pdf_selection_map/
      source_manifest.capabilities.project_lid_to_pdf = true
      source_manifest.capabilities.resolve_pdf_selection = true
      exit 0
  if PDF hard gates fail:
      write alignment_report.json
      do not expose valid pdf_source_map/pdf_selection_map
      source_manifest.capabilities.view_pdf = maybe true
      source_manifest.capabilities.project_lid_to_pdf = false
      source_manifest.capabilities.resolve_pdf_selection = false
      exit non-zero unless --allow-degraded-pdf
```

This separates:

```text
Markdown semantic build:
  still valid if source.txt/base.json pass

Hybrid PDF-first build:
  invalid if hard gates fail

Runtime reader:
  can open Markdown fallback, but must not pretend PDF overlay/selection is safe
```

**User answer**: Agreed.

**Locked result**: PDF hard-gate failure invalidates only the hybrid PDF capabilities, not the Markdown/LID semantic build. PH3 writes `alignment_report.json`, withholds usable `pdf_source_map`/`pdf_selection_map` capability exposure, and exits non-zero by default; `--allow-degraded-pdf` may keep the degraded Markdown fallback artifact for manual inspection.

---

## Grill Q17 - source_manifest PDF artifact status machine

**Pre-check before asking/recommending**:
- Local implementation checked: `packages/core/src/source-manifest.ts` currently represents PDF source map state as `status: "not_provided" | "provided"` plus `may_project_lid_to_pdf_region`, which cannot express failed/degraded artifact states, failure reasons, or report paths.
- Zotero document-worker checked: worker actions return explicit `error` objects on action failure; PDF structure inference failures can fall back to paragraph blocks while recording fallback reasons such as `inference_error` and `fallback_blocks_coalesced`.

**Question**: Should `source_manifest` PDF artifact/capability state upgrade from boolean/`provided` fields to a `status + reason + report_path` state machine?

**Recommendation**: Yes.

```ts
type PdfArtifactStatus =
  | { status: "not_provided" }
  | { status: "available"; path: string; sha256: string }
  | { status: "failed"; reason: PdfFailureReason; report_path: string }
  | { status: "degraded"; reason: PdfFailureReason; report_path: string; path?: string };

type PdfFailureReason =
  | "missing_original_pdf"
  | "pdf_extract_failed"
  | "alignment_hard_gate_failed"
  | "freshness_mismatch"
  | "selection_map_invalid"
  | "clean_room_violation";

type PdfCapabilityStatus =
  | { available: true }
  | { available: false; reason: PdfFailureReason; report_path?: string };

type PdfCapabilities = {
  view_pdf: PdfCapabilityStatus;
  project_lid_to_pdf: PdfCapabilityStatus;
  resolve_pdf_selection: PdfCapabilityStatus;
};
```

Recommended `source_manifest` shape:

```ts
original_pdf: PdfArtifactStatus;
pdf_source_map: PdfArtifactStatus;
pdf_selection_map: PdfArtifactStatus;
capabilities: PdfCapabilities;
```

Key constraints:

```text
provided/path exists does not mean available
failed artifacts are not runtime-consumable maps
degraded/failed states must point to alignment_report.json when applicable
capability=false must carry a reason so frontend fallback is explainable
```

**User answer**: Agreed.

**Locked result**: `source_manifest` must represent PDF artifacts and PDF capabilities with explicit status, reason, and report path fields. Runtime availability is derived from `available` capability states, not from mere file presence. `provided` is no longer sufficient to mean a map is safe to consume.

---

## Grill Q18 - LLM repair as alignment-only candidate

**Pre-check before asking/recommending**:
- Local plan checked: ADR-0062 and `docs/切片方案-paper-pdf-first-hybrid.md` say LLM repair is optional, off by default, and may only propose text repair; deterministic alignment decides acceptance. They do not yet state whether repair may rewrite `source.txt`, `base.json`, or LID spans.
- Zotero document-worker checked: its block cluster `repair` is an internal model pass that adjusts candidate labels inside a fixed runtime/threshold pipeline; it does not make generated output the independent source of truth.

**Question**: Should PH6 `--repair-with-llm` only generate alignment-only repaired text candidates, and never modify `source.txt`, `base.json`, LID spans, or canonical Markdown?

**Recommendation**: Yes. Repair is only a candidate input to deterministic alignment; it is not a semantic-source rewrite.

```ts
type AlignmentRepairCandidate = {
  lid: string;
  original_source_span: { start: number; end: number };
  original_text_hash: string;
  repaired_text: string;
  repair_reason:
    | "hyphenation"
    | "ligature"
    | "pdf_line_break"
    | "header_footer_noise"
    | "unicode_normalization";
  proposed_by: "llm";
};
```

```text
repair flow:
  failed LID/block
    -> collect original Markdown text + nearby PDF words
    -> LLM proposes repaired_text candidate
    -> deterministic align(repaired_text, pdfWindow)
    -> if thresholds pass:
         pdf_source_map.entry.alignment.repair = candidate metadata
         source_span still points to original source.txt
       else:
         remain unmapped
```

Hard constraints:

```text
repair must not:
  rewrite source.txt
  rewrite base.json
  change SourceBlock.span
  change LID text offsets
  create page/bbox directly
  create citation evidence
```

**User answer**: Agreed.

**Locked result**: PH6 repair is alignment-only. LLM output may propose normalized/repaired text for deterministic rematching, but the canonical Markdown, `source.txt`, `base.json`, LID spans, citation evidence, and stored offsets remain unchanged.

---

## Grill Q19 - normalization provenance and original-offset discipline

**Pre-check before asking/recommending**:
- Local implementation checked: `packages/core/src/md-adapter.ts`, `packages/core/src/segment.ts`, generated `Span`, `book.text`, `reader.highlight`, and memory `TextRange` all use UTF-16 code unit offsets against the original source/LID text. Invalid highlight ranges are rejected rather than degraded.
- Zotero document-worker checked: `src/dom/html-to-blocks.ts` normalizes text with NFC and whitespace collapse while recording delta maps from normalized positions back to raw positions. This keeps processing text normalized without losing original-coordinate traceability.

**Question**: Should PH2 alignment explicitly produce normalization provenance mapping normalized match positions back to original Markdown/PDF positions, instead of storing only final confidence?

**Recommendation**: Yes. Keep provenance as build/debug data, not as a semantic source rewrite.

```ts
type TextNormalizationTrace = {
  original_hash: string;
  normalized_hash: string;
  transforms: Array<
    | "unicode_nfc"
    | "collapse_whitespace"
    | "dehyphenate_line_break"
    | "normalize_ligature"
    | "strip_page_furniture"
  >;
  delta_map: Array<{ normalized_offset: number; original_offset_delta: number }>;
};

type PdfSourceMapEntry = {
  lid: string;
  source_span: { start: number; end: number };
  alignment: {
    matched_chars: number;
    total_chars: number;
    confidence: number;
    method: "md_to_pdf_words_v1";
    source_normalization?: TextNormalizationTrace;
    pdf_normalization?: TextNormalizationTrace;
  };
};
```

Constraints:

```text
source_span is always original source.txt UTF-16 offset
highlight/note range is always original LID text UTF-16 offset
normalized offsets are alignment/debug-only
quote_markdown is assembled only from original source.txt ranges
delta_map missing means offset-sensitive repair/selection results cannot be accepted
```

**User answer**: Agreed.

**Locked result**: PH2/PH3 must record normalization provenance and delta maps for offset-sensitive alignment. Runtime semantic offsets always resolve back to original `source.txt` / LID UTF-16 coordinates; normalized text positions never become citation, memory, or highlight coordinates.

---

## Grill Q20 - keep runtime source map lightweight and move heavy provenance to report

**Pre-check before asking/recommending**:
- Local implementation checked: `Book::load` currently reads several sidecars as whole JSON files; `pdf_selection_map` has already been locked as per-page lazy shards because char-level data is too heavy to keep in the always-loaded book state.
- Zotero document-worker checked: `structure-index.js` materializes page text entries on demand and tracks LRU/cache stats; it does not put all derived/debug text geometry into every runtime lookup path.

**Question**: Should normalization traces, repair candidates, rejected matches, and per-step alignment details live in `alignment_report.json`, rather than being inlined into runtime-loaded `pdf_source_map.json`?

**Recommendation**: Yes. Split the runtime map from the debug/build report.

```text
pdf_source_map.json  // runtime-facing, lightweight
  entries:
    lid
    source_span
    status
    best pdf_regions
    confidence
    method
    repair_used?: boolean
    report_ref?: string

alignment_report.json  // build/debug-facing, rich
  hard_gates
  diagnostics
  per_lid:
    lid
    normalized source/pdf hashes
    normalization delta maps
    repair candidates
    rejected matches
    failure reason
    nearby pdf window summary
```

Constraints:

```text
Frontend PH4 loads pdf_source_map.json as a whole
Runtime selection resolver loads pdf_selection_map page shards lazily
alignment_report.json is fetched only for diagnostics/debug UI or build logs
pdf_source_map entries may point to report sections by id, but do not inline heavy traces
```

**User answer**: Agreed.

**Locked result**: `pdf_source_map.json` remains a lightweight runtime contract for overlay/jump/status. Heavy normalization provenance, repair candidates, rejected matches, and detailed per-LID alignment traces live in `alignment_report.json`, referenced from the source map only by compact ids when needed.

---

## Grill Q21 - explicit primary_region for LID jump

**Pre-check before asking/recommending**:
- Local plan checked: ADR-0062 says `pdf_source_map` projects LIDs to PDF `pageIndex + bbox`; `docs/切片方案-paper-pdf-first-hybrid.md` has `pdf_regions[]` but does not yet define multi-region ordering, primary jump target, union behavior, or cross-page LID handling.
- Zotero document-worker checked: annotation/highlight positions keep `rects[]`; render/preview code may compute bounding rects from those rects when needed, but rect arrays remain the underlying visual provenance. Annotation rects are sorted in page reading order before use.

**Question**: Should `pdf_source_map` store both `regions[]` and an explicit `primary_region`, instead of making the frontend infer the jump target from a list of regions?

**Recommendation**: Yes.

```ts
type PdfRegion = {
  region_id: string;
  granularity: "word" | "line" | "block";
  pageIndex: number;
  bbox: [number, number, number, number];
  source_range?: { start: number; end: number };
  confidence: number;
};

type PdfSourceMapEntry = {
  lid: string;
  source_span: { start: number; end: number };
  status: "word_mapped" | "line_fallback" | "block_fallback" | "unmapped" | "excluded";
  primary_region?: PdfRegion;
  regions: PdfRegion[];
};
```

Region ordering:

```text
1. source_range.start ascending, if present
2. pageIndex ascending
3. PDF reading order within page: top-to-bottom, then left-to-right after viewport conversion rules
```

Primary region selection:

```text
first region with highest confidence and earliest source_range
fallback: earliest pageIndex + reading-order region
```

Frontend behavior:

```text
LID jump:
  use primary_region

LID overlay/highlight:
  draw all regions

scroll-derived current LID:
  rank intersecting regions by area overlap, then confidence, then reading order
```

**User answer**: Agreed.

**Locked result**: `pdf_source_map` entries must include all `regions[]` plus a build-selected `primary_region`. Jump behavior uses `primary_region`; overlay/highlight draws all regions. Region order and primary selection are deterministic build outputs, not frontend guesses.

---

## Grill Q22 - page_region_index for frontend overlay and hit-test

**Pre-check before asking/recommending**:
- Local plan checked: PH4 must support PDF overlay, PDF click/scroll deriving LID, and LID citation jump. Q21 locked `regions[] + primary_region`, but no page-oriented hit-test index exists yet.
- Zotero document-worker checked: PDF annotation import resolves ranges by page-local chars; `structure-index.js` exposes page-local intersection (`getPageEntriesIntersecting(pageIndex, rect)`) rather than scanning all document entries for every hit-test.

**Question**: Should `pdf_source_map.json` include a `page_region_index` in addition to entries, so PH4 can overlay and hit-test by page without rebuilding indexes at runtime?

**Recommendation**: Yes. Keep it lightweight and whole-file loadable.

```ts
type PdfPageRegionIndexItem = {
  lid: string;
  region_id: string;
  bbox: [number, number, number, number];
  confidence: number;
  status: "word_mapped" | "line_fallback" | "block_fallback";
};

type PdfSourceMap = {
  version: "pdf_source_map.v1";
  book_id: string;
  pages: PdfPageMeta[];
  entries: PdfSourceMapEntry[];
  page_region_index: Array<{
    pageIndex: number;
    regions: PdfPageRegionIndexItem[];
  }>;
};
```

Build-time guarantees:

```text
page_region_index only includes mapped regions
regions sorted in PDF reading order
region_id points back to entries[].regions[]
no duplicated region_id within a page
bbox already validated inside page bounds
```

Frontend usage:

```text
render page overlay:
  page_region_index[pageIndex].regions

PDF click/hit-test:
  candidates = page_region_index[pageIndex].regions.filter(intersects point/rect)
  rank by intersection area, confidence, reading order
  selected lid = candidates[0].lid
```

**User answer**: Agreed.

**Locked result**: `pdf_source_map` provides both LID-oriented `entries` and page-oriented `page_region_index`. LID jumps use entries/primary regions; PDF page overlay and hit-test use the page index. The build owns both indexes and their consistency.

---

## Grill Q23 - separate page_excluded_index for page furniture

**Pre-check before asking/recommending**:
- Local plan checked: ADR-0062 says page furniture is excluded from the semantic source and recorded in the report; `docs/切片方案-paper-pdf-first-hybrid.md` has `excluded_regions[]` but does not define frontend hit-test behavior for excluded PDF areas.
- Zotero document-worker checked: `page-furniture.js` marks repeated edge-band page furniture as `flowClass = "excluded"`; downstream outline/reference/math handling skips excluded blocks. Exclusion is an explicit state, not invisible deletion.

**Question**: Should `excluded_regions` also have a separate `page_excluded_index`, so frontend hit-tests can explain "excluded page furniture" without deriving a LID?

**Recommendation**: Yes. Keep excluded regions separate from mapped regions.

```ts
type PdfExcludedRegion = {
  excluded_id: string;
  pageIndex: number;
  bbox: [number, number, number, number];
  text?: string;
  reason:
    | "repeated_page_furniture"
    | "page_number"
    | "outside_body_band";
  confidence: number;
};

type PdfSourceMap = {
  entries: PdfSourceMapEntry[];
  page_region_index: Array<{
    pageIndex: number;
    regions: PdfPageRegionIndexItem[];
  }>;
  excluded_regions: PdfExcludedRegion[];
  page_excluded_index: Array<{
    pageIndex: number;
    regions: Array<{
      excluded_id: string;
      bbox: [number, number, number, number];
      reason: PdfExcludedRegion["reason"];
    }>;
  }>;
};
```

Frontend behavior:

```text
PDF click:
  1. check page_region_index mapped regions
     -> if hit, derive LID
  2. else check page_excluded_index
     -> if hit, show "page furniture / not a citation anchor"
  3. else treat as unmapped visual area
```

Constraints:

```text
excluded region never has lid
excluded region never creates note/highlight anchor
excluded region can appear in debug/report UI
excluded region can be drawn with a muted overlay only in debug mode
```

**User answer**: Agreed.

**Locked result**: Excluded PDF regions are indexed separately from mapped LID regions. `page_excluded_index` supports explainable frontend hit-tests for page furniture, but excluded regions never derive LIDs, memory anchors, highlights, notes, or citations.

---

## Grill Q24 - partial PDF selection resolve semantics

**Pre-check before asking/recommending**:
- Local implementation checked: current HTML selection only opens the popover when it can produce at least one LID range and non-empty quote text; `reader.highlight` rejects invalid UTF-16 ranges with `INVALID_RANGE` rather than degrading.
- Zotero document-worker checked: Mendeley highlights without rects are skipped; oversized annotation positions may be split, but a single rect that cannot fit raises an error. The worker does not invent a location for unresolvable highlights.

**Question**: Should PDF selection resolve support explainable `partial` results, while save actions persist only the resolved LID ranges, instead of making any partial selection a full hard failure?

**Recommendation**: Yes.

```ts
type PdfSelectionResolveResponse = {
  status: "resolved" | "partial" | "unresolved";
  anchor_lid?: string;
  quote_markdown: string;
  ranges: Array<{
    lid: string;
    range: { start: number; end: number };
    pdf_rects: Array<{ pageIndex: number; bbox: [number, number, number, number] }>;
  }>;
  unresolved_rects: Array<{
    pageIndex: number;
    bbox: [number, number, number, number];
    reason: "unmapped" | "excluded" | "below_confidence" | "no_selection_chars";
  }>;
  warnings: string[];
};
```

Behavior:

```text
status=resolved:
  show normal Highlight / Note / Ask AI actions

status=partial:
  show actions, but label "Only mapped text will be saved"
  highlight saves only ranges[]
  note/ask quote_markdown includes only resolved LID text
  unresolved_rects are not stored as memory/citation anchors

status=unresolved:
  no Highlight / Note / Ask AI actions
  show reason: unmapped/excluded/no selectable text
```

Hard constraints:

```text
never synthesize a range for unresolved rects
never anchor memory to page/bbox
never silently save a partial selection as if complete
```

**User answer**: Agreed.

**Locked result**: PH5 PDF selection resolve returns `resolved | partial | unresolved`. Partial selections may save only resolved LID/range segments and must visibly disclose unresolved PDF rects; unresolved selections expose no highlight/note/ask actions.

---

## Grill Q25 - quote_markdown source-order assembly

**Pre-check before asking/recommending**:
- Local implementation checked: current HTML selection uses `rangeToMarkdown()` for quote text and LID/range offsets for highlight persistence. Note/Ask currently normalize selected quote text for inline display, while highlight storage uses exact LID ranges.
- Zotero document-worker checked: annotation rects are sorted into visual reading order before text recovery; recovered annotation text is derived from page chars, not from raw drag order. This confirms ordering is an explicit design choice, not an incidental browser event order.

**Question**: Should PDF selection `quote_markdown` be assembled from resolved ranges in original LID/source order, rather than PDF visual rect order or user drag order?

**Recommendation**: Yes. Use source order for quote truth; keep PDF visual order for overlay/debug.

```ts
type PdfSelectionResolvedRange = {
  lid: string;
  range: { start: number; end: number };
  source_order: number;
  pdf_order: number;
  pdf_rects: Array<{ pageIndex: number; bbox: [number, number, number, number] }>;
};

quote_markdown =
  ranges
    .sort(by source_order)
    .map(r => book.text(r.lid).slice_utf16(r.range))
    .join("\n\n")
    .trim()
```

Behavior constraints:

```text
quote_markdown source is original source.txt/LID ranges only
cross-LID separator is exactly blank line "\n\n"
duplicate/overlapping ranges within same LID are merged before quote generation
if source_order and pdf_order diverge, report warning: "selection visual order differs from source order"
note/ask uses quote_markdown; overlay uses pdf_rects
```

**User answer**: Agreed.

**Locked result**: PDF selection `quote_markdown` is generated from original source/LID ranges sorted by source order, with `\n\n` between LIDs. PDF visual order is retained only for overlay and diagnostic warnings.

---

## Grill Q26 - merge overlapping and adjacent PDF selection ranges

**Pre-check before asking/recommending**:
- Local implementation checked: memory ids include `(book_id|type|anchor|content|range)`, so identical LID/range saves are idempotent, but adjacent or overlapping ranges become distinct highlights. Current rendering merges overlapping highlight ranges visually, but persistence still stores each range separately.
- Zotero document-worker checked: annotations are deduplicated by type/position/comment; very large annotation positions may be split only for size limits. It does not intentionally preserve duplicate fragmented annotations for one logical selection.

**Question**: Should PDF selection resolve merge overlapping/adjacent ranges per LID before returning, to avoid saving one user selection as fragmented highlights?

**Recommendation**: Yes.

```ts
function mergeResolvedRanges(ranges):
  group by lid
  sort each group by range.start
  merge if next.start <= current.end + MERGE_GAP_UTF16
  output groups in source_order
```

Default:

```text
MERGE_GAP_UTF16 = 1
```

Return constraints:

```text
same lid ranges are non-overlapping and sorted
adjacent ranges caused by PDF word/char gaps are merged
separate LIDs remain separate ranges
group highlight id is used only when merged output has >1 range
quote_markdown is generated after merge
```

Example:

```text
before:
  1.2 [10, 18]
  1.2 [18, 25]
  1.2 [26, 30]

after:
  1.2 [10, 30]
```

**User answer**: Agreed.

**Locked result**: PDF selection resolve merges overlapping or near-adjacent ranges within the same LID before returning. Persistence, quote generation, and group highlight decisions operate on the merged ranges, not raw char-hit fragments.

---

---

## Grill Q14 - pdf_selection_map storage boundary

**Pre-check before asking/recommending**:
- Local plan/ADR checked: `docs/adr/0062-paper-pdf-first-hybrid-source-markdown-semantic-pdf-visual-map.md` and `docs/切片方案-paper-pdf-first-hybrid.md`.
- Zotero document-worker checked: PDF annotation/import paths resolve text by page (`pageIndex -> getPageChars(...)`), and structure processing also organizes `chars/objects` per page. Relevant references found around `src/pdf/index.js:getPageChars`, annotation processing with `position.pageIndex`, and `src/pdf/text.js:getRangeByHighlight/getRange`.

**Question**: Should `pdf_selection_map` be stored as a manifest plus per-page shards instead of one whole-document JSON file?

**Recommendation**: Yes. Keep `pdf_source_map.json` as the lightweight front-end map, but store char-level `pdf_selection_map` as:

```text
pdf_selection_map/
  manifest.json
  pages/
    0000.json
    0001.json
```

Backend resolve flow:

```text
POST /reader/pdf_selection.resolve { page_rects }
  -> collect unique pageIndex
  -> load only the required page shards
  -> resolve PDF chars to LID ranges
  -> return quote_markdown + ranges
```

**User answer**: Agreed.

**Locked result**: `pdf_selection_map` is a backend-oriented, char-level, per-page sharded artifact. Frontend consumes `pdf_source_map` for overlay/jump; backend lazily loads `pdf_selection_map/pages/*.json` only when resolving PDF selection.

---

## Grill Q15 - semantic annotation projection back to PDF

**Pre-check before asking/recommending**:
- Zotero document-worker checked: `src/pdf/text.js` and `src/pdf/structure/util.js` include char-range-to-rect logic such as range text/rect construction and page-position generation. This confirms a useful separation: semantic range can be projected to PDF rects as a derived display artifact.
- Local implementation checked: current highlights persist through `/reader/highlight` as `LID + UTF-16 range + source_session_id`, and multi-LID highlights use one group id. Existing memory semantics are already LID/range based.

**Question**: Should PH5 add reverse projection from existing semantic annotations (`LID + range`) back to PDF overlay rects, instead of only displaying rect snapshots created from PDF selections?

**Recommendation**: Yes. Add a backend projection endpoint:

```ts
type PdfRangeProjectRequest = {
  ranges: Array<{
    lid: string;
    range: { start: number; end: number } | null;
  }>;
};

type PdfRangeProjectResponse = {
  projected: Array<{
    lid: string;
    range: { start: number; end: number } | null;
    pdf_rects: Array<{
      pageIndex: number;
      bbox: [number, number, number, number];
    }>;
    status: "exact" | "lid_region_fallback" | "unmapped";
  }>;
};
```

**User answer**: Agreed.

**Locked result**: PH5 must support `LID/range -> PDF rects` projection. Memory remains `LID/range` only; PDF rect snapshots are display/cache metadata, not semantic truth. Exact projection uses `pdf_selection_map`; whole-LID or failed range projection falls back to `pdf_source_map` regions where possible.

---

## Grill Q16 - PDF text-layer selection geometry boundary

**Pre-check before asking/recommending**:
- Zotero document-worker checked: `src/pdf/text.js` resolves highlight rects through chars (`rects -> chars -> range/text`), `src/pdf/renderer.js` keeps PDF rects separate from viewport rects, and annotation read/write paths model positions as `pageIndex + rects`.
- Local reader checked: current HTML reader derives `quote_markdown` and `LID/range` from DOM selection, but memory persists only LID/range. PDF reader should preserve that product semantics.

**Question**: Should PH4/PH5 use the pdf.js text layer/browser Range only to collect selection client rects, convert them to `pdf_user_space`, and send those rects to `/reader/pdf_selection.resolve`, while not trusting text-layer text as the quote?

**Recommendation**: Yes.

```text
User selects PDF text layer
  -> browser Range.getClientRects()
  -> group rects by page canvas/text-layer
  -> viewport.convertToPdfPoint(...)
  -> POST /reader/pdf_selection.resolve { page_rects }
  -> backend uses pdf_selection_map chars
  -> returns quote_markdown + LID ranges
```

**Constraints**:
- Text layer provides selection geometry only.
- Backend is the only resolver of quote and LID ranges.
- `quote_markdown` comes from `source.txt` / LID ranges, not from pdf.js selected text.
- Do not save pdf.js text offsets as memory anchors.

**User answer**: Agreed.

**Locked result**: PH4/PH5 will use pdf.js text-layer selection for natural selection geometry, but quote/highlight ranges are accepted only from backend resolve results. Browser-selected PDF text is not semantic truth.

---

## Grill Q17 - PDF page rendering virtualization

**Pre-check before asking/recommending**:
- Zotero document-worker checked: `src/pdf/renderer.js` has explicit canvas pixel limits (`MAX_CANVAS_PIXELS`), `renderArea(pageIndex, rect, scale)`, and PDF cleanup/destroy boundaries. It treats rendering as bounded work, not an unbounded full-document canvas dump.
- Local reader checked: current HTML reader loads a window of `visible_lids` and extends the window at scroll edges; it does not keep the whole book DOM hydrated at once.

**Question**: Should PH4 `PdfReaderPane` render only visible pages plus neighboring pages, rather than rendering the entire PDF when opening a paper?

**Recommendation**: Yes. Use virtualized page rendering:

```ts
type PdfPageRenderState =
  | { status: "placeholder"; pageIndex: number; heightEstimate: number }
  | { status: "rendering"; pageIndex: number }
  | { status: "ready"; pageIndex: number; canvas: HTMLCanvasElement; scale: number }
  | { status: "error"; pageIndex: number; message: string };

renderWindow = visiblePages +/- 2 pages
```

**Behavior boundary**:
- Load PDF metadata/page count and `pdf_source_map` up front.
- Render current LID page plus neighbors on open.
- Use visible-page detection to render visible/preload pages and dispose far-away canvases/text layers.
- On zoom, rerender only visible/preload pages.

**User answer**: Agreed.

**Locked result**: PH4 uses page virtualization. `pdf_source_map` is lightweight and can be loaded as a whole; page canvases and text layers are lazy, bounded, and disposable.

---

## Grill Q18 - source_manifest-driven PDF capability discovery

**Pre-check before asking/recommending**:
- Local implementation checked: `packages/core/src/source-manifest.ts`, `packages/core/src/zod.ts`, and `skills/build/pass1-batch.ts` already produce `source_manifest.json` and support `--original-pdf` / `--pdf-source-map`. The current real paper fixture has `attachments: []`, so paper profile does not imply a PDF exists.
- Zotero document-worker checked: worker entrypoints are explicit actions such as `pdf.getFulltext`, `getStructuredDocumentText`, and `pdf.renderArea`; it does not rely on UI probing arbitrary file paths.

**Question**: Should PH4 frontend discover PDF-first capability from `source_manifest.attachments`, rather than hard-coding `/book/pdf/original` probes or switching solely on `profile_id === "paper"`?

**Recommendation**: Yes. `paper` means the semantic profile; `source_manifest` states whether the current book has a usable PDF and maps.

```text
if profile_id == "paper"
  && source_manifest has original_pdf
  && pdf_source_map.status == "available":
    render PdfReaderPane
else:
    render Markdown ReaderPane + warning/fallback
```

**User answer**: Agreed.

**Locked result**: PH4 PDF-first activation is driven by book-scoped `source_manifest.attachments`. Paper profile alone is insufficient; missing PDF/map falls back to the existing Markdown reader with an explicit degraded state.

---

## Grill Q19 - book-scoped PDF and map artifacts

**Pre-check before asking/recommending**:
- Local implementation checked: `packages/core/src/source-manifest.ts` currently records the incoming `original_pdf_path` and `pdf_source_map_path`; `skills/build/pass1-batch.ts` writes `source_manifest.json` but does not copy PDF/map artifacts into `.understand-book/<book_id>/`.
- Local server checked: `/book/assets/*` already uses a book-scoped safe file-serving boundary, but there is no equivalent `/book/pdf/*` route yet.
- Zotero document-worker checked: PDF actions receive a PDF buffer through explicit worker actions; the worker does not rely on UI access to arbitrary local filesystem paths.

**Question**: Should PH3 copy/write `paper.pdf`, `pdf_source_map.json`, and `pdf_selection_map/` into the current `.understand-book/<book_id>/` directory, with `source_manifest` recording only book-relative artifact paths?

**Recommendation**: Yes. Make the hybrid build self-contained:

```text
.understand-book/<book_id>/
  source.txt
  base.json
  source_manifest.json
  sources/
    original.pdf
  pdf_source_map.json
  pdf_selection_map/
    manifest.json
    pages/0000.json
```

`source_manifest` records:

```ts
attachments: [{
  kind: "original_pdf",
  path: "sources/original.pdf",
  pdf_source_map: { status: "available", path: "pdf_source_map.json" },
  pdf_selection_map: { status: "available", path: "pdf_selection_map/manifest.json" }
}]
```

**User answer**: Agreed.

**Locked result**: PH3 writes PDF and maps as book-scoped artifacts. Build inputs may be external paths, but runtime manifests expose only paths relative to the current book directory. The frontend must never depend on user-local absolute paths.

---

## Grill Q20 - freshness hashes for hybrid PDF artifacts

**Pre-check before asking/recommending**:
- Zotero document-worker checked: `getStructuredDocumentText` requires a `sourceHash`, PDF processing can read PDF fingerprint, and model/assets manifests use hashes for integrity.
- Local implementation checked: build resume and paper sidecar flows already use `content_hash` to prevent stale artifacts from being reused; `asset_manifest` records copied image `sha256`.

**Question**: Should PH3 write freshness anchors into `source_manifest`, `pdf_source_map`, and `pdf_selection_map/manifest`, and should runtime reads enforce them instead of treating them as debug metadata?

**Recommendation**: Yes.

```ts
type HybridSourceFreshness = {
  source_sha256: string;
  pdf_sha256: string;
  pdf_fingerprint?: string;
  build_profile: "paper_pdf_hybrid_v1";
};

type PdfSourceMap = {
  version: "pdf_source_map.v1";
  book_id: string;
  freshness: HybridSourceFreshness;
  map_content_hash: string;
  entries: PdfSourceMapEntry[];
};

type PdfSelectionMapManifest = {
  version: "pdf_selection_map.v1";
  book_id: string;
  freshness: HybridSourceFreshness;
  pages: Array<{
    pageIndex: number;
    path: string;
    char_count: number;
    mapped_char_count: number;
    sha256: string;
  }>;
};
```

**User answer**: Agreed.

**Locked result**: Freshness hashes are strong validation fields. `source_manifest`, `pdf_source_map`, and `pdf_selection_map` must agree on source/PDF identity; mismatches become explicit degraded/error states and must not be silently ignored.

---

## Grill Q21 - split PDF capabilities instead of one PDF switch

**Pre-check before asking/recommending**:
- Zotero document-worker checked: PDF functions are separated into explicit worker actions such as `pdf.getFulltext`, `getStructuredDocumentText`, `pdf.renderArea`, `pdf.renderAnnotations`, and `pdf.hasAnnotations`; each action has its own success/failure boundary.
- Local implementation checked: paper projections already use `available + warning` style degraded outputs, and `source_manifest` has an early `may_project_lid_to_pdf_region` flag, but it does not yet distinguish viewing, LID projection, and PDF selection resolving.

**Question**: Should PH4/PH5 expose separate PDF capabilities instead of a single "PDF available" switch?

**Recommendation**: Yes.

```ts
type PdfCapabilities = {
  view_pdf: {
    available: boolean;
    reason?: string;
  };
  project_lid_to_pdf: {
    available: boolean;
    reason?: string;
  };
  resolve_pdf_selection: {
    available: boolean;
    reason?: string;
  };
};
```

**Frontend behavior**:
- `view_pdf=true`, `project_lid_to_pdf=false`: render PDF without LID overlay/jump; show map unavailable.
- `project_lid_to_pdf=true`, `resolve_pdf_selection=false`: enable PDF overlay/jump, but disable PDF highlight/Quote source.
- `resolve_pdf_selection=true`: enable PDF selection -> quote/highlight/note/Ask AI.

**User answer**: Agreed.

**Locked result**: `source_manifest` or `/book/source_manifest` must expose independent PDF capabilities for PDF viewing, LID-to-PDF projection, and PDF-selection resolving. Frontend enables features progressively from these capability flags.

---

## Grill Q22 - PDF page identity, labels, and rotation boundary

**Pre-check before asking/recommending**:
- Zotero document-worker checked: annotation processing distinguishes stable `pageIndex` from display `pageLabel`; `resolveDestination` returns `pageIndex + rect`; renderer converts PDF-space rects through pdf.js viewport at render time. Rotation/view are handled as page metadata and viewport concerns.
- Local plan checked: current PDF map draft only had `pageIndex + bbox`; it did not yet separate stable page keys, display labels, and rotation metadata.

**Question**: Should PH1/PH3 pages metadata include stable `pageIndex`, display-only `page_label`, and page box/rotation, while all persisted bboxes remain unrotated PDF user-space?

**Recommendation**: Yes.

```ts
type PdfPageMeta = {
  pageIndex: number;
  page_label: string;
  width: number;
  height: number;
  rotate: 0 | 90 | 180 | 270;
  view: [number, number, number, number];
};

type PdfCoordinateSystem = {
  space: "pdf_user_space";
  origin: "bottom_left";
  unit: "pt";
  rotation_applied: false;
};
```

**Constraints**:
- Routes and shard names use `pageIndex`, never `page_label`.
- UI may display `page_label`.
- Map bboxes are stored in unrotated PDF user-space.
- Frontend uses pdf.js viewport to derive CSS rects for current zoom/rotation.
- Selection rects sent to backend are converted back to the same PDF user-space.

**User answer**: Agreed.

**Locked result**: `pageIndex` is the only stable page key; `page_label` is display-only; persisted map bboxes do not pre-apply rotation.


• 同意点已锁：PH3 只做 hybrid foundation，不自动跑完整 paper projection 链。下一题进入 PH4 PDF-first reader surface；我先查 Zotero `document-worker` 的 PDF render/area 设计，再给问题和推荐。

• 我先查了 Zotero `document-worker` 的相关设计：它的 renderer 明确用 pdf.js 自己控制 canvas，所有 rect 都先保留在 PDF page-space，再通过 `page.getViewport({ scale })` 转成 viewport rect；`renderArea` 也是按 page + rect 裁剪渲染，不走浏览器 iframe/embed。项目本地 ADR-0062 也已经要求 PDF viewer 由 app 控制。

  Grill 问题 11：PH4 的 paper 主阅读区是否直接切换为受控 pdf.js canvas surface，而不是把 PDF 放进旁路 preview / iframe / embed？

  我的推荐：是。paper profile 下 center reader surface 应该是 PDF-first：

  ```text
  if profile_id == "paper" && source_manifest.attachments.original_pdf exists:
      CenterPane = PdfReaderPane
  else:
      CenterPane = ReaderPane  // current Markdown/LID reader
  ```

  `PdfReaderPane` 的职责：

  ```ts
  type PdfReaderPaneProps = {
    pdfUrl: "/api/book/pdf/original";
    sourceMap: PdfSourceMap;
    selectedLid: string | null;
    annotations: MemoryRecord[];
  };

  LID -> PDF:
    selected/citation lid
    -> sourceMap.entries[lid].best region
    -> scroll PDF page into view
    -> draw overlay rects

  PDF scroll/click:
    visible page + viewport center/click point
    -> intersect sourceMap regions
    -> representative lid
    -> update semantic reader anchor
  ```

  边界：

  ```text
  Do:
    - render PDF pages with pdf.js canvas
    - overlay mapped LID regions/highlights
    - use pdf_source_map for jump/click
    - keep Markdown source preview as fallback/debug

  Do not:
    - iframe/embed PDF
    - use browser PDF viewer selection model as product contract
    - turn page/bbox into citation anchor
    - make source preview the main paper surface
  ```

  原因：iframe/embed 会挡住 overlay、selection resolve、LID click、annotation projection；PDF-first 的价值正是“页面忠实 + LID 语义锚”同时成立。受控 pdf.js surface 才能把 Zotero 式 page-space rect 转换和我们自己的 LID/memory 体系接起来。

  你是否同意 PH4 把 paper profile 的中心阅读区切成受控 `PdfReaderPane`，而当前 Markdown `ReaderPane` 退为非 paper 默认与 unmapped/source preview fallback？


› 同意


• 同意点已锁：paper profile 中心阅读区用受控 `PdfReaderPane`，Markdown `ReaderPane` 退为非 paper 默认和 fallback。下一题定 PDF scroll 与现有 `reader.state`/viewport 的关系；我先查 Zotero 的 page/position 模型和本项目现有 reader viewport 边界。

• 我查了两边：
  - Zotero 侧：annotation/position 都围绕 `pageIndex + rects + offset/top`，PDF 视觉位置和文本 offset 是可互推的，但它没有我们这种 LID 语义锚。
  - 本项目侧：`reader.state().viewport.anchor_lid/top_lid` 是 agent、`book.query` 默认 anchor、session 恢复、progress 的共同真相源；当前 session 只存每本书的 `top_lid`。

  Grill 问题 12：PDF 滚动时，是否要同步更新现有 `reader.state` 的 LID anchor，而不是新增一个独立的 “PDF 当前页” 真相源？

  我的推荐：要同步，但保持分层：

  ```text
  Canonical semantic position:
    reader.state.viewport.anchor_lid / top_lid  // 仍然是 LID

  Visual PDF position:
    pageIndex + scrollTop + zoom                // UI/session display state, not citation anchor
  ```

  PH4 行为建议：

  ```ts
  onPdfScrollSettled():
    visiblePdfRect = current viewport center or top band
    candidate = sourceMap.bestMappedLidNear(visiblePdfRect)
    if candidate.lid != reader.state.viewport.anchor_lid:
        POST /reader/goto { lid: candidate.lid }   // 或 thin reanchor endpoint
        update left rail / agent anchor / progress
  ```

  约束：

  ```text
  - 不把 pageIndex 变成 reader.state anchor
  - 不让 agent 默认拿 PDF page 问答
  - 不在每个 scroll tick 调后端；debounce / settle 后同步
  - unmapped PDF 区域不改 reader.state，保留最近 mapped LID
  - PDF zoom/page scroll 可单独保存为 display restore metadata，但不参与 citation/memory
  ```

  理由：如果 PDF scroll 不回写 LID anchor，用户读到第 8 页时 agent 仍以旧 Markdown anchor 回答，会破坏“读到哪问到哪”。但如果把 `pageIndex` 做成新真相源，又会冲掉现有 LID citation、memory、query、session 体系。最小改法是：PDF 视觉位置派生当前 LID，现有 reader state 继续做语义真相。

  你是否同意 PH4 采用“PDF scroll 派生 LID，并同步现有 reader.state；PDF page/zoom 只做视觉恢复状态”的设计？


› 同意


• 同意点已锁：PDF 滚动只派生并同步现有 LID anchor，page/zoom 只作视觉恢复状态。下一题定 PH4/PH5 的 selection 事件边界；我先查 Zotero 的 selection rects/text 逻辑和当前 Web selection/highlight 保存路径。

• 我查了：
  - Zotero selection：用 selection rects 命中 char center，得到 char range，再从 chars 还原 text 和 line rects。
  - 当前 Web reader：HTML selection 已经走 `rangeToMarkdown(range)` 还原公式/Markdown，再用 `selectionRanges(range)` 生成 `{ lid, start, end }`；跨 LID 高亮用同一个 `highlight-group:*` 作为 `source_session_id` 保存多条 memory。

  Grill 问题 13：PDF selection resolve 的返回值是否应直接对齐当前 HTML selection popover 的数据模型，而不是引入一套 PDF 专属 highlight/note 模型？

  我的推荐：是，PDF selection 只换“选区解析器”，不换 memory/UI 语义。

  ```ts
  // POST /reader/pdf_selection.resolve
  type PdfSelectionResolveRequest = {
    page_rects: Array<{
      pageIndex: number;
      rects: Array<[number, number, number, number]>; // pdf_user_space
    }>;
  };

  type PdfSelectionResolveResponse = {
    anchor_lid: string;
    quote_markdown: string;
    ranges: Array<{
      lid: string;
      range: { start: number; end: number }; // original LID text UTF-16
      pdf_rects: Array<{
        pageIndex: number;
        bbox: [number, number, number, number];
      }>;
    }>;
    confidence: number;
    warnings: string[];
  };
  ```

  前端复用现有动作：

  ```text
  Highlight:
    if ranges.length > 1 -> groupId = highlight-group:uuid
    for each range -> api.highlight(lid, range, groupId)

  Note:
    open note editor anchored at anchor_lid
    initial content may include quote_markdown

  Ask AI:
    askDraft = { lid: anchor_lid, quote: quote_markdown }
  ```

  约束：

  ```text
  - memory 仍只存 LID/range/content
  - pdf_rects 只用于前端立即 overlay 或未来 display metadata，不做 citation anchor
  - resolve 失败时不保存 highlight/note，提示“selection unmapped”
  - PDF selection 输出 quote_markdown 必须来自 source.txt/LID range 拼接，而不是 PDF text 自身
  ```

  理由：这能让 PDF reader 和 HTML reader 在 highlight、note、Ask AI、quote source 上保持同一套产品语义；PDF 只负责把视觉选区翻译成已有的 LID/range。尤其 `quote_markdown` 从 source.txt 取，才能保持公式、Markdown 标记、citation truth 一致，不被 PDF text extraction 的连字符/空格噪声污染。

  你是否同意 PDF selection resolve 复用当前 HTML selection popover/memory 模型，并让 `quote_markdown` 由 LID 原文 range 拼接生成？


› 同意


• 同意点已锁：PDF selection 只替换解析器，复用现有 popover/memory 语义，`quote_markdown` 从 LID 原文 range 拼接。下一题需要定 `pdf_selection_map` 的大小和加载边界；我先查 Zotero 的按页 chars 获取和本项目 artifact/loader 读取习惯。

• 我查了：
  - Zotero：`getPageChars(pdfDocument, ..., pageIndex)` 是按页取 chars；annotation import 也是按 annotation 的 `pageIndex` 取该页 chars，再 resolve range/text。结构处理也围绕 pageDataItem 的 `chars` 分页组织。
  - 本项目：当前 sidecar 多数是启动时整文件 `read_to_string + serde_json::from_str`；但 `pdf_selection_map` 如果 char-level 全量进内存，长论文会明显比 `paper_lexicon/book_structure` 大得多。

  Grill 问题 14：`pdf_selection_map` 是否要按页分片存储，而不是单个全量 JSON 文件？

  我的推荐：要按页分片。

  ```text
  pdf_selection_map/
    manifest.json
    pages/
      0000.json
      0001.json
      0002.json
  ```

  ```ts
  interface PdfSelectionMapManifest {
    version: "pdf_selection_map.v1";
    book_id: string;
    coordinate_system: PdfCoordinateSystem;
    pages: Array<{
      pageIndex: number;
      path: string;          // pdf_selection_map/pages/0000.json
      char_count: number;
      mapped_char_count: number;
      sha256?: string;
    }>;
  }

  interface PdfSelectionMapPage {
    pageIndex: number;
    chars: PdfMappedChar[];
    word_index: PdfMappedWord[];
  }
  ```

  后端 resolve 流程：

  ```text
  POST /reader/pdf_selection.resolve { page_rects }
    -> collect unique pageIndex
    -> load only those page json files
    -> resolve chars to LID ranges
    -> return quote_markdown + ranges
  ```

  `pdf_source_map.json` 仍保持单文件，因为它是轻量 LID→region 索引，前端需要直接消费；`pdf_selection_map` 是后端交互解析用，按页 lazy load 更合理。

  理由：这跟 Zotero 的按页 chars 模型一致，也避免 `Book::load` 把全篇 char geometry 常驻内存。PH4 渲染和跳转只需要 `pdf_source_map`；PH5 只有用户框选时才需要具体页的 char-level map。

  你是否同意 `pdf_selection_map` 采用 manifest + per-page shards，后端按 selection 涉及页懒加载？

› 同意







我查了：

  - Zotero 的模型 runtime 把 threshold、schemaVersion、shape/stats 这类参数放在 metadata/runtime json 里；很多 rule 常量仍在代码里，但 fallback 会记录 limit
    等关键触发参数。

  - 本项目已有 profile artifact header/version、confidence threshold 常量、pass2 audit reason 等模式。对这次 PDF hybrid 来说，阈值直接影响 hard gate 和
    selection 结果，不能只藏在代码里。

  - Q26 锁了 MERGE_GAP_UTF16 = 1，但这类参数如果不落进 artifact，后续 report 很难复现。

  Grill 问题 27：PDF hybrid 构建是否应把所有影响验收/对齐/selection 的参数写入 alignment_report.config，并把 config hash 写入 pdf_source_map /
  pdf_selection_map freshness？

  我的推荐：要。

  type PdfHybridConfig = {
    version: "paper_pdf_hybrid_config.v1";
    alignment: {
      word_confidence_threshold: 0.85;
      fallback_confidence_threshold: 0.65;
      mapped_leaf_lid_ratio_gate: 0.80;
      lookback_words: 80;
      lookahead_words_min: 800;
      lookahead_block_multiplier: 8
;
    };
    page_furniture: {

      edge_band_ratio: 0.12;
      min_repeated_pages: 3;
      min_repeated_page_ratio: 0.25
;
    };
    selection: {
      merge_gap_utf16: 1;
      min_char_confidence: number;
    };
  };

  alignment_report.config = PdfHybridConfig
  alignment_report.config_hash = sha256(stable_json(config))

  pdf_source_map.freshness.config_hash = alignment_report.config_hash
  pdf_selection_map.manifest.freshness.config_hash = alignment_report.config_hash

  约束：

  - Any hard gate threshold must be in config.
  - Any parameter affecting source_map/selection_map output must be in config.
  - Diagnostics-only display settings do not need config_hash.
  - Config changes make old maps stale unless explicitly accepted as compatible.

  理由：PDF alignment 是 deterministic，但 deterministic 不等于可复现，除非阈值和窗口参数也被记录。Zotero 的 metadata/threshold 方式和本项目 pass2 audit 的
  做法都指向同一原则：影响产物的参数必须可追踪。

  你是否同意把 PDF hybrid 参数落进 alignment_report.config，并用 config_hash 参与 source/selection map freshness 校验？


› 同意

---

## Grill Q28 - book-scoped PDF file route with range support

**Pre-check before asking/recommending**:
- Zotero document-worker checked: PDF input is passed through explicit buffer/worker actions, and rendering is controlled through pdf.js/render-area APIs. It does not rely on UI access to arbitrary local filesystem paths.
- Local implementation checked: `crates/server` already has a book-scoped `/book/assets/*` safe file-serving boundary, but there is no `/book/pdf/*` route. Existing asset serving does not implement HTTP `Range`.

**Question**: Should PH4 add a dedicated `/book/pdf/original` file route with `HEAD` and single-byte-range support from v1, instead of reusing `/book/assets/*` or exposing arbitrary local paths?

**Recommendation**: Yes.

```http
GET /book/pdf/original
HEAD /book/pdf/original
Range: bytes=start-end
```

```text
Response:
  200 full PDF
  206 single byte range
  416 invalid range
```

**Constraints**:
- Serve only the current book's `source_manifest.attachments[].path` for the `original_pdf` attachment, e.g. `sources/original.pdf`.
- Reject absolute paths, `..`, and any path outside the current book directory.
- Use `Content-Type: application/pdf`.
- Return `Content-Length` and `Accept-Ranges: bytes`.
- Support only a single byte range in v1; multipart ranges are out of scope.
- PDF capability still comes from `source_manifest`; file presence alone does not make PDF features available.

**User answer**: Agreed.

**Locked result**: PH4 adds a book-scoped `/book/pdf/original` route with `GET`, `HEAD`, and single byte-range support. The route serves only the current book's manifest-declared original PDF artifact and never exposes arbitrary filesystem paths. `Range` support is included for pdf.js performance, while PDF feature availability remains driven by `source_manifest` capability state.

---

## Grill Q29 - source manifest and PDF source map REST boundaries

**Pre-check before asking/recommending**:
- Zotero document-worker checked: PDF/document capabilities are exposed through explicit worker actions such as `pdf.getFulltext`, `getStructuredDocumentText`, and `pdf.renderArea`; metadata/source hash travels through explicit structured payloads rather than UI probing files implicitly.
- Local implementation checked: current server exposes separate book projection endpoints such as `/book/asset_manifest`, `/book/paper_metadata`, `/book/paper_lexicon`, and `/book/paper_reading_guide`; `source_manifest.json` exists in build output but is not yet exposed through REST. `reader.state` is session state, and `profileManifest` is static profile capability, not per-book artifact availability.

**Question**: Should PH4 add dedicated `GET /book/source_manifest` and `GET /book/pdf_source_map` endpoints so the frontend discovers PDF-first capability through explicit manifest/map contracts, instead of embedding these JSON payloads into `reader.state`, `profileManifest`, or `/book/assets/*`?

**Recommendation**: Yes.

```http
GET /book/source_manifest
GET /book/pdf_source_map
```

Recommended behavior:

```text
/book/source_manifest:
  - missing file returns canonical markdown + attachments=[]
  - is the only frontend capability-discovery entry point for PDF-first
  - exposes original_pdf / pdf_source_map / pdf_selection_map status + reason + report_path

/book/pdf_source_map:
  - serves only the book-relative map declared available by source_manifest
  - validates book_id / version / freshness / config_hash
  - stale or degraded maps return explicit degraded/stale errors instead of silently serving bad data
```

**Constraints**:
- Do not expose the full `pdf_selection_map` to the frontend; selection resolution remains backend-only through `/reader/pdf_selection.resolve`.
- Keep `reader.state` limited to current reading/layout session state.
- Keep `profileManifest` limited to static profile capability; it must not claim the current book has a PDF.
- Keep `/book/assets/*` for render assets only; do not overload it with source/map contracts.

**User answer**: Agreed.

**Locked result**: PH4 frontend discovery is `GET /book/source_manifest -> capability check -> GET /book/pdf_source_map` when projection is available. Source manifest and source map are explicit book projection endpoints. `reader.state`, `profileManifest`, and `/book/assets/*` do not carry PDF artifact availability or source-map payloads, and `pdf_selection_map` remains backend-only for selection resolution.

---

## Grill Q30 - pdf_source_map error semantics

**Pre-check before asking/recommending**:
- Zotero document-worker checked: worker actions return structured errors when an action cannot produce a valid result; degraded/fallback extraction is recorded explicitly rather than being silently treated as a normal payload.
- Local implementation checked: the server already uses a `ToolError { error_code, category, message }` envelope, while paper projections use `available + warning` for user-facing degraded read models. `source_manifest` is the right place to explain capability status; `pdf_source_map` is a runtime payload consumed only when safe.

**Question**: For `GET /book/pdf_source_map`, when the source map file exists but its capability is `degraded`, `stale`, or `failed`, should the endpoint return an error envelope instead of `200` with an unusable payload?

**Recommendation**: Yes. Use `/book/source_manifest` for state explanation and `/book/pdf_source_map` only for a usable map.

```text
/book/source_manifest:
  200
  capability.project_lid_to_pdf = {
    status: "degraded" | "stale" | "failed",
    reason,
    report_path
  }

/book/pdf_source_map:
  200 only if map is usable
  404 PDF_SOURCE_MAP_MISSING
  409 PDF_SOURCE_MAP_DEGRADED
  409 PDF_SOURCE_MAP_STALE
  500 PDF_SOURCE_MAP_INVALID
```

**Constraints**:
- `source_manifest` is the status/explanation endpoint and should return displayable degraded state whenever possible.
- `pdf_source_map` is a consumption endpoint and must not return a half-usable map.
- Frontend capability discovery must not be based on catching `/book/pdf_source_map` errors; discovery goes through `/book/source_manifest`.
- Error responses use the existing `ToolError { error_code, category, message }` envelope. If a distinct HTTP 409 category is needed, define that in the implementation slice; otherwise map stale/degraded to the nearest existing validation/conflict category without changing payload shape.

**User answer**: Agreed.

**Locked result**: `/book/pdf_source_map` returns `200` only for a validated, usable source map. Missing, degraded, stale, or invalid maps produce explicit error envelopes. User-facing degraded explanations live in `/book/source_manifest`, which is the sole frontend capability-discovery endpoint.

---

## Grill Q31 - PDF selection resolve status vs HTTP errors

**Pre-check before asking/recommending**:
- Zotero document-worker checked: PDF annotation/selection logic skips annotations without rects and resolves text only when rect/char data is available; it does not invent positions for unresolvable annotations. Invalid action execution is surfaced as a worker error.
- Local implementation checked: current HTML selection opens actions only when it can produce at least one LID range and quote text. `reader.highlight` rejects invalid UTF-16 ranges with `INVALID_RANGE`, but a user selecting an unmapped visual area is a normal product outcome, not a malformed API request.

**Question**: Should `POST /reader/pdf_selection.resolve` return HTTP `200` for valid requests whose selection result is `resolved`, `partial`, or `unresolved`, using the response body status to express business outcome, while reserving error envelopes for request/capability/artifact failures?

**Recommendation**: Yes.

```text
200:
  status = "resolved"    // normal Highlight / Note / Ask AI actions
  status = "partial"     // save only resolved ranges; disclose unmapped areas
  status = "unresolved"  // no save actions; show reason

4xx/5xx:
  invalid request shape
  PDF selection capability unavailable
  selection map missing/stale/invalid
  page shard read/parse failure
```

```ts
type PdfSelectionResolveResponse = {
  status: "resolved" | "partial" | "unresolved";
  anchor_lid?: string;
  quote_markdown: string;
  ranges: PdfSelectionResolvedRange[];
  unresolved_rects: PdfSelectionUnresolvedRect[];
  warnings: string[];
};
```

**Constraints**:
- `unresolved` is a valid business result, not an exception.
- `partial` must not be silently saved as complete; UI must disclose that only mapped text will be saved.
- HTTP errors mean the resolver cannot run trustworthily or the request/artifacts are invalid.
- Save actions consume only `ranges[]`; `unresolved_rects[]` never become memory anchors.

**User answer**: Agreed.

**Locked result**: `/reader/pdf_selection.resolve` returns HTTP `200` for `resolved`, `partial`, and `unresolved` selection outcomes. Error envelopes are reserved for invalid requests, unavailable PDF selection capability, stale/invalid selection-map artifacts, or backend shard failures. Frontend save actions are gated by response status and use only resolved LID ranges.

---

## Grill Q32 - batch semantic range projection back to PDF

**Pre-check before asking/recommending**:
- Zotero document-worker checked: annotation rendering paths operate over annotation lists with `pageIndex + rects` positions, and render helpers compute projected areas in batches rather than forcing one request per annotation.
- Local implementation checked: frontend currently loads all memory annotations with `api.recall({})`, keeps them as `MemoryRecord[]`, and filters/sorts highlights/notes locally. Memory persists semantic `LID + UTF-16 range`, not PDF rects.

**Question**: Should PH5 expose a batch `LID/range -> PDF rects` projection endpoint so the frontend submits visible-page/nearby annotations together, instead of calling once per memory record or LID?

**Recommendation**: Yes.

```http
POST /reader/pdf_ranges.project
```

```ts
type PdfRangeProjectRequest = {
  items: Array<{
    client_id: string;        // mem_id or UI-local id
    lid: string;
    range: { start: number; end: number } | null;
  }>;
  page_filter?: number[];     // optional: return rects only on visible/preload pages
};

type PdfRangeProjectResponse = {
  projected: Array<{
    client_id: string;
    lid: string;
    range: { start: number; end: number } | null;
    status: "exact" | "lid_region_fallback" | "unmapped";
    pdf_rects: Array<{ pageIndex: number; bbox: [number, number, number, number] }>;
    warnings: string[];
  }>;
};
```

**Behavior boundary**:
- Exact `range` projection uses `pdf_selection_map` char shards.
- Whole-LID highlight or failed range projection falls back to `pdf_source_map` primary/all regions where possible.
- `page_filter` only filters returned rects; it does not change the semantic projection result.
- Response is per-item; one `unmapped` item must not fail the whole batch.
- HTTP errors are reserved for unavailable map capability, invalid request shape, or backend shard/map read failures.

**User answer**: Agreed.

**Locked result**: PH5 uses a batch `POST /reader/pdf_ranges.project` endpoint for projecting semantic annotations back to PDF overlays. It returns per-item `exact | lid_region_fallback | unmapped` statuses and supports optional visible-page filtering. Memory remains semantic `LID/range`; PDF rects are derived display data.

---

## Grill Q33 - frontend-owned PDF overlay coordinate conversion

**Pre-check before asking/recommending**:
- Zotero document-worker checked: PDF page-space rects remain the data truth; rendering converts them inside each page/container to viewport/CSS coordinates. The preview keeps canvas rendering and highlight overlay as separate layers, with canvas scaled for DPR and overlay positioned independently.
- Local implementation checked: current `ReaderPane.vue` is an HTML prose reader that derives current LID from DOM `[data-lid]` elements. There is no PDF page component or overlay layer yet, so PH4 must introduce that boundary explicitly instead of mixing PDF display state into existing Markdown spans.

**Question**: Should PH4 keep `pdf_source_map` and backend endpoints in stable PDF user-space while each frontend PDF page component converts page bboxes to overlay CSS coordinates locally?

**Recommendation**: Yes.

```ts
type PdfOverlayRegion = {
  lid: string;
  pageIndex: number;
  bbox: [number, number, number, number]; // pdf_user_space
  status: "word_mapped" | "line_fallback" | "block_fallback";
  confidence: number;
};

function pdfRectToCssPercent(
  bbox: [number, number, number, number],
  pageView: [number, number, number, number],
): { left: string; top: string; width: string; height: string } {
  const [vx1, vy1, vx2, vy2] = pageView;
  const [x1, y1, x2, y2] = bbox;
  const w = vx2 - vx1;
  const h = vy2 - vy1;

  return {
    left: `${((x1 - vx1) / w) * 100}%`,
    top: `${((vy2 - y2) / h) * 100}%`,
    width: `${((x2 - x1) / w) * 100}%`,
    height: `${((y2 - y1) / h) * 100}%`,
  };
}
```

**Constraints**:
- `pdf_source_map` stores only PDF user-space bboxes.
- No CSS px, DPR, zoom, viewport, or responsive layout coordinates are persisted.
- Each frontend page component owns canvas rendering plus overlay coordinate conversion.
- Zoom, resize, and DPR changes recompute overlays from the same stable source map.
- Backend selection/range endpoints also speak PDF user-space, not CSS pixels.

**User answer**: Agreed.

**Locked result**: PH4 uses PDF user-space as the only persisted/backend coordinate system. The frontend PDF page component converts source-map bboxes into CSS overlay rectangles at render time. Rendering state such as zoom, DPR, canvas size, and responsive width stays frontend-local and never makes the source map stale.

---

## Grill Q34 - PDF scroll sync through existing reader.goto

**Pre-check before asking/recommending**:
- Zotero document-worker checked: the preview uses `IntersectionObserver` for lazy PDF page rendering and keeps PDF regions linked bidirectionally with structured/html blocks. PDF overlay click scrolls the corresponding text block, and text block click scrolls the PDF region.
- Local implementation checked: `ReaderPane.vue` derives the current reading LID from visible DOM `[data-lid]` elements using a probe line, then emits `current-lid`. The existing semantic session state is still `reader.state().viewport.top_lid/anchor_lid`, and the existing mutable position commands are `/reader/goto` and `/reader/scroll`.

**Question**: Should PH4 reuse existing `/reader/goto(lid)` as the semantic position write path for LIDs derived from PDF scrolling, instead of adding a new `reader.pdf_position.sync` command?

**Recommendation**: Yes.

```text
PDF scroll/resize/render settle
  -> probe at 28% viewport height
  -> find topmost/highest-confidence mapped region crossing probe
  -> derived_lid
  -> if derived_lid != last_synced_lid:
       debounce 250ms
       POST /reader/goto { lid: derived_lid }
       apply reader.state profile/layout only
       do not force Markdown window render in PDF mode
```

**Constraints**:
- `/reader/goto` remains the only semantic position mutation.
- No `pageIndex`, PDF `scrollTop`, zoom, or page-local visual offset is written into `reader.state`.
- PDF visual restore metadata may be separate display state later, but it is not the semantic anchor.
- Unmapped PDF areas do not update semantic position.
- Frontend must debounce and deduplicate; no POST on every scroll tick.
- In PDF mode, `goto` response updates agent/layout/session position but must not automatically switch the center surface back to Markdown.

**User answer**: Agreed.

**Locked result**: PH4 synchronizes the current PDF visual position back to the existing semantic reader state by deriving a mapped LID on the frontend and calling `/reader/goto(lid)` after debounce. No new semantic PDF-position command is introduced. PDF page, zoom, and scroll restore remain display-only state outside `reader.state`.

---

## Grill Q35 - official pdfjs-dist thin adapters

**Pre-check before asking/recommending**:
- Zotero document-worker checked: it directly wires Zotero/pdf.js source, defines `PDFJSDev`, registers a resolver hook, and carries worker/build assets and Zotero-specific dependency boundaries. This is exactly the integration surface ADR-0062 forbids us to inherit.
- Local implementation checked: `packages/web` is Vite + Vue and currently has no `pdfjs-dist`; `packages/core` also has no PDF dependency. ADR-0062 and the PDF-first slice plan require a permissively licensed Mozilla pdf.js path and prohibit Zotero fork paths, worker protocol, models, wasm assets, and derived code.

**Question**: Should PH1/PH4 limit the PDF dependency to official `pdfjs-dist` and implement separate thin adapters for build-time geometry extraction and web rendering, instead of introducing a shared Zotero-like document-worker layer?

**Recommendation**: Yes.

```text
packages/core
  pdf/extract-text-geometry.ts
    input: pdf file path / ArrayBuffer
    output: PdfTextGeometry + extraction_report
    dependency: pdfjs-dist

packages/web
  pdf/PdfPage.vue or pdf/render.ts
    input: /book/pdf/original + pdf_source_map
    output: canvas pages + overlay
    dependency: pdfjs-dist + Vite worker setup
```

Recommended frontend worker wiring:

```ts
import * as pdfjs from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.mjs?url";

pdfjs.GlobalWorkerOptions.workerSrc = workerUrl;
```

**Constraints**:
- No Zotero pdf.js fork paths.
- No Zotero worker protocol.
- No `structured-document-text`, ONNX, wasm, or other Zotero document-worker assets.
- Build-time extractor and frontend viewer may share project-owned contract/types, but not a shared runtime worker abstraction in v1.
- PH1 extraction tests validate geometry output; PH4 web typecheck/build/smoke validate viewer bundling and rendering.

**User answer**: Agreed.

**Locked result**: PH1 and PH4 use official `pdfjs-dist` through two narrow project-owned adapters: one build-time text geometry extractor in `packages/core`, and one Vite/Vue rendering adapter in `packages/web`. They share only project-owned PDF contracts/types. No shared document-worker runtime layer is introduced in v1.

---

## Grill Q36 - all page shells with lazy canvas rendering

**Pre-check before asking/recommending**:
- Zotero document-worker checked: the preview creates page containers for every PDF page, but renders page canvases lazily with `IntersectionObserver`. On resize it recomputes page dimensions and rerenders only visible pages.
- Local implementation checked: the current HTML reader uses a LID window buffer driven by `/reader/scroll`, but PH4 acceptance only requires a PDF surface, mapped LID overlays, citation jump, and Markdown fallback for unmapped LIDs. Thumbnails, printing, and complex text selection are explicitly out of scope for PH4.

**Question**: Should PH4 use a minimal PDF rendering model with all page shells present and lazy canvas rendering, instead of reusing the existing LID window buffer or building complex page virtualization immediately?

**Recommendation**: Yes.

```text
on pdf_source_map + pdf document loaded:
  for each page:
    create PageShell {
      pageIndex,
      dimensions,
      div height from fit-to-width,
      overlay regions from pdf_source_map filtered by pageIndex
      canvas not rendered yet
    }

IntersectionObserver(root: pdfPane, rootMargin: "800px"):
  when page enters preload range:
    render canvas if stale
    mount/update overlay
  when page leaves far range:
    optional release canvas bitmap after timeout
```

**Constraints**:
- Do not drive PDF reading through `/reader/scroll`.
- `/reader/scroll` remains HTML/LID-window specific.
- PDF surface may call `/reader/goto` for semantic anchor sync and citation jumps.
- PH4 does not implement thumbnails or full page recycling with dynamic spacer math.
- Canvas render cache stays frontend-local, e.g. visible pages plus a small prefetch band.
- Page shells preserve stable scroll heights so citation jumps can scroll before canvas rendering completes.

**User answer**: Agreed.

**Locked result**: PH4 renders PDF with all page shells in the scroll flow and lazy canvas/overlay work for visible or near-visible pages. It does not reuse the HTML reader's LID window buffer and does not introduce full PDF page virtualization in v1. `/reader/scroll` remains specific to the HTML/LID reader path.

---

## Grill Q37 - deterministic primary PDF region for citation jumps

**Pre-check before asking/recommending**:
- Zotero document-worker checked: the preview jumps from a structured/html block to the first linked PDF region element using `scrollIntoView({ block: "center" })`, while all linked regions can still be highlighted. Structure utilities can aggregate child `pageRects`, but the UI jump target is a single deterministic region.
- Local implementation checked: current `doGoto(lid)` updates semantic reader position and loads the HTML window, while `openSourcePreview` provides the Markdown quote/source fallback. PH4 acceptance explicitly requires mapped LID citations to scroll to the PDF region and unmapped LIDs to fall back to Markdown source preview.

**Question**: Should PH4 use a deterministic `primary_region` rule for LID citation jumps: scroll to the best PDF region when mapped, and open Markdown source preview only when unmapped?

**Recommendation**: Yes.

```ts
interface PdfSourceMapEntry {
  lid: string;
  status: "word_mapped" | "line_fallback" | "block_fallback" | "unmapped" | "excluded";
  primary_region?: PdfRegion;
  pdf_regions: PdfRegion[];
  alignment: PdfAlignmentInfo;
}
```

Recommended jump behavior:

```text
gotoLidInPaperMode(lid):
  entry = pdfSourceMap.entriesByLid[lid]
  if entry?.primary_region:
    scroll PDF page shell to primary_region center
    visually pulse all regions for that LID
    POST /reader/goto(lid) for semantic sync
  else:
    open Markdown Quote source preview for lid
    keep PDF surface active
```

**Constraints**:
- Do not use page/bbox as citation anchor.
- Do not silently jump to a low-confidence nearby LID when the requested LID is unmapped.
- `unmapped` and `excluded` entries use Markdown fallback.
- If multiple regions exist, `primary_region` is deterministic and produced by the map builder.
- UI may highlight all regions for the LID, but the scroll target is exactly one primary region.

**User answer**: Agreed.

**Locked result**: `pdf_source_map` entries include a deterministic `primary_region` for mapped LIDs. PH4 citation jumps scroll the PDF surface to that primary region and pulse all regions for the LID. Unmapped or excluded LIDs fall back to Markdown source preview without inventing a nearby PDF target.

---

## Grill Q38 - mutually exclusive center reader surface

**Pre-check before asking/recommending**:
- Zotero document-worker checked: its preview UI is a diagnostic side-by-side `pdf-panel + html-panel + divider` surface with independent scroll positions. That is useful for structure extraction review, but it is not a necessary product shape for a reading app.
- Local implementation checked: the current app center column mounts one `ReaderPane`, while Markdown fallback already exists through the `sourcePreview` modal. Paper-specific navigation is already moving into the shared `LeftRail` minimap, and `RightRail` remains the shared agent/notes surface.

**Question**: Should PH4 introduce a mutually exclusive center `ReaderSurface` that shows `PdfReaderPane` for paper books with usable PDF capability and the existing `ReaderPane` otherwise, instead of adding a Zotero-style PDF/Markdown dual-pane reader?

**Recommendation**: Yes.

```vue
<component
  :is="activeReaderSurface"
  v-bind="readerSurfaceProps"
  @goto="doGoto"
  @current-lid="onCurrentLid"
  @selection-resolved="onSelectionResolved"
/>
```

```ts
type ReaderSurfaceKind = "html" | "pdf";

const activeReaderSurface =
  isPaperProfile && sourceManifest?.capabilities.project_lid_to_pdf.status === "available"
    ? "pdf"
    : "html";
```

**Behavior boundary**:
- PDF surface is the main reading surface for paper profile when PDF capability is available.
- PDF surface owns PDF scroll, overlays, citation jumps, and PDF selection resolve.
- HTML surface remains the default for `technical_learning` and for paper books without a usable PDF map.
- Markdown source preview remains a modal/temporary fallback, not a second permanent reader.

**Constraints**:
- Do not add a persistent side-by-side Markdown pane in PH4.
- Do not duplicate note/highlight/agent rails per surface.
- `LeftRail` and `RightRail` stay shared.
- Surface switching must not change LID, memory, or citation semantics.

**User answer**: Agreed.

**Locked result**: PH4 uses one center reading surface at a time: `PdfReaderPane` for paper books with usable PDF capability, otherwise the existing HTML `ReaderPane`. Zotero-style permanent PDF/Markdown split view is out of scope for PH4. Markdown remains available through existing source-preview fallback paths.

---

## Grill Q39 - source manifest v2 as PDF capability manifest

**Pre-check before asking/recommending**:
- Zotero document-worker checked: structure outputs carry explicit `sourceHash`, PDF fingerprint, and source metadata/properties. Source identity is represented as structured metadata rather than inferred from UI file state.
- Local implementation checked: `packages/core/src/source-manifest.ts` still reflects the ADR-0046 side-preview attachment model. It has `attachments[].pdf_source_map.status` limited to `not_provided | provided`, and the current real paper fixture has `attachments: []`. Q29-Q32 and Q37 already require explicit PDF capability discovery, stale/degraded status, selection-map availability, range projection, and source-map primary regions.

**Question**: Should PH3 upgrade `source_manifest.json` from a PDF side-preview attachment manifest into a book source capability manifest that records PDF file, source map, selection map, alignment report, and freshness state?

**Recommendation**: Yes.

```ts
interface SourceManifest {
  version: "source_manifest.v2";
  book_id: string;
  canonical_source: {
    kind: "markdown" | "epub";
    path: string;
    truth_file: "source.txt";
    participates_in_lid: true;
    citation_anchor: "lid";
    sha256?: string;
  };
  original_pdf?: {
    path: string;                 // book-relative
    sha256?: string;
    fingerprint?: string;
    citation_anchor: false;
  };
  capabilities: {
    project_lid_to_pdf: PdfCapability;
    resolve_pdf_selection: PdfCapability;
    project_ranges_to_pdf: PdfCapability;
  };
}

type PdfCapabilityStatus =
  | "unavailable"
  | "available"
  | "degraded"
  | "stale"
  | "failed";

interface PdfCapability {
  status: PdfCapabilityStatus;
  reason?: string;
  artifact_path?: string;
  report_path?: string;
  config_hash?: string;
  source_sha256?: string;
  pdf_sha256?: string;
}
```

Recommended capability mapping:

```text
project_lid_to_pdf
  -> pdf_source_map.json

resolve_pdf_selection
  -> pdf_selection_map/manifest.json

project_ranges_to_pdf
  -> pdf_selection_map + pdf_source_map usable together

alignment/debug
  -> alignment_report.json
```

**Constraints**:
- `source_manifest` is capability discovery, not the map payload.
- `/book/pdf_source_map` returns a map only when `project_lid_to_pdf.status == available`.
- `pdf_selection_map` remains backend-only; the manifest may disclose status/path, not page shard contents.
- `profileManifest` remains static profile capability and must not claim the current book has a PDF.
- Legacy `source_manifest` v1 can be read as no PDF-first capability unless upgraded by the hybrid build.

**User answer**: Agreed.

**Locked result**: PH3 upgrades `source_manifest.json` to `source_manifest.v2`, centered on per-book PDF-first capability discovery. PDF file identity, source-map availability, selection-map availability, range-projection availability, alignment report path, and freshness hashes live under `capabilities`. The manifest remains a discovery/status contract, while actual map payloads stay behind dedicated endpoints.
---

## Grill Q40 - frontend loads full public PDF source map

**Pre-check before asking/recommending**:
- Zotero document-worker checked: preview/structure paths keep structured blocks and `pageRects` available while page canvas rendering is lazy; the heavy char/selection work is separate from ordinary page rendering.
- Local implementation checked: current server exposes small book artifacts such as `/book/asset_manifest`, but has no PDF/source-map endpoints yet. PH4 needs citation jumps, current-LID probing, and mapped/unmapped fallback without an extra request per visible page.

**Question**: Should PH4 load the full public `pdf_source_map` once on the frontend, instead of paging overlay regions by PDF page?

**Recommendation**: Yes. Load `source_manifest` and `pdf_source_map` once, then build local frontend indexes.

```http
GET /book/source_manifest
GET /book/pdf_source_map
```

```ts
interface PdfSourceMap {
  version: "pdf_source_map.v1";
  book_id: string;
  coordinate_system: PdfCoordinateSystem;
  pages: PdfPageMeta[];
  entries: PdfSourceMapEntry[];
  excluded_regions: PdfExcludedRegion[];
  report: PdfAlignmentSummary;
}
```

```ts
entriesByLid: Map<string, PdfSourceMapEntry>
regionsByPage: Map<number, PdfOverlayRegion[]>
primaryRegionByLid: Map<string, PdfRegion>
```

**Constraints**:
- `pdf_source_map` is frontend-public because it contains only LID, bbox, confidence, and primary-region projection data.
- `pdf_selection_map` remains backend-only and is consumed through resolver/project endpoints.
- If source maps become too large later, add `GET /book/pdf_source_map?page=`, but v1 does not prepay that pagination complexity.
- `/book/pdf_source_map` returns a map only when `source_manifest.capabilities.project_lid_to_pdf.status == "available"`.

**User answer**: Agreed.

**Locked result**: PH4 loads the complete public `pdf_source_map` once and builds local `entriesByLid`, `regionsByPage`, and `primaryRegionByLid` indexes. Char-level selection and range projection data stay backend-private behind `/reader/pdf_selection.resolve` and `/reader/pdf_ranges.project`.

---

## Grill Q41 - dedicated current-book original PDF route

**Pre-check before asking/recommending**:
- Zotero document-worker checked: its preview fetches the PDF as bytes and passes `data` into pdf.js `getDocument`; worker actions also pass PDF buffers into render/structure routines. The viewer does not need arbitrary local path access.
- Local implementation checked: the current server already has a constrained binary route pattern for `/book/assets/...`, resolved under the current book directory and rejecting traversal. There is no PDF route yet, and Q39 makes `source_manifest.v2.original_pdf.path` the PDF discovery point.

**Question**: Should PH4 serve the original PDF through a dedicated current-book endpoint resolved from `source_manifest.v2`, instead of putting PDFs under `/book/assets/*` or letting the frontend fetch arbitrary file paths?

**Recommendation**: Yes.

```http
GET /book/pdf/original
```

Resolution rule:

```text
/book/pdf/original
  -> read source_manifest.json
  -> require original_pdf.path
  -> require capabilities.project_lid_to_pdf.status in available|degraded
  -> resolve original_pdf.path under current book_dir
  -> reject absolute/path traversal/colon/backslash
  -> return application/pdf bytes
```

Frontend PH4 can use whole-buffer loading in v1:

```ts
const pdfBytes = await fetch("/api/book/pdf/original").then(r => r.arrayBuffer());
const doc = await pdfjs.getDocument({ data: pdfBytes }).promise;
```

**Constraints**:
- The PDF is a visual artifact, not a citation source.
- `/book/pdf/original` serves only the current opened book's declared PDF.
- It is not a general static file server.
- Keep it separate from `/book/assets/*`; assets are render bundles, while the PDF is the visual source.
- PH4 can load the whole PDF buffer first; HTTP Range support is deferred to a later performance slice if large PDFs become a real bottleneck.

**User answer**: Agreed.

**Locked result**: PH4 adds dedicated `GET /book/pdf/original` as the only frontend PDF loading path. The route resolves the declared `source_manifest.v2.original_pdf.path` under the current book directory, rejects unsafe paths, returns `application/pdf`, and uses whole-buffer loading in v1. Range requests are out of scope for PH4.

---

## Grill Q42 - source manifest endpoint normalization

**Pre-check before asking/recommending**:
- Zotero document-worker checked: structure output carries explicit `sourceHash` and `metadata.source.properties`; source identity/freshness is structured metadata, not inferred from UI state.
- Local implementation checked: there is no `/book/source_manifest` route yet. Current `packages/core/src/source-manifest.ts` is the legacy v1 attachment model with `attachments[].pdf_source_map.status = not_provided | provided`; server currently exposes `/book/asset_manifest`, whose missing-file behavior returns a safe empty manifest.

**Question**: Should `GET /book/source_manifest` be a normalization endpoint that always returns a v2 capability shape, upgrading legacy v1 and synthesizing "PDF unavailable" for missing manifests, instead of raw-file passthrough where the frontend must handle v1/missing/error branches?

**Recommendation**: Yes.

```http
GET /book/source_manifest
```

Normal response shape:

```ts
interface SourceManifestV2 {
  version: "source_manifest.v2";
  book_id: string;
  canonical_source: {
    kind: "markdown" | "epub" | "unknown";
    path: string | null;
    truth_file: "source.txt";
    participates_in_lid: true;
    citation_anchor: "lid";
    sha256?: string;
  };
  original_pdf?: {
    path: string;
    sha256?: string;
    fingerprint?: string;
    citation_anchor: false;
  };
  capabilities: {
    project_lid_to_pdf: PdfCapability;
    resolve_pdf_selection: PdfCapability;
    project_ranges_to_pdf: PdfCapability;
  };
}
```

Normalization rules:

```text
v2 file exists and valid
  -> return as-is, after schema validation

legacy v1 file exists
  -> convert canonical_source
  -> convert attachments[0].pdf_source_map.provided to project_lid_to_pdf status
  -> mark selection/range projection unavailable unless v2 artifacts exist

source_manifest.json missing
  -> return synthesized v2:
     canonical_source.kind = "unknown"
     canonical_source.path = null
     all PDF capabilities = unavailable, reason = "missing_source_manifest"

source_manifest.json invalid JSON/schema
  -> HTTP error SOURCE_MANIFEST_INVALID
```

**Constraints**:
- Frontend clients consume only the normalized v2 shape.
- Missing `source_manifest.json` is a valid old-book/no-PDF state, not an error.
- Invalid JSON/schema is an error because it indicates a corrupted build artifact.
- Legacy v1 attachment details do not leak into frontend branching.

**User answer**: Agreed.

**Locked result**: `/book/source_manifest` returns a v2 capability response for valid v2, converted legacy v1, and missing-manifest cases. Invalid manifest JSON/schema returns `SOURCE_MANIFEST_INVALID`. The frontend decides PDF availability from `manifest.capabilities.project_lid_to_pdf.status` without understanding legacy attachment shapes.

---

## Grill Q43 - PDF capability status gates

**Pre-check before asking/recommending**:
- Zotero document-worker checked: PDF structure output records `sourceHash` and PDF metadata in structured source metadata, so source freshness and identity should be explicit artifact state rather than frontend inference.
- Local implementation checked: existing paper/read-tools projections use explicit `available:false + warning` for missing optional sidecars, while corrupted sidecars fail fast. Frontend already has `paperWarnings` and Markdown source-preview fallback paths.

**Question**: Should `source_manifest.v2.capabilities.project_lid_to_pdf.status` gate the reader so that only `available` auto-enables PDF-first, `degraded` allows manual PDF opening with warnings, and `stale | failed | unavailable` disable the PDF surface?

**Recommendation**: Yes.

```ts
type PdfCapabilityStatus =
  | "available"
  | "degraded"
  | "stale"
  | "failed"
  | "unavailable";
```

Frontend gate:

```ts
function defaultReaderSurface(manifest): "pdf" | "html" {
  return manifest.capabilities.project_lid_to_pdf.status === "available"
    ? "pdf"
    : "html";
}

function canOpenPdfManually(manifest): boolean {
  return manifest.capabilities.project_lid_to_pdf.status === "available"
      || manifest.capabilities.project_lid_to_pdf.status === "degraded";
}
```

Status semantics:

```text
available
  -> default PDF-first; source map, PDF, and hashes match; citation jump is trusted

degraded
  -> PDF and partial/low-confidence map are usable, but not trusted enough for default PDF-first
  -> UI may offer manual PDF opening with warning
  -> unmapped LIDs fall back to Markdown source preview

stale
  -> artifact hashes do not match current source/PDF; PDF map must not drive UI
  -> requires rebuild

failed
  -> hybrid build attempted and failed; PDF surface disabled
  -> report/alignment error summary may be displayed

unavailable
  -> old book or no PDF; normal HTML reader
```

Endpoint gates:

```text
/book/pdf_source_map
  available -> 200
  degraded -> 200 with report/warnings
  stale/failed/unavailable -> capability error

/book/pdf/original
  available|degraded -> 200
  stale/failed/unavailable -> capability error or not found

/reader/pdf_selection.resolve
/reader/pdf_ranges.project
  available -> 200
  degraded -> 200, but may return partial/unresolved
  stale/failed/unavailable -> capability error
```

**Constraints**:
- Stale maps must never silently drive PDF overlays or selection resolution.
- Degraded maps are opt-in/manual and must surface warnings.
- HTML/Markdown reader remains the safe fallback for all non-available default states.

**User answer**: Agreed.

**Locked result**: `available` is the only status that auto-selects PDF-first. `degraded` permits manual PDF use with warnings and fallback behavior. `stale`, `failed`, and `unavailable` disable the PDF surface and PDF map/selection endpoints.

---

## Grill Q44 - sharded PDF selection map

**Pre-check before asking/recommending**:
- Zotero document-worker checked: annotation handling loads chars by `pageIndex` and resolves highlight rects against that page's chars; it does not treat whole-document char geometry as ordinary viewer payload. It also splits oversized annotation positions, which reinforces that char/position data needs physical boundaries.
- Local implementation checked: `.understand-book/<book_id>/` already separates formal artifacts from `.build/` intermediates, and existing resume flows use shard-like per-window files. Q7 locks char-level selection data as required; Q40 locks it as backend-only rather than frontend-public.

**Question**: Should `pdf_selection_map` use a `manifest.json + per-page shard` layout instead of one giant `pdf_selection_map.json`?

**Recommendation**: Yes.

```text
.understand-book/<book_id>/
  pdf_source_map.json
  pdf_selection_map/
    manifest.json
    pages/
      00000.json
      00001.json
      ...
```

```ts
interface PdfSelectionMapManifest {
  version: "pdf_selection_map.v1";
  book_id: string;
  coordinate_system: PdfCoordinateSystem;
  source_sha256: string;
  pdf_sha256: string;
  page_count: number;
  pages: Array<{
    pageIndex: number;
    shard_path: string;
    char_count: number;
    mapped_char_count: number;
    unmapped_char_count: number;
    sha256: string;
  }>;
}

interface PdfSelectionPageShard {
  version: "pdf_selection_page.v1";
  book_id: string;
  pageIndex: number;
  chars: PdfMappedChar[];
  words?: PdfMappedWord[];
}
```

Resolver behavior:

```text
/reader/pdf_selection.resolve(rects)
  -> group rects by pageIndex
  -> load only required page shards
  -> intersect chars with rects
  -> map chars to LID UTF-16 ranges
  -> return quote_markdown + ranges + unresolved_rects
```

**Constraints**:
- `pdf_selection_map/manifest.json` may be referenced from `source_manifest.v2` capability metadata.
- Page shards are backend-private and not a frontend public contract.
- Missing shard or sha mismatch returns an artifact/capability error; it must not silently degrade to coarse LID selection.
- `Book::load` does not preload selection shards; resolver endpoints read required shards on demand.

**User answer**: Agreed.

**Locked result**: `pdf_selection_map` is stored as a backend-only manifest plus per-page shards. Selection/range endpoints load only the required page shards, verify shard identity, and return semantic LID/range results. The frontend never consumes the full char-level selection map directly.

---

## Grill Q45 - source.txt is reconciled canonical source, not raw Markdown

**Pre-check before asking/recommending**:
- Zotero document-worker checked: PDF structure generation derives text nodes from PDF chars/blocks with page anchors, and text selection uses char ranges to recover text. This supports treating PDF text as an explicit verification source rather than merely a visual sidecar.
- Local implementation checked: current `source.txt` is written directly from `loadBookWindows(book).source`; for Markdown input that means raw `paper.md` is copied through. `segment(SourceBlock[])` then assigns LID spans over that text, so any OCR/conversion/cleanup error in Markdown becomes a stable citation/range error.

**Question**: Should `source.txt` be redefined as the PDF+Markdown reconciled canonical source, with LIDs generated only after reconciliation, instead of treating raw `paper.md` as semantic truth?

**Recommendation**: Yes.

Old assumption:

```text
paper.md
  -> source.txt
  -> markdownToBlocks(source.txt)
  -> segment()
  -> LID tree
```

Revised source-prep:

```text
paper.md + paper.pdf
  -> extract PDF text geometry
  -> reconcile markdown draft against PDF text
  -> source.txt
  -> source_reconciliation_report.json
  -> markdownToBlocks(source.txt)
  -> segment(SourceBlock[])
  -> LID tree
  -> pdf_source_map.json
  -> pdf_selection_map/*
```

```ts
type SourceBlockReconcileStatus =
  | "verified"
  | "auto_repaired"
  | "needs_review"
  | "pdf_unmatched"
  | "md_unmatched";
```

**Constraints**:
- `paper.md` is a draft source, not truth.
- `source.txt` is the canonical Markdown text used for LID spans and `book.text`.
- LIDs must be generated from `source.txt`, not raw `paper.md`.
- Safe deterministic repairs may enter `source.txt`; unresolved semantic conflicts must be recorded and gated.
- `source_reconciliation_report.json` records what changed, what failed, and what needs review.
- ADR-0062 and the PDF-first slice plan must be revised: "`source.txt` is original Markdown text" is no longer valid for OCR/conversion-derived Markdown.

**User answer**: Agreed.

**Locked result**: For paper PDF-first builds, `source.txt` is the reconciled canonical source produced from `paper.md + paper.pdf`. Raw Markdown is only an input draft. LID spans, `book.text`, highlights, notes, and citations are all based on reconciled `source.txt`. PDF alignment and selection maps are built against that reconciled source.





• Grill Q46 - 自动修复边界

  Pre-check：

  - Zotero 已查：它从 PDF chars 恢复文本时会根据 spaceAfter / lineBreakAfter / paragraphBreakAfter 还原空白；并且明确处理 OCR PDF “每行像单独段落”的情况。结
    构推断失败时，它会标记 degraded/fallback，而不是假装结构可靠。

  - 本地已查：构建链路已有强门禁习惯：pending 默认拒绝收口，--allow-partial 只允许 smoke/救急；语义抽取里证据不足宁可 pending/omit，不编造。

  问题：source reconciliation 的自动修复是否只允许“版面/编码层面的确定性安全修复”，而把任何可能改变语义、结构层级或增删正文内容的差异都打成 needs_review，不
  自动进入 source.txt？

  我的推荐：同意，自动修复范围要窄。

  可以自动进 source.txt 的修复：

  safe_auto_repair:
    - 空白折叠/恢复：多个空格、PDF 换行、段内软换行
    - 行尾断词：cardio-\nmyopathy -> cardiomyopathy，需 PDF/MD 两侧高置信支持
    - Unicode 规范化：NFC/NFKC、常见连字 fi/fl
    - PDF 页眉页脚/页码排除，不进入正文
    - OCR 常见孤立换行导致的段内合并，但不跨标题/章节边界

  必须 needs_review 的情况：

  needs_review:
    - MD 与 PDF 出现不同词、不同数字、不同单位、不同基因/蛋白/方法名
    - 标题层级冲突，可能改变 LID tree
    - PDF 有整句/整段而 MD 缺失
    - MD 有整句/整段而 PDF 找不到
    - 表格、公式、图注结构无法可靠线性化
    - 多处候选匹配同一文本，无法唯一定位

  建议门禁：

  interface SourceReconciliationSummary {
    verified_blocks: number;
    auto_repaired_blocks: number;
    needs_review_blocks: number;
    pdf_unmatched_blocks: number;
    md_unmatched_blocks: number;
    canonical_source_status: "available" | "degraded" | "failed";
  }

  needs_review_blocks == 0
    -> canonical_source_status = available

  needs_review_blocks > 0 but below explicit threshold and user passes --allow-degraded-source
    -> canonical_source_status = degraded
    -> 可以生成 source.txt，但 source_manifest 标 warning，PDF-first 不自动 available

  otherwise
    -> failed
    -> 不生成可信 LID/base

  理由：你说的 OCR 错误确实必须用 PDF 纠偏，但一旦自动修复越过“空白/编码/断词”这条线，它就可能把错误稳定写进 LID 和 citation。语义差异必须留给人工或显式修复
  步骤。

  你是否同意：自动修复只覆盖确定性版面/编码问题；语义、结构、内容增删冲突一律 needs_review，默认阻止生成可信 LID？


› 我认为，如果用户开启了llm review就可以把need review交给llm


• 这个会改变 Q46 的门禁：needs_review 不一定只能人工处理，可以进入一个可选 LLM review/repair 阶段。但我会先查本地已有 LLM repair / deterministic gate 设计和
  Zotero 的降级边界，再给修正版推荐。

---

## Grill Q46 - LLM review may repair format, not content

**Pre-check before asking/recommending**:
- Zotero document-worker checked: PDF text recovery uses char flags such as `spaceAfter`, `lineBreakAfter`, and `paragraphBreakAfter` to reconstruct whitespace. OCR-like line fragmentation is handled as layout/text recovery, not as semantic rewriting.
- Zotero document-worker checked: its `repair.onnx` path repairs block segmentation labels inside a thresholded model pipeline. It does not make generated/repaired text an independent truth source.
- Zotero document-worker checked: structure extraction records degraded/fallback reasons such as inference errors and overlarge line counts; uncertain structure is labeled as degraded/fallback.
- Local design checked: earlier Q18 locked LLM repair as alignment-only and non-mutating, but Q45 changed the source model so `source.txt` is now the reconciled canonical source. Q46 therefore needs a narrower revised rule.

**Question**: If the user explicitly enables `--llm-review`, may `needs_review` blocks be sent to an LLM review stage, while forbidding the LLM from changing content?

**Recommendation**: Yes. LLM review may propose formatting/structure normalization only; it must not modify semantic content. Accepted output still requires deterministic validation before entering reconciled `source.txt`.

```text
source reconciliation:
  deterministic_safe_repair
    -> verified / auto_repaired

  needs_review + --llm-review
    -> LLM proposes format-only patch
    -> deterministic content-equivalence check
    -> deterministic realignment against PDF chars/words
    -> if pass: llm_format_repaired enters source.txt
    -> else: remains needs_review / failed
```

```ts
type SourceBlockReconcileStatus =
  | "verified"
  | "auto_repaired"
  | "llm_format_repaired"
  | "needs_review"
  | "pdf_unmatched"
  | "md_unmatched";
```

Allowed LLM patch scope:

```text
format_only:
  - paragraph line wrapping / unwrapping
  - heading/list/table-like Markdown formatting when text tokens are unchanged
  - hyphenation or ligature normalization only when deterministic equivalence proves the same token sequence
  - local split/merge proposal for blocks, if the concatenated normalized text is unchanged
```

Forbidden LLM patch scope:

```text
content_change:
  - add/remove sentence, clause, number, unit, gene/protein/method/dataset name
  - replace a word with a different word
  - summarize, translate, paraphrase, or "fix" scientific meaning
  - choose page/bbox mapping
  - directly create LID spans or citation evidence
```

Acceptance gate:

```text
normalize(original_tokens) == normalize(candidate_tokens)
and deterministic alignment confidence >= threshold
and regenerated SourceBlock/LID spans are consistent
and source_reconciliation_report records original, candidate, model, and validation result
```

**User answer**: LLM is useful for finding the correct format, but must not modify content.

**Locked result**: `--llm-review` is allowed only as a format-repair assistant. It may propose Markdown/paragraph/block formatting repairs for `needs_review` blocks, but content changes are forbidden. A candidate can enter reconciled `source.txt` only after deterministic content-equivalence and alignment checks pass. If semantic content differs, the block remains `needs_review` or fails the build.

---

## Grill Q47 - content equivalence gate for LLM format repair

**Pre-check before asking/recommending**:
- Zotero document-worker checked: DOM/structure text processing uses NFC normalization, whitespace collapse, and delta maps so normalized processing positions remain traceable back to original positions.
- Zotero document-worker checked: PDF selection recovers text/ranges from char streams; processing may normalize, but recovered text/range remains grounded in source character positions.
- Local design checked: ADR-0024/ADR-0031 define `source.txt`, `Span`, and highlight ranges as UTF-16 coordinate systems. Earlier Grill Q8/Q19 already locked normalized-to-raw delta maps for alignment and selection.

**Question**: Since LLM review may only repair format and not content, should PH source reconciliation define a deterministic `content_equivalence` check as a hard gate before `llm_format_repaired` may enter `source.txt`?

**Recommendation**: Yes. The gate should compare normalized token streams, not raw strings.

```ts
interface ContentEquivalenceResult {
  equivalent: boolean;
  reason?:
    | "token_added"
    | "token_removed"
    | "token_replaced"
    | "number_changed"
    | "unit_changed"
    | "identifier_changed"
    | "math_changed";
  original_token_index?: number;
  candidate_token_index?: number;
}
```

Allowed equivalent changes:

```text
- whitespace / line wrapping
- Markdown heading/list/table marker changes
- Unicode NFC/NFKC normalization
- fi/fl ligature normalization
- deterministic hyphen unwrap when token sequence is unchanged
```

Forbidden content changes:

```text
- word token changed
- number changed
- unit changed
- gene/protein/method/dataset/metric identifier changed
- punctuation changes that alter math, DOI, URL, code, formula, or reference text
- sentence added/removed
```

Acceptance gate:

```text
content_equivalence(original, candidate) == true
and deterministic_pdf_alignment(candidate) >= threshold
  -> llm_format_repaired may enter source.txt

otherwise
  -> remains needs_review / failed
```

**User answer**: Agreed.

**Locked result**: `content_equivalence` is a hard deterministic gate for LLM format repair. It compares normalized token streams and treats numbers, units, scientific identifiers, formulas, DOI/URL/code, and content-bearing token changes as immutable. LLM candidates that fail equivalence cannot enter reconciled `source.txt`, even if they make the formatting look better.

---

## Grill Q48 - unresolved reconciliation blocks stop trusted book build

**Pre-check before asking/recommending**:
- Zotero document-worker checked: PDF structure extraction may record degraded/fallback structure results, but that degradation is explicit and does not turn uncertain source text into a trusted canonical source.
- Zotero document-worker checked: identity/freshness fields such as `sourceHash` are treated as real validity inputs, not UI hints.
- Local design checked: existing build flows treat pending work as a stop condition by default; `--allow-partial` is reserved for smoke/emergency. Earlier Q16 allows PDF capability degradation when PDF alignment fails, but Q45 made source reconciliation part of the semantic source foundation, so it needs a stricter gate.

**Question**: After deterministic repair and optional `--llm-review` format repair, if any `needs_review` / `pdf_unmatched` / `md_unmatched` remains, should the build refuse to write trusted `.understand-book/<book_id>/source.txt` and `base.json`?

**Recommendation**: Yes. Unresolved source reconciliation must block the canonical book build. The tool may write review artifacts under `.build/`, but not a trusted readable book base.

```text
paper.md + paper.pdf
  -> source_reconciliation
  -> if unresolved == 0:
       write .understand-book/<book_id>/source.txt
       write base.json
       continue pdf maps
     else:
       write .build/source-reconciliation/report.json
       write .build/source-reconciliation/review-draft.md
       exit non-zero
       do not write trusted source.txt/base.json
```

Diagnostic-only option:

```text
--emit-review-draft
  -> writes review-draft.md + report.json under .build/
  -> never writes .understand-book/<book_id>/base.json
```

**User answer**: Agreed, and we should add a page for human repair.

**Locked result**: Unresolved source reconciliation is a hard stop for trusted `source.txt` / `base.json` generation. The build may emit `.build` review artifacts, but they are diagnostic/editing inputs only. A human repair page is in scope as the mechanism for resolving review items before rerunning the trusted build.

---

## Grill Q49 - dedicated source reconciliation review page

**Pre-check before asking/recommending**:
- Zotero document-worker checked: its preview page puts PDF pages, rendered structure HTML, overlays, progress/status, and errors into a dedicated inspection surface. It is a review/preview tool, not the normal reader surface.
- Zotero document-worker checked: the useful transferable design is the product shape, not the implementation: PDF context, structured text, overlay, and status side by side.
- Local implementation checked: the current app already has separate modal/surface concepts for opening books, source preview, notes, and debug panels. Source reconciliation is build-time review, not a normal reader annotation workflow.

**Question**: Should the human repair UI be a dedicated "Source reconciliation review" page/mode for resolving `needs_review` items, rather than being mixed into the normal reader?

**Recommendation**: Yes. Make it an independent review surface that edits draft/review artifacts only.

```text
/review/source-reconciliation
  input:
    .build/source-reconciliation/report.json
    .build/source-reconciliation/review-draft.md
    original paper.pdf
    original paper.md

  output:
    reviewed-draft.md
    review-decisions.json
```

Suggested layout:

```text
Left: issue list
  - status: needs_review / pdf_unmatched / md_unmatched
  - reason: number_changed / structure_conflict / table_linearization ...
  - block id / approximate page

Center: PDF context
  - page render
  - highlighted candidate PDF region
  - nearby text

Right: Markdown draft editor
  - editable block text / formatting
  - token diff against PDF text
  - content_equivalence result
  - Apply format fix / Mark unresolved
```

Boundaries:

```text
- Page writes only reviewed-draft.md + review-decisions.json
- It must not directly write .understand-book/<book_id>/source.txt or base.json
- After save, user reruns the hybrid build
- The build reruns deterministic content_equivalence + alignment gates
- UI state never bypasses deterministic gates
```

**User answer**: Agreed.

**Locked result**: Source reconciliation gets a dedicated review page/mode. It is a build-review UI for editing review draft/decision artifacts, not a normal reader surface. Trusted `source.txt` and `base.json` are generated only by rerunning the build and passing deterministic gates.

---

## Grill Q50 - source review runs outside normal reader state

**Pre-check before asking/recommending**:
- Zotero document-worker checked: its preview page is an independent inspection surface that loads a PDF buffer, extracts structure, renders PDF pages, renders structured HTML, and paints overlays. It is not the normal Zotero reader.
- Local server checked: current `/book/*`, `/reader/*`, `/memory/*`, and `/agent/*` endpoints are centered on an opened trusted book with `base.json` and `source.txt`. Source reconciliation failure occurs before a trusted book base exists, so it cannot depend on normal reader state.

**Question**: Should the human repair page be served by an independent `source-review` build-review mode over `.build/source-reconciliation/*`, instead of being mounted on the current reader `/book/*` session state?

**Recommendation**: Yes. Use a separate review server/mode so the normal reader can keep assuming that an opened book is trusted and built.

```text
hybrid build fails reconciliation
  -> writes:
     .build/source-reconciliation/report.json
     .build/source-reconciliation/review-draft.md
     .build/source-reconciliation/context.json

user runs:
  source-review --work-dir .build/source-reconciliation

review app:
  GET  /review/source-reconciliation/state
  POST /review/source-reconciliation/save
  GET  /review/source-reconciliation/pdf/original
```

```ts
interface SourceReviewState {
  report: SourceReconciliationReport;
  draft_markdown: string;
  issues: SourceReviewIssue[];
  pdf_available: boolean;
}
```

Boundaries:

```text
- review server does not need Book::load
- it does not expose /reader/* /memory/* /agent/*
- it does not write .understand-book/<book_id>/*
- save writes only reviewed-draft.md / review-decisions.json
- rerunning the hybrid build is the only way to produce trusted source.txt/base.json
```

**User answer**: Agreed. Also, when `needs_review` appears, the user should get a choice: review personally, which opens the review page, or trust the LLM and hand the review to the LLM.

**Locked result**: Source review runs as an independent build-review mode over `.build/source-reconciliation/*`, not inside normal reader state. A follow-up decision is required for the user choice between manual review and LLM-assisted review.

---

## Grill Q51 - review choice is the UI/CLI entrypoint for existing Q46/Q47 rules

**Pre-check before asking/recommending**:
- Zotero document-worker checked: preview/review tooling offers diagnostic inspection, but no separate "trust model as truth" path. Its automated repair remains thresholded pipeline work.
- Local design checked: Q46 already allows `--llm-review` as a format-only assistant; Q47 already requires deterministic `content_equivalence` and alignment gates; Q49/Q50 already define the manual review page and independent source-review mode.

**Question**: Is the proposed "trust LLM" option just the already-agreed `--llm-review` flow, where the LLM batch-processes all `needs_review` items and any failed gate falls back to manual review?

**Recommendation**: Yes. Treat this as UX/CLI wiring, not a new semantic rule.

```text
needs_review found
  -> user chooses:

manual review
  -> open source-review page
  -> user edits reviewed-draft.md
  -> rerun build gates

llm review
  -> run Q46/Q47 LLM format-review flow for all issues
  -> content_equivalence + deterministic alignment gates
  -> all pass: continue trusted source.txt/base.json generation
  -> any fail: open source-review page with remaining issues
```

Boundaries:

```text
- "Trust LLM" means trust it to perform format-review work, not to judge correctness.
- LLM still cannot modify content.
- Formal artifacts are still generated only by deterministic build gates.
- Failed LLM review items become manual review issues.
```

**User answer**: Agreed.

**Locked result**: The user-facing choice is `manual review` versus `llm review`. `llm review` is exactly the Q46/Q47 format-only LLM review flow applied to all review items; failed items fall back to the Q49/Q50 manual source-review page. No new trust model is introduced.



 Pre-check：Zotero preview 用 npm --workspace preview run dev 启动，Vite 配置会打开 /preview/
，也支持 URL hash 自动加载文件和 drag/drop；这是“显式打开
  review/preview 工具”的模式。本项目现有构建 CLI 遇 pending 都是非交互式：打印 pending ids、给出下一步命令、process.exit(1)；不会在 batch 命令里阻塞询问或自
  动开 GUI。

  Grill Q52：当 hybrid build 遇到 needs_review 时，是否也应保持非交互式默认行为：打印两条明确命令让用户选，而不是在构建过程中阻塞式询问或自动打开页面？

  我的推荐：是。构建命令默认非交互、可脚本化；用户选择通过下一条显式命令表达。

  hybrid build detects needs_review
    -> writes .build/source-reconciliation/*
    -> exits 1
    -> prints:

  Review required: 12 issues

  Option A - manual review:
    npm run source-review -- --work-dir .build/source-reconciliation

  Option B - LLM format review:
    npm run source-review:llm -- --work-dir .build/source-reconciliation

  After either path:
    npm run build:paper-hybrid -- paper.md --pdf paper.pdf --reviewed-draft .build/source-reconciliation/reviewed-draft.md

  边界：

  - 不在 batch build 中弹交互问题
  - 不默认打开浏览器
  - `source-review --open` 可以显式打开页面
  - `source-review:llm` 可以显式全权跑 LLM format review
  - CI / agent / 脚本可稳定处理 exit code

  理由：这贴合当前工具链风格，也避免“自动打开 UI”在 headless/CI/agent 环境里变成副作用。Zotero preview 的 server.open 是 dev preview 工具行为；我们的正式
  build 应先保持 CLI 可控。

  你是否同意：needs_review 默认只输出 review artifacts + 下一步命令并退出；人工页面和 LLM review 都由用户显式运行？





› 但是这样是不是有点问题，就是我们进行pdf和md交叉验证生成source.txt其实是预构建中的一环，如果出现needs_review还得让用户换成我们的网页然后再自己打开人工页面
  吗，说到底我们能否codex，claude，opencode插件形态的预构建可视化到我们的网页端呢，可以看进度（预购建到哪部，窗口完成了几个等等），消耗的token数，codex等让
  用户的选择，need-review就弹出人工页面

---

## Grill Q53 - Codex / opencode feasibility for Build Workbench executors

**Pre-check before asking/recommending**:
- Zotero document-worker checked: `preview/index.html` is an independent preview surface with status text, file open/drop entrypoints, PDF/structured-text panels, and explicit `Loading -> Extracting structure -> Rendering -> done/error` state.
- Zotero document-worker checked: `src/index.js:createProgressReporter` and `pdf/structure/structure.js:reportPageProgress` treat progress as best-effort telemetry that must not affect extraction.
- Local build design checked: current build already has deterministic `.build/` resume/status artifacts for Pass1, profile sidecar, paper metadata, paper lexicon, Pass2, and BookStructure. Status is computed from disk, not from agent memory.
- Codex official manual checked: Codex supports `codex exec --json` JSONL event streams, `--output-schema`, resumable non-interactive runs, SDK control, and app-server JSON-RPC with streamed turn/item events.
- opencode official docs checked: opencode supports `opencode run --format json`, `opencode serve` HTTP server, SDK client, session status/messages, permission responses, SSE/global events, and token/cost stats. Current local PATH does not have `opencode` installed.

**Question**: Is it feasible to make Codex / opencode drive prebuild tasks from a Web Build Workbench, with the web app showing progress, token/cost, pending units, and `needs_review` choices?

**Recommendation**: Yes, but only if Codex/opencode are treated as executor adapters. The authoritative build state must remain our own `.build/jobs/<job_id>/state.json` and deterministic artifacts.

```ts
interface BuildExecutorAdapter {
  kind: "codex" | "opencode" | "claude" | "manual";
  start(task: BuildTask): Promise<ExecutorRunHandle>;
  events(run: ExecutorRunHandle): AsyncIterable<ExecutorEvent>;
  cancel(run: ExecutorRunHandle): Promise<void>;
}

interface ExecutorEvent {
  type: "started" | "progress" | "token_usage" | "message" | "tool" | "permission_needed" | "completed" | "failed";
  external_session_id?: string;
  usage?: { input_tokens?: number; output_tokens?: number; total_tokens?: number; cost_usd?: number };
  payload?: unknown;
}
```

Recommended implementation shape:

```text
Build Workbench
  -> creates BuildJob
  -> chooses executor adapter
  -> adapter runs one bounded task, e.g. paper-metadata window 7
  -> adapter event stream is normalized into ExecutorEvent
  -> write output candidate under .build/<stage>/<unit>.candidate.json
  -> deterministic write/gate script validates and commits artifact
  -> BuildJob status recomputed from disk
```

Boundaries:

```text
- Executor event stream is telemetry, not truth.
- A completed Codex/opencode run does not mean a build unit is done.
- Unit is done only after deterministic write/gate accepts the output artifact.
- Permission/user choice events are routed to Build Workbench.
- needs_review opens source-review inside Build Workbench or triggers LLM format review.
- CLI/headless remains the same job state machine without the web UI.
```

**Feasibility assessment**:
- Codex: high feasibility. Best paths are `codex exec --json` for simple bounded tasks, Codex SDK for server-side orchestration, or app-server JSON-RPC for deeper product integration.
- opencode: high feasibility in design, medium local readiness. Its docs expose `run --format json`, `serve`, HTTP APIs, SDK, sessions, permission response endpoints, events, and stats. It is not installed locally right now, so implementation would first need install/auth validation.
- Main risk is not API access. The main risk is job ownership: if we let agent session state become the build state, resume/debug/review will become fragile. Keep disk artifacts as truth and adapters as workers.

**Locked result**: Build Workbench can support Codex/opencode/Claude-style executors. The adapter layer normalizes their events into our `ExecutorEvent`, while `.build/jobs/<job_id>/state.json` plus deterministic stage artifacts remain authoritative. Codex is immediately viable from the official non-interactive/SDK/app-server surfaces; opencode is viable by documented API shape but needs local installation and a small spike before committing to it as a first-class adapter.

---

## Grill Q54 - Build Workbench runs as build mode, not normal reader state

**Pre-check before asking/recommending**:
- Zotero document-worker checked: preview is a separate `/preview/` tool surface. It has local-file/test-PDF middleware and status/progress UI, and it is not the normal reader state for a trusted library item.
- Zotero document-worker checked: preview can load a local PDF path via `/__api/local-pdf`, render pages, render structured text, and paint overlays without requiring a pre-existing trusted document database.
- Local server checked: current `crates/server` API is centered on an already loaded `Book`; `/book/*`, `/reader/*`, `/memory/*`, `/agent/*`, and `/profile/*` assume trusted `base.json` / `source.txt` exist.
- Local web checked: current `packages/web` app is a normal reader surface with book library, reader viewport, memory, agent chat, layout state, and paper projection clients. It has no build job state or source reconciliation review state.

**Question**: Should the Build Workbench run as a separate build-mode server/API namespace, rather than being mounted directly into the normal reader session?

**Recommendation**: Yes. Use a dedicated build mode with its own state and endpoints. It may reuse the same frontend app shell/components, but it must not depend on `Book::load` or normal `/reader/*` state.

```text
normal reader mode:
  requires trusted .understand-book/<book_id>/base.json
  exposes /book/* /reader/* /memory/* /agent/*

build workbench mode:
  works before trusted base exists
  exposes /build/* and /review/source-reconciliation/*
  reads/writes only source inputs and .build/jobs/* artifacts
```

Suggested API boundary:

```text
GET  /build/jobs
POST /build/jobs
GET  /build/jobs/:job_id
GET  /build/jobs/:job_id/events
POST /build/jobs/:job_id/start
POST /build/jobs/:job_id/cancel
POST /build/jobs/:job_id/decision

GET  /review/source-reconciliation/:job_id/state
POST /review/source-reconciliation/:job_id/save
GET  /review/source-reconciliation/:job_id/pdf
```

State ownership:

```text
.understand-book/<book_id>/.build/jobs/<job_id>/state.json
.understand-book/<book_id>/.build/jobs/<job_id>/events.jsonl
.understand-book/<book_id>/.build/source-reconciliation/*
.understand-book/<book_id>/.build/<stage>/<unit>.json
```

Boundaries:

```text
- Build Workbench does not expose memory/note/highlight APIs.
- Build Workbench does not treat page/bbox as citation truth.
- Build Workbench cannot write trusted source.txt/base.json except by invoking deterministic build close steps.
- Normal reader can link to a finished build, but cannot open an unfinished build as a trusted book.
- CLI/headless and web build mode operate over the same disk job state.
```

Reasoning: Source reconciliation and early PDF/Markdown verification happen before a trusted `Book` exists, so mounting this inside normal reader state would force the reader to handle half-built books and invalid citation anchors. A build-mode server keeps the invariant clean: reader reads trusted books; workbench produces trusted books.

**User answer**: Agreed. If no prebuild artifacts exist, Build Workbench must be a prerequisite flow and cannot be skipped.

**Locked result**: Build Workbench is a separate build-mode surface, not normal reader state. When a target source has no trusted prebuild artifacts, the product must route the user into Build Workbench first instead of allowing a normal reader session.

---

## Grill Q55 - missing trusted artifacts route to Build Workbench before reader

**Pre-check before asking/recommending**:
- Zotero document-worker checked: preview has explicit empty states (`PDF pages will appear here`, `Structured text will appear here`) and transitions through loading/extraction/rendering before showing a structured document. It does not pretend an unloaded/unprocessed PDF is already a readable structured item.
- Zotero document-worker checked: preview errors remain status/errors in the preview tool, not silent fallback into a normal library reader.
- Local server checked: `crates/server` startup and `/book/open` both use `Book::load`; current library listing only includes `.understand-book/<id>` directories with `base.json`.
- Local build checked: `pass1-batch.ts` is the step that writes trusted `base.json` and `source.txt`; later sidecar/batch steps depend on those files and fail fast when missing.
- Local build skill checked: `.build/` is build-only and `Book::load` does not read it; pending defaults to refusing close, with `--allow-partial` only as an explicit emergency/smoke exception.

**Question**: Should the product define a hard readiness gate: if a user opens `paper.md + paper.pdf` or any source path without trusted `base.json/source.txt`, the app must route to Build Workbench and cannot open the normal reader?

**Recommendation**: Yes. The normal reader should only open trusted prebuild outputs. Source files or incomplete `.build/` directories enter Build Workbench.

```ts
type OpenTargetReadiness =
  | { kind: "trusted_book"; book_dir: string; book_id: string }
  | { kind: "build_required"; source_path: string; pdf_path?: string; suggested_book_id: string }
  | { kind: "build_in_progress"; job_id: string; state_path: string }
  | { kind: "build_review_required"; job_id: string; review_kind: "source_reconciliation" }
  | { kind: "invalid"; reason: string };
```

Routing:

```text
Open target
  if directory has valid base.json + source.txt:
    open normal reader
  else if matching .build/jobs/* has active/incomplete job:
    open Build Workbench at that job
  else if source path exists:
    create BuildJob and open Build Workbench
  else:
    show invalid target error
```

UI behavior:

```text
Book library:
  - Built tab: trusted books only
  - Builds tab: in-progress / review-required jobs
  - New build: choose md/epub/pdf inputs

Normal reader:
  - never creates ad-hoc LIDs from raw source
  - never opens .build-only state as a book
```

Boundary:

```text
- No "quick preview as reader" fallback.
- Raw PDF/Markdown preview is allowed only inside Build Workbench/review mode.
- `--allow-partial` cannot produce a normal reader book unless the trusted minimum artifacts pass schema/gates.
```

Reasoning: This preserves the central invariant: reader actions, memory, citations, highlights, and agent answers are LID-based and require a trusted `source.txt/base.json`. If those do not exist, the correct product state is "build required", not "reader with degraded truth".

**User answer**: Agreed.

**Locked result**: Opening a target must first run readiness detection. Only `trusted_book` enters the normal reader. Missing, incomplete, or review-required prebuild artifacts route to Build Workbench or an explicit invalid-target error.

---

## Grill Q56 - BuildJob uses an explicit stage DAG, not a free-form task queue

**Pre-check before asking/recommending**:
- Zotero document-worker checked: preview exposes coarse deterministic phases (`Loading`, `Extracting structure`, `Rendering`, done/error), and its progress callback is best-effort telemetry, not the source of extraction truth.
- Local build skill checked: current build is already a staged pipeline with explicit dependencies: segmentation/windowing -> Pass1 -> profile sidecar / paper metadata / paper lexicon -> Pass2 -> BookStructure -> final readable projections.
- Local build skill checked: each LLM-backed stage follows `status -> input -> executor/subagent -> write -> batch/close`, with disk artifacts and content hashes as resume truth.
- Local PDF-first design checked: PH source reconciliation and hybrid foundation must precede trusted `source.txt/base.json`; `needs_review` can pause the job before normal build stages.

**Question**: Should `BuildJob` store an explicit stage DAG with dependencies and per-stage readiness, rather than a free-form list of agent tasks?

**Recommendation**: Yes. A stage DAG is necessary for resumability, UI clarity, and safe parallelism.

```ts
type BuildStageId =
  | "source_reconciliation"
  | "hybrid_foundation"
  | "pass1"
  | "paper_metadata"
  | "paper_lexicon"
  | "profile_sidecar"
  | "pass2"
  | "book_structure"
  | "finalize";

interface BuildStageState {
  id: BuildStageId;
  status: "blocked" | "ready" | "running" | "needs_user" | "failed" | "done" | "skipped";
  depends_on: BuildStageId[];
  units_total: number;
  units_done: number;
  units_pending: string[];
  active_unit?: string;
  artifact_paths: string[];
  blocker?: string;
}

interface BuildJobState {
  job_id: string;
  book_id: string;
  source: { markdown_path?: string; epub_path?: string; pdf_path?: string };
  stages: Record<BuildStageId, BuildStageState>;
  current_stage?: BuildStageId;
  status: "ready" | "running" | "needs_user" | "failed" | "done";
}
```

Recommended dependency shape for paper PDF-first:

```text
source_reconciliation
  -> hybrid_foundation
      -> pass1
          -> paper_metadata
          -> paper_lexicon
          -> profile_sidecar
              -> pass2
                  -> book_structure
                      -> finalize
```

Parallelism rule:

```text
After pass1 closes:
  paper_metadata, paper_lexicon, profile_sidecar may run independently.

Pass2 waits for:
  pass1 + profile_sidecar

BookStructure waits for:
  profile_sidecar + pass2

Finalize waits for:
  minimum trusted reader set, profile-specific required projections
```

State truth:

```text
Stage status is recomputed from:
  - deterministic status scripts
  - expected artifact files
  - content hashes / freshness hashes
  - review state

Executor events may update active_unit and telemetry,
but cannot mark a stage done without deterministic artifact validation.
```

UI consequence:

```text
Build Workbench shows:
  - stage timeline/DAG
  - per-stage done/pending counts
  - current executor and token usage
  - blocked reason and required user action
  - resume button for each ready stage
```

Reasoning: This preserves the existing build discipline while making it visible. A free-form queue would make it too easy for Codex/opencode/Claude adapters to run tasks out of order or mark progress that deterministic gates have not accepted.

**User answer**: Agreed.

**Locked result**: `BuildJob` uses an explicit stage DAG as its authoritative state shape. Executor events from Codex/opencode/Claude/manual adapters are telemetry only; a stage can be marked `done` only after deterministic artifact validation and status recomputation accept the expected files.

---

## Grill Q57 - state.json is a UI/runtime snapshot; `.build` artifacts remain truth

**Pre-check before asking/recommending**:
- Zotero document-worker checked: progress callbacks are monotonic best-effort telemetry and explicitly must not affect extraction.
- Local build scripts checked: `build-status.ts`, `paper-metadata-status.ts`, and `profile-sidecar-status.ts` recompute `done/pending` from artifact existence plus `content_hash`.
- ADR-0042 checked: existing build-resume principle is "no durable status bits as truth"; resume is determined by artifact existence and hash validation.

**Question**: With Build Workbench introducing `state.json`, should stage status still be recomputed from artifacts/hash/status scripts rather than treating `state.json.stages[*].status` as authoritative?

**Recommendation**: Yes. `state.json` may cache UI view and active run information, but it must not be the completion truth.

```ts
interface BuildJobState {
  job_id: string;
  cached_view: {
    stages: Record<BuildStageId, BuildStageView>;
    updated_at: string;
  };
  active_run?: {
    stage: BuildStageId;
    unit_id?: string;
    executor: "codex" | "opencode" | "claude" | "manual";
    external_session_id?: string;
  };
}

interface StageStatusComputer {
  stage: BuildStageId;
  compute(input: BuildInput, artifactRoot: string): StageComputedStatus;
}
```

Rules:

```text
GET /build/jobs/:id
  -> read state.json for active_run/telemetry
  -> run stage status computers
  -> merge into UI view
  -> return computed status

stage done =
  expected artifacts exist
  + content/freshness hashes match
  + schema/gate passes

state.json status alone never marks done
```

**User answer**: Agreed. Judgement truth remains under the build artifact directory.

**Locked result**: Build Workbench may persist `state.json` for UI/runtime state, but stage readiness/done/pending is authoritative only when recomputed from build artifacts under the build directory plus deterministic hashes and gates.

---

## Grill Q58 - Build Workbench artifact root stays under each book workspace

**Pre-check before asking/recommending**:
- Zotero document-worker checked: preview can load a local PDF path via `/__api/local-pdf`, but processed structure carries `sourceHash`; the file path is an input handle, not the ownership root for trusted artifacts.
- Local project checked: existing convention is `.understand-book/<bookId>/.build/`, not repo-root `./build`.
- `CONTEXT.md` and ADR-0042 checked: build workspace is `.understand-book/<bookId>/.build/`, build-only, and `Book::load` never reads it.

**Question**: Should Build Workbench keep the artifact root fixed at each book's `.understand-book/<bookId>/.build/`, rather than using repo-root `./build` or a `build/` directory beside the source file?

**Recommendation**: Yes. Treat the user's shorthand "`./build`" as "the `.build/` directory inside the book workspace."

```text
.understand-book/<bookId>/
  source.txt
  base.json
  paper_metadata.json
  ...
  .build/
    jobs/<job_id>/state.json
    jobs/<job_id>/events.jsonl
    source-reconciliation/
    hybrid-foundation/
    pass1/
    paper-metadata/
    paper-lexicon/
    profile-sidecar/
    pass2/
    book-structure/
```

Reasons:
- Existing status/write/batch scripts already work against `.understand-book/<bookId>/.build/<stage>`.
- Trusted artifacts and build intermediates stay in one book workspace, making open/resume/cleanup/migration simpler.
- Repo-root `./build` would mix books, inputs, and jobs.
- Source-adjacent `build/` would pollute the user's paper directory and break the current `.understand-book` isolation convention.

**User answer**: Agreed.

**Locked result**: Build Workbench's authoritative build artifact root is `.understand-book/<bookId>/.build/`. Any references to "`./build`" in discussion mean this book-local `.build/` directory unless explicitly stated otherwise.

---

## Grill Q59 - job records are orchestration logs; accepted artifacts stay in stage directories

**Pre-check before asking/recommending**:
- Zotero document-worker checked: processed output carries `sourceHash`; progress/session handling is separate from the extracted structure.
- Local build scripts checked: current stage truth lives directly under `.understand-book/<bookId>/.build/<stage>/`, for example `pass1/`, `paper-metadata/`, `profile-sidecar/`, `pass2/`, and `book-structure/`.
- ADR-0042 checked: per-window artifact path is the resume truth; no job/session state is needed to decide done/pending.

**Question**: Should `.build/jobs/<job_id>` store only orchestration logs, telemetry, UI state, and candidates, while accepted stage truth artifacts continue to live under `.build/<stage>/...` rather than `.build/jobs/<job_id>/<stage>/...`?

**Recommendation**: Yes. `jobs/<job_id>` records how a run happened; `.build/<stage>` records the book's currently accepted, verifiable intermediate artifacts.

```text
.understand-book/<bookId>/.build/
  jobs/
    <job_id>/
      state.json
      events.jsonl
      executor/
        codex-*.jsonl
        opencode-*.jsonl

  pass1/
    0.json
    1.json

  paper-metadata/
    0.json

  profile-sidecar/
    0.json

  pass2/
    3.json

  book-structure/
    units/<lid>.json
    stitch.json
```

Rules:

```text
executor completes unit
  -> write candidate under jobs/<job_id>/executor or temp
  -> deterministic write command validates
  -> accepted artifact is atomically written to .build/<stage>/<unit>.json
  -> status recomputation sees it as done
```

Reasons:
- Existing status/write/batch scripts remain compatible.
- Multiple jobs for the same book can continue the same stage artifact set.
- Canceled or failed jobs do not strand truth artifacts in private job directories.
- Events, cost, tokens, permission history, and raw executor logs stay under the job directory and do not pollute verifiable stage artifacts.

**User answer**: Agreed.

**Locked result**: `.build/jobs/<job_id>` is for orchestration records, telemetry, logs, and candidates. Accepted stage artifacts continue to be written to `.build/<stage>/...` and are the only inputs to deterministic stage status recomputation.

---

## Grill Q60 - BuildJob tracks input identity and becomes stale when inputs change

**Pre-check before asking/recommending**:
- Zotero document-worker checked: every structured extraction requires `sourceHash`; processed output records source identity instead of trusting UI file path.
- Local design checked: Q20 already locked freshness hashes for `source_manifest`, `pdf_source_map`, and `pdf_selection_map`.
- Local build checked: current stage artifacts use per-unit `content_hash`; source changes make status recomputation mark stale/pending.
- `source_manifest.v2` checked: it covers finished trusted book capability, but BuildJob starts earlier, before trusted `source_manifest.json` may exist.

**Question**: Should BuildJob record input fingerprints and enter `stale_input` when source files change, rather than continuing to drive the old active run?

**Recommendation**: Yes. BuildJob is the Build Workbench orchestration record, not the build truth. It answers which inputs this run was created for, which executor is active, which stage/unit is running, what events/tokens/cost were observed, and which user decisions are pending.

```ts
interface BuildInputFingerprint {
  markdown?: {
    path: string;
    sha256: string;
    size_bytes: number;
    mtime_ms?: number;
  };
  pdf?: {
    path: string;
    sha256: string;
    size_bytes: number;
    mtime_ms?: number;
    pdf_fingerprint?: string;
  };
  reviewed_draft?: {
    path: string;
    sha256: string;
  };
}

interface BuildJobState {
  job_id: string;
  book_id: string;
  input: BuildInputFingerprint;
  status: "ready" | "running" | "needs_user" | "failed" | "done" | "stale_input";
}
```

Layering:

```text
.build/<stage>/...
  = truth layer
  = accepted intermediate artifacts
  = status scripts recompute done/pending here

.build/jobs/<job_id>/...
  = orchestration layer
  = input identity, active executor run, event log, token/cost telemetry, user decisions
```

Rules:

```text
GET /build/jobs/:id
  -> recompute current input fingerprint from paths
  -> if differs from state.input:
       status = stale_input
       active_run must not continue
       stage status may still be shown as artifact recomputation result
       user must choose: create new job or explicitly restart/rebase
```

Boundaries:
- Input fingerprint is job freshness, not stage completion truth.
- Accepted artifacts are still judged by `.build/<stage>` hashes and gates.
- Stale job does not delete artifacts.
- A new job may reuse still-valid `.build/<stage>` artifacts if deterministic status says done.

**User answer**: Agreed.

**Locked result**: BuildJob records source input fingerprints and enters `stale_input` when the current source files no longer match the job input. This protects the Workbench orchestration UI from continuing an executor run against changed inputs, while artifact reuse remains governed only by deterministic stage status recomputation.

---

## Grill Q61 - default BuildJob creation reuses the active job for the same input fingerprint

**Pre-check before asking/recommending**:
- Zotero preview checked: URL hash stores the currently loaded file path and scroll state for convenience, but extraction identity is still carried separately as `sourceHash`.
- Local `deriveBookId` checked: `bookId` is deterministic from source path or `--book-id`; this is workspace identity, not run identity.
- Q60 checked: BuildJob records input fingerprint and goes `stale_input` when input changes.
- Q59 checked: accepted artifacts are not job-private.

**Question**: When creating a BuildJob, should Build Workbench default to reusing an incomplete job for the same input fingerprint, rather than creating a new job on every Build click? Should explicit "new run/restart" remain available?

**Recommendation**: Yes. The default should reopen/resume the current active job for the same source inputs, while an explicit new-run action creates a new job record.

```text
POST /build/jobs { book_id, input_paths }
  -> compute input fingerprint
  -> find latest job where:
       job.book_id == book_id
       job.input_fingerprint == current_fingerprint
       job.status in ready | running | needs_user | failed
  -> if found:
       return existing job
     else:
       create new job
```

Explicit new-run path:

```text
POST /build/jobs { ..., mode: "new_run" }
  -> always create a new job_id
  -> still shares .build/<stage> accepted artifacts
```

Status semantics:

```text
same input + incomplete job:
  reopen/resume existing job

same input + done job:
  show finished job; "rebuild" creates new job only if user asks

changed input:
  old job stale_input
  create/reopen job for new fingerprint
```

Reasons:
- `bookId` identifies the workspace.
- `input fingerprint` identifies the source file set.
- `job_id` identifies one visible/cancelable/resumable orchestration record.
- Default reuse avoids duplicate jobs after refresh/reopen.
- Explicit new run preserves debugging and re-execution.
- `.build/<stage>` truth remains shared by all jobs for the book.

**User answer**: Agreed.

**Locked result**: Build Workbench defaults to reusing the latest incomplete job for the same `book_id + input_fingerprint`. A new `job_id` is created only when no reusable job exists or when the user explicitly starts a new run/rebuild.

---

## Grill Q62 - user decisions are recorded in job events and, when build-affecting, stage decision artifacts

**Pre-check before asking/recommending**:
- Zotero preview checked: interactive UI state such as active block, scroll, hover, and click is ephemeral; durable output is not derived from UI-only state.
- Existing source-review design checked: Q49/Q50 already define `reviewed-draft.md` and `review-decisions.json` as build-review artifacts.
- Build Workbench design checked: Q53/Q54 introduced `/build/jobs/:job_id/decision`, `permission_needed`, and `needs_review` choices, but had not fixed where decisions persist.
- Local `.build` checked: current accepted stage artifacts are direct stage files; there is no job decision artifact yet.

**Question**: Should Build Workbench write user decisions in two places: job event log for audit/timeline, and a stage decision artifact when the decision changes build readiness or next steps?

**Recommendation**: Yes. If a build-affecting decision exists only in `jobs/<job_id>/events.jsonl`, a restarted/new job would not be able to recompute stage state from build artifacts.

Example:

```text
User chooses "LLM review"
  -> append .build/jobs/<job_id>/events.jsonl
       { type: "decision", decision_kind: "source_review_mode", value: "llm" }

  -> write/update .build/source-reconciliation/review-decisions.json
       { mode: "llm_review", decided_at, decided_by: "user" }
```

Rules:

```text
job events:
  - audit trail
  - UI timeline
  - executor permission response history
  - not used as stage truth

stage decision artifacts:
  - consumed by deterministic status computers
  - survive new/restarted jobs
  - scoped to the stage they affect
```

Suggested layout:

```text
.build/
  jobs/<job_id>/events.jsonl

  source-reconciliation/
    report.json
    reviewed-draft.md
    review-decisions.json

  executor-decisions/
    permissions.jsonl       # optional, only if a permission changes future run policy
```

Boundaries:
- `Cancel run` writes only a job event, not a stage decision.
- `Open manual review` may write only a job event; `Save reviewed draft` writes a stage artifact.
- `Choose llm review` writes a stage decision because it changes `source_reconciliation` readiness.
- Executor permission, if it only allows one command, remains a job event; if it changes future policy, it must be written as a policy artifact.

**User answer**: Agreed.

**Locked result**: User decisions are dual-recorded when they affect build flow: job event log for audit/UI timeline, and stage-scoped decision artifacts for deterministic status recomputation. Non-build-affecting UI actions remain job events only.

 Grill Q63

  Pre-check:

  - Zotero preview checked: failures/cancel-like outcomes in the preview page only update status/error UI; they do not write durable structure truth.
  - Q62 checked: build-affecting decisions must write stage artifacts; non-build-affecting UI actions remain job events.
  - Q53 checked: executor adapters may emit permission_needed, failed, completed, token_usage; events are telemetry, not truth.
  - Local build checked: deterministic write commands are the only path that turns candidates into .build/<stage> accepted artifacts.

  Question: executor permission / cancel / retry 这类运行控制事件，是否默认只属于 jobs/<job_id>/events.jsonl，不写 stage decision artifact，除非它改变了未来
  构建策略？

  我的推荐：是。把它们默认定义为 run-control events。

  type JobEvent =
    | { type: "permission_needed"; run_id: string; request: PermissionRequest }
    | { type: "permission_response"; run_id: string; response: "allow_once" | "deny_once" }
    | { type: "cancel_requested"; run_id: string; by: "user" }
    | { type: "run_failed"; run_id: string; error: string }
    | { type: "retry_requested"; stage: BuildStageId; unit_id?: string };

  不写 stage artifact 的事件：

  - allow this one command
  - deny this one command
  - cancel current executor run
  - retry current unit
  - open details/logs

  - remember this executor policy for future units
  - choose manual vs llm review
  - save reviewed draft
      permission remember_for_stage
    -> job event
    -> .build/executor-decisions/policy.json or stage-specific decision artifact
    
    -> append job event
    -> state.active_run cleared
    -> no stage artifact changed

  retry
› 同意，还有对于codex的工具用户做选择也要加入啊

  openai-docs skill，后续若需要核对 Codex 官方能力，只看官方来源。

• Ran Get-Content -Raw C:\Users\Lenovo\.codex\skills\.system\openai-docs\SKILL.md
  └ ---
    name: "openai-docs"
    … +168 lines (ctrl + t to view transcript)
    - When falling back to web search, restrict to official OpenAI domains (developers.openai.com, platform.openai.com) and cite sources.

• 我还需要把 Codex 这一类“工具调用时用户做选择”的来源说清楚：它在本设计里属于 executor permission response。为避免只凭记忆，我会先用 openai-docs 的 Codex
  manual helper 拉取/核对官方 Codex 手册，再把 Q63 和下一问落稳。

官方 Codex manual 已可用且是当前缓存。我会只读 approvals/permissions 相关段落，用它来限定 “Codex 工具用户选择” 的落盘语义。

• Running (Get-Content C:\Users\Lenovo\AppData\Local\Temp\openai-docs-cache\codex-manual.md -Encoding UTF8)[1600..1850]

---

## Grill Q63 - run-control events include Codex tool approval choices

**Pre-check before locking result**:
- Zotero document-worker checked: progress callbacks are monotonic best-effort telemetry; progress errors are swallowed and must not affect extraction. Worker action errors are returned as response errors, not written as durable structure truth. Inference failures can create local fallback output, but preview/status UI state is not authoritative.
- Codex manual checked: Codex separates sandbox capability from approval policy. Approval prompts can cover sandbox escalation, network, external writes, side-effecting app/MCP calls, and granular categories such as sandbox approvals, execpolicy-rule prompts, MCP prompts, `request_permissions` prompts, and skill-script approvals.
- Local Build Workbench decisions checked: Q62 says build-affecting user decisions need stage decision artifacts; Q53/Q54 say executor events are telemetry unless deterministic gates accept artifacts.

**Question**: Should executor permission / cancel / retry / Codex tool-choice events default to `jobs/<job_id>/events.jsonl`, and write stage decision artifacts only when the user choice changes future build policy?

**Recommendation**: Yes. Treat one-off Codex approval choices as run-control events; only remembered policy choices become build-affecting decision artifacts.

```ts
type JobEvent =
  | { type: "permission_needed"; run_id: string; request: PermissionRequest }
  | { type: "permission_response"; run_id: string; response: "allow_once" | "deny_once" | "allow_for_stage" | "deny_for_stage" | "remember_policy" }
  | { type: "codex_tool_choice"; run_id: string; tool_call_id: string; choice: "allow_once" | "deny_once" | "allow_rule" | "deny_rule" }
  | { type: "cancel_requested"; run_id: string; by: "user" }
  | { type: "run_failed"; run_id: string; error: string }
  | { type: "retry_requested"; stage: BuildStageId; unit_id?: string };
```

Rules:

```text
allow_once / deny_once / cancel / retry
  -> append job event only
  -> no accepted stage artifact changes

remember_policy / allow_for_stage / deny_for_stage
  -> append job event
  -> write policy artifact, e.g. .build/executor-decisions/policy.json
  -> status computers may consume policy only if it affects future readiness

Codex tool user choices
  -> represented as permission_response or codex_tool_choice events
  -> remain run-local unless the user explicitly asks to persist a rule/policy
```

**User answer**: Agreed, with the additional requirement that Codex tool user choices must be included.

**Locked result**: Run-control events include Codex tool approval choices. One-off choices remain job events under `.build/jobs/<job_id>/events.jsonl`; persisted or stage-scoped permission policy is dual-recorded as a job event plus a policy/decision artifact that deterministic status recomputation can consume.

---

## Grill Q64 - permission schema is adapter-neutral, executor-native payload is nested

**Pre-check before asking/recommending**:
- Zotero document-worker checked: it has progress/status/error/fallback patterns; progress is best-effort and cannot affect extraction truth. It has no durable permission/user-approval model to reuse.
- Codex manual checked: Codex separates sandbox capability from approval policy. Approval prompts may cover sandbox escalation, network, external writes, side-effecting app/MCP tools, skill scripts, and related granular categories.
- Local Build Workbench decisions checked: Q53/Q56/Q57/Q62/Q63 lock executor events as telemetry unless deterministic stage artifacts and gates accept output.

**Question**: Should Build Workbench define its own adapter-neutral `PermissionRequest` / `PermissionPolicy` schema, rather than copying Codex approval policy/config shapes into the public Workbench contract?

**Recommendation**: Yes. Codex approval/tool-choice details should be recorded, but as `executor="codex"` native payload inside a neutral envelope.

```ts
interface PermissionRequest {
  request_id: string;
  run_id: string;
  executor: "codex" | "opencode" | "claude" | "manual";
  category:
    | "sandbox_escalation"
    | "network"
    | "filesystem"
    | "mcp_tool"
    | "skill_script"
    | "shell_command"
    | "destructive_action"
    | "other";
  action_summary: string;
  risk_level?: "low" | "medium" | "high" | "critical";
  scope_hint: "once" | "stage" | "job" | "profile";
  native?: unknown;
}

interface PermissionPolicy {
  policy_id: string;
  executor?: "codex" | "opencode" | "claude";
  category: PermissionRequest["category"];
  scope: "stage" | "job" | "profile";
  decision: "allow" | "deny";
  matcher: {
    command_prefix?: string[];
    tool_name?: string;
    mcp_server?: string;
    filesystem_root?: string;
    network_domain?: string;
  };
}
```

Reasoning:
- Build Workbench must support Codex/opencode/Claude/manual executors.
- Codex tool choices are important audit events, but Codex config should not become the Workbench public model.
- A neutral envelope supports unified UI, audit, and replay; `native` preserves executor-specific details for debugging and adapter replay.

**User answer**: Agreed.

**Locked result**: Build Workbench's public permission model is adapter-neutral. Codex approval/tool-choice details are saved under executor-native payloads, while Workbench UI, event log, policy persistence, and status recomputation consume the neutral `PermissionRequest` / `PermissionPolicy` layer.

---

## Grill Q65 - correction: Workbench "approval" is primarily build-direction choice, not repeated file-operation permission

**Pre-check before asking/recommending**:
- Zotero document-worker checked: it exposes preview file selection, source hash, processing options such as native ONNX, status/progress, and fallback/error reporting. It does not provide a durable user-decision model for build orchestration.
- Local Build Workbench decisions checked: Q62 already distinguishes build-affecting decisions from run-control events. Q63/Q64 discussed executor permission/tool choices, but had drifted toward operational approval semantics.
- User corrected the framing: the target approval surface is for users to choose prebuild/build direction, not mainly to avoid repeated file-operation approval prompts.

**Question asked**: Should persisted `PermissionPolicy` be consumed by a Workbench permission broker to auto-answer matching executor permission requests, without changing executor global config or sandbox?

**Recommendation given**: Yes, but this recommendation was misframed for the user's intended approval surface.

**User answer**: Disagree with the premise. The approval system is mainly for users to make prebuild direction choices, not for repeated file-operation approval.

**Locked correction**: Do not treat Build Workbench "approval" as primarily executor permission reuse. The primary product object is a build-direction decision that can affect stage readiness, source reconciliation, repair strategy, executor route, and next build steps. Executor permission remains a secondary run-control concern.

---

## Grill Q66 - BuildDecisionRequest is separate from ExecutorPermissionRequest

**Pre-check before asking/recommending**:
- Zotero document-worker checked: it exposes processing inputs/options and preview status, but has no durable build-decision model.
- Local Q62 checked: build-affecting decisions must be dual-recorded as job event plus stage artifact.
- Q65 correction checked: Workbench approval is primarily about prebuild/build direction choices, not repeated file-operation permission prompts.

**Question**: Should Build Workbench model its main user intervention as `BuildDecisionRequest`, separate from executor `PermissionRequest`?

**Recommendation**: Yes. `PermissionRequest` stays for Codex/opencode/Claude runtime/tool permissions. User choices that decide prebuild direction should be `BuildDecisionRequest` and should normally write stage decision artifacts.

```ts
type WorkbenchUserIntervention =
  | BuildDecisionRequest
  | ExecutorPermissionRequest;

interface BuildDecisionRequest {
  decision_id: string;
  job_id: string;
  stage: BuildStageId;
  kind:
    | "source_reconciliation_mode"
    | "hybrid_source_strategy"
    | "alignment_repair_strategy"
    | "executor_selection"
    | "review_acceptance"
    | "artifact_conflict_resolution"
    | "continue_or_restart";
  prompt: string;
  options: Array<{
    id: string;
    label: string;
    consequence: string;
    recommended?: boolean;
  }>;
  default_option_id?: string;
  blocks_stage_until_answered: boolean;
}
```

Examples:

```text
source_reconciliation:
  - use cleaned Markdown as semantic source
  - ask LLM to draft repair candidates
  - stop for manual review

alignment_repair:
  - accept deterministic low-confidence fallback
  - run optional LLM text repair
  - mark LIDs unmapped and continue

executor_selection:
  - Codex
  - opencode
  - manual paste
```

Persistence:

```text
User answers BuildDecisionRequest
  -> append jobs/<job_id>/events.jsonl
  -> write .build/<stage>/decision.json or review-decisions.json
  -> status computer consumes it
  -> stage becomes ready / blocked / needs_user
```

**User answer**: Agreed.

**Locked result**: Workbench main approval surface is `BuildDecisionRequest`, not executor permission. Build-direction choices and executor/tool permissions are separate models; build-direction answers default to stage-scoped decision artifacts plus job events.

---

## Grill Q68 - natural-language prebuild creates a confirmable sidecar plan before sidecar generation

**Pre-check before asking/recommending**:
- Zotero document-worker checked: it extracts a structured document and visualizes it in a split preview, with PDF overlays linked to structured HTML blocks. It does not have natural-language sidecar generation, but it reinforces that generated structure must be inspectable visually, not only emitted as JSON.
- Local paper rule pack checked: existing sidecars/projections include `paper_metadata.json`, `paper_lexicon.json`, `discourse_index.json`, `book_structure.json`, and `PaperReadingGuide` projection. These are schema/gate-backed artifacts, not free-form agent notes.
- Local Build Workbench direction checked: Q66 separates build-direction decisions from executor permissions. User natural-language build intent belongs to build-direction planning, not runtime permission approval.

**Question**: For natural-language driven prebuild, should the system first generate a confirmable `sidecar_plan.json` / form draft, then only after user confirmation build the concrete sidecar?

**Recommendation**: Yes. The form is not just a prompt for the agent; it is the structured contract between user intent, deterministic planner, executor, validator, and visualization.

```text
user natural-language need
  -> agent drafts SidecarBuildIntent / sidecar_plan.json
  -> user confirms or edits form
  -> deterministic planner compiles SidecarBuildSpec
  -> executor extracts candidate records
  -> schema/gate validates evidence
  -> accepted sidecar is written
  -> Web visualizes sidecar
```

```ts
interface SidecarBuildIntent {
  user_request: string;
  target_view: "timeline" | "concept_map" | "comparison_table" | "argument_map" | "custom";
  source_scope: {
    lids?: string[];
    sections?: string[];
    whole_book?: boolean;
  };
  output_contract: {
    sidecar_id: string;
    schema: JsonSchema;
    required_evidence: "lid_required";
    visualization: "table" | "graph" | "cards" | "timeline";
  };
}

interface SidecarBuildSpec {
  sidecar_id: string;
  stage: "custom_sidecar";
  input_lids: string[];
  extractor_prompt: string;
  output_schema: JsonSchema;
  validation_rules: string[];
  visualization_hint: string;
}
```

Rules:

```text
agent may:
  - parse user need
  - propose sidecar schema
  - propose visualization shape
  - extract candidate records

agent must not:
  - bypass user confirmation for a new sidecar contract
  - write final trusted sidecar without schema/gate
  - create facts without LID evidence
```

Default sidecar options are required: the user should not start from a blank schema unless they choose `custom`.

**User answer**: Agreed, with the additional requirement that sidecars need default options.

**Locked result**: Natural-language sidecar prebuild is a two-step flow: generate a confirmable sidecar plan/form first, then build the sidecar only after user confirmation. The Workbench must provide default sidecar options, with `custom` as an escape hatch rather than the starting point.

---

## Grill Q69 - ReasoningPath 的公共积木与私人运行时实例

**决策**: 采用 A：持久化可复用认知步骤、Reasoning Pattern 与带证据的局部路径片段，运行时按目标和 LearnerState 组装私人 `PathInstance`。

**否决**:
- 完整预构建图：会过早假设唯一正确理解路径，且全量构建成本过高。
- 完全运行时生成：难复用、难评估，无法沉淀可检查的路径资产。

**命门**: `PathInstance` 及其选择过程进入 `InteractionTrace`，不得自动反写公共 ReasoningPath；ReasoningPath 表达认知推进，不替代现有 LID `route`。
**何时回头**: 实测证明公共路径片段复用率极低，或单目标存在稳定且唯一的完整教学路径。

---

## Grill Q70 - 情境路径进度与长期知识状态分层

**决策**: 采用 A：`PathProgress` 保存当前 `PathInstance` 的情境化短期进度，`LearnerKnowledgeState` 保存可跨问题和资源复用的长期动态判断。

**否决**:
- 统一覆盖所有节点：会把提示下完成当前步骤误提升为全局掌握。
- 只保留长期状态：会丢失连续教学中“刚才已经想到哪一步”的精细上下文。

**命门**: 当前路径证据只直接更新 `PathProgress`；它可成为长期状态 Evidence，但不得自动等同于 Concept、Skill 或 Pattern 已掌握。
**何时回头**: 实测表明运行时可从短窗口 Trace 无损且低成本地重建全部路径进度。

---

## Grill Q71 - Learning Trace、Evidence 与 State 的本地持久化

**决策**: 采用本地 SQLite `learning.db`；Trace/Evidence 是可回放依据，`PathProgress` 与 `LearnerKnowledgeState` 是带 Estimator 版本和 Evidence watermark 的持久化物化投影。

**否决**:
- 全部继续写入 `memory.json`：高频追加、关联反查与版本化重算会突破单文档模型。
- 只保存 State：Estimator 升级后无法解释或重放旧判断。
- 引入图数据库：用户状态查询不需要新的图存储，公共图仍由版本化 artifact 拥有。

**命门**: 公共 KnowledgeSpace/ReasoningPath 不进入用户库，`LearnerContext` 每回合现场投影；本决策触发 ADR-0075 的 SQLite 回头条件。
**何时回头**: Trace 规模和查询实测证明原子 JSON 仍可满足追加、反查、恢复与重算，或未来账户同步要求更换存储边界。

---

## Grill Q72 - 顶层架构压缩为 TeachingMap + LearningMemory + TutorLoop

**决策**: 顶层仅保留 `TeachingMap`、`LearningMemory` 与 `TutorLoop`；其余名词降为内部数据结构、投影或函数。

**否决**:
- 把每个领域名词升成独立子系统：会把产品动机、领域概念、处理阶段与存储实现混在同一层。
- 把 SQLite 或 LLM 放进领域主叙事：前者是存储实现，后者是执行器，都不是产品中心。

**命门**: 架构压缩只减少顶层边界，不得抹掉公共教学资产、私人学习证据与运行时教学决策之间的所有权差异。
**何时回头**: 某个内部能力出现独立生命周期、独立伸缩或独立信任边界，且实测无法继续由三者内部承担。

---

## Grill Q73 - ReasoningEpisode 是 TeachingMap 内的源锚定论证片段

**决策**: 取消 `SourceReasoning` 领域实体；`TeachingMap` 以 `ReasoningEpisode` 保存目标导向、公共且源锚定的作者论证片段，多源融合降为 `construct_reasoning_episode(target)` 内部职责。

**否决**:
- 保留 `SourceReasoning` 与 `ReasoningEpisode` 两层名词：二者会循环定义，并制造第四个顶层边界的错觉。
- 直接等同 `discourse_index`：局部修辞关系不等于完整的前提—推论—结论链。

**命门**: 每个推理步骤与连接必须标明来源层及 Evidence LID；跨层仍无法闭合的缺口必须显式保留，不得用书外知识伪装成作者推理。
**何时回头**: 多源融合职责出现独立生命周期、伸缩或信任边界，或实测证明单一 discourse 层已能无损覆盖目标推理链。

---

## Grill Q74 - key_stop 预构建与其他目标按需展开

**决策**: 将 `BookStructure.key_stops` 作为默认预构建 `ReasoningEpisode` 的重点书签；key_stop 之外的源锚定 claim、公式或当前问题在被明确请求时按需构造 Episode。

**否决**:
- 只有 key_stop 才能构造 Episode：32 个重点书签不能覆盖全书所有有效问题。
- 从 discourse 图枚举所有路径：组合爆炸，且会产生大量没有教学目标的路径。

**命门**: key_stop 只是单个重点入口，不是大节点或 Episode 范围；每个 Episode 仍须围绕一个明确目标有界展开。
**何时回头**: 实测表明 32 个预构建 Episode 的命中率过低，或按需构造的延迟无法满足交互阅读。

---

## Grill Q75 - ReasoningEpisode 的最小完整论证边界

**决策**: `ReasoningEpisode` 只收录支撑目标所必需的最小完整论证链；论证连接无法由源证据闭合时显式记录缺口并停止扩张。

**否决**:
- 固定截取目标前后若干段：物理邻近不能保证论证完整，也会漏掉跨节依赖。
- 默认收录整章：会混入后续解法、例子和旁支，使一个 Episode 失去单一目标。

**命门**: “最小”不得以删掉必要前提为代价，“完整”只要求忠实呈现作者实际给出的论证，不得用书外知识补洞。
**何时回头**: 实测无法稳定判断必要前提，或多数目标都需要接近整章才能形成可理解论证。

---

## Grill Q78 - 掌握值只作投影，Evidence 多轴状态作持久真相

**决策**: `new / learning / mastered`、单一 0–1 mastery 与 qualitative pass 只作目标特定投影；持久真相是带 assistance、context、来源与 Estimator watermark 的多轴 Evidence。

**否决**:
- 单一 mastery 或定性布尔值：无法区分 recognition、recall、application、transfer，也会混淆提示后与独立表现。
- 计划拥有掌握状态：改计划可能改身份、删证据，并阻断跨书与跨计划复用。

**命门**: `LearningPlanRevision` 与复习策略只消费 `LearningMemory` 的状态投影，不得拥有、覆盖或删除 Trace、Evidence、`LearnerKnowledgeState`。
**何时回头**: 多轴状态无法比单值投影改善下一动作选择，且实测证明原始 Evidence 仍可无损回放与重估。
**用户答案**: 同意。

---

## Grill Q79 - Pass1 图谱是 LearningObject 候选源，不是长期状态键

**本轮状态**: `CONFIRMED`。

**共享对话语义核对**:
- Book/Global Knowledge Space 描述公共知识结构；Learner State 是其上的私人动态覆盖，不能直接修改公共图节点。
- Learning/Reasoning Path 描述认知推进，不等同普通 Knowledge Graph；知识对象还需支持能力目标与可判定 Evidence。

**源码核对**:
- `agents/pass1-local-extractor.md:28-30,64`：Pass1 只抽 `entity | concept | claim`；实体/概念 ID 是模型给出的规范化名，claim ID 含书内 LID。
- `packages/core/src/pass1-reduction.ts:725-779`：确定性门只校验字段边界、节点 ID 唯一、LID 属于当前输入和边端存在，不校验语义同一性、学习粒度或跨书身份。
- `packages/core/src/merge.ts:57-70`：跨窗口仅按完全相同的 `node.id` 合并；没有 entity resolution、同义归并、同名消歧或过宽节点拆分。
- `packages/core/src/zod.ts:62-83,727-732`：公共基座只有三类图节点及书内图边；节点没有稳定语义 revision、知识对象 kind、评估契约或跨资源 identity。
- `packages/core/src/reviewed-source-candidate.ts:442-458`：来源结构迁移会生成 LID migration map，但新 candidate base 的 graph 为空；现有实现没有把 Pass1 节点身份迁移成长期键。
- `packages/core/src/hybrid-foundation-apply.ts:450-452,580-598`：只有 LID 序列完全相同时才原样保留旧图；这不是语义节点迁移。

**推荐**: Pass1 图谱作为 `TeachingMap` 的 book-scoped 候选池和来源索引；正式知识点经 candidate → resolver → validator → service-issued ID 晋升后，才得到可被计划、Evidence 与长期状态引用的 `LearningObjectRef`。

```ts
interface LearningObjectCandidate {
  candidate_id: string;
  proposed_kind: KnowledgeObjectKind;
  canonical_statement: string;
  source_bindings: Array<{
    teaching_map_version: string;
    book_id: string;
    graph_node_ids: string[];
    evidence_lids: string[];
  }>;
  origin: "pass1_graph" | "book_structure" | "reasoning_episode" | "runtime";
}

type PromotionResult =
  | { status: "resolved"; target: LearningObjectRef }
  | { status: "split_required"; candidates: LearningObjectCandidate[] }
  | { status: "mapping_gap"; reason: string }
  | { status: "promoted"; target: LearningObjectRef };
```

允许的映射不是固定一对一：

```text
Pass1 node -> 0 个 LearningObject   // 仅导航、引用或粒度不适合学习
Pass1 node -> N 个 LearningObject   // 节点过宽，需要拆成多个可学习命题/步骤
N 个 Pass1 nodes -> 1 LearningObject // 跨章节或跨书表达同一稳定对象
其他 TeachingMap 资产 -> candidate // procedure、pattern、reasoning episode 不必等待 Pass1 有对应节点
```

**问题**: 是否确认，Pass1 知识图谱只作为正式知识点的候选来源与 source binding，而不是直接把每个 `graph_node.id` 当成 `LearnerKnowledgeState` 的长期键；只有经 `TeachingMap` resolver/validator 晋升的稳定 `LearningObjectRef` 才能进入学习计划、Evidence 和长期状态？

**我的推荐**: 是。这样保留 Pass1 已有抽取与 LID 锚定价值，同时避免把模型命名、书内 LID、导航粒度和版本漂移固化成用户“会了什么”的身份真相。

**用户答案**: 同意（“可以”）。本轮只锁定候选源与正式身份的边界；跨书匹配算法、稳定 ID 生成、拆分标准和晋升门禁细节仍未锁定。

**由 Q69/Q71/Q72/Q79 推导出的生命周期边界**:

```text
公共、预构建或按需构建
  canonical book/source artifacts
  TeachingMap: LearningObjectRef + source bindings
               + reusable ReasoningEpisode / ReasoningPattern / path fragments

TutorLoop 现场选择或生成
  当前目标与相关 LearningObjectRef
  选择哪段书内来源、以什么顺序/方式呈现
  私人 PathInstance、下一认知台阶
  Agent 的问题、提示、解释与用户可见回答
  对用户动作和 Agent 教学动作的观察

LearningMemory 持久沉淀
  InteractionTrace: 发生过什么
  LearningEvidence: 这些事实对学习状态提供什么依据
  PathProgress / LearnerKnowledgeState: 带版本与 watermark 的投影

只在当前采样中存活
  LearnerContext / prompt assembly
  未采用的候选步骤
  模型隐藏推理
```

这里“书的呈现内容在现场”只指**选择哪段既有书内容、怎样编排和解释**；书的规范内容本身仍是公共来源资产。Agent 的公开输出与采用的下一步属于现场结果，但其可观察事实进入 `InteractionTrace`，不会随回合消失。仓库现有 `TurnEvidenceLedger` / “本轮证据账本”仅约束当前回答可引用哪些书内来源，不是这里的持久 `LearningEvidence`（见 `CONTEXT.md:13-14`、`crates/runtime/src/orchestrator.rs:621-674`、`docs/architecture.md:222-226`）。

---

## Grill Q80 - 下一认知台阶采用受约束的现场合成

**本轮状态**: `CONFIRMED`。

**已锁定前提**:
- Q69：公共层保存可复用认知步骤、Reasoning Pattern 与局部路径片段；私人 `PathInstance` 在运行时组装。
- Q70/Q78：当前路径表现不自动等于长期掌握；状态更新必须保留能力轴、assistance 与 context。
- Q79：运行时可以发现新的 LearningObject candidate，但不能直接创造长期身份或改写公共 `TeachingMap`。

真正未决的是：现场的“下一步思考方向”必须完全来自预构建步骤，还是允许 Agent 临场发明。三个方向会产生截然不同的系统：

```text
A. 只选预构建步骤
   coverage 安全但僵硬；新问题或新措辞一旦没有现成 step 就断路。

B. Agent 自由生成下一步
   灵活，但可能脱离目标、来源与 LearningObject，且难以复放和评估。

C. 受约束的现场合成（推荐）
   current goal
   + relevant LearningObjectRef / LearnerKnowledgeState
   + source bindings / ReasoningEpisode / reusable path fragments
       -> Agent or planner proposes bounded next-step candidates:
          one target-matched cognitive move
          semantic source-support assessment, with explicit gaps
       -> deterministic gate checks:
          one structured move with valid goal + LearningObjectRef bindings
          source references and quoted text belong to available evidence
          assistance/context explicitly recorded
          no direct TeachingMap or mastery mutation
       -> TutorLoop selects one and renders question / hint / explanation
          unresolved source support -> continue around the evidence gap under Q88
       -> chosen decision + observable result enter InteractionTrace
```

在 C 中，现场新合成的 step 可以完成当前教学，但默认只是私人运行时对象；只有以后经过独立的去重、来源和质量审查，才可能晋升为公共可复用 path fragment。记录的是结构化的选择结果与依据引用，不是模型隐藏推理。

**问题**: 是否确认采用 C：`TutorLoop` 可以现场合成“下一认知台阶”，但必须从当前目标、稳定 `LearningObjectRef`、Learner 状态和可用来源/路径素材出发，经确定性边界门后才能执行；它既不受限于只能照搬预构建 step，也不能由 Agent 无约束自由发挥？

**我的推荐**: 是。这样既保留现场个性化和长尾覆盖，又让每一步可追溯、结构可校验，并守住公共教学资产与长期学习状态的写入边界。

**用户答案**: 选择 C。

**锁定结果**: `TutorLoop` 采用受约束的现场合成：下一认知台阶必须绑定当前目标与稳定 `LearningObjectRef`，受 Learner 状态和来源/路径素材约束，经确定性结构与引用边界门后执行；语义支持和认知动作是否合适由模型结合来源判断，程序不把引用合法等同于推论成立。来源推论未闭合时按 Q88 围绕证据缺口继续带读，并暂停依赖该推论的判定。采用结果与可观察反馈进入 `InteractionTrace`，但不得直接改写公共 `TeachingMap` 或长期学习状态。

---

## Grill Q81 - LearningMemory 采用证据脊柱，不复制三份可编辑真相

**本轮状态**: `CONFIRMED`。

**DeepTutor 源码核对**:
- L1 同时包含追加式 trace 与工作区 snapshot；snapshot 的 fingerprint 能识别原实体修改，但 L2 增量游标只记录 `surface:entity_id`，同 ID 内容变化不会自动重新摄取。
- L2 把新实体抽成按产品 surface 分组的 Markdown 事实；引用必须来自当前 chunk，具备有用的局部证据门。
- L3 把 L2 综合为 `recent/profile/scope/preferences`，读取时直接拼接四份文档；当前 L3 update 已改为引用裸 surface 名，而 L3 audit 仍按旧 `m_xxx` entry 引用检查，证据链并不闭合。
- Markdown 可读、可编辑、可 audit/undo 是很好的治理面，但 L2/L3 本身都是可变存储，不能直接承担我们已经锁定的可回放学习状态真相。

**本仓库源码核对**:
- 当前 `MemoryDocument` 是版本化、原子替换的单一 JSON 真相，`ProfileFact` 已有 typed evidence、scope/applicability、source/trust、supersession、forget exclusion 与 durable `ReviewJob` watermark。
- 每个 resident turn 自动生成有预算且冻结的 `ReaderProfileSnapshot`；不依赖 Agent 自愿 `memory.recall`，这一点强于 DeepTutor 的全 L3 tool/read concat。
- 当前 technical-learning 投影仍以 LID、read/qa/note/highlight 计数及 `ProfileFact::Capability(understood_concept)` 表示学习活动或理解；它没有 `LearningObjectRef × capability axis × assistance × context`，也没有记录 Tutor 动作与可观察结果，已不足以兑现 Q70/Q71/Q78/Q79。

三个方向:

```text
A. 复制 DeepTutor 的 L1/L2/L3 文件层
   trace/snapshot -> surface Markdown -> profile/scope Markdown
   优点是直观；代价是 mutable Markdown、ID-only cursor、surface 分库和证据链断裂
   会削弱我们已有的 typed truth / revision / replay contract。

B. InteractionTrace 直接估计 LearnerKnowledgeState
   结构最少；但中间判断不可检查，Estimator 升级时难解释“哪条观察为何支持哪一轴”，
   也难区分观察事实、解释性 Evidence 与当前 State。

C. 证据脊柱 + 派生视图（推荐）
   InteractionTrace (append-only observed facts)
      -> LearningEvidence (versioned interpretation ledger)
      -> PathProgress / LearnerKnowledgeState (rebuildable projections)
      -> per-turn LearnerContext (bounded, frozen, not persisted in chat)

   LearningEvidence key dimensions:
      LearningObjectRef × capability_axis × assistance × context
   Every evidence item:
      exact trace_refs + source + estimator_version + created_at
   Every state projection:
      estimator_version + evidence_watermark + source TeachingMap version
```

在 C 中，DeepTutor 的三层只作为**逻辑蒸馏参照**，不成为三套物理真相：

```text
一个逻辑 LearningMemory 边界
  memory.json / ProfileFact ledger
    拥有稳定的用户背景、目标、讲解偏好、约束与用户明示的通用能力背景

  learning.db / learning evidence ledger（承 Q71）
    拥有 InteractionTrace、LearningEvidence、PathProgress、LearnerKnowledgeState

  LearnerContext
    每回合把相关 ProfileFact + 当前路径状态 + 对象级长期状态做有界合成
```

唯一权威判定:

```text
若判断绑定 LearningObjectRef 且回答“该对象在哪个能力轴上表现如何”
  -> 只能由 LearningEvidence / LearnerKnowledgeState 拥有

若信息回答“这个人通常是谁、想要什么、偏好怎样讲、有什么长期约束”
  -> 由 ProfileFact 拥有

ProfileFact::Capability 可保留“用户明示有 Rust 工作背景”这类通用背景能力，
但不得再保存或推导“已掌握 LearningObject X”这种对象级学习状态。
```

**问题**: 是否确认采用 C：只借 DeepTutor 的分层蒸馏思想；在一个逻辑 `LearningMemory` 边界内保留 `ProfileFact` 与 `learning.db` 两类语义明确的权威账本，以 `InteractionTrace → LearningEvidence → 可重建 State` 作为学习状态唯一证据链，并禁止 `ProfileFact` 与 `LearnerKnowledgeState` 同时拥有对象级掌握判断？

**我的推荐**: 是。它既复用当前已经很成熟的画像治理能力，又为真正的学习状态建立可回放、可重估、不会被 Markdown 摘要覆盖的证据脊柱；DeepTutor 最好的可见性和渐进压缩可以作为派生视图/UI 借入，而它的游标与证据链缺陷不会进入真相模型。

**用户答案**: 同意采用 C。

**锁定结果**: `LearningMemory` 采用 `InteractionTrace → LearningEvidence → 可重建 State` 的唯一学习证据链；稳定用户上下文与对象级学习状态由语义不同的权威账本分别拥有，`ProfileFact` 不得保存或推导绑定 `LearningObjectRef` 的掌握判断，二者只在每回合有界 `LearnerContext` 中合成。

---

## Grill Q82 - InteractionTrace 采用独立的语义事件流

**本轮状态**: `CONFIRMED`。

**本仓库源码核对**:
- `AgentChatTurn` 以一次 `/agent/chat` 用户请求为持久化单元：请求前先写入稳定 `turn_id + user_turn_ordinal + pending_assistant`，结束时再一次性写入完整 `OuterOutcome` 或失败错误。
- `OuterOutcome.turns` 只是一次用户请求内的 Provider 采样次数；现有 `trace[]` 是 `{tool,args,result_digest,query_audit}` 工具调用摘要，`effects[]` 是该请求内的副作用清单。三者粒度不同，均不是对象级学习观察账本。
- 失败回合以 `outcome = None` 落盘；若前序工具已经产生观察或副作用、后续 Provider 采样才失败，这些步骤不会出现在 Agent history 的 outcome 中，不能靠历史完整回放。
- Highlight/Note/Layout 等 effect 的“保留、撤销、应用、忽略”发生在原回合结束后的独立 UI/API 操作；当前 `handled` 只存在于前端内存，刷新历史即清空，Agent history 只证明 effect 曾被提出，不证明最终处置。
- 当前 `record_query_observation` 在 `book.query` 时写一条长期 `qa` 记录，只能证明某问题曾在某 LID 触发查询；它没有 Tutor 刺激、Learner 回应、assistance、能力轴或处置结果，不能充当 `LearningEvidence` 的事实输入。

三个方向:

```text
A. 直接把 AgentChatTurn / OuterOutcome 当 InteractionTrace
   复用成本最低；但一个 Tutor 问题与下一回合 Learner 回答会被切开，
   失败步骤和 effect 后续处置缺失，学习事实还会耦合聊天删除与历史迁移。

B. 把每次 Provider message / ToolCall 都当 InteractionTrace event
   细节最全；但它把 Provider 编排噪声、重试和工具实现当成学习语义，
   后端一换轨迹就变，还容易保存无关 Tool body 或模型隐藏推理。

C. 独立的追加式语义事件流（推荐）
   chat turn / tool step / effect 只提供关联来源，不拥有学习事实；
   每个已发生的可观察语义动作在发生时单独追加，跨聊天回合用因果引用连接。

   interaction_id                 // 关联一次可能跨多个 chat turn 的教学交换
     tutor_step_selected          // 只记采用的公开决策与约束引用，不记隐藏推理
     tutor_action                 // question | hint | explanation | example | navigation
     learner_action               // answer | question | self_report | skip | note | navigation
       responds_to_event_id?      // 将下一 chat turn 的回答连回上一 Tutor 刺激
     observable_result            // succeeded | failed | kept | undone | dismissed
     interaction_closed

   every event:
     event_id + sequence + occurred_at
     interaction_id + chat_turn_id? + responds_to_event_id?
     LearningObjectRef[] + path/step ref? + observable payload
```

C 的边界:
- `InteractionTrace` 只保存实际发生、用户可见或系统可验证的事实；不保存候选的隐藏推理、Provider 重试过程或原始 Tool result body。
- 事件以语义观察为原子追加单位；一次聊天请求只是关联维度，不是必须等到终答后才能提交的事务边界。
- `AgentChatTurn` 继续负责可恢复 transcript，现有 `TraceStep` 继续负责用户可见工具轨迹；二者可引用同一 `chat_turn_id/event_id`，但都不是学习状态真相。
- 事件 payload 的正文保留、脱敏与删除传播策略留给后续 Grill，本问只锁定所有权、粒度与因果形态。

**问题**: 是否确认采用 C：`InteractionTrace` 建成独立于 Agent 对话历史和低层工具轨迹的追加式语义事件流，以“一个已发生的可观察动作/结果”为原子记录，并允许用 `interaction_id + responds_to_event_id` 把跨聊天回合的 Tutor 刺激、Learner 回应和 effect 最终处置连成可回放因果链？

**我的推荐**: 是。这样 `LearningEvidence` 引用的是稳定、可恢复的观察事实，而不是一次请求结束时才凑出的 UI outcome；它既能保留失败与撤销语义，也不会把 Provider 编排、聊天 transcript 或模型隐藏推理升级成学习真相。

**用户答案**: 同意采用 C；同时要求 Agent 获得出题与多样交互能力，`InteractionTrace` 必须记录 Agent 实际向用户解释、提问和展示的内容，因为用户接触的信息既来自书，也来自 Agent 回答。

**锁定结果**: `InteractionTrace` 是独立于 Agent 对话历史和低层工具轨迹的追加式语义事件流，以实际发生的可观察动作或结果为原子记录，并用因果引用连接跨聊天回合的 Tutor 刺激、Learner 回应和最终处置。每次 Learner 行为必须关联当时实际交付的教学呈现；该呈现同时覆盖书源内容与 Agent 生成内容，但“曾向用户展示”不等于“已成为书内规范事实”或“已证明用户掌握”。出题工具与多样交互的具体协议留给 Q83 继续收敛。

---

## Grill Q83 - 结构化教学交互采用一个带类型变体的呈现工具

**本轮状态**: `CONFIRMED`。

**后续修订**：2026-09-16 Q102 / [ADR-0130](docs/adr/0130-agent-rich-presentation-and-read-time-authoring.md) 开放通用回答内容的现场 HTML/CSS/JavaScript 制作，教学交互可采用其自由视觉呈现。本节保留当时讨论；其中“不能生成任意 UI”的全面限制已被修订，类型化教学行为、实际呈现与帮助条件记录及 Q84 判题边界继续成立。

**由用户补充锁定的需求**:
- Agent 应能主动出题，并用不止自由文本问答的一组交互捕捉 Learner 的显式行为。
- 系统必须保存 Agent 实际解释、提问和展示的内容；用户的信息环境由书源内容与 Agent 生成内容共同组成。
- “已交付到界面”只证明系统提供了什么，不证明用户看完或理解；后者必须由后续显式行为与 Evidence 判断。

**为什么必须把教学条件与用户行为成对保存**:

不能只记录“用户回答了什么”，还必须记录“Agent 在回答之前究竟让用户看到了什么”。同一句用户回答发生在不同教学条件下，Evidence 含义完全不同：

```text
情况一：
Agent 没给任何帮助
用户独立答对

情况二：
Agent 已经展示了答案
用户照着答案复述

情况三：
Agent 先给了两个提示
用户随后答对
```

三种情况表面上都是“用户答对”，但不得生成同一条学习判断。因此 `InteractionTrace` 必须保存完整的一对：

```text
Agent 实际交付的内容与交互条件
  ↓
用户随后产生的显式行为
```

“Agent 实际交付的内容与交互条件”至少包括：
- Agent 自己生成的解释、问题、提示、例子和反馈；
- Agent 从书中选择并展示的原文、图示或公式；
- 题目、选项、排序项、匹配项等完整题面；
- 当时向用户开放了哪些回答方式；
- 用户回答前已经获得了多少、哪一级提示或答案暴露。

来源必须继续分开：

```text
书中原文             -> 书源内容
Agent 的解释或例子   -> Agent 生成内容
```

Agent 的解释构成用户接触的信息，因而必须记录；但“Agent 曾经说过”不等于“书里就是这样说的”，也不等于“Agent 的说法一定正确”。本决定承接并细化三项已落盘边界：
- `grill.md` Q82 已确认 `InteractionTrace` 是独立的学习过程事件流；
- `CONTEXT.md` 的 `InteractionTrace` 记录用户接触了什么以及双方做了什么；
- `CONTEXT.md` 的 `TutorPresentation` 表示 Agent 实际交付的教学内容和交互条件。

还必须区分“交付”与“理解”：

```text
系统可以证明：某段内容已经交付到用户界面。
系统不能仅凭此证明：用户认真看完并理解了它。
```

用户是否真正处理了这些信息，只能由回答、选择、排序、请求提示、跳过、改答等后续显式行为与版本化 `LearningEvidence` 继续判断；呈现事件本身不是掌握证据的结论。

**本仓库源码核对**:
- Resident Agent 当前注册 31 个 handler，覆盖 Book、Artifact、Reader、Memory、Profile 与来源呈现，没有 Tutor/Quiz/Assessment capability；继续为每种题型增加独立工具会直接扩大已受预算治理的工具面。
- `AgentAnswerPart` 当前只有 `Markdown` 与 `Sources`；结构化题目没有一等回答块。
- `AgentEffect` 只承载 goto、标注、布局和论文地图等可撤销 Reader 副作用。题目/练习是教学呈现，不应伪装成 Reader mutation。
- `/agent/chat` 当前只接收自由文本和可选书内选区，没有 `interaction_id + typed response`；因此选择、排序、匹配、跳过、请求提示等行为都会退化为难以可靠解析的自然语言。
- ADR-0036 已将 quiz 明确留作后续“主动探测”信号，并与自由文本反馈、viewport 弱信号和画像慢先验区分；本问正是在补该基建。

三个方向:

```text
A. 继续只让 Agent 用 Markdown 出题，用户在文本框回答
   最简单，但选项、排序、匹配、改答、跳过、请求提示都会变成自然语言猜测；
   系统很难确定用户究竟看到了哪套题面、执行了什么动作。

B. 每种交互一个 Agent tool
   tutor.ask_choice / tutor.ask_ordering / tutor.ask_matching / ...
   每个 schema 很直观；但题型越多工具越膨胀，共同字段、来源门和事件写入重复，
   增加一种交互就要改工具暴露、Prompt、Adapter 与生命周期治理。

C. 一个有界的结构化呈现工具 + 版本化类型变体（推荐）
   普通 Markdown/来源回答由 Runtime 自动捕获为 TutorPresentation；
   只有需要专门 UI 的练习，Agent 才调用暂名 tutor.present_interaction。

   TutorInteractionSpec {
     learning_object_refs
     target_capability_axis
     prompt                       // Agent 实际向用户说的话
     materials[]                  // book source refs | agent-generated material
     assistance_state
     kind:
       free_response
       | single_choice
       | multiple_choice
       | ordering
       | matching
       | evidence_selection
       | self_assessment
     variant_payload              // 由 kind 决定的封闭 schema
   }
```

C 的运行形态:

```text
普通解释/提示
  Agent final answer
    -> Runtime 自动捕获精确 Markdown + source refs
    -> TutorPresentation event

结构化练习
  Agent -> tutor.present_interaction(validated spec)
    -> 一等 Interaction answer block
    -> Runtime 记录已交付的完整题面、选项/材料、来源和 assistance
    -> 用户显式 submit / change / skip / request_hint
    -> typed Learner event responds_to presentation event
```

边界:
- Agent 只能从受支持的 `kind` 中选择和填充内容，不能现场发明任意 HTML/JavaScript 或新的行为语义。
- Runtime/Server 分配身份、做 schema 与来源门、捕获最终交付内容；Agent 不负责“记得给自己写日志”。
- 交互只捕捉用户在组件上的显式动作，不把鼠标轨迹、停留时间或普通 viewport 变化直接解释成理解。
- `self_assessment` 只记录用户自评，不自动等于掌握；正确答案、开放题 rubric 与确定性/模型判分边界留给下一轮 Grill。

**问题**: 是否确认采用 C：普通 Agent 解释由 Runtime 自动记录；结构化出题则只提供一个带封闭、版本化题型变体的 `tutor.present_interaction` 工具，把题目作为一等 `TutorPresentation` 回答块展示，并把用户的选择、排序、匹配、自由回答、证据选择、跳过和请求提示等显式动作作为类型化事件写入 `InteractionTrace`？

**我的推荐**: 是。它既给 Agent 足够多样的教学手段，又不让工具面和 UI 变成 Agent 可任意编程的无边界协议；最重要的是，系统能自动保存“用户当时收到的完整信息”以及“用户随后明确做了什么”。

**用户答案**: 同意采用 C，并要求把“Agent 先展示了什么决定 Learner 行为如何解释”的完整场景、来源边界和交付/理解边界写入 Q83，而不是只保留压缩结论。

**锁定结果**: 普通 Agent 解释、提示与来源展示由 Runtime 自动捕获为 `TutorPresentation`；需要专门 UI 的练习统一通过一个有界、版本化、带封闭题型变体的结构化呈现工具生成一等交互回答块。Runtime 保存完整题面、书源与 Agent 生成材料、可用回答方式及 assistance 状态，用户的 submit/change/skip/request_hint 等显式行为以类型化事件响应对应呈现。交互不属于 `AgentEffect`，Agent 不能生成任意 UI，也不负责自觉写学习日志；题目判分与 Evidence 强度留给 Q84。

---

## Grill Q84 - 判题标准先冻结，答题结果与学习证据分层

**本轮状态**: `CONFIRMED`。

**已锁定前提**:
- Q78：持久学习真相是带 assistance、context、来源和版本的多轴 Evidence，不是单一 mastery 分数。
- Q81：`InteractionTrace → LearningEvidence → LearnerKnowledgeState` 是唯一对象级学习证据链。
- Q82/Q83：系统保存完整 `TutorPresentation` 与类型化 Learner 行为；独立答对、答案暴露后复述、提示后答对不得生成同强度 Evidence。

**本仓库源码核对**:
- 当前运行时没有 quiz answer key、rubric、attempt assessment 或结构化判分协议；旧 `quiz_result` 只存在于已废弃的 ReaderProfile 草案，不能作为现行契约。
- 当前 `technical_learning` 仍保留 `understood_concept` 等旧画像兼容路径，但没有 `LearningObjectRef × capability axis × assistance × context` 判定，不能承接新题目结果。
- `book.query` 已有一条可借鉴但不能照搬的边界：模型判断开放语义是否 supported/uncertain/unsupported，确定性 gate 只检查每项 obligation 恰有一个判断、引用来自冻结证据且 quote 为来源精确子串。程序不会假装能纯规则理解任意自然语言。
- 项目 B2 红线禁止“AI 生成内容 + AI 自评”成为唯一正确性闭环；但开放解释题又确实无法用字符串相等可靠判定。

真正要分开的不是一种结果，而是两层结果：

```text
ResponseAssessment
  回答在这道具体题的冻结标准下是否 correct / partial / incorrect / uncertain

LearningEvidence
  结合 ResponseAssessment + 用户回答前看过什么 + assistance + attempts + context，
  这次表现对某个 LearningObjectRef 的某条 capability axis 提供多强、什么方向的依据
```

例如：

```text
独立答对          -> ResponseAssessment = correct；Evidence 可支持独立 recall/application
两个提示后答对    -> ResponseAssessment = correct；Evidence 只能支持 assisted performance
答案展示后复述正确 -> ResponseAssessment = correct；不得据此支持独立 recall
```

三个方向:

```text
A. 出题 Agent 看完回答后直接判断并更新 LearnerKnowledgeState
   最灵活；但标准可事后漂移，同一个模型既出题又给自己判题，
   判错也会直接污染长期状态，无法解释或重放。

B. 所有题都只允许确定性判分
   单选、排序、匹配很可靠；但自由解释、推导、比较和迁移题只能退化成关键词匹配，
   会牺牲最有价值的深度学习交互。

C. 先冻结 AssessmentContract，再按题型分层判定（推荐）
   Agent 可以在出题时提出答案键或 rubric；Runtime 在用户作答前验证、版本化并冻结，
   用户回答后不得临时更换标准，也不得从判题结果直接写长期 State。
```

C 的精确形态:

```text
before presentation:
  Agent proposes TutorInteractionSpec + AssessmentContract
  Runtime validates schema + LearningObjectRef + source/rubric bindings
  Runtime freezes contract before the user sees or answers the question

after learner response:
  single/multiple choice -> deterministic key comparison
  ordering/matching      -> deterministic order/pair comparison
  evidence selection     -> deterministic source/range membership first;
                            semantic adequacy only when the contract explicitly requires it
  free response          -> frozen atomic rubric obligations
                            + source-bounded semantic assessment
                            + deterministic completeness/source gate
                            + uncertain is a legal result
  self assessment        -> record only; no correctness verdict

then:
  ResponseAssessment event
    -> Evidence builder combines full TutorPresentation, assistance, attempts and context
    -> versioned LearningEvidence
    -> Estimator may rebuild PathProgress / LearnerKnowledgeState
```

`AssessmentContract` 至少冻结：

```text
target LearningObjectRef[]
target capability_axis
evaluator_kind + evaluator_version
answer key or atomic rubric obligations
accepted source/rule bindings
partial/uncertain policy
feedback/reveal policy
```

边界:
- 答案键/rubric 可以由 Agent 提议，但必须在用户作答前冻结；未到 reveal 时不得混入用户可见 `TutorPresentation`。
- 封闭题的“选对了”可以确定性判断，但 Evidence 强弱仍取决于 assistance、尝试次数和答案是否已暴露。
- 开放题的模型语义判断必须保存 rubric、模型/Estimator 版本、来源和 uncertainty，且不得单独把 State 推成 mastered。
- rubric 已执行但证据不足以确定时记录 `uncertain`；判定未执行、Provider 不可用或其他技术失败时记录 `unassessed`。两者都保留原始 Trace 等待重估，绝不记成用户答错。

**问题**: 是否确认采用 C：每道可判题交互都在用户作答前冻结版本化 `AssessmentContract`；封闭题优先由确定性规则判定，开放题只允许在冻结 rubric 与来源约束下作带 uncertainty 的语义评估；两者都先生成 `ResponseAssessment`，再结合完整呈现、assistance、attempts 与 context 形成 `LearningEvidence`，绝不由出题 Agent 直接改写长期学习状态？

**我的推荐**: 是。这既保留自由解释、推导和迁移题的价值，又守住“技术失败不是答错、答对不自动等于掌握、模型判断必须可追溯重估”三条底线。

**用户答案**: 同意采用 C。

**锁定结果**: 每道可判题 `TutorInteraction` 必须在 Learner 作答前把版本化 `AssessmentContract` 与确切题面、目标 `LearningObjectRef`、能力轴、答案键或原子 rubric、来源/规则、判定器版本及 partial/uncertain/reveal 策略一并冻结；Agent 看见回答后不得改动标准，未满足 reveal 条件的答案也不得进入用户可见呈现。封闭题优先确定性比较，开放题只允许在冻结 rubric 和来源约束下作可追溯、可返回 uncertain 的语义判断，自评不产生客观正确性。每次提交先形成 `ResponseAssessment`；rubric 已执行但证据不足时记为 uncertain，判定未执行或技术失败时记为 unassessed，两者都不得退化为 incorrect。随后 Evidence builder 才结合完整 `TutorPresentation`、assistance、attempts 与 context 形成版本化 `LearningEvidence`，由 Estimator 重建投影；出题 Agent 和单次判题结果都无权直接改写长期学习状态。

---

## Grill Q85 - TutorLoop 采用稳定控制骨架、类型化教学动作与独立呈现框架

**本轮状态**: `CONFIRMED`（上一版因把算法示例误泛化为全局教学流程而重开；重开后采用修订 C+）。

> **已撤销草案**：下面从“问题重构说明”到“撤销的锁定结果”保留上一轮推导，作为为什么必须重开的审计记录；其中 direction-first 对做题机器的纠偏仍有价值，但算法题流程和算法专属 Evidence 轴不再生效。

**问题重构说明**: 上一版 Q85 直接讨论“提示、反馈和重试怎样计分”，问题顺序错误：它默认连续练习是主流程，容易让 Q83/Q84 的结构化交互与评分基础设施反过来塑造产品，使 `TutorLoop` 退化为做题机器。本轮撤回该问法；精确 assistance 暴露序列仍然必要，但只能在教学姿态锁定后作为留痕机制继续讨论。

**用户明确的教学目标**:
- 不追求记住“这道题怎么做”，而是形成“看到某种结构，自动联想到某种工具”的可迁移能力。
- Agent 默认只提供一个能让 Learner 继续推导的思考方向，不立即给算法名、完整步骤、标准答案或连续题目。
- 学习核心是“为什么成立”的推导链，不是题号、答案、代码模板或每日完成题量。
- Learner 先产生自己的问题和暴力思路，再识别浪费、约束或不变量，由此推到策略；关掉答案后重新表达或实现，未来再以主动回忆而不是重看进行复习。
- 学习状态必须区分“独立推出”“知道策略但实现不出来”“连策略也想不起”等不同情况；遗忘和重新推导是正常学习过程，不是一次失败。
- 长期组织单位优先是跨题复用的题型/结构模式，而不是“第几题对应哪个算法”。

用户给出的“盛最多水的容器”不是要记忆 `LeetCode 11 = 双指针`，而是保留这条推导核心：

```text
目标：最大化面积
  -> 面积 = 宽 × 短板
  -> 指针向内移动时，宽必然减小
  -> 若还想让面积有增大的可能，只能尝试提高短板
  -> 移动高边不会提高当前短板
  -> 因而移动较矮的一侧

可迁移模式：
  当搜索范围的一个维度必然变差时，
  下一步必须改变另一个真正可能改善目标的瓶颈维度。
```

这与既有领域模型的正确对齐是：

```text
ReasoningEpisode
  保存某个目标为什么成立的源锚定最小论证链

ReasoningPattern
  抽出可跨题迁移的“结构/约束 -> 排除无效方向 -> 工具/策略 -> 成立理由”

TutorLoop
  不默认展示完整 Episode，也不默认发一套题
  -> 结合 Learner 当前已经想到的位置
  -> 只选择或合成一个最小思考方向
  -> 等待 Learner 继续外显推理

LearningEvidence
  分开记录结构识别、理由推导、策略回忆、实际执行与跨情境迁移
  -> 不用单一 correct/incorrect 抹平差异
```

三个产品方向:

```text
A. Assessment-first
   题目 -> 提交 -> 判分 -> 下一题
   可测量，但会让数据模型支配教学体验，最容易变成做题机器。

B. Explanation-first
   Agent 先讲算法和代码 -> Learner 复述或模仿
   体验顺畅，但用户没有先形成自己的问题，也很难知道自己能否重新推导。

C. Direction-first guided discovery（用户选择）
   情境/问题
     -> Learner 先想并外显当前思路
     -> 找到暴力解法浪费在哪里、哪个约束真正起作用
     -> Tutor 只给一个不代替推导的思考方向
     -> Learner 继续推出策略及其成立理由
     -> 必要时再看完整解释或实现
     -> 关闭答案后重建
     -> 未来通过间隔回忆与迁移场景再次唤起该模式
```

**边界**:
- Q83 的 `TutorInteraction` 仍保留，但只服务稀疏诊断、主动回忆、排序/匹配确有教学价值或用户明确选择结构化作答的场景；它不是默认聊天外壳。
- Q84 的 `AssessmentContract` 与 `ResponseAssessment` 仍保留，但它们只约束“确实需要判题时怎样不污染证据”，不负责决定下一步教学一定要出题。
- `TutorLoop` 处于明确教学目标下且用户未要求直接答案时，才默认采用 direction-first；用户明确要答案、完整解释或代码时仍应直接响应，但该回合不能伪装成独立回忆证据。
- “一个思考方向”是一个有目标、可继续作答的最小认知动作，不是把答案切碎后逐句喂给用户，也不是用单选题诱导出既定答案。
- 完成题数、连续正确率和刷题速度可以作为活动统计，但不得成为教学策略的主优化目标。
- 间隔复习原则已进入后续设计输入；`1/2/4/7/14` 是否固定、怎样跨题迁移和何时主动抽查仍需分别 Grill，不能在本轮偷渡成固定实现。

**上一轮误读**: 用户用算法学习说明“不应背答案、也不应连续判题”，但这并不授权把算法题的结构、策略、实现流程提升为所有内容的统一教学模型。

**撤销的锁定结果**: “结构识别 → 策略推导 → 实现 → 迁移”只适用于部分问题求解或技能学习场景，不能作为全局 `TutorLoop`；原 Q86 依赖该错误前提，也一并撤回，等待本轮重新确认后再决定 assistance 升级。

---

**重开后的本仓库核对**:
- 当前 build/runtime 架构只有 `technical_learning` 与 `paper` 两个已落地 content profile，但 ADR-0048 已明确 profile 是可插拔规则包，不能把现有 `technical_learning` 语义当作项目全局真理；未来文学、历史、法律等类型不应继续堆特殊分支。
- ADR-0061 锁定“同机制换策略”：共享 loop、route 与命令面保持稳定，profile 只替换 policy、projection 与 UI affordance。这为教学层提供了直接类比。
- `discourse_index` 已覆盖 informative、argumentative、procedural、descriptive、meta，并区分 definition、classification、cause/effect、example/counterexample、comparison、procedure、claim support、research method、evidence、result interpretation 与 limitation；仓库事实本身已经否定“所有内容都是问题求解”。
- `BookStructure.key_stop` 同时允许 definition、formula、claim、example、turning_point、warning、summary；一个 key stop 需要的认知活动不可能共享同一条算法题步骤。
- 当前尚无 `LearningObjectKind`、`TargetCognitiveAct` 或 `TeachingMove` 的可执行 schema/enum，因此本轮只能锁定领域语义与分层原则，不能偷渡未经验证的完整类型集合。

**“苏七”部署帖对照带来的修订**:
- 原帖把问题驱动、教材锚定、跨会话记忆、人物关系和共同目标打包为一个体验，但没有课堂 transcript、提示升级合同或对照数据，不能据此把“全程提问”锁成全局教学真理。
- 原帖所称的苏格拉底式教学应收敛为 `GuidedInquiryPolicy`：它在引导学习意图下优先让 Learner 执行当前目标认知活动，是 `TeachingMove` 选择策略，不是 content profile、固定流程或评分模式。
- 人物、语气、陪伴和共同目标收敛为可选 `PresentationFrame`：它影响同一动作怎样呈现，但不得改变知识目标、来源、assistance、判题合同或 Evidence；关系与参与度也不是对象级能力证据。

算法示例暴露的通用原则应缩到这一层：**在引导学习时，不用完整答案或连续判题替代 Learner 的目标认知活动；每次只选择一个与当前对象和目标匹配的有界教学动作。** “目标认知活动”本身不能固定成推导算法：

```text
学习对象/目标                 Learner 可能需要做的事                 合适的一个方向可能是

术语、事实、人物、事件         回忆、定位、放入时空或主题关系          给检索线索、时间锚、关系锚
定义或概念                     解释边界、区分近邻、识别实例            给例子/反例、对比维度、边界案例
分类或比较框架                 找到分类轴、比较维度、例外               让 Learner 先选轴或解释一项差异
因果机制或系统模型             追踪关系、预测干预、解释结果             聚焦一个因果连接或反事实变化
主张与论证                     重建前提—证据—担保—结论、处理反驳       指向缺失前提、证据强度或反例
程序、方法或实践技能           执行步骤、判断前置条件、诊断偏差         让 Learner 预测下一步或定位失败点
公式或数量模型                 解释变量、假设、单位、关系与适用边界     聚焦一个变量变化、约束或量纲
实证论文                       区分问题、方法、证据、结果、主张、限制   要求把某个主张锚回方法与证据
叙事、历史或解释性文本         追踪视角、动机、转折、语境与多种解释     提醒看某个视角、文本证据或时间转折
算法或求解问题                 识别浪费/约束、提出策略、论证并实现       用户给出的 Hot 100 流程，只是这一行
```

因此真正可共享的只能是控制骨架，而不是教学步骤：

```text
TutorLoopTurn {
  interaction_intent          // 直接求解、解释、引导学习、复习、探测……
  content_profile
  learning_object_ref
  provisional_object_kind
  target_capability
  learner_context
  source_affordances
}

select_teaching_move(
  interaction_intent,
  content_profile,
  object_kind,
  target_capability,
  learner_context,
  source_affordances
) -> one bounded TeachingMove

render_tutor_presentation(
  selected_move,
  optional_presentation_frame
) -> TutorPresentation

one turn:
  确定当前目标认知活动
    -> 选择一个匹配对象/目标的 TeachingMove
    -> 通过可选 PresentationFrame 渲染并实际交付 TutorPresentation
    -> 等待或接收 Learner 的显式行为
    -> 只在对应目标能力上形成 Evidence
    -> 再决定下一步；不假设所有场景都要“答题”或“推导”
```

三个方向:

```text
A. 把算法流程换成一条更抽象的全局教学流程
   表面通用；实际上仍会假设所有对象都经历相同步骤，只是把“暴力解法”换成了模糊名词。

B. 每个 content profile 定义一整条固定教学流程
   比 A 更贴近体裁；但 profile 粒度太粗，同一本书内部也同时有定义、事实、论证、公式、程序和案例。

C+. 稳定控制骨架 + 类型化教学动作 + 独立呈现框架（推荐）
   TutorLoop 只统一“目标 -> 选一个动作 -> 交付 -> 观察 -> Evidence -> 适配”；
   content profile 提供来源与体裁 policy，学习对象类型与目标能力共同决定具体 TeachingMove。
   GuidedInquiryPolicy 只影响何时优先让 Learner 先执行目标认知活动；
   PresentationFrame 只影响动作怎样呈现，不参与知识真相、判题或 Evidence。
   算法引导、论证重建、概念辨析、事实检索、程序练习和文本解释只是不同策略，不共享固定步骤。
```

边界:
- `ReasoningEpisode` 只服务确有论证链的目标，不升级成所有教学对象的共同基类。
- `ReasoningPattern` 可表达因果、证据—主张、分类、程序或求解等可复用结构，但只是 `TeachingMap` 中一种可选资产。
- “每次只给一个方向”泛化为“每次只交付一个有界、目标匹配的 TeachingMove”；方向可以是问题、线索、对比、例子、反例、来源聚焦、关系支架或局部解释，不一定要求 Learner 继续推导。
- 问句只是 `TutorPresentation` 的表面形式：教学支架、诊断探测、主动回忆、正式判题与修辞过渡必须保留不同目的和 Evidence 语义，只有确实需要判题时才冻结 `AssessmentContract`。
- `GuidedInquiryPolicy` 只在引导学习意图、目标认知活动适合且 Learner 具备必要前置时优先促使 Learner 生成、辨析、预测或重建；它不要求所有新事实都由 Learner 猜出。
- `PresentationFrame` 可以表达沟通风格、Tutor voice、人物设定、关系语境与模态，但不得添加未记录提示、改变来源主张、修改 assistance/判题或用人物关注、好感、失望和参与时长生成能力证据。
- 用户明确要求直接答案、总结、翻译或完整解释时，`interaction_intent` 优先；系统不能强迫所有阅读进入苏格拉底式教学。
- Q83/Q84 继续只是可选交互与可靠判题基础设施，不定义教学内容和节奏。
- object kind、target capability、TeachingMove 与 PresentationFrame 的正式枚举、组合合法性和 policy override 顺序留给确认后的后续 Grill。

**问题**: 是否确认采用修订 C+：`TutorLoop` 只共享“识别交互意图与目标认知活动 → 按 content profile、learning object kind、target capability、Learner 状态和来源条件选择一个有界 `TeachingMove` → 通过可选 `PresentationFrame` 渲染并交付 → 观察 → 形成对应轴 Evidence → 适配”的控制骨架；`GuidedInquiryPolicy` 只是引导学习时的动作选择策略，人物、语气、陪伴和共同目标只是呈现框架，二者都不得成为全局教学步骤或改写来源、assistance、判题与 Evidence？

**我的推荐**: 是。它保留“不要用答案替代学习活动”，同时把认知动作与体验呈现解耦：前者由对象、能力目标和状态决定，后者可以提供稳定而可选的陪伴感，却不能污染教学真相和能力证据；整体仍与 Profile Plugin Framework 的“机制稳定、策略可换”保持同构。

**用户答案**: 同意。

**锁定结果**: `TutorLoop` 只拥有“目标 → 有界 `TeachingMove` → `TutorPresentation` → 显式 Learner 行为 → 对应轴 `LearningEvidence` → 适配”的稳定控制骨架，不规定全局教学步骤。`GuidedInquiryPolicy` 是明确引导学习意图下、结合 content profile、learning object kind、target capability、Learner 状态与来源条件优先促使 Learner 执行目标认知活动的动作选择策略，不等于全程提问、content profile 或评分模式；显式直接答案、总结、翻译与完整解释请求仍优先。已采用动作再通过可选 `PresentationFrame` 渲染为实际 `TutorPresentation`；人物、语气、陪伴、共同目标与模态只能改变呈现方式，不得改变目标认知活动、来源主张、assistance 暴露、`AssessmentContract` 或 `LearningEvidence`，人物关系、情绪反应、停留时长与参与度本身不得成为对象级能力证据。问句只是表面形式，教学支架、诊断、主动回忆和正式判题必须保留不同 Evidence 语义；Q83/Q84 继续只提供可选结构化交互与可靠判题基础设施。正式 enum、组合合法性、policy 优先级、assistance 升级、复习调度与关系体验的产品安全细则留给后续 Grill。

---

## Grill Q86 - 教学意图采用回合覆盖、会话默认与长期偏好的分层优先级

**本轮状态**: `CONFIRMED`。

**已锁定前提**:
- Q85：`GuidedInquiryPolicy` 只有在交互意图为引导学习时才参与 `TeachingMove` 选择；显式直接答案、总结、翻译与完整解释请求优先。
- `PresentationFrame` 与教学意图分层：喜欢某个人物或语气不等于授权系统把所有请求改成苏格拉底式教学。
- “苏七”案例同时需要两种作用域：课程层长期采用问题引导，但 Learner 在具体一步仍可连续提出问题并获得直接解释。

**本仓库核对**:
- `TaskNeed` 是 Runtime 为工具发现冻结的单回合路由输入，`CONTEXT.md` 已明确它不是持久用户意图；不能把 Tutor 的跨回合教学姿态塞进 `TaskNeed`。
- `ProfilePayload::ExplanationPreference` 已支持 Global/Book scope、content-profile/domain applicability、UserStated/AgentInferred 来源和确认/过期治理，适合保存“通常喜欢怎样讲”的长期弱默认。
- 当前 `ProfileScope` 只有 Global 与 Book，没有 Session；Reader 虽有 session overlay/mode，但它们服务布局或论文视图，不是通用 Tutor 教学会话合同。
- 因此“当前这一问要什么”“这段学习会话默认怎样教”“用户通常偏好怎样讲”目前是三个不同生命周期，不能由一条可覆盖的 preference 同时拥有。

三个方向:

```text
A. 一个长期全局偏好拥有教学模式
   用户一旦选择“苏格拉底式”，后续所有问题都默认反问。
   连续性最强，但会把普通问答、翻译和直接求解强行改造成教学会话，
   也无法表达“这一章引导学习，但这一处请直接解释”。

B. 每回合由 Agent 重新推断，不保存会话默认
   不需要新会话状态；但相邻回合会在讲解、追问和测验间漂移，
   用户必须反复重申“不要直接给答案”或“这一处直接告诉我”。

C. 回合显式意图 > 会话默认 > 已确认长期偏好（推荐）
   当前回合明确表达的需求拥有最高优先级；
   用户显式开启的临时教学会话模式只为后续含糊教学回合提供默认；
   已确认 ProfileFact 只在教学语境已经成立且前两层沉默时作弱默认；
   三层都没有足够信号时保持中性响应，不擅自进入连续引导或测验。
```

C 的候选形态:

```text
resolve_interaction_intent(
  explicit_current_turn,
  active_tutor_session_mode?,
  confirmed_explanation_preference?,
  bounded_request_inference
) -> ResolvedInteractionIntent

priority:
  explicit current turn
    > active explicit session mode
    > confirmed preference, only inside an established teaching context
    > neutral bounded fallback
```

边界:
- “这一处直接解释”只覆盖当前回合，不自动关闭仍有效的引导学习会话；“接下来都直接讲”才改变会话默认。
- `TutorSessionMode`（暂名）必须由用户显式开启、切换或结束；Agent 可以建议，不能因一次答题、沉默、停留时间或画像推断而悄悄切换。
- Agent 推断的讲解偏好继续走现有 ProfileFact 待确认治理；未确认推断不能激活 `GuidedInquiryPolicy`。
- 长期 `ExplanationPreference` 只能在已经确定为教学语境时帮助选默认姿态，不得把普通查问、总结、翻译或直接求解变成多回合教学。
- 每回合实际采用的 `ResolvedInteractionIntent`、覆盖来源与有效会话模式进入 `InteractionTrace`，但都不是能力 Evidence。
- 会话状态具体存在哪里、模式的正式 enum、何时自然失效和 UI 怎样展示留给确认后的后续 Grill；本问只锁定所有权与优先级。

**问题**: 是否确认采用 C：TutorLoop 以“当前回合显式意图 > 用户显式开启的 Tutor 会话默认 > 已确认长期讲解偏好 > 中性兜底”解析教学意图；当前回合可以临时覆盖会话姿态而不改写它，长期偏好只在教学语境已成立时作为弱默认，Agent 不得依据沉默、停留时间、一次表现或未确认画像推断擅自开启或切换引导学习？

**我的推荐**: 是。它既保留“苏七”那种一门课内稳定的引导体验，也保留 Learner 随时说“这里直接告诉我”的局部控制权；最重要的是，长期偏好、临时会话合同和当前请求不再互相覆盖成一份含混真相。

**用户答案**: 同意。

**锁定结果**: `TutorLoop` 以“当前回合显式意图 → 用户显式建立的 `TutorSessionMode` → 已确认长期 `ExplanationPreference` → 中性兜底”解析每回合 `ResolvedInteractionIntent`。当前回合明确要求直接答案、总结、翻译、完整解释、引导学习、复习或探测时拥有最高优先级；一次局部覆盖不自动改写仍有效的会话默认，只有“接下来都……”这类显式范围变更才切换会话姿态。`TutorSessionMode` 必须由用户显式开启、切换或结束，Agent 只能建议；长期讲解偏好只在教学语境已经成立且前两层沉默时作为弱默认，未确认 Agent 推断不得激活 `GuidedInquiryPolicy`。沉默、停留时间、一次表现和画像推断都无权擅自开启或切换教学模式。实际解析结果、生效来源和有效会话默认进入 `InteractionTrace` 供回放，但都不是能力 Evidence，也不得复用单回合工具路由 `TaskNeed` 充当教学意图或会话状态。具体会话状态所有权、正式模式集合、持久化与自然失效规则留给后续 Grill。

---

## Grill Q87 - TutorSession 采用独立、可回放的私人教学会话状态

**本轮状态**: `CONFIRMED`。

**已锁定前提**:
- Q69/Q70：私人 `PathInstance` 在运行时组装，`PathProgress` 保存当前路径的情境化短期进度；二者不修改公共 `TeachingMap`，也不自动等于长期掌握。
- Q71/Q81/Q82：`LearningMemory` 保存私人可回放的 Trace/Evidence/State；Agent 对话历史只负责 transcript，不能拥有学习事实或状态真相。
- Q86：`TutorSessionMode` 是用户显式建立的临时教学默认，不是长期 `ExplanationPreference`，也不是单回合 `ResolvedInteractionIntent`。

**本仓库核对**:
- `AgentChatTurn` 以一次 `/agent/chat` 请求为持久化单元，失败回合、跨回合响应和后续 effect 处置都不能仅靠 transcript 完整回放；Q82 已明确聊天历史不是学习真相。
- 现有“读时会话边界”由用户显式控制，idle 只触发 review，不自动创建新对话；聊天 context compression 也是审核边界，不能暗中改变教学合同。
- Reader 的 session overlay/mode 保存 viewport、布局、论文 minimap 等界面状态；它不拥有教学目标、认知路径或讲解姿态。
- `ProfileFact` 只有 Global/Book scope，适合稳定背景、目标和弱偏好；把一次课程的临时模式写成 ProfileFact 会使它越过当前目标长期生效。
- Q82/Q70 已在领域设计中确定由 `InteractionTrace` 保存用户显式动作、由 `PathProgress` 表达可重建短期进度；这为教学会话事件与投影提供设计基础，二者尚非现有可执行能力。

三个方向:

```text
A. TutorSessionMode 绑定 Agent chat session
   建新聊天就重置，聊天删除或迁移也带走模式。
   实现最省；但课程可能跨聊天、重启和 context compression，
   教学合同会被 UI/transcript 生命周期意外改变。

B. TutorSessionMode 写成 Book/Global ProfileFact
   跨聊天和应用重启延续最直接。
   但临时会话姿态会伪装成长期偏好，难表达暂停、恢复、局部目标和历史 revision，
   也会使“这次引导学习”污染以后对同一本书的普通查问。

C. 独立 TutorSession + 可回放生命周期（推荐）
   TutorSession 是 LearningMemory 内的私人教学会话状态，绑定一个有界学习目标/范围，
   引用当前 PathInstance/PathProgress，并由显式生命周期事件重建当前状态；
   Agent chat、Reader UI 和 ProfileFact 只引用它，不拥有它。
```

C 的候选形态:

```text
TutorSession {
  tutor_session_id
  goal_scope
  status                  // candidate only: active | paused | ended
  current_mode
  current_path_instance_ref?
  revision
}

TutorSessionStarted
  -> TutorSessionModeChanged*
  -> TutorSessionPaused / TutorSessionResumed*
  -> TutorSessionEnded

current TutorSession
  = projection(events through revision)
```

边界:
- 开启、切换、暂停、恢复与结束都是用户显式动作或由用户确认的 Agent 提议；Agent 不得因 idle、答错、停留时间、重启或 context compression 自动产生模式变更事件。
- 当前回合的 `ResolvedInteractionIntent` 只引用有效 `tutor_session_id + revision` 并可临时覆盖 mode，不修改会话投影。
- TutorSession 可跨 Agent chat 和应用重启恢复；聊天 transcript 的删除、压缩或迁移不应暗中改变会话状态，具体删除传播留给隐私/保留策略 Grill。
- 目标完成时 Agent 可以提议结束或切换，但“模型认为已经学会”不能自动结束会话；用户必须确认。
- 会话范围外的普通查问不得继承该 mode；scope 的正式形态、并发会话规则、事件 schema、存储表和 UI 留给确认后的后续 Grill。
- `TutorSession` 是 `LearningMemory` 内部私人状态，不新增第四个顶层系统，也不属于 `LearningEvidence` 或 `LearnerKnowledgeState`。

**问题**: 是否确认采用 C：建立独立、绑定有界学习目标/范围且可由生命周期事件重建的私人 `TutorSession`，由 `LearningMemory` 唯一拥有当前 mode、关联 PathInstance/PathProgress 与 revision；Agent chat、Reader session 和 ProfileFact 只能引用它，不能拥有或隐式改写它，并且 idle、重启、context compression、一次表现或模型判断都不得自动切换或结束会话？

**我的推荐**: 是。这样“这一门课继续采用引导学习”可以跨聊天和重启稳定恢复，同时又不会膨胀成长期全局偏好；所有模式变化都有用户动作、revision 和可回放因果来源。

**用户答案**: 同意（2026-09-07）。

**锁定结果**: `TutorSession` 是 `LearningMemory` 独立持有的私人教学会话，绑定有界学习目标/范围，拥有默认教法与会话生命周期，并引用当前 `PathInstance` / `PathProgress`；其当前状态由显式生命周期事件按 revision 重建。聊天、Reader 与 ProfileFact 只引用它，会话可跨聊天与应用重启延续；开启、暂停、恢复、切换和结束由用户显式动作或用户确认的提议触发，idle、聊天压缩、一次表现或模型判断均不能暗中改变它。当前回合可以覆盖教法而不改写会话默认，范围外查问不继承它。2026-09-29 已收敛范围、单一当前会话、全局开关及持久化边界，见 [ADR-0118](adr/0118-learning-memory-owned-replayable-tutor-session.md)、[ADR-0141](adr/0141-global-tutor-control-and-session-ownership.md) 与 [切片方案 §3](切片方案-Tutor全局模式与教学闭环.md#3-全局控制与教学会话)。

---

## Grill Q88 - 来源推论未闭合时的教学推进

**决策**: 来源不足时围绕缺口带读，暂停依赖它的判定。

**否决**:
- 来源不足就停止整个教学：寻找证据、检查前提和比较解释本身可推进理解。
- 将可能解释教成结论：引用合法不证明推论成立，假设不能成为既定答案。
- 暂停本轮全部学习判断：具有独立评价依据的证据辨析等表现仍可形成 Evidence。

**命门**: 先补查直接相关来源；仍未闭合时保留已支持事实，围绕缺口继续带读，必要补充明确作为假设。只暂停依赖争议推论的正确性与掌握判断；实际交付内容、帮助与回应照常进入 `InteractionTrace`。
**何时回头**: 新来源或独立规则足以支持或反驳该推论时，重新评估受影响的判断。
**展开**: Q80 的结构/语义职责分离、Q73/Q75 的作者论证缺口与 Q84 的题目判定/Evidence 分层共同约束本规则。
**用户答案**: 同意（2026-09-07）；本轮状态 `CONFIRMED`。

---

## Grill Q89 - 教学会话目标与参考材料范围

**本轮状态**: `CONFIRMED`，2026-09-29；目标、焦点、材料范围及消息关联已收敛。
**已确认方向**: 以用户的会话学习意图及明确约束划定范围，参考材料单独记录；Agent 可在此范围内补查、补讲并调整当前学习焦点，最终目标、要求达到的能力深度或持续学习任务发生变化时，由用户表达或接受方向变更。
**收敛结果**: 会话分列用户意图/约束、Agent 当前焦点、带来源和角色的材料范围；消息预提交固定会话与控制 revision，范围外请求沿普通行为，临时参考其他材料不改目标。首版一个当前会话，显式切换暂停原会话；见 [切片方案 §3](切片方案-Tutor全局模式与教学闭环.md#3-全局控制与教学会话)。

---

## Grill Q90 - Agent 原生学习环境的上位分工

**决策**: 系统提供原生学习环境，Agent 作教学选择。

**否决**:
- 系统穷举用户表达并固定编排教学：无法覆盖开放意图与现场学习差异。
- 只提供工具名称或角色说明：Agent 缺少当前状态、对象关系与行动反馈。

**命门**: 环境提供真实状态、对象语义、可发现且可执行的能力与实际结果反馈；Agent 结合目标和环境决定具体教学动作。TeachingMap、LearningMemory 与 TutorLoop 保持原有所有权，事实与判断分开，持久写入与副作用由系统落实，环境不成为第四份可编辑真相。
**何时回头**: 真实交互表明某类教学选择必须增加明确策略支持时，限定补充该策略，不把所有教学改成固定流程。
**展开**: [ADR-0119](docs/adr/0119-agent-native-learning-environment-and-teaching-agency.md)。后续先明确 Agent 的环境观察、能力发现、动作与反馈，再回到 Q89 的范围细节。
**用户答案**: 对（2026-09-07）；本轮状态 `CONFIRMED`。

---

## Grill Q91 - 每回合自动提供环境说明与当前现场

**决策**: 稳定环境说明与精简当前现场自动提供，相关细节按需读取。

**否决**:
- 让 Agent 每回合从工具目录重新猜测现场：已知的当前状态和操作语义应由环境主动提供。
- 把全部历史与资产塞入每回合：当前决策只需要相关现场及可继续查询的入口。

**命门**: 自动观察包含当前材料、位置、选区与用户指代，有效 TutorSession 的目标和默认教法，最近实际交付的教学动作、已给帮助、用户回应与待处理事项，相关学习观察、派生判断及依据，以及可用资料、历史和操作的查询入口。系统组织已有事实并保留未知；Agent 负责理解语义和选择教学动作。判断学习表现时必须取得对应的实际呈现、帮助条件与用户回应，不能只凭概括性历史摘要；具体原文随回合附带还是按需读取继续细化。
**何时回头**: 真实回合持续缺少决策所需事实，或无关信息挤占当前现场时，调整自动提供与按需读取的边界。
**展开**: Q90 的环境分工落到回合观察；观察来自既有权威状态，LearnerContext 只是其中的私人学习状态视图。2026-09-29 的 [切片方案 §9](切片方案-Tutor全局模式与教学闭环.md#9-resident-中的-tutorloop) 已明确自动观察与按需读取边界，具体字段与预算由 T7/T12 实施固定。
**用户答案**: 同意（2026-09-07）；本轮状态 `CONFIRMED`。

---

## Grill Q92 - 模糊目标下的学习入口

**决策**: 以暂定目标开始，在学习中澄清需求与教法。

**否决**:
- 先完成目标、水平与教法配置才开始：用户可能无法预先描述需求或诊断困难。
- 把起步推断固化为用户画像：当前教学所用判断应能随真实回应修正。

**命门**: 正式学习所需预构建已就绪且用户表达开始或继续学习后，Agent 依据当前材料与已知历史提出可修正的暂定学习目标，直接组织一个有帮助的起步动作，在实际互动中逐步澄清需求、观察基础并调整局部讲法；用户可随时纠偏，完整画像与细化目标不作为开始的前置条件。
**何时回头**: 起步动作持续无法带来理解或暴露需求时，缩小动作或增加真正影响方向的澄清。
**展开**: [Apple Onboarding](https://developer.apple.com/design/human-interface-guidelines/onboarding) 支持通过使用逐步理解产品的入口原则；[Learning Styles: Concepts and Evidence](https://pubmed.ncbi.nlm.nih.gov/26162104/) 未找到足够证据支持仅按学习风格标签匹配教法。既有 Q91 提供当前现场与相关历史，Q83/Q85 区分实际呈现、教学动作与表现依据；Zotero document-worker 的结构文本、文档动作和结果反馈可作环境参考，未提供相应学习入口设计。
**用户答案**: 同意（2026-09-07）；本轮状态 `CONFIRMED`。
**后续结果**: Q93 已明确用户会话意图与 Agent 当前学习焦点的分工，以及局部调整与方向变更的区别；Q94 明确正式学习以预构建就绪为前提。具体状态与动作表达继续设计。

---

## Grill Q93 - 会话意图与当前学习焦点

**决策**: 会话保留用户意图，Agent 在其范围内调焦点。

**否决**:
- 每次补背景、换例子或换参考材料都重新确认目标：局部教学选择应能连贯推进。
- 用 Agent 的当前推断覆写用户意图：教学焦点变化不能替用户决定最终要完成的事。

**命门**: TutorSession 保留用户想完成的事及其明确约束，允许最初表达模糊；Agent 在此范围内调整暂定学习目标、必要背景、局部讲法与参考材料。用户表达与 Agent 当前解释保持可区分，学习意图与材料范围分别表达；改变最终目标、要求达到的能力深度或新增持续学习任务时，由用户表达或接受方向变更，用户已经明确表达的新方向直接采用。
**何时回头**: 真实教学中局部补讲持续超出原意图或约束时，将超出的学习方向作为新提议处理。
**展开**: `packages/core/src/build-intent.ts` 已区分 `user_goal` 与 `source_scope`，但其确认状态服务构建计划；ADR-0093 的昂贵构建确认不充当教学焦点调整规则。ADR-0118/Q87 确立会话所有权和显式生命周期，Q92 允许暂定目标入口；Zotero document-worker 的结构文本和文档动作反馈没有相应教学目标模型。
**用户答案**: 同意（2026-09-07）；本轮状态 `CONFIRMED`。
**后续收敛**: 2026-09-29 由 [ADR-0141](adr/0141-global-tutor-control-and-session-ownership.md) 与 [切片方案 §3](切片方案-Tutor全局模式与教学闭环.md#3-全局控制与教学会话) 明确范围、回合关联、显式操作及会话事件。

---

## Grill Q94 - 正式学习对象尚未就绪时的教学入口

**决策**: 可先进入阅读器，正式学习须预构建就绪。

**否决**:
- 预构建完成前一直阻止进入阅读器：等待会造成产品损失，可信原文可用后应允许先阅读。
- 仅凭来源锚点启动正式教学：正式学习需要预构建教学资产，原文可读不等于教学就绪。

**命门**: 阅读器可进入与正式学习就绪分别表达；可信来源满足既有阅读门槛后允许进入阅读器，正式 TutorLoop 以所需预构建完成并达到就绪要求为前提。Q79/Q80/Q85 对正式 LearningObjectRef 的要求保持成立；Q92 的暂定目标入口适用于正式学习已就绪时，完整画像与细化目标不作额外前置条件。
**何时回头**: 正式学习所需资产或支持范围发生变化时，重新界定就绪条件。
**展开**: `packages/core/src/build-workbench.ts`、`crates/server/src/lib.rs` 与 ADR-0093 允许可信基础层就绪后先阅读；该入口不证明正式教学能力已就绪。Zotero document-worker 的结构文本与渲染入口可作阅读能力参考，没有相应正式教学就绪规则。
**用户答案**: 更正为不同意原提议（2026-09-07）：允许先进入是为减少预构建等待造成的产品损失，正式学习必须依赖预构建；本轮按此更正为 `CONFIRMED`。
**后续收敛**: 首版就绪单位按 Q95；2026-09-29 由 [ADR-0142](adr/0142-grounded-teaching-map-and-whole-source-readiness.md) 确定四类必需资产与验收。全局开关开启但资产未就绪时显示准备状态，保留普通阅读/问答，见 [切片方案](切片方案-Tutor全局模式与教学闭环.md)。

---

## Grill Q95 - 正式学习的预构建就绪单位

**决策**: 首版按整本书或整篇论文开放正式学习。

**否决**:
- 某一段或章节资产就绪即开放正式学习：首版需要整份材料的公共教学基座支持起点选择、前置关系与跨章节联系。
- 等所有个性化路径和具体问题预先生成：这些内容由读时 Agent 结合用户现场选择或合成。

**命门**: 整本书或整篇论文满足正式学习所需的公共教学基座要求后，才开放 TutorLoop；准备期间仍按 Q94 允许先进入阅读器。个性化路径、具体问题与提示按 Q80/Q85 在读时选择或合成，用户无需预先提供完整画像或细化目标。
**何时回头**: 产品明确支持独立章节作为正式学习单元时，重新评估就绪单位。
**展开**: [ADR-0120](docs/adr/0120-whole-source-prebuild-gate-for-formal-learning.md)。`BuildSourceScope` 已支持 whole_book 与局部 LID/section，但构建范围表达不等于正式学习就绪单位；Zotero document-worker 的结构文本生成和打包入口没有对应教学就绪规则。
**用户答案**: 同意（2026-09-07）；本轮状态 `CONFIRMED`。
**后续收敛**: 必需产物集合、覆盖回执、独立来源抽样和发布条件已由 [ADR-0142](adr/0142-grounded-teaching-map-and-whole-source-readiness.md) 与 [切片方案 §7](切片方案-Tutor全局模式与教学闭环.md#7-构建接入质量与版本发布) 承接。

---

## Grill Q96 - 正式学习必需的公共教学基座

**本轮状态**: `CONFIRMED`，2026-09-29；四类基座及预构建方法随完整方案获准落档。
**确认方向**: 正式学习的公共基座需要四类信息：可信原文及来源锚点；覆盖整份材料的结构、主题线与重点入口；正式学习对象及有依据的对象关系；围绕预构建重点形成、可追溯到来源的公共认知素材。它们共同支持 Agent 定位依据、理解全局、识别学习对象并展开重点内容。
**素材边界**: 论证内容可用 ReasoningEpisode，定义边界、比较框架、因果关系或方法步骤等按内容采用适用的 ReasoningPattern 与局部片段；来源缺口按 Q75/Q88 显式保留。全书就绪单位按 Q95，重点入口与其他目标按需展开仍承接 Q74；个性化路径、具体问题与提示按 Q80/Q85 在读时组织。
**查证依据**: `book-structure.ts` 已有 spine、throughlines、key_stops、depends_on 与 evidence_lids，`zod.ts` 的 Pass2 关系已有 prerequisite、supports、contrasts 等候选语义；它们不能直接替代 Q79 的正式对象身份。Q69/Q73/Q74/Q75 已界定公共认知素材的复用、来源和展开边界，`build-workbench.ts` 尚未表达上述正式学习就绪合同。Zotero document-worker 提供结构文本、outline 与渲染入口，没有学习对象或认知素材的对应模型。
**落档**: [ADR-0142](adr/0142-grounded-teaching-map-and-whole-source-readiness.md)；[切片方案 §5–7](切片方案-Tutor全局模式与教学闭环.md#5-正式对象与关系构造) 明确对象合同、重点素材、覆盖义务、来源抽样与版本发布，T3–T5 待实施。

---

## Grill Q97 - 正式学习对象与 User Understanding Space 的共同锚点

**决策**: 用户理解空间以公共对象及关键关系为参照。

**否决**:
- 只记录概念的单项掌握结果：会漏掉关系理解、个人解释和可能的混淆。
- 将私人解释或系统猜测当成公共知识：用户理解与材料事实各有来源和所有权。

**命门**: User Understanding Space 是 LearningMemory 内有证据、可修正的私人理解视图，涵盖对对象及可学习关键关系的理解、实际表现、个人解释与有依据的理解假设，以公共正式学习对象为共同参照。承载独立理解内容的关键关系可获得正式学习对象引用；用户原话与系统推断分开，无证据保持未知，不通过公共关系自动推导用户掌握前置知识。
**何时回头**: 真实学习中存在无法通过公共对象及相关私人解释表达的重要理解差异时，重新评估私人视图的表达范围。
**展开**: `zod.ts` 的 Pass1 图只有 entity/concept/claim，`merge.ts` 按 node.id 合并；Pass2 audit 已有关系类型、端点证据和支持层级，但未形成正式学习对象身份。ADR-0097 的 book.concept 只召回候选并要求回读原文；Q79/Q81/Q84 已分别确定对象晋升、私人证据链和判题职责。Zotero document-worker 的引用、图表与 outline 构造只提供文档结构联系。ETS 的 [Evidence-Centered Design for Learning](https://www.ets.org/research/policy_research_reports/publications/report/2011/imbu.html) 和 [Assessment as Evidential Reasoning](https://www.ets.org/Media/Research/pdf/gorin_assessment_evidential_reasoning.pdf) 提供学习者判断、观察证据与活动设计相互对齐的参考。
**用户答案**: 同意（2026-09-07）；本轮状态 `CONFIRMED`，确认用户空间的表达范围和共同参照。
**后续收敛**: Q96 与 Q101 已确认；发现/归并、关系身份、跨材料对应及用户理解空间已由 [ADR-0142](adr/0142-grounded-teaching-map-and-whole-source-readiness.md)、[ADR-0143](adr/0143-teaching-trace-assessment-and-learning-evidence.md) 承接。首版按来源保留身份，跨材料不凭名称迁移能力状态。

**对象构造进展**: Q98 确认对象粒度与能力维度的分工，Q99 确认复用公共语义资产、回读原文并定向补缺的主路线；2026-09-29 按含义和条件进行同义归并、异义拆分，系统接纳后分配稳定身份，名称相同或文本相近不能直接决定同一对象。
**关系构造（已收敛）**: 由定义、论证、方法步骤和跨节连接提出有来源依据的候选关系，明确端点、角色、关系含义与成立条件；区分材料中的知识关系、目标相关的教学依赖和用户实际表现出的理解关联，导航边与出现顺序无需一律对象化。

---

## Grill Q98 - 正式学习对象的粒度与拆分依据

**决策**: 按内容含义与需分别追踪的理解划分对象。

**否决**:
- 按题目、教法或帮助条件创建对象：会把同一内容的能力与活动差异固化为不同身份。
- 将章节主题直接视为单项掌握对象：会掩盖其中需要分别理解的内容。

**命门**: 对象须有来源依据、可独立讨论，按需要分别追踪且影响教学选择的不同理解内容拆分；同一内容的识别、解释、应用留在能力维度。允许复合对象与子对象并存，对象与活动不要求一对一，公共身份不随私人表现改变，包含关系本身不推出整体或局部掌握。
**何时回头**: 真实教学持续无法在现有对象与能力维度上表达重要理解差异时，重新评估相应内容边界。
**展开**: “样本均值”是对象，计算、解释和选用分别承接能力证据；“均值与中位数对异常值的反应差异”可承载独立关系理解，具体数据属于活动材料。Q78 已锁定多轴 Evidence，Q79 留下拆分标准；`agents/pass1-local-extractor.md` 与 `book-structure.ts` 分别提供候选和结构入口。Zotero document-worker 的 `src/pdf/structure/structure.js` 提供文档联系，未定义学习粒度。
**用户答案**: 同意（2026-09-07）；本轮状态 `CONFIRMED`。
**后续收敛**: 候选发现、语义整理、关系身份、来源接纳与版本处理按 [切片方案 §5](切片方案-Tutor全局模式与教学闭环.md#5-正式对象与关系构造)；Q96 已确认。

---

## Grill Q99 - 正式对象构建对已有公共语义资产的复用

**决策**: 复用公共语义资产，回读原文并定向补缺。

**否决**:
- 将现有条目逐条机械晋升：已有抽取未完成正式对象的粒度、语义和来源判断。
- 默认重新执行全部语义抽取：已有公共分析可以承担候选发现与全书组织。

**命门**: 主要复用 Pass1、profile sidecar 的 discourse 与 formula semantics、BookStructure 及按计划启用的 Pass2；沿锚点回读必要原文，按 Q98 整理对象与关系，缺少必要内容时定向补充，再按 Q79 晋升。Agent 负责语义判断，系统负责来源引用、结构合同与身份分配；公共产物提供用户理解空间的参照，私人理解由实际互动与学习证据形成。
**何时回头**: 已有公共产物无法提供必要候选或系统性遗漏内容时，重新评估对应抽取职责与覆盖范围。
**展开**: `agents/profile-sidecar-extractor.md`、`profile-sidecar-build.ts`、`book-structure.ts` 与 `merge.ts` 分别界定已有语义产物和合并职责；Zotero document-worker 的 `src/pdf/structure/structure.js` 提供文档联系。Pass2 按 ADR-0098 保持构建计划内的可选增强，未启用时由其他公共资产与定向原文查找支撑必要关系。
**用户答案**: 同意（2026-09-07）；本轮状态 `CONFIRMED`。
**后续收敛**: [切片方案 §5–7](切片方案-Tutor全局模式与教学闭环.md#5-正式对象与关系构造) 确定对象整理、重点认知素材与质量发布三项构建职责，接入现有 BuildPlan/DAG/预算/恢复；Q96 已确认。

---

## Grill Q100 - 目标能力与条件约束下的教学前置依赖

**决策**: 前置依赖注明能力与条件，教学顺序读时决定。

**否决**:
- 仅凭对象相连或书中顺序规定先学后学：无法区分直观理解、应用与独立推导的前置需求。
- 将证据未知视为必须重学：缺少历史记录不能证明用户欠缺相应能力。

**命门**: 公共教学前置依赖说明目标对象及能力、所需前置对象及能力、适用条件和依据；读时 Agent 结合当前目标、私人学习证据与可提供的帮助选择实际教学顺序。“这个用户现在需要补什么”是可修正的私人教学判断，不直接改写公共关系。
**何时回头**: 真实教学中的前置需要持续无法通过对象、能力与适用条件表达时，重新评估依赖语义。
**展开**: 直观理解导数可以从图像和变化量开始，从极限定义独立推导则需要相关极限能力。`agents/pass2-longrange-linker.md:65`、`zod.ts` 的 Pass2AuditEdge 与 `book-structure.ts` 的 depends_on 尚未独立表达这些能力要求；Zotero document-worker 的结构引用与 outline 没有相应私人教学模型。Q78/Q97 确定多轴证据与未知边界，Q90 确定 Agent 的教学选择职责。
**用户答案**: 同意（2026-09-07）；本轮状态 `CONFIRMED`。
**后续收敛**: [切片方案 §5](切片方案-Tutor全局模式与教学闭环.md#5-正式对象与关系构造) 明确目标能力、前置能力、条件及依据字段，关系身份按 Q101，覆盖按 §7。

---

## Grill Q101 - 可学习关系的正式身份与图中表达

**本轮状态**: `CONFIRMED`，2026-09-29；随完整方案获准落档。
**确认方向**: 承载独立理解内容的关系使用既有 `LearningObjectRef`，由该正式对象统一拥有具体关系含义、参与对象及其角色、成立条件与来源依据；`TeachingMap` 中相应的图连线通过引用或投影表达该对象，私人理解证据也引用同一正式对象。
**晋升边界**: Pass1、Pass2 和 sidecar 中的现有关系继续作为候选与来源线索；导航或结构联系无需一律成为学习对象。关系候选与已有正式断言表达同一内容时，经过语义确认复用已有身份，候选来自节点还是边不决定正式身份。
**身份边界**: 两端对象和关系类型相同，不能直接证明两项关系具有相同含义；具体内容与条件须参与语义判断。图中相应连线不另行持有一份可独立改写的关系含义，关系学习证据也不自动推出端点对象的全部能力状态。
**例子**: 均值与中位数的计算规则差异，以及它们对异常值的反应差异，可以承载需分别追踪的理解；一个笼统的 contrasts 标签不足以区分这些内容。用户对其中某项关系的解释证据应指向该具体关系对象。
**查证依据**: `zod.ts` 的 GraphEdge 只有端点、类型、方向、scope 与 weight，`merge.ts:40` 的 edgeKey 按端点、类型和方向去重；CONTEXT 中这些边的现有读时职责是召回路标。FormulaSemantics 的 composition 已分别表达 meaning、terms 与 evidence_lids，但没有正式学习身份。Zotero document-worker 的引用、图表联系与 outline 同样服务文档联系。Q79/Q97/Q98 已确立稳定引用、可学习关系与对象粒度，本轮进一步明确晋升后关系内容的所有权和图中表达。
**落档**: [ADR-0142](adr/0142-grounded-teaching-map-and-whole-source-readiness.md) 与 [切片方案 §5](切片方案-Tutor全局模式与教学闭环.md#5-正式对象与关系构造)；T3–T5 承担实现，历史证据不会因对象拆并静默改绑。

---

## Grill Q102 - 通用富呈现与读时内容制作

**本轮状态**：`CONFIRMED`，2026-09-16；RP1–RP7 已实现，正式教学 RP8 由 2026-09-29 的 [Tutor 切片方案](切片方案-Tutor全局模式与教学闭环.md) 承接。

**已确认范围**：可交互内容覆盖普通回答的重点组织、图文与比较，也覆盖交互讲解、教具及持续更新的内容。首版允许 Agent 在回答内容区域内遵循共同样式，现场编写、预览和修改 HTML/CSS/JavaScript；可复用组件提供共同基础，现场表达不受预置形态集合限制。

**选择题**：首版现场编写和修改 HTML/CSS/JavaScript，或先限定为组合预置组件。
**用户答案**：“前者”。此前用户同意落 ADR 和切片方案，并要求先补充普通回答呈现与 coding agent 能力的讨论。

**已说明取舍**：现场制作增加等待与执行需求；需运行、观察并修正，代码能运行不证明模型或推导正确；现有任意 UI 限制需明确修订。普通富回答独立于正式教学就绪，正式教学沿用呈现条件、类型化行为、判题与学习证据边界。

**落档**：[ADR-0130](docs/adr/0130-agent-rich-presentation-and-read-time-authoring.md)、[RP0–RP8 切片方案](docs/切片方案-Agent富呈现与读时内容制作.md)、CONTEXT 中 AgentPresentation、PresentationState、交互教具及教学呈现定义。

**修订关系**：Q83 对任意生成 UI 的全面限制改为自由视觉与宿主行为语义分工；Q84/Q85 的教学判断边界继续成立。ADR-0094 的 Blueprint 保持数据型，采用独立呈现能力承载代码。Q101 后于 2026-09-29 随完整 Tutor 方案收敛。

**实施结果**：RP1–RP7 已完成预览宿主、现场制作、当前状态追问与重开恢复，证据见 [富呈现切片方案](切片方案-Agent富呈现与读时内容制作.md)。正式教学接入按 Tutor T6/T8–T12 推进，T13 完整验收。

---

## Tutor 设计收敛（2026-09-29）

**状态**：`CONFIRMED`，设计落档；实现见 [T0–T13](切片方案-Tutor全局模式与教学闭环.md)。

**全局交互**：Tutor 是应用级开关，由 LearningMemory 持有，跨 Reader、聊天、演示和重启延续；关闭暂停教学、保留进展，当前回合直接讲解不改开关。首版一个当前 TutorSession，用户意图、Agent 焦点和参考材料分列，显式切换/结束与页面导航分别表达。

**演示接入**：展开内容后在同一工作区继续原对话。提问冻结发送时的内容版本和现场，回答就地出现，恢复与新版本由用户显式选择；正式行为由宿主关联活动、确切呈现及已暴露帮助。

**预构建**：复用既有公共语义资产，整理正式对象及可学习关系，再围绕 key_stops 按缺失定义、前提、证据和连接构造认知素材。检索包含未连接但必要的 discourse 片段，有界预览后回读原文；真实来源缺口与预算不足分开。确定性合同/引用/覆盖检查加独立来源抽样，通过后发布整份 TeachingMap 版本。

**学习闭环**：实际呈现与显式行为进入追加 Trace；可判题活动先冻结合同，判定形成 ResponseAssessment；带精确依据的 LearningEvidence 再驱动路径与知识状态。帮助条件、能力轴和多次尝试保留，未知不自动变成不会；用户理解空间是私人投影。

**实施边界**：沿用现有 Resident、构建执行、富呈现与来源能力，顶层保持 TeachingMap、LearningMemory、TutorLoop。设计决定已收敛，字段细化与验证归对应切片；不再以历史候选状态阻止进入实施。

**ADR**：[0141](adr/0141-global-tutor-control-and-session-ownership.md)、[0142](adr/0142-grounded-teaching-map-and-whole-source-readiness.md)、[0143](adr/0143-teaching-trace-assessment-and-learning-evidence.md)、[0144](adr/0144-shared-presentation-conversation-workspace.md)。
