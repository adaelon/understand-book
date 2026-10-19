# 把齿轮式解释改写为另一个主题

本模板把已经验证的制作机制保留下来：柔和灯光与材质、正交取景、指针和键盘旋转、轴向拆解、参数驱动的多图联动、可重复时间定位、Reader 现场恢复及静态导出。

## 读取与写入

1. 读取 `card.md`、`example/index.html`、本说明和 `example/author.json`。HTML 内含全部业务代码；超过返回预算时沿 `next_offset` 接着读取。
2. 观察第二主题时读取 `adaptations/spring.html` 和 `adaptations/spring-author.json`。两个作品由同一份 `runtime.js` 生成。
3. HTML 作为 `write.html`；其余内容字段以对应 author.json 为起点改写。每次写入声明 `libraries:["three"]`。原有 `data-presentation-library` 标签由宿主重建；不要粘贴 Three 源码，不加载 CDN，不保留 ESM import。
4. 根据新问题更新 `readable_content`、`assumptions`、`initial_state` 和 `state_contract`，声明确切 `method_refs`。书籍依据仍由当前任务自己建立。

## 改哪里

| 部位 | 新主题要做什么 | 保留什么 |
|---|---|---|
| `defaults` 与参数控件 | 参数、单位、有效范围、初始条件 | 控件都有稳定 id；`data-param` 对应状态名 |
| `derive(state)`、`period(state)` | 从模型一次计算位置、角度、速度、力或其他结果；给出每周期实际时长 | 纯计算，不读 DOM，不修改状态，不累加上一帧 |
| `createObjects(scene)` | 建立主题部件、颜色、材质和真实拓扑 | 对象身份稳定、每帧不重建材质或几何 |
| `updateObjects(objects,model,state)` | 从本帧结果更新位置；拆解只改变展示偏移 | 同样参数与定位得到同样几何；替换资源时释放旧资源 |
| `bounds(model,state)` | 返回覆盖全部运动/拆解的包围盒列表 `[xmin,ymin,zmin,xmax,ymax,zmax]` | 取景覆盖完整运动，不能逐帧贴合当前旋转网格 |
| `drawDiagrams(model,state)`、`describe(model,state)` | 用同一结果画二维关系并给出含单位的 DOM 结论 | 读数不是第二套算法；同对象颜色与名称一致 |
| 图解直接操作 | 将拖动或按键换算成模型参数或 `seek` 的周期位置 | 统一定位入口，结束操作后 `commitState` |

`runtime.js` 负责渲染器、灯光、稳定相机、尺寸变化、帧调度、轨道操作、播放、状态桥及释放。`canvas2D` 提供按实际尺寸和像素密度准备的画布。改写成无周期过程时明确重新定义时间合同与控件，不能把任何步骤都叫作“周期”。

## 已验证的跨题替换

| 关系 | 齿轮 | 弹簧振子 |
|---|---|---|
| 模型 | `ω₂/ω₁ = −z₁/z₂`，理想功率守恒 | `x=A cos(√(k/m)t)`，机械能守恒 |
| 一个周期 | 输入轮一整转，实际时长 `60/rpm` s | 一次完整往复，实际时长 `2π√(m/k)` s |
| 三维对象 | 渐开线齿轮、轮毂、轴、端盖 | 质量块、螺旋弹簧、导轨、盖板 |
| 二维映射 | 相同转角的节圆图与扭矩图 | 相同时刻的位移曲线与能量图 |
| 零量边界 | 转速为零禁用播放；零扭矩隐藏整根力矩箭头 | 零振幅时位移、速度、力与能量均为零 |

## 状态和导出

所有参数、`yaw`、`tilt`、`explode`、`semantic_state`、`transition_progress` 都是标量。数学意义或定义域变更时同时修改 state_contract，避免把旧现场当成新意义。相机不改变模型，参数变化暂停并回到初始条件；拆解保留当前位置并暂停。重开通过注册的 restorer 恢复，不能在初始化时重复恢复。

渲染器开启 `preserveDrawingBuffer:true`，state reader 返回前同步绘制当前帧，不推进时间、不改变 playing。宿主静态捕获把可见 Canvas 转成 PNG，随后按原布局导出。无 WebGL 时显示可读提示并保留二维图解；三维操作停止。

## 制作与验收

本仓库的可编辑源是 `runtime.js`、`gear.js`、`gear-model.js`、`spring.js`、`spring-model.js`、`style.css`、`shell.html`；`node scripts/build-spatial-method.mjs` 生成完整作品和元数据。Agent 直接读取完整 HTML 改写即可，不需要运行构建脚本。

先按 checks.md 独立代入新公式，再观察实际几何和读数；核对小屏、短屏、拖动、反向定位、保存重开、导出图中三维像素。正式三环境 preview 必须对当前候选执行。浏览器验证入口为 `node scripts/verify-spatial-method.mjs`；真实 Author 验证入口见库根目录 README 的验证记录。

## 已知限制

当前迁移证明来自两个明确的物理模型，尚未证明 Agent 能独立迁移所有主题，也没有学习效果结论。复杂机械仿真、任意三维文件加载与外部贴图不属于此模板的实现能力。运行核心采用正交相机和四周期观察窗，换成其他叙事范围时需要明确改写。
