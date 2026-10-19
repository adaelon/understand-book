# 随书计算代码

在本书目录执行：

```bash
python examples/book_cases.py --output examples/results.json
python examples/quickstart.py
python examples/standalone_model.py
```

三条命令仅使用 Python 3 标准库。`book_cases.py` 维护正文的练习参数，`quickstart.py` 与附录 B 的代码块一致，`standalone_model.py` 与附录 D 的完整代码一致。

`upstream/last_hit_positioning_v3.py` 原样取自 Deeki-Sora/last_hit_positioning 的提交 `50db387dd1218f5a0578183bdfd031cdb9375573`。原作者版权与完整 MIT 许可在 [upstream/LICENSE](upstream/LICENSE)。

这些参数是教学用构造场景。输出单位为练习格、秒、格每秒；`Sim.ok` 与 `window_valid` 的区别见附录 B。
