# Windows Setup 编译方法

> **直接基于最新 `main` 编译，无需、也不要为编译 Setup 新建 branch。**

本文说明如何从已推送的 `main` 提交构建 Understand Book 的 Windows x64 安装包，并让 Setup 后续安装的 Codex 插件精确锁定到同一个 Git 提交。

## 1. 前置环境

- Windows x64
- Git
- Node.js、pnpm
- Rust MSVC toolchain 与 Visual Studio C++ Build Tools
- WebView2 与 Tauri/NSIS 所需工具
- 项目依赖中提供的 Bun；不需要单独手工调用 Bun

本流程在以下版本上验证通过：Node.js `v24.9.0`、pnpm `10.34.2`、Rust/Cargo `1.96.0`。

## 2. 锁定最新 main

在仓库根目录执行：

```powershell
git fetch origin
git switch main
git pull --ff-only origin main

$mainCommit = (git rev-parse HEAD).Trim()
$originMain = (git rev-parse origin/main).Trim()
if ($mainCommit -ne $originMain) {
  throw "本地 main 与 origin/main 不一致，停止编译"
}

git status --short
```

如果工作区干净，可以直接继续。如果工作区存在需要保留的未提交内容，不要清理、stash 或新建 branch；改用本文末尾的 detached worktree 方法。

## 3. 安装冻结依赖

```powershell
pnpm install --frozen-lockfile
```

依赖已完整缓存时可以使用：

```powershell
pnpm install --frozen-lockfile --offline
```

## 4. 冷构建预生成 sidecar

全新检出中还没有 `understand-book-build-x86_64-pc-windows-msvc.exe`。正式发布脚本会先执行 parity 门禁，因此必须先生成一次 sidecar：

```powershell
node apps/desktop/scripts/build-sidecar.mjs
```

若跳过此步，常见错误是 `understand-book-build-x86_64-pc-windows-msvc.exe ENOENT`。

## 5. 编译 Setup

将 marketplace source 精确 pin 到当前 `main` 提交，避免以后安装时漂移到另一个插件快照：

```powershell
$mainCommit = (git rev-parse HEAD).Trim()
$env:UNDERSTAND_BOOK_MARKETPLACE_SOURCE = "adaelon/undertand-book@$mainCommit"

pnpm -C apps/desktop package:windows
if ($LASTEXITCODE -ne 0) {
  throw "Windows Setup 编译失败：exit code $LASTEXITCODE"
}
```

仓库远端名称当前就是 `undertand-book`，这里不是拼写修正点。

`package:windows` 会依次执行：

1. automatic-build Node/Bun parity；
2. 发布插件 manifest、skill hash 与 thin-plugin 形状校验；
3. marketplace release config 校验；
4. Web production build；
5. build sidecar 与 Book MCP 的构建、smoke；
6. Tauri release build、NSIS bundle 和 canonical installer 导出。

## 6. 产物与校验

最终交付文件：

```text
dist/UnderstandBookSetup.exe
```

NSIS 原始产物：

```text
target/release/bundle/nsis/Understand Book_0.2.0_x64-setup.exe
```

编译完成后执行：

```powershell
$setup = "dist/UnderstandBookSetup.exe"
$bundle = "target/release/bundle/nsis/Understand Book_0.2.0_x64-setup.exe"

if (-not (Test-Path -LiteralPath $setup)) {
  throw "未生成 $setup"
}

$setupHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $setup).Hash
$bundleHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $bundle).Hash
if ($setupHash -ne $bundleHash) {
  throw "canonical Setup 与 NSIS bundle 不一致"
}

Get-Item -LiteralPath $setup | Select-Object FullName, Length, LastWriteTime
Write-Output "SHA256=$setupHash"
Get-AuthenticodeSignature -LiteralPath $setup
```

当前项目尚未配置代码签名，因此 `Get-AuthenticodeSignature` 返回 `NotSigned` 是已知状态，不等于编译失败。

## 7. 工作区有未提交内容时

使用 detached worktree 从远端 `main` 的确定提交构建。发布 prompt 受字节级 SHA-256 合同约束，Windows 全局 `core.autocrlf=true` 会把 LF 转成 CRLF 并触发 `release_prompt_hash_mismatch`；因此创建 worktree 时必须只对该次检出关闭换行转换。该方式不会创建 branch：

```powershell
git fetch origin
$mainCommit = (git rev-parse origin/main).Trim()
$buildDir = Join-Path (Split-Path -Parent (Get-Location)) "understand-book-setup-$($mainCommit.Substring(0, 12))"

git -c core.autocrlf=false worktree add --detach $buildDir $mainCommit
Set-Location $buildDir

pnpm install --frozen-lockfile
node apps/desktop/scripts/build-sidecar.mjs
$env:UNDERSTAND_BOOK_MARKETPLACE_SOURCE = "adaelon/undertand-book@$mainCommit"
pnpm -C apps/desktop package:windows
if ($LASTEXITCODE -ne 0) {
  throw "Windows Setup 编译失败：exit code $LASTEXITCODE"
}
```

验证并复制 `dist/UnderstandBookSetup.exe` 后，回到主仓库再移除临时 worktree。不要在临时 worktree 中创建 branch 或提交 Tauri 自动生成的文件。

## 8. 常见失败

- `UNDERSTAND_BOOK_MARKETPLACE_SOURCE is required`：按第 5 节设置精确 Git source。
- `understand-book-build... ENOENT`：先运行第 4 节的 sidecar 预生成命令。
- `release_prompt_hash_mismatch`：`agents/*.md` 被检出为 CRLF；即使当前工作区干净，也改用第 7 节的 LF-safe detached worktree，不要修改冻结 hash 或放宽 parity 门禁。
- Rust `build-script-build` 显示“拒绝访问”：不要在受执行策略限制的 `C:\tmp` 中构建，改用正常仓库路径或同磁盘 detached worktree。
- NSIS `Internal compiler error #12345: error creating mmap`：先检查 `TEMP/TMP` 所在磁盘的剩余空间；空间不足时，在空间充足的磁盘创建本次构建专用临时目录，将 `$env:TEMP` 与 `$env:TMP` 指向该目录后完整重跑 `package:windows`，不要删除系统临时目录或放宽打包门禁。
- Vite chunk-size 或 `ts-rs failed to parse serde attribute`：当前属于 warning；最终仍必须同时确认命令退出码为 0 且 Setup 文件实际存在。
