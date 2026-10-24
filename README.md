# ZeroLaunch Windows 系统设置插件

ZeroLaunch 第三方插件：把常用 Windows 设置页（Wi-Fi、蓝牙、显示、更新、隐私等）作为数据源候选项加入搜索，选中后经 cmd 执行 `start ms-settings:...`（或经典控制面板程序）打开对应页面。设置页可逐项勾选启用项，取消勾选的设置不会进入候选缓存，从而降低无关候选与匹配噪声。

插件只依赖 `zerolaunch-plugin-api`（trait/类型）与 `zerolaunch-plugin-sdk-rust`（`run()`、`host()`），不依赖 Tauri 与宿主源码。

## 功能

- **数据源组件**：`WindowsSettingsSource` 实现 `DataSource`，候选项直接进入宿主候选缓存，参与默认搜索排序。
- **逐项开关**：设置 → 搜索管道 → 数据源 → Windows 设置数据源，勾选需要参与搜索的设置项（默认全选 64 项）；应用后立即刷新候选缓存。
- **cmd 执行**：每个候选项的目标是 `ExecutionTarget::Command("start ...")`，由宿主命令执行器以 `cmd /D /S /C` 打开。
- **中英双语**：候选项名称与设置项标签共用 `i18n/` 语言包；关键字同时包含中英文别名。

## 目录结构

```
├── Cargo.toml          # 依赖 zerolaunch-plugin-sdk-rust / plugin-api
├── manifest.toml       # 插件清单（[plugin] 元数据、运行时命令、data_source 能力、图标），打包时位于 zip 根
├── package.py          # 打包脚本（cargo build --release + 生成安装 zip）
├── src/main.rs         # 启动骨架：init() + plugin::app().run()（随模板同步，尽量别改）
├── src/plugin.rs       # 插件实现：主组件 + WindowsSettingsSource(DataSource) + 内置设置项表
├── icon.svg            # 插件图标（清单 [icon] 声明，随包分发）
├── i18n/               # 语言包：候选项名称与设置项标签（宿主加载时并入翻译目录）
└── .github/workflows/  # CI（check/build/打包）、推 tag 自动发 Release、模板同步入口
```

`src/main.rs` 只做启动（`init()` 预置插件 id → `plugin::app().run()`），十来行、随模板同步；插件实现与装配都在 `src/plugin.rs`——主组件实现 `Plugin` / `Configurable`，设置数据源实现 `DataSource` / `Configurable`，并在 `pub fn app() -> PluginApp` 里用 `with_data_source(...)` 挂载。

## 快速开始

1. **改标识与元数据**——插件级元数据只写在 `manifest.toml` 的 `[plugin]` 段（`id` / `name` / `version` / `description` / `author` / `mode` / `triggerKeywords` / `supportedOs` / `priority` 必填），宿主读清单构造，插件代码不声明也不上报；`Cargo.toml` 的 `package.version` 与 `[plugin].version` 保持一致。唯一要在代码里对齐的是 `Configurable::core()` 的组件 ID（见下）。

2. **实现 `Plugin` trait**（`src/plugin.rs` 是完整可跑示例，写法与 SDK 文档一致）：
   - `Configurable::core()`：返回组件级描述符 `ComponentCore`（组件 ID / 名称 / 描述 / 类型 / 优先级）。组件 ID 与清单 `[plugin].id` 一致。`mode` 决定形态——`panel`（沉浸式：候选项/热键唤醒后接管窗口）或 `inline`（行内：触发词前缀路由）；`hotkey` 仅 `panel` 形态生效。
   - `query()`：返回 `QueryResponse::List`（标准搜索结果，走搜索栏/CLI）或 `QueryResponse::CustomPanel`（面板自渲染，`data` 可承载任意 JSON，`keep_search_bar` 决定是否保留搜索栏）。
   - `execute_action()`：动作执行（打开文件等经 `host()` 平台 API）；`interaction_policy()` 声明面板按键绑定。
   - 插件级元数据（像 `metadata()` 那样在进程内声明 id、触发词、优先级）已不属于插件侧契约，不用实现。
