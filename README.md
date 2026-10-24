# ZeroLaunch Windows 系统设置插件

ZeroLaunch 第三方插件：把 64 条常用 Windows 设置页（Wi-Fi、蓝牙、显示、更新、隐私等）作为数据源候选项加入搜索，选中后经 cmd 执行 `start ms-settings:...`（或经典控制面板程序）打开对应页面。设置页可逐项勾选启用项，取消勾选的设置不会进入候选缓存，从而降低无关候选与匹配噪声。

插件只依赖 `zerolaunch-plugin-api`（trait/类型）与 `zerolaunch-plugin-sdk-rust`（`run()`、`host()`），不依赖 Tauri 与宿主源码。

## 功能

- **数据源组件**：`WindowsSettingsSource` 实现 `DataSource`，候选项直接进入宿主候选缓存，参与默认搜索排序；`mode = "inline"` 且无触发词，不走触发词路由（`query()` 防御式返回空）。
- **逐项开关**：设置 → 搜索管道 → 数据源 → Windows 设置数据源，多选勾选需要参与搜索的设置项（默认全选 64 项）。应用后立即刷新候选缓存，无需重启宿主；仅内置 id 生效，未知 id 被丢弃。
- **cmd 执行**：每个候选项的目标是 `ExecutionTarget::Command("start ...")`，由宿主命令执行器以 `cmd /C` 打开。
- **候选项图标**：统一取 `%SystemRoot%\ImmersiveControlPanel\SystemSettings.exe`，提取失败时前端回退默认图标。
- **中英双语**：候选项标题与设置项标签共用 `i18n/` 语言包（中文含 zh-Hant 回退简体，其余语言用英文包）；关键字为跨语言英文别名，中/英文查询都能命中。

## 目录结构

```
├── Cargo.toml          # 依赖 zerolaunch-plugin-sdk-rust / plugin-api
├── manifest.toml       # 插件清单（[plugin] 元数据、runtime、组件能力、图标），打包时位于 zip 根
├── package.py          # 打包脚本（cargo build --release + 生成安装 zip 与市场元数据附件）
├── src/main.rs         # 启动骨架：init() + plugin::app().run()（随模板同步，别改）
├── src/plugin.rs       # 本插件全部业务实现
├── icon.svg            # 插件图标（[icon].path 声明，随包分发）
├── i18n/               # zh-Hans.json / en.json：标题字面量与设置项标签
└── .github/workflows/  # CI（check/build/打包）、推 tag 自动发 Release、模板同步入口
```

## 实现要点

`src/plugin.rs` 里的关键结构：

| 名称 | 作用 |
|---|---|
| `PLUGIN_ID` / `SOURCE_ID` | 主组件 id（与 `[plugin].id` 一致）与数据源组件 id；设置挂在数据源上，宿主的 SettingsChanged 事件据此刷新候选缓存。 |
| `ENTRIES` | 内置设置项表：`id`（同时是 i18n 键后缀）、`command`（完整 `start ...` 命令）、`keywords`（英文别名）。 |
| `SettingsState` | 已启用 id 集合，`RwLock` 包裹，默认全选。 |
| `SettingsPatch` | 宿主下发的设置载荷，字段缺失/null 时保留当前值。 |
| `load_catalog()` | 按宿主 locale 选择内嵌语言包（`include_str!`，编译进二进制）。 |
| `app()` | 装配：主组件 + `with_data_source(WindowsSettingsSource)`。 |

**新增/删除设置项**：改 `ENTRIES` 表，并在 `i18n/zh-Hans.json`、`i18n/en.json` 两份语言包同步增删 `entry.<id>.name`（漏配时候选标题回退为 id）。选项列表与默认值都由 `ENTRIES` 推导，无需另改 schema。

## 打包与安装

```bash
python package.py              # cargo build --release 后打包（无系统 Python 用 uv run --python 3.12 python package.py）
python package.py --no-build   # 复用现有产物直接打包
```

产物为 `dist/zerolaunch-plugin-windows-settings-v<版本>.zip`，同时产出市场元数据附件 `dist/manifest.toml` 与 `dist/icon.svg`（插件市场卡片直接读这两项展示图标/名称/版本/能力标签，不下载整包）。

本地安装：设置 → 插件管理 → 安装本地插件，选择该 zip；或解压到 `%USERPROFILE%\.ZeroLaunch-rs\plugins\com.ghost-him.windows-settings\` 后重新加载。

## 发布

推 tag 自动发布（`.github/workflows/release.yml`）：`git tag v0.1.0 && git push origin v0.1.0`。tag 需形如 `v<major>.<minor>.<patch>`，且与 `manifest.toml [plugin].version`、`Cargo.toml version` 一致，否则 CI 失败。补发：Actions → 「发布插件」→ Run workflow，填该 tag。

## 调试

- **插件日志**：`%USERPROFILE%\.ZeroLaunch-rs\plugin-logs\com.ghost-him.windows-settings.log`（与宿主日志分离；卸载时删除，加载失败时保留）。
- **CLI**（宿主运行时，`--json` 为全局开关需放在子命令前）：`zerolaunch-cli plugins info com.ghost-him.windows-settings` 看清单详情、`plugins logs com.ghost-him.windows-settings --tail 50` 看日志。
- **改完冒烟**：`cargo check` 零错误、`python package.py` 能出包，装入宿主后搜索 "wifi"/"蓝牙" 应出现对应候选并打开设置页。

## 维护边界

`src/main.rs`、`package.py`、`.github/workflows/ci.yml`、`sync-template.yml` 是模板同步下发的框架文件（由 actions-template-sync 定期合入，改动在模板仓库做）；业务改动集中在 `src/plugin.rs`、`i18n/`、`manifest.toml`、`Cargo.*`，这些路径在 `.github/.templatesyncignore` 中保留本仓库内容。
