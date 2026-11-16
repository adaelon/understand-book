# process-reveal v1 检查

1. 默认输入经 [3,4,5] → [4,5] 得到 9；各节点及最终读数一致。
2. increment=0,threshold=8 时筛选为空且求和为 0；回到第 1 步后后续节点不可见。
3. 分别定位 .35、2+.999 和终点，回退再重复定位；图形和读数应一致，终点过渡为 0。
4. 在 320×420、640×240、960×720 操作前后步、播放暂停、参数与位置；无横向溢出，短屏通过滚动到达控件。
5. 使用实际 presentation bridge 捕获当前参数和小数位置，关闭页面再恢复；读数、SVG 几何和控件一致且暂停。出现错误时修正对应计算、渲染或恢复路径。

执行入口：`node scripts/verify-presentation-method-library.mjs`。结果见 [首批库记录](https://github.com/adaelon/understand-book/blob/625b4405af3b011e87d4eb256f819c8075f88613/docs/performance/presentation-method-library-v1/README.md)。