3. **（可选）设置项**：实现 `Configurable`——`setting_schema()` 声明后宿主设置页自动渲染，`apply_settings()` / `get_settings()` 应用与回读。
4. **（可选）i18n**：面向用户的文本用 `t_key("key")`，译文放 `i18n/zh-Hans.json`、`en.json`；面板侧用 `host.t(key)`（同键）。
5. **（可选）面板**：`[ui].panelEntry` 指向 `ui/panel.mjs`，导出 `mount(rootEl, host)`，在宿主 Shadow DOM 内执行。
6. **装配与额外组件**：`src/plugin.rs` 导出 `pub fn app() -> PluginApp`（骨架调它）；需要额外组件就在这里挂载，如 `PluginApp::new(MyPlugin::new()).with_score_booster(Booster::new())`。

## 开发要点

| 主题 | 要点 |
|---|---|
| 骨架 vs 业务 | `src/main.rs`（十来行启动骨架）在同步集合里，**尽量别改**；确需改动就照常改，同步 PR 人工合并。业务代码一律写 `src/plugin.rs`，追加模块放 `src/plugin/`（同时把 `src/plugin/` 加进 `.github/.templatesyncignore`）。 |
| manifest 键名 | 宿主 schema 用 camelCase（`panelEntry` / `settingsEntry` / `resultItemEntry`）。写成 snake_case **不报错但被静默忽略**；`package.py` 对这三个键做了预检告警。 |
| 插件 id | 必须匹配 `[a-z][a-z0-9]*(\.[a-z][a-z0-9_-]*)+\z`（全小写反向域名），写在清单里；代码里的组件 ID（`ComponentCore::new` 第一个参数）与它一致。 |
| 插件元数据 | 唯一来源是 `manifest.toml` 的 `[plugin]` 段（必填：`id`/`name`/`version`/`description`/`author`/`mode`/`triggerKeywords`/`supportedOs`/`priority`；可选：`hotkey`），宿主读取后构造，插件进程既不声明也不上报。缺字段或取值非法时清单解析失败、插件加载失败，报错里给出具体字段。 |
| 插件图标 | 由 `[icon].path` 声明（相对插件目录，随包分发，支持 svg/png/ico/jpg/webp/gif，按扩展名定类型）；文件缺失只在宿主日志告警、不阻断加载。仅 `mode = "panel"` 形态在插件列表/底部栏/搜索栏前缀展示，行内形态声明了也不显示。 |
| 组件描述符 | 插件进程只声明组件级信息：`Configurable::core()` 的 `ComponentCore`（组件 ID / 名称 / 描述 / 类型 / 优先级），经 `plugin/get_components` 交宿主。组件文案可用 `t_key()` 本地化。 |
| SDK 版本 | 依赖 `zerolaunch-plugin-* = "0.2.3"`：清单里的 `mode`/`triggerKeywords`/`supportedOs`/`priority` 由 0.2 起的清单 schema 支持，不要降级。 |
| `t_key` 时机 | `t_key()` 依赖握手注入的插件 id；在 `run()` 之前构造组件文案（如 `ComponentCore` 的 name/description）时调用**必须先 `zerolaunch_plugin_sdk_rust::init()`**——骨架 `main()` 已含，业务文件里无需再调。 |
| 语言包结构 | 键与 `t_key` 路径一致、可带点号（`"booster.name"` ↔ `t_key("booster.name")`）；值只能是字符串或嵌套对象，出现数字/布尔会让宿主拒绝加载整包。 |
| 面板样式 | 用宿主 CSS 变量（`--bg-primary`、`--text-primary` 等）即自动跟随宿主主题，不要写死颜色。 |
| 面板销毁 | `mount` 内创建的定时器 / window 级监听器必须清理：`host.onDestroy(cb)` 注册回调，或让 `mount` 返回 cleanup 函数（二选一）。否则反复开关面板会线性累积泄漏。 |
| 打包布局 | `manifest.toml` 必须在 zip 根；`bin/` 内 exe 的文件名取自 `[runtime].command`；`ui/`、`i18n/` 与 `[icon].path` 指向的图标按原相对路径打入；`extra/` 内容并入 zip 根，放必须与 exe 同目录的运行时文件（如 `Everything64.dll`）。 |
| 宿主兼容 | 加载期只校验协议 major 版本；manifest 没有宿主版本下限字段（旧模板的 `minHostVersion` 已废弃，写了也会被忽略）。 |

