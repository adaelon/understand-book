# TO14 数学内容核对

本次独立判据是正时间两段行程的恒等式：

`(v1*t1 + v2*t2)/(t1+t2) - (v1+v2)/2 = (v1-v2)*(t1-t2)/(2*(t1+t2))`。

因此直接取算术平均成立当且仅当速度相等或时间相等。固定有理数算例及实际计算结果见 [math-oracle.json](math-oracle.json)，复算入口为 `evals/tool-usability/math-oracle.py`。10/10、用时1/2检测遗漏等速分支；10/20、用时1/1检测等时分支；60/30、用时1/2对应原题40而非45。

逐份读取最终交付和持久化 `move_selected.payload.move`，按上述条件核对，没有发现本批样本把等时写成无例外的必要条件。

| 样本 | 实际答案中的条件原话 | 持久化教学目标／动作 | 判定 |
|---|---|---|---|
| [baseline 1](runs/baseline-tutor-continue-1/result.json) | “两段用时相等时就可以”；给出一般加权式 | learning_focus 是“用时相等时可以”，expected_performance 解释45与40 | 充分条件正确；未断言唯一条件 |
| [baseline 2](runs/baseline-tutor-continue-2/result.json) | 原运行没有交付答案 | 目标是“在等时条件下为何成立”；prompt 的其他情况表述限定“遇到不同速度” | 目标正确；不能算交付成功 |
| [baseline 3](runs/baseline-tutor-continue-3/result.json) | “等时间能保证……；等路程本身不能保证” | prompt 明确“保证成立的条件，不把它说成唯一可能情况” | 正确 |
| [candidate 1](runs/candidate-tutor-continue-1/result.json) | “两段速度不同时……用时是否相等” | prompt 同样限定两段速度不同；目标要求解释等时充分条件 | 正确 |
| [candidate 2](runs/candidate-tutor-continue-2/result.json) | “相等时间允许……；相等路程本身不保证” | 目标解释等时成立及与等路程的区别 | 正确 |
| [candidate 3](runs/candidate-tutor-continue-3/result.json) | “两段速度不同时，用时相等才能……；如果两段速度本来就相同，用时不同也仍会得到那个相同的速度” | prompt 明确要求补等速分支 | 两分支均正确 |

五份真实交付的60/30等距算例为40，等时算例为45，计算正确。baseline 2 的 [原 CLI 最终输出](runs/baseline-tutor-continue-2/sample-02/metrics.json) 已返回一般加权式和“遇到不同速度时”的限定，但当时被执行器误丢弃；[无模型回放](diagnostic-replay/baseline-tutor-continue-2/result.json)完成了同一内容的交付。它只用于定位执行器问题，不补入真实成功分子。

## 已知限制

这份核对只判断已列出的数值、等时／等速条件及持久目标是否过度概括。教学顺序、读者是否理解、是否迁移到新题没有真人或独立语义评审验收；机器汇总中的 `semantic_assessment=needs_review` 因而保留。历史 TC2A 的错误证据仍成立，本批未重现不能解释为全局解决。
