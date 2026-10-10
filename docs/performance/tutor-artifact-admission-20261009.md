# Tutor 直接使用已有基础成品

2026-10-09，Asia/Hong_Kong。本地修复已完成、未提交；20:32:02 HKT 已部署，见 [上线记录](tutor-artifacts-deployment-20261009/README.md)。

## 行为

Tutor 对本机和封存发布统一读取现有 Pass1 基座、discourse 与 BookStructure。缺少私有构建目录、阶段回执或 `publication.json.tutor_readiness` 不再阻止教学；旧 preparing/ready 快照不决定当前准入。原文与基座须可用，discourse 和结构须可读，保留来源归属检查。完整基础成品无需重建、重新导入或改选发布版本。

可选教学地图、对象和认知素材继续使用原有资料接入规则；其缺失不阻止来源教学。基础成品实际缺失时仍提示具体阶段。

## 验证

- 先复现：移走测试工作区的 `.build`，保留全部基础成品，旧代码仍返回 `pass1` 接纳回执缺失。新增用例失败，日志 `D:/codex-build/tutor-existing-artifacts-red.log`。
- 相关 Server 回归执行 `cargo test -p server --lib -- tutor_ mu5_ mu6a_teaching jl3_teaching jl5_teaching`。47 项通过、5 项真实模型/浏览器宿主用例按定义忽略；2 项新旧发布测试在准备数据时因封存文件只读失败，尚未触及行为断言。
- 测试准备阶段临时解除测试发布清单的只读属性，移除旧版没有的字段后恢复原权限；针对上述 2 项重跑，全部通过。相关回归合计 49 项通过。
- 覆盖本机无构建历史、封存发布无就绪字段、过时 preparing 快照、基础文件缺失/无法解析、结构属于其他书、论文无私有 reconciliation、来源版本不符、实际 HTTP 就绪接口及精确授权；原有教学回合、可选资料和学习接续测试通过。
- 回归日志：`D:/codex-build/tutor-artifact-admission-tests.log`、`D:/codex-build/tutor-artifact-publication-tests.log`。无模型调用、无生产写入。

## 已知限制

T19 真实教学效果和成本验收仍待实施。基础成品确实缺失的发布继续提示具体缺项。
