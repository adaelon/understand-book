# 齿轮的约定

一份中文交互讲解作品。围绕一对外啮合直齿轮，连续解释齿数、转速、扭矩、接触线与渐开线。参考 Ciechanowski 的解释节奏，文字、模型与界面独立制作。

## 制作范围

用户已观看并认可这份作品。三维观察与图解联动已提取为 [spatial-linked-views@1](../../skills/presentation/methods/spatial-linked-views/v1/card.md)，附齿轮主例、弹簧振子改写和正式 Author / Reader 接入。本目录保留完整讲解作品；模板的维护与验收见 [三维模板记录](../../docs/performance/presentation-spatial-template-v1/README.md)。

模型：20° 标准压力角、相同模数、主动轮 20 齿、从动轮 20/30/40/60 齿；理想刚性无损传动。正面从主动轮看，主动轮逆时针、从动轮顺时针。

状态到画面：`参数 / 转角 → 同一齿形与传动模型 → 三维总览 + 二维节圆 / 力矩图`。接触细节独立定位同一对齿面，渐开线构造独立控制展开量。

## 制作步骤

- [x] 渐开线几何、啮合相位、速度与力矩模型；标准尺寸、48 组传动参数、28 组双齿面接触位置与展开线几何通过独立核对。
- [x] 三维总览、四段解释、直接操作、保存恢复与画面导出。
- [x] 桌面和窄屏浏览器验证、逐图检查、修正后提供本地预览。

## 运行与验证

在仓库根目录执行 `python -m http.server 4198 --bind 127.0.0.1 --directory examples/gear-transmission`，打开 <http://127.0.0.1:4198/>。页面脚本与 Three.js 均在本目录，运行时不请求外部资源。

`node --test examples/gear-transmission/model.test.mjs` 验证标准尺寸、传动关系、接触点确实位于两侧绘制齿面、展开线与渐开线的几何关系。

启动预览后运行 `node examples/gear-transmission/verify.mjs`，使用本机 Edge 验证所有齿数、参数改变、暂停、相机拖动与键盘操作、分解、单圈观察、直接拖动齿轮、接触定位、保存重开和 PNG 导出。覆盖 1440、768、390、320 像素宽度；结果和截图见 [verification/report.json](verification/report.json)。

验收修正包括：将齿面倒角向内收缩以保持理论外廓、修正渐开线末端裁切、让分解视图自动取景、让离屏后重新进入的图形立即同步当前状态。截图已人工查看，作品的表达质量仍由实际观看反馈决定。

## 文件职责

- `model.js`：标准齿形、传动和接触位置；无绘制依赖。
- `assembly.js`：三维齿轮、倒角、光照、轴向分解、取景与相机操作。
- `diagrams.js`：节圆与转角轨迹、切向力、齿面接触、渐开线展开。
- `app.js`：同一组传动参数、播放控制、可见性、明确保存与恢复、高清 PNG。
- `index.html` / `style.css`：完整中文讲解、适应窄屏的版面与操作入口。

## 来源

- 解释方式与物理关系：[Gears](https://ciechanow.ski/gears/)、[Mechanical Watch](https://ciechanow.ski/mechanical-watch/)。没有复制其图形、文章或绘图实现。
- 齿形：[KHK — Involute Gear Profile](https://khkgears.net/new/gear_knowledge/gear_technical_reference/involute_gear_profile.html)。
- 三维绘制：本地固定 Three.js 0.180.0，MIT 许可见 `vendor/THREE-LICENSE.txt`。

## 已知边界

齿根使用径向过渡，未模拟制造刀具形成的齿根过渡曲线；仅渐开线有效齿面参与本演示的接触解释。不模拟齿面弹性、摩擦损失、轴承与实际负载动力学。三维模型沿轴向分解时暂停传动。

这份作品的本地保存与 PNG 导出独立于 Reader；尚未接入产品的 Author 交付与长图导出。