## 调试与验证

- **插件日志**：`%USERPROFILE%/.ZeroLaunch-rs/plugin-logs/<plugin-id>.log`（与宿主日志分离）。加载失败时日志保留供排查，插件正常卸载时被删除。
- **CLI**（宿主运行时可直查）——`zerolaunch-cli.exe`（别名 `zl`，`--json` 是全局开关，必须写在子命令**前面**）：
  - `query "<触发词> <文本>"` 查插件响应形状（`--json query "..."` 输出原始 JSON，验证 `CustomPanel` 载荷最直接）；
  - `plugins list` / `plugins info <id>` / `plugins logs <id> --tail 50`：已装插件、清单详情、日志；
  - `session`：当前会话模式。
- **改完冒烟**：`cargo check` 零错误、`python package.py` 能出包；面板改动在宿主设置页重新加载插件后即可预览。

## 发布

推 tag 自动发布（`.github/workflows/release.yml`，随模板同步下发）：`git tag v0.1.0 && git push origin v0.1.0` —— GitHub Actions 构建 + 打包，把 `dist/zerolaunch-plugin-<短id>-v<版本>.zip` 作为附件发到 Release。要求 tag 形如 `v<major>.<minor>.<patch>`，且与 `manifest.toml [plugin].version`、`Cargo.toml version` 三处一致（不一致直接失败）。补发：Actions → 「发布插件」→ Run workflow，填该 tag。

本地打包（同一套流程）：

```bash
python package.py              # 等价于 cargo build --release 后打包（无 Python 时用 uv run --python 3.12 python package.py）
python package.py --no-build    # 复用现有产物直接打包
```

产物 `zerolaunch-plugin-<插件短id>-v<版本号>.zip`（短 id = `[plugin].id` 末段，`com.example.hello-world` → `hello-world`）。
插件市场按 `/releases/latest` 的这个 zip 附件自动安装，别改产物名与 zip 布局。

同一目录下还有**市场元数据附件**（`package.py` 一并产出，发布 CI 会连同 zip 一起上传）：

- `dist/manifest.toml`：清单原文（固定名）；
- `dist/<图标文件名>`：`manifest.toml [icon].path` 声明的图标文件（仅当声明了 `[icon]`）。

宿主「插件市场」卡片直接读这两个附件展示图标、插件名/版本/作者与能力标签，**不下载整包**；缺少它们时卡片退回仓库描述 + 仓库所有者头像，安装不受影响。图标附件名取图标文件自身文件名（Release 附件名不含目录分隔符），宿主按 `[icon].path` 的末段匹配——所以图标放插件根目录即可：指向 `ui/`、`i18n/` 时会与那两个目录的整目录打包叠加，同一个 zip 里出现两条同名条目。

安装：设置 → 插件管理 → 安装本地插件，选择该 zip；或手动解压到 `%USERPROFILE%/.ZeroLaunch-rs/plugins/<plugin-id>/` 后重新加载。

## 模板同步（框架文件自动更新）

同步跑在插件仓库这一侧（`.github/workflows/sync-template.yml`，模板与各插件仓库逐字节一致，改动在模板仓库做）。业务文件（`src/plugin.rs`、`ui/`、`i18n/`、`Cargo.*`、`manifest.toml`、`README.md`）按 `.github/.templatesyncignore` 保留本仓库内容，其余框架文件（`src/main.rs` 骨架、`package.py`、`ci.yml`、`sync-template.yml` 等）由 [actions-template-sync](https://github.com/AndreasAugustin/actions-template-sync) 合入，有差异时自动提 PR（无差异不提）——**宿主/SDK API 变更靠这一步送进插件仓库**。

- **触发**：每周一 02:23 UTC 定时跑一次（GitHub 高峰时段可能延迟）；模板刚改完想立刻同步，到本仓库 Actions 页手动运行 `sync-template`。
- **首次配置**：配 secret `TEMPLATE_SYNC_TOKEN`（PAT，`contents: write` + `workflow`）——同步会推 `.github/workflows/` 下的文件，默认 `GITHUB_TOKEN` 没有 `workflow` 权限会失败；不配则需在 Settings → Actions → General 勾选 *Allow GitHub Actions to create and approve pull requests*，且同步 PR 的 CI 需人工批准。
