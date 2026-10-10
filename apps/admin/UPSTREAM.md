# shadcn-admin 来源

导入日期：2026-10-08。上游：[satnaing/shadcn-admin](https://github.com/satnaing/shadcn-admin)，版本 2.2.1，提交 `e16c87f213a5ba5e45964e9b67c792105ec74d26`。

按需导入 `src/components/ui` 的按钮、输入、标签、弹窗、表格、侧栏及其依赖，`layout/main`、移动端 hook、主题 provider、主题变量和 cookie 工具；`lib/utils` 仅保留 cn。MIT 原文保存在 `LICENSE.shadcn-admin`，构建包携带 `public/LICENSE.shadcn-admin.txt`。

业务导航与页面在上述组件上接入本项目 Rust API。未导入示例业务、模拟登录、模拟 Token 或 Clerk。Vite 6 / TypeScript 5 与仓库工具版本保持一致；运行依赖按采用版本锁定。
