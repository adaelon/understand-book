// Diagnostic expectations, not human calibration approval. Keep these labels
// outside each job's input so the judge receives only answers and source views.
export function sourceJudgmentProbes() {
  const citation = (highlighted_quote, context_after = '') => ({ id: 'C1', valid: true,
    highlighted_quote, context_before: '', context_after, view_status: 'complete' });
  const make = (id, answer, citations, support, location) => ({ id, expected: { support, location },
    job: { id, protocol: 'reading-task-quality-v2', kind: 'absolute', input: {
      question: '核对回答的来源支持与定位；来源只代表已展示的片段。', answerable: true,
      requirements: [
        { requirement_id: 'evidence:support', kind: 'source_support', evaluator: 'semantic', description: '论断由实际交付来源支持' },
        { requirement_id: 'evidence:location', kind: 'source_location', evaluator: 'semantic', description: '读者可由答案引用直接找到依据' },
      ], reference_evidence: [], candidates: [{ candidate: 'A', citations,
        answer_spans: answer.map((text, i) => ({ id: `a${i + 1}`, text })) }],
    } } });
  const formula = citation('本节把每件商品的净收益定义为售价减去成本。', '\n计算式为 $g=p-c$，p 为售价，c 为成本。');
  const storage = citation('样本应置于低于 5 度的环境，并避光保存。');
  const two = [citation('样本应置于低于 5 度的环境。'),
    { ...citation('样本应避光保存。'), id: 'C2' }];
  return [
    make('formula-only-after', ['净收益可写为 $g = p - c$，其中 p 是售价，c 是成本。[[source:C1]]'], [formula], 'met', 'met'),
    make('formula-wrong-sign', ['净收益为 $g=p+c$。[[source:C1]]'], [formula], 'unmet', 'unmet'),
    make('recap-inherits', ['保存要求是低于 5 度且避光。[[source:C1]]', '总结：低温、避光。'], [storage], 'met', 'met'),
    make('recap-adds-guarantee', ['保存要求是低于 5 度且避光。[[source:C1]]', '因此无论保存多久，样本都不会变质。'], [storage], 'unknown', 'unknown'),
    make('complementary-both-attached', ['保存要求是低于 5 度且避光。[[source:C1]][[source:C2]]'], two, 'met', 'met'),
    make('complementary-one-attached', ['保存要求是低于 5 度且避光。[[source:C1]]'], two, 'met', 'unmet'),
    make('absence-local', ['本次读到的这一段没有说明保存期限。[[source:C1]]'], [storage], 'met', 'met'),
    make('absence-whole-book', ['书中从未说明保存期限。[[source:C1]]'], [storage], 'unknown', 'unknown'),
    make('keyword-highlight-with-full-after', ['样本须低于 5 度并避光保存。[[source:C1]]'],
      [citation('保存', '要求是：样本应置于低于 5 度的环境，并避光保存。')], 'met', 'unmet'),
  ];
}
