# 忠实转写样例与短音频复核

来源：`cs336-2026-p2-pytorch-vq27 / vq27-local-20261008`。以下文字沿音频识别顺序、保留口语和机器来源属性；时间均为原片位置。原始响应包含更细词时刻和置信度。此文件不写回书库正文。

## V05 · underflow / overflow

> [09:43.020–09:44.500] You'll get underflow, you'll get overflow,

来自 [review-overflow.json](responses/review-overflow.json)。用户直接听核确认两个词都出现，因此画面 `Underflow!` 与字幕 `overflow` 并不直接证明字幕错误。完整句子的其他措辞及逐词边界保留机器属性。

## V04 · hidden 被求和

> [20:05.020–20:09.660] And anything that is not mentioned here, the hidden, uh, gets summed out.

来自 [review-hidden.json](responses/review-hidden.json)。用户直接听核确认 `hidden gets summed out` 可听清；矩阵维度和输出签名来自独立画面来源，未填入该转写。

## V07 · 停顿候选

> [10:49.040–10:55.000] Okay, so let me actually skip over this this part

来自 [pause.json](responses/pause.json)。同一段的前 4 秒独立识别只返回 “Okay, so”；用户给出“好像说了要接着下一个part”的不确定语义。这段有语音的机器证据，不能作为整段静音样本；具体逐字内容和停顿界限继续待核。

## 已知问题

局部裁剪改变了上下文，`NANDs/NANs` 等术语拼写仍可能变化。上面两个明确术语／短语以用户听核为独立依据，其余文字不作为人工真值。用户原始回复见 [listening-review.json](listening-review.json)。
