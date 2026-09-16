# DeepMate Agent 指南

面向在本仓库工作的 AI agent 的项目级规范。核心文档：[README.md](README.md)、[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)、[docs/DESIGN_SYSTEM.md](docs/DESIGN_SYSTEM.md)。

## 发版规范（硬性规定）

**禁止使用 GitHub CI / Actions 发版。** 用户明确要求（2026-08-27），tag 触发的 release workflow 已从仓库删除，不要重建，不要用 `gh run`/`gh workflow` 做任何发布动作。发布全部在本地构建、本地上传。普通推送触发的 `ci.yml`（fmt/clippy/test）保留，仅作质量门禁。

### 发版步骤（macOS arm64 主机）

1. **同步版本号（3 处，全部从旧号 sed 到新号）：**
   - `Cargo.toml`（workspace.package.version — **唯一真源**：各 crate 与 `deepmate-desktop` 全部 `version.workspace = true` 继承）
   - `apps/desktop/package.json`
   - `apps/desktop/package-lock.json`（第 3、9 两行）
   - 不需要改 `src-tauri/Cargo.toml`（已继承）或 `tauri.conf.json`（已删 version 字段，回落到 Cargo 版本）。
2. **只有一个 Cargo.lock**（根目录）。src-tauri 已并入根 workspace，构建用 `--locked`。
3. **CHANGELOG.md** 顶部加 `## [x.y.z] - 日期` 段落（Keep a Changelog 格式）。该段落之后会被提取为 GitHub Release body。
4. **发布前验证全绿：**
   ```bash
   make verify                             # Rust 门禁 + 前端 lint/tsc/test
   (cd apps/desktop && npm run build)      # 前端生产构建
   ```
5. **本地构建：**
   ```bash
   cargo build --release --workspace --locked --target aarch64-apple-darwin
   cd apps/desktop && npm run tauri build -- --target aarch64-apple-darwin
   # 产物：target/aarch64-apple-darwin/release/{deepmate,deepmate-desktop}
   #       target/aarch64-apple-darwin/release/bundle/dmg/DeepMate_x.y.z_aarch64.dmg
   ```
6. **dist 整理**（命名必须带 target 三元组）：
   - `deepmate-<ver>-aarch64-apple-darwin.dmg`（从 bundle/dmg/ 拷贝）
   - `deepmate-<ver>-aarch64-apple-darwin.tar.gz`（内含 `deepmate`、`deepmate-desktop`、`README.md`、`LICENSE-MIT`、`LICENSE-APACHE`）
   - 两个文件各自的 `.sha256`（`shasum -a 256`，内容为 `<hash>  <filename>`）
7. **创建 Release：**
   ```bash
   awk -v marker="## [x.y.z]" 'index($0, marker) == 1 { found=1; next } found && /^## \[/ { exit } found { print }' CHANGELOG.md > release-notes.md
   gh release create vX.Y.Z --title "DeepMate vX.Y.Z" --notes-file release-notes.md dist/<产物…>
   ```
8. **发布后验证：** `gh release view vX.Y.Z` 核对标题、body、产物数量；用产物二进制做冒烟测试（如 `DSH_HOME=$(mktemp -d) deepmate snapshot export/list/import`、`doctor`）。

### 发版铁律

- **Release 标题统一 `DeepMate vX.Y.Z`**。不要用裸 tag 名（`v0.5.0`）或无 `v` 的变体（`DeepMate 0.6.0`）——历史 6 个 Release 已于 2026-08-27 全部改齐，保持下去。
- **一个版本号对应一个不可变 tag。** 发现有错不移动旧 tag，升新号另发（如 0.6.x 出错 → v0.7.0）。
- manifest 版本号无 `v` 前缀（`0.6.0`），git tag 带 `v`（`v0.6.0`），CHANGELOG 段落无 `v`（`## [0.6.0]`）。
- 版本节奏：每批功能升一位 minor；进 1.0.0 由用户决定。
- 本地主机只产 macOS arm64 产物；Linux/Windows/Intel 需要对应平台，不要假装能发。

## 其他项目约定

- **质量门禁**：根 workspace `make ci`；另有 core purity gate（`crates/deepmate-core` 内不得出现 deepseek/dsh 字样，CI 强制）。
- **编译缓存**：src-tauri 已并入根 workspace，全项目只有一个 `target/`（2026-09-16 重构）。`profile.dev` 用 `debug = "line-tables-only"` 控制 debug 缓存体积；target/debug 膨胀到数 GB 时跑 `make cache-clean` 清增量编译垃圾（Cargo 自己从不清理）。
- **桌面端**：Tauri 2 + React + Tailwind（v4，CSS-first）；设计令牌只存在于 `apps/desktop/src/styles.css` 的 `@theme`，组件/页面禁止硬编码视觉值（见 docs/DESIGN_SYSTEM.md）。
- **bindings 再生成**：改了 Tauri 命令面/模型后跑 `cargo test -p deepmate-desktop --lib export_bindings_headless`（debug 启动时也会自动导出），生成物是 `apps/desktop/src/shared/api/bindings.ts`（前端唯一 import 的路径）；生成文件勿跑 prettier（`.prettierignore` 已排除）。
- **i18n**：新增 UI 文案必须同时加 `apps/desktop/src/locales/en.json` 和 `zh.json`；托盘菜单在 Rust 侧按语言分支。
- **文档同步**：改了架构/设计/命令面，同一批更新 README.md 与 docs/，不允许文档漂移（Slint 时代的教训）。
