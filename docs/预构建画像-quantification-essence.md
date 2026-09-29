# quantification-essence 预构建画像

统计对象:`.understand-book/quantification-essence/source.txt`
统计时间:2026-07-04

## 书本规模

- book_id:`quantification-essence`
- 源文件大小:`335,861` bytes
- 源文本长度:`154,448` chars
- 行数:`2,911`
- LID nodes:`2,761`
- 叶子 LID:`2,625`
- 叶子类型:
  - paragraph:`1,699`
  - formula:`894`
  - code:`32`

## 窗口规模

- 窗口数:`46`
- 每窗叶子数:min `4` / p50 `64` / avg `57.1` / p90 `77` / max `80`
- 每窗正文字符:min `136` / p50 `2,996` / avg `3,257` / p90 `5,572` / max `10,276`
- Pass1 `[LID]` 输入字符:总 `185,398`,单窗 avg `4,030`

## Pass1 产物规模

- `.build/pass1/*.json`:`46` files,`322,339` bytes
- Pass1 原始抽取:
  - nodes before merge:`1,058`
  - edges before merge:`854`
- 最终 `base.json`:
  - size:`1,191,601` bytes
  - graph_nodes:`894`
  - graph node types:claim `370`,concept `509`,entity `15`
  - graph_edges:`1,325`
  - edge scopes:local `816`,long_range `509`

## PB6 Sidecar 产物规模

- `.build/profile-sidecar/*.json`:`46` files,`1,282,943` bytes
- 逐窗 artifact:
  - discourse items:`2,625`
  - formula semantics candidates:`687`
- batch 后正式产物:
  - `discourse_index.json`:`614,530` bytes,`2,582` items,dropped `45`
  - `formula_semantics.json`:`584,954` bytes,`686` items,pending `0`

## Pass2 产物规模

- long-range candidates:`2,053`
- candidate windows:`21`
- skipped windows:`25`
- 每候选窗 candidates:min `18` / p50 `79` / avg `97.8` / p90 `184` / max `308`
- 当前有效 `.build/pass2/*.json`:`21` files,`796,985` bytes
- 注意:`.build/pass2/` 目录中仍有旧残留 `12.json`、`13.json`;当前 `pass2-status` 将其对应窗口判为 skipped,不参与 batch。
- Pass2 分类结果:
  - accepted:`509`
  - pending:`119`
  - rejected:`1,425`
  - gate_dropped:`0`
- batch 后正式产物:
  - `long_range_candidates.json`:`1,142,092` bytes
  - `pass2_audit.json`:`697,504` bytes
  - `base.json` long_range_edges:`509`

## Token / 成本口径

精确 per-call token 没有落盘;artifacts 中没有 `usage` / `input_tokens` / `output_tokens` 字段。本轮 Codex goal 报告的总 token 是 `265,860`,但这是编排会话口径,不等于各 subagent API 的精确账单。

可复算代理值:

- Pass1 input chars:`185,398`,粗略 token-equivalent `~92.7k`
- PB6 sidecar input chars:`229,790`,粗略 `~114.9k`
- Pass2 packet input chars:`2,060,026`,粗略 `~1.03M`
- 输出 JSON bytes:
  - Pass1 artifacts:`322,339`
  - Sidecar artifacts:`1,282,943`
  - Pass2 effective artifacts:`796,985`

## 结论

这本书真正的成本重心在 Pass2。原因是 Pass2 packet 会携带 source text、source nodes、discourse/formula sidecar 投影和 candidate_targets;21 个候选窗口合计输入字符超过 `2.06M`。
