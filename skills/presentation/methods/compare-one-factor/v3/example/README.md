# 连续演示与学习者接手

直接打开 index.html。它从 @2 派生，保留同尺度跑道、距离曲线、共同时间与直接定位。B 的输入速度和已应用速度分开保存，改速度不自动改变图；应用时保留共同时间。

正式 Author 先读取 card、HTML 和 author.json，将 HTML 填入 write.html，并声明实际读取过的 `compare-one-factor@3`。把元数据中的 teaching_binding.design_ref、block_refs 换成当前设计及关系引用，HTML 中 `reportTeachingHelp('compare-speed', …)` 的 move_id 换成实际交付的教学动作；三个区域的 state_fields 保留其显示语义。source_ref_ids 按实际材料重写，完成三环境预览后交付。

教师顺序：observe → set_parameters(input_speed=2) → invoke(apply) → reveal(relation) → annotate(annotation)。每次 act 的 based_on 使用上一条实际保存回执。输入改为 2 时仍显示 10 米；应用后 10 秒走 20 米；揭示共同时间下的倍数关系后即可把操作留给用户。可以直接解释公式，按需要停下来等待回答。

result 的帮助字段是 applied_speed、time_s；pending input_speed 不触发演示帮助。relation、annotation 分别由 revealed、annotated 控制。教师回调不调用 reportTeachingHelp，宿主在保存动作时记录真实帮助。手动“显示关系”通过原 reportTeachingHelp 记录帮助，只有学习者实际填写并提交的原话才可作为表达。

保存同时保留 input_speed、applied_speed、time_s、revealed、annotated。恢复只重画已保存现场，不执行应用、不重播教师动作。控制按钮、滑杆和图中拖动与教师回调共用函数，教师停止后保持可操作。初始值 1 m/s、10 秒；A 固定 1 m/s，B 限于 0–3 m/s。

本版的纯模型仍为 d=v·t，使用 @2 的固定坐标。author 元数据、语义 observation 和可见读数必须随主题改写一起更新。[检查](../checks.md)覆盖算术、恢复、帮助顺序和用户接手。
