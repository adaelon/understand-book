# EX12.4 实施与交接

- [x] 固定三类请求、原首轮历史和发布材料；建立仅测试使用的指导替换与请求记录入口。
- [x] 检验两组使用相同工具和原生适配器，历史与确切版本可读，阶段模块替换正确。
- [x] 每类请求旧指导／阶段指导各运行一次，保留真实失败及完整产物。
- [x] 沿实际产物串读、操作、检查数值与完整性；必要修订另记原因。
- [x] 最终 11 项纠错补丁及可读正文离线完成，七条操作路径在三环境验证通过。
- [x] 记录结论、代码链路、方案状态并覆写 checkpoint_ex。
- [ ] 最终修正候选经原生 author 应用、三环境 preview、后续模型图片检查及 deliver；DeepSeek 402 余额不足阻塞。

输入和边界：沿用 ADR-0154 / EX12.4；Linux 部署与 Reader 连续使用属于 EX12.5。预测与静态请求使用同一发布 1.3 材料，输入不指定画面、控件数量或布局。

运行前装配失败：初次 mechanism 两组因旧测试夹具在同一临时父目录使用 learning.db 而未能 admission，模型调用为零；已为各组使用独立 memory/learning 目录。失败目录保留为 `mechanism-*-admission-failure`。补充 admission 检查通过后才启动六次真实生成。

首批暴露的指导问题：prediction-staged 与 static-staged 在 prepare(global) 后直接 write，后续仍加载 global；static-staged 为概念区别生成完整参数实验长页。将指导修为 ex12.v4：global 明确交接 local，完整制作后交接 review；共同正文明确以当前问题界定范围及静态表达用途。首批指导快照在 `guidance-initial/`；完成首批后分别复验受影响的三个 staged 请求，原样保留初次结果。

v4 已完成：原问题两组交付，阶段组仍首写全篇、无 review；预测组来源列表漏项后无进展；静态题仍扩展较大。修订为 ex12.v5，区分可运行 HTML 与当前局部范围，并补来源列表 schema/错误恢复提示。来源权限用例与指导隔离 2 通过、指导组装 15 通过。v5 原问题两组及预测阶段组复验；若成品需要修正，后续仅以明确实际反馈追加独立 correction run，不重新抽样挑页。

接续失败及预算修复：feedback 与 feedback2 均在压缩时耗尽输出。后者仍为 16,384，是 adapter 将归档 CatalogMatch 改成 ExplicitOverride，故新预算未生效。已让记录 adapter 返回原档案并对 admission 全值断言；3 项 server 定向测试通过，20 项压缩测试通过。使用正确 profile 补跑 mechanism 两组（catalog），并以相同 feedback 进入 feedback3；旧批次保留且不宣称精确复现原配置。

正确 profile 对照完成：catalog 旧组因截图检查资格重复 deliver 中止；阶段组已跑通 local 原型、反馈、展开、review 与交付，仍漏练习。static-feedback3 revision 2 已通过三步／重置和三视口；mechanism-feedback3 已补全章节，但独立操作检出减半 B* 旁注、320px sticky 遮挡、练习 1-4 四档值未补。最终反馈只修这些局部并同步可读内容。feedback4 在 237 项压缩达到 32k 后中止，当前预算为 Flash catalog 64k（其余仍16k）；20+3 定向测试通过。

feedback5 在 64k 上限下完成压缩：首次 37,269 输出因重复 source ID 被拒绝，现有一次修复输出 47,810 后通过。随后进入 local，写入前返回 402 Insufficient Balance，停止真实调用。25 次运行、326 条 chat 响应记录、28,109,986 已知 token（含缓存输入）的明细完整保留。

剩余修正已保存为 local-correction/patch.json（确切基底 presentation-1790920818511542500-6 revision 1），11 项替换可精确重放；离线内容与三环境观察、独立数值验证已保存。状态 local_verified_not_native_delivered，不修改原版本，不生成交付回执。余额恢复后只补原生应用／观察／交付链路，不重跑已完成矩阵。EX12.4 暂不标完成，EX12.5 待此前置验收。

稳定性复核更正：mechanism-staged-v5 的 response-19 已 prepare(review)，request-20 已加载 review 指导，随后直接 deliver；此前报告的“未进入 review”误记已修正。遗漏已存在于全局框架并延续到 review；阶段切换成功不能作为完整性和内容正确性的证明。
