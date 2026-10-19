# part-whole v1 检查

1. A=40 时 B=36、C=24，宽度分别为 152、136.8、91.2，任何展开位置总宽度 380。
2. A=0 时 A 宽度为 0；A=100 时 B/C 宽度为 0，均无负宽度，合回无缝拼成整体。
3. 分别定位 .35、0+.999 和终点，回退再重复定位；图形和读数应一致，终点过渡为 0。
4. 在 320×420、640×240、960×720 操作前后步、播放暂停、参数与位置；无横向溢出，短屏通过滚动到达控件。
5. 使用实际 presentation bridge 捕获当前参数和小数位置，关闭页面再恢复；读数、SVG 几何和控件一致且暂停。出现错误时修正对应计算、渲染或恢复路径。

执行入口：`node scripts/verify-presentation-method-library.mjs`。结果见 [首批库记录](../../../../../docs/performance/presentation-method-library-v1/README.md)。
