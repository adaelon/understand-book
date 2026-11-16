# object-transform v1 检查

1. s=1,u=1 时角点为 (2,1)，实际 SVG 顶点为 (380,130)，面积为 1。
2. s=-1,u=.35 时右上角为 (.65,1)，四点鞋带面积仍为 1；s=0 全程保持原形。
3. 分别定位 .35、0+.999 和终点，回退再重复定位；图形和读数应一致，终点过渡为 0。
4. 在 320×420、640×240、960×720 操作前后步、播放暂停、参数与位置；无横向溢出，短屏通过滚动到达控件。
5. 使用实际 presentation bridge 捕获当前参数和小数位置，关闭页面再恢复；读数、SVG 几何和控件一致且暂停。出现错误时修正对应计算、渲染或恢复路径。

执行入口：`node scripts/verify-presentation-method-library.mjs`。结果见 [首批库记录](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/presentation-method-library-v1/README.md)。
