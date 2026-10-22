# PDF 文字层优先导入 — 2026-10-22

用户确认：PDF 单文件导入，优先使用已有文字生成 source.txt 和阅读定位；缺字页面或区域使用 OCR 补充，不再要求提供或校对 Markdown。阅读与用户批注无需等待语义预构建。

## 已实现

1. PDF 文字与位置一起生成 `source.txt`、LID 和选区映射。重复文本无需再次搜索定位；双栏先按列阅读，选区序号与正文顺序一致。原文符号按字面保留。
2. 无文字页面、文字覆盖不足的图像区域按需调用本地 OCR；已有文字保留，重叠识别词不重复加入。空白页不再阻断整份文件导入。
3. 新建书籍和 Workbench 只需 PDF，自动准备正文与映射并进入 Reader。原有 MD+PDF 接口与校对流程继续服务已有材料。
4. 本机导入 PDF 后直接打开原文。文字准备在后台进行；划线、笔记和阅读位置保存至本机。缓存按页保存，重开时接续未完成页；普通 Reader、专注阅读和本机阅读器均可选择 OCR 补出的文字。
5. 英文、简体中文 OCR 模型和浏览器识别引擎随网页与离线阅读器打包。纯文字 PDF 不启动 OCR worker，不请求识别模型。OCR 取消会结束未完成识别。

## 主要链路

- `pdf-geometry.ts → pdf-native-source.ts → pdf-native-foundation.ts`：文字、字符坐标、正文跨度同源；沿已有公开映射格式输出 `canonical_source.kind = pdf_text`。
- `document-import.ts / workbench-stage-runner.ts`：导入 PDF 时直接完成正文和阅读映射，不创建待处理的 Markdown 校对决定。
- `pdf-ocr.ts`：按页/区域识别、保留已有文字、生成可选文字坐标；`pdf-ocr-browser.ts` 与 Node 适配分别负责裁剪渲染和 worker 加载。
- `LocalPdfReader.vue → local-materials.ts → local_materials.rs`：原件显示、后台文字准备、逐页缓存、划线和位置持久化；正文保存为材料目录下的 `source.txt`。
- `pdf-ocr-text-layer.ts → PdfReaderPane.vue / pdf-focus.ts`：将补充文字放入现有 PDF 文字选择层。

## 验证证据

| 范围 | 已执行结果 |
| --- | --- |
| 新正文与 OCR | `pdf-native-source.test.ts`：4 项通过，覆盖纯文字不启动 OCR、重复句、字面符号、双栏、真实英文/中文扫描、混合覆盖去重、取消识别 |
| 文档导入 | `document-import.test.ts`：5 项通过，含空白页、恢复导入、Pass1 保留 PDF 来源 |
| Workbench | `workbench-stage-runner.test.ts`：12 项通过，含 PDF 单文件直接进入 Reader 和旧校对路径 |
| 旧映射回归 | `hybrid-foundation.test.ts` 20 项、`hybrid-foundation-v2.test.ts` 14 项、goldset 4 项通过 |
| 前端接口和 Reader | manifest 与 PdfReaderPane 25 项、专注阅读 9 项通过；Workbench 的 8 项验证覆盖 PDF 单文件提交与旧资料显示 |
| 实际浏览器 | `pdf-direct-reader.spec.ts` 2 项通过：鼠标选词、划线/笔记、缩放、翻页、重开、删除；扫描 OCR 与缓存重用；1280×900 和 390×844 视口 |
| 本机磁盘 | `cargo test -p understand-book-desktop local_materials::tests`：4 项通过，包含逐页写入期间保留批注、重开、位置和正文顺序 |
| 服务接口 | `pdf_only_import_automatically_reaches_reader_and_resolves_selection`：通过。实际提交 PDF、启动后台准备、重载 Book、解析 PDF 选区 |
| 构建 | core / web 类型检查、server 编译检查、网页生产构建和桌面离线阅读资源构建通过 |

Windows 上测试进程池采用 `--pool=threads --maxWorkers=1`。首次 OCR 集成发现 Tesseract 7 的语言对象初始化问题，改用其标准 `langPath` 加载本地模型；没有修改第三方库。

界面截图：[桌面](screenshots/pdf-text-import/native-desktop.png)、[窄屏](screenshots/pdf-text-import/native-narrow.png)、[扫描页划线](screenshots/pdf-text-import/scan-highlight.png)。中文 OCR 夹具为项目原创短句的固定栅格图，避免测试机器缺少中文字体时生成空方框。

## 已知边界

- 自动识别补充普通文字，公式、复杂表格、图示和版式仍以原 PDF 为准。复杂排版的线性正文不保证还原全部结构。
- OCR 区域由文字层和图像对象覆盖情况判断；内置识别语言为英文与简体中文。
- 本机直接阅读/划线与云端 AI 深度准备保持各自的数据归属，本机批注不会自动上传。
- 本轮完成源码、测试与阅读资源构建，未发布线上服务或更新已安装的 Windows 客户端。

工作区已有大量未提交变更；本轮就地增量修改并保留原有成果。
