//! Windows 系统设置数据源插件——**业务文件**：不参与模板同步。
//!
//! 插件 = 组件壳（WindowsSettingsPlugin，纯 Plugin 主组件）+ WindowsSettingsSource
//! （DataSource，候选采集）。每个候选项对应一条设置打开命令（`start ms-settings:...`
//! 或经典控制面板项），执行时由宿主命令执行器以 `cmd /C <命令>` 打开。
//!
//! 设置页可逐项勾选启用项；取消勾选的设置不会进入候选缓存，从而减少无关候选与匹配噪声。
//!
//! 说明：候选标题直接使用语言包中的字面量（搜索结果标题不经过 key-or-literal 翻译），
//! 语言包同时供 `t_key()` 生成设置项选项标签，避免同一份显示名在两处重复维护。

use std::collections::HashSet;
use std::sync::RwLock;

use async_trait::async_trait;
use serde::Deserialize;
use zerolaunch_plugin_api::config::{
    ComponentCore, ComponentType, ConfigError, Configurable, FieldUiMetadata, SchemaKind,
    SchemaNode, SettingDefinition, WidgetHint,
};
use zerolaunch_plugin_api::services::icon_request::IconRequest;
use zerolaunch_plugin_api::{
    CachedCandidateData, DataSource, ExecutionTarget, Plugin, PluginContext, PluginError,
    PluginHandle, Query, QueryResponse, SearchCandidate,
};
use zerolaunch_plugin_sdk_rust::{host, t_key, PluginApp};

/// 插件主组件 id（与清单 `[plugin].id` 一致）。
const PLUGIN_ID: &str = "com.ghost-him.windows-settings";
/// 数据源组件 id（设置挂在该组件上，宿主的 SettingsChanged 事件会刷新候选缓存）。
const SOURCE_ID: &str = "com.ghost-him.windows-settings.source";
/// 设置字段：启用的设置项 id 列表。
const ENABLED_SETTINGS: &str = "enabled_settings";

/// 一条可搜索的 Windows 设置项。
///
/// - `id`：设置项标识，同时作为语言包键后缀（`entry.<id>.name`）；
/// - `command`：经 cmd 执行的完整命令（`start ...`）；
/// - `keywords`：跨语言别名（英文小写），保证中/英文查询都能命中。
struct SettingsEntry {
    id: &'static str,
    command: &'static str,
    keywords: &'static [&'static str],
}

/// 内置设置项表。命令均取自微软官方 ms-settings URI 列表或经典控制面板程序；
/// 新增/删除条目只需改这里与两份语言包中的同名键。
const ENTRIES: &[SettingsEntry] = &[
    // ── 网络 ──────────────────────────────────────────────
    SettingsEntry {
        id: "wifi",
        command: "start ms-settings:network-wifi",
        keywords: &["wifi", "wi-fi", "wlan", "无线网络", "无线局域网", "网络"],
    },
    SettingsEntry {
        id: "network-status",
        command: "start ms-settings:network-status",
        keywords: &["network", "internet", "网络", "网络状态"],
    },
    SettingsEntry {
        id: "airplane",
        command: "start ms-settings:network-airplanemode",
        keywords: &["airplane", "flight", "飞行模式"],
    },
    SettingsEntry {
        id: "mobile-hotspot",
        command: "start ms-settings:network-mobilehotspot",
        keywords: &["hotspot", "mobile hotspot", "热点", "移动热点"],
    },
    SettingsEntry {
        id: "vpn",
        command: "start ms-settings:network-vpn",
        keywords: &["vpn", "虚拟专用网络"],
    },
    SettingsEntry {
        id: "proxy",
        command: "start ms-settings:network-proxy",
        keywords: &["proxy", "代理"],
    },
    SettingsEntry {
        id: "ethernet",
        command: "start ms-settings:network-ethernet",
        keywords: &["ethernet", "lan", "以太网", "有线网络"],
    },
    // ── 设备 ──────────────────────────────────────────────
    SettingsEntry {
        id: "bluetooth",
        command: "start ms-settings:bluetooth",
        keywords: &["bluetooth", "蓝牙"],
    },
    SettingsEntry {
        id: "devices",
        command: "start ms-settings:connecteddevices",
        keywords: &["devices", "connected devices", "设备", "已连接设备"],
    },
    SettingsEntry {
        id: "printers",
        command: "start ms-settings:printers",
        keywords: &["printer", "printers", "打印机", "扫描仪"],
    },
    SettingsEntry {
        id: "mouse",
        command: "start ms-settings:mousetouchpad",
        keywords: &["mouse", "touchpad", "鼠标", "触摸板"],
    },
    SettingsEntry {
        id: "pen",
        command: "start ms-settings:pen",
        keywords: &["pen", "stylus", "触控笔", "笔"],
    },
    SettingsEntry {
        id: "usb",
        command: "start ms-settings:usb",
        keywords: &["usb"],
    },
    SettingsEntry {
        id: "typing",
        command: "start ms-settings:typing",
        keywords: &["typing", "keyboard", "输入", "键盘"],
    },
    SettingsEntry {
        id: "device-manager",
        command: "start devmgmt.msc",
        keywords: &["device manager", "driver", "设备管理器", "驱动"],
    },
    // ── 个性化 ────────────────────────────────────────────
    SettingsEntry {
        id: "personalization",
        command: "start ms-settings:personalization",
        keywords: &["personalization", "个性化"],
    },
    SettingsEntry {
        id: "background",
        command: "start ms-settings:personalization-background",
        keywords: &["background", "wallpaper", "背景", "壁纸"],
    },
    SettingsEntry {
        id: "colors",
        command: "start ms-settings:colors",
        keywords: &["color", "colors", "颜色", "主题色"],
    },
    SettingsEntry {
        id: "themes",
        command: "start ms-settings:themes",
        keywords: &["theme", "themes", "主题"],
    },
    SettingsEntry {
        id: "lockscreen",
        command: "start ms-settings:lockscreen",
        keywords: &["lock screen", "lockscreen", "锁屏"],
    },
    SettingsEntry {
        id: "taskbar",
        command: "start ms-settings:taskbar",
        keywords: &["taskbar", "任务栏"],
    },
    SettingsEntry {
        id: "fonts",
        command: "start ms-settings:fonts",
        keywords: &["font", "fonts", "字体"],
    },
    SettingsEntry {
        id: "start-menu",
        command: "start ms-settings:personalization-start",
        keywords: &["start menu", "开始菜单", "开始"],
    },
    // ── 系统 ──────────────────────────────────────────────
    SettingsEntry {
        id: "display",
        command: "start ms-settings:display",
        keywords: &["display", "screen", "monitor", "显示", "屏幕", "显示器", "分辨率"],
    },
    SettingsEntry {
        id: "nightlight",
        command: "start ms-settings:nightlight",
        keywords: &["night light", "nightlight", "夜间模式", "护眼"],
    },
    SettingsEntry {
        id: "sound",
        command: "start ms-settings:sound",
        keywords: &["sound", "audio", "volume", "声音", "音频", "音量"],
    },
    SettingsEntry {
        id: "notifications",
        command: "start ms-settings:notifications",
        keywords: &["notification", "notifications", "通知"],
    },
    SettingsEntry {
        id: "focus",
        command: "start ms-settings:quiethours",
        keywords: &["focus assist", "quiet hours", "专注", "勿扰"],
    },
    SettingsEntry {
        id: "power",
        command: "start ms-settings:powersleep",
        keywords: &["power", "sleep", "电源", "睡眠"],
    },
    SettingsEntry {
        id: "battery",
        command: "start ms-settings:batterysaver",
        keywords: &["battery", "battery saver", "电池", "节电"],
    },
    SettingsEntry {
        id: "storage",
        command: "start ms-settings:storagesense",
        keywords: &["storage", "disk", "存储", "磁盘", "空间"],
    },
    SettingsEntry {
        id: "multitasking",
        command: "start ms-settings:multitasking",
        keywords: &["multitasking", "多任务"],
    },
    SettingsEntry {
        id: "remote-desktop",
        command: "start ms-settings:remotedesktop",
        keywords: &["remote desktop", "远程桌面"],
    },
    SettingsEntry {
        id: "about",
        command: "start ms-settings:about",
        keywords: &["about", "system info", "specs", "关于", "系统信息", "配置"],
    },
    SettingsEntry {
        id: "date-time",
        command: "start ms-settings:dateandtime",
        keywords: &["date", "time", "time zone", "日期", "时间", "时区"],
    },
    SettingsEntry {
        id: "language",
        command: "start ms-settings:regionlanguage",
        keywords: &["language", "region", "语言", "区域"],
    },
    SettingsEntry {
        id: "search",
        command: "start ms-settings:search",
        keywords: &["search", "indexing", "搜索", "索引"],
    },
    SettingsEntry {
        id: "developers",
        command: "start ms-settings:developers",
        keywords: &["developer", "developers", "开发人员", "开发者"],
    },
    // ── 隐私与安全 ────────────────────────────────────────
    SettingsEntry {
        id: "privacy",
        command: "start ms-settings:privacy",
        keywords: &["privacy", "隐私"],
    },
    SettingsEntry {
        id: "camera",
        command: "start ms-settings:privacy-webcam",
        keywords: &["camera", "webcam", "相机", "摄像头"],
    },
    SettingsEntry {
        id: "microphone",
        command: "start ms-settings:privacy-microphone",
        keywords: &["microphone", "mic", "麦克风"],
    },
    SettingsEntry {
        id: "location",
        command: "start ms-settings:privacy-location",
        keywords: &["location", "定位", "位置"],
    },
    SettingsEntry {
        id: "windows-update",
        command: "start ms-settings:windowsupdate",
        keywords: &["windows update", "update", "更新", "系统更新"],
    },
    SettingsEntry {
        id: "defender",
        command: "start ms-settings:windowsdefender",
        keywords: &["defender", "security", "windows security", "安全", "病毒", "防护"],
    },
    SettingsEntry {
        id: "recovery",
        command: "start ms-settings:recovery",
        keywords: &["recovery", "reset", "恢复", "重置"],
    },
    SettingsEntry {
        id: "troubleshoot",
        command: "start ms-settings:troubleshoot",
        keywords: &["troubleshoot", "疑难解答"],
    },
    // ── 应用与账户 ────────────────────────────────────────
    SettingsEntry {
        id: "apps",
        command: "start ms-settings:appsfeatures",
        keywords: &["apps", "installed apps", "应用", "已安装"],
    },
    SettingsEntry {
        id: "default-apps",
        command: "start ms-settings:defaultapps",
        keywords: &["default apps", "默认应用"],
    },
    SettingsEntry {
        id: "startup-apps",
        command: "start ms-settings:startupapps",
        keywords: &["startup", "startup apps", "启动项", "开机启动"],
    },
    SettingsEntry {
        id: "accounts",
        command: "start ms-settings:yourinfo",
        keywords: &["account", "accounts", "账户", "账号"],
    },
    SettingsEntry {
        id: "sign-in",
        command: "start ms-settings:signinoptions",
        keywords: &["sign in", "login", "password", "登录", "密码"],
    },
    SettingsEntry {
        id: "optional-features",
        command: "start ms-settings:optionalfeatures",
        keywords: &["optional features", "features", "可选功能", "功能"],
    },
    // ── 辅助功能与游戏 ────────────────────────────────────
    SettingsEntry {
        id: "accessibility",
        command: "start ms-settings:easeofaccess-display",
        keywords: &["accessibility", "ease of access", "辅助功能", "无障碍"],
    },
    SettingsEntry {
        id: "gaming",
        command: "start ms-settings:gaming-gamebar",
        keywords: &["game bar", "gaming", "游戏", "游戏栏"],
    },
    SettingsEntry {
        id: "game-mode",
        command: "start ms-settings:gaming-gamemode",
        keywords: &["game mode", "游戏模式"],
    },
    SettingsEntry {
        id: "activation",
        command: "start ms-settings:activation",
        keywords: &["activation", "activate", "激活"],
    },
    SettingsEntry {
        id: "maps",
        command: "start ms-settings:maps",
        keywords: &["maps", "offline maps", "地图", "离线地图"],
    },
    SettingsEntry {
        id: "backup",
        command: "start ms-settings:backup",
        keywords: &["backup", "备份"],
    },
    // ── 经典控制面板 ──────────────────────────────────────
    SettingsEntry {
        id: "services",
        command: "start services.msc",
        keywords: &["services", "服务"],
    },
    SettingsEntry {
        id: "program-features",
        command: "start appwiz.cpl",
        keywords: &["programs", "programs and features", "uninstall", "程序和功能", "卸载"],
    },
    SettingsEntry {
        id: "power-options",
        command: "start powercfg.cpl",
        keywords: &["power options", "电源选项"],
    },
    SettingsEntry {
        id: "network-connections",
        command: "start ncpa.cpl",
        keywords: &["network connections", "adapter", "网络连接", "网卡"],
    },
    SettingsEntry {
        id: "system-properties",
        command: "start sysdm.cpl",
        keywords: &["system properties", "environment variables", "系统属性", "环境变量"],
    },
    SettingsEntry {
        id: "control-panel",
        command: "start control",
        keywords: &["control panel", "控制面板"],
    },
];

/// 数据源设置状态：启用的设置项 id 集合。
struct SettingsState {
    enabled: HashSet<String>,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            enabled: ENTRIES.iter().map(|entry| entry.id.to_string()).collect(),
        }
    }
}

/// 宿主下发的设置载荷。字段缺失/为 null 时保留当前值（避免部分载荷误清空）。
#[derive(Deserialize)]
struct SettingsPatch {
    #[serde(default)]
    enabled_settings: Option<Vec<String>>,
}

/// 插件主组件：仅承载组件清单，不参与触发词路由。
struct WindowsSettingsPlugin {
    core: ComponentCore,
}

impl WindowsSettingsPlugin {
    fn new() -> Self {
        Self {
            core: ComponentCore::new(
                PLUGIN_ID.to_string(),
                t_key("plugin.name"),
                t_key("plugin.description"),
                ComponentType::Plugin,
                100,
            ),
        }
    }
}

#[async_trait]
impl Configurable for WindowsSettingsPlugin {
    fn core(&self) -> &ComponentCore {
        &self.core
    }

    fn setting_schema(&self) -> Vec<SettingDefinition> {
        vec![]
    }
}

#[async_trait]
impl Plugin for WindowsSettingsPlugin {
    async fn init(
        &self,
        _ctx: &PluginContext,
        _handle: Option<std::sync::Arc<PluginHandle>>,
    ) -> Result<(), PluginError> {
        Ok(())
    }

    async fn query(
        &self,
        _ctx: &PluginContext,
        _query: &Query,
    ) -> Result<QueryResponse, PluginError> {
        // 无触发词路由，查询不应到达；防御式返回空结果。
        Ok(QueryResponse::Empty)
    }

    async fn execute_action(
        &self,
        _ctx: &PluginContext,
        _action_id: &str,
        _payload: serde_json::Value,
    ) -> Result<(), PluginError> {
        Ok(())
    }
}

/// Windows 设置数据源：把启用的设置项转换为搜索候选。
struct WindowsSettingsSource {
    core: ComponentCore,
    settings: RwLock<SettingsState>,
}

impl WindowsSettingsSource {
    fn new() -> Self {
        Self {
            core: ComponentCore::new(
                SOURCE_ID.to_string(),
                t_key("source.name"),
                t_key("source.description"),
                ComponentType::DataSource,
                100,
            ),
            settings: RwLock::new(SettingsState::default()),
        }
    }
}

#[async_trait]
impl Configurable for WindowsSettingsSource {
    fn core(&self) -> &ComponentCore {
        &self.core
    }

    fn setting_schema(&self) -> Vec<SettingDefinition> {
        let ids: Vec<String> = ENTRIES.iter().map(|entry| entry.id.to_string()).collect();
        let labels: Vec<String> = ENTRIES
            .iter()
            .map(|entry| t_key(&format!("entry.{}.name", entry.id)))
            .collect();

        vec![SettingDefinition {
            key: ENABLED_SETTINGS.to_string(),
            schema: SchemaNode {
                kind: SchemaKind::Array {
                    items: Box::new(SchemaNode {
                        kind: SchemaKind::String {
                            enum_values: ids.clone(),
                            enum_labels: labels,
                            min_length: None,
                            max_length: None,
                            pattern: None,
                        },
                        default: None,
                    }),
                    item_widget: None,
                    min_items: None,
                    max_items: None,
                },
                default: Some(serde_json::json!(ids)),
            },
            ui: FieldUiMetadata {
                pointer: format!("/{ENABLED_SETTINGS}"),
                label: t_key("field.enabled.label"),
                description: t_key("field.enabled.desc"),
                note: Some(t_key("field.enabled.note")),
                group: None,
                order: 0,
                visible: true,
                read_only: false,
                sensitive: false,
                visible_when: None,
                widget: Some(WidgetHint::MultiSelect),
                action: None,
                detail_action: None,
            },
        }]
    }

    fn get_settings(&self) -> serde_json::Value {
        let enabled = {
            let state = self.settings.read().expect("设置状态锁被污染");
            state.enabled.clone()
        };
        let list: Vec<&str> = ENTRIES
            .iter()
            .filter(|entry| enabled.contains(entry.id))
            .map(|entry| entry.id)
            .collect();
        serde_json::json!({ ENABLED_SETTINGS: list })
    }

    async fn apply_settings(&self, settings: serde_json::Value) -> Result<(), ConfigError> {
        let patch: SettingsPatch = serde_json::from_value(settings)
            .map_err(|e| ConfigError::ValidationFailed(e.to_string()))?;
        if let Some(enabled) = patch.enabled_settings {
            // 只保留内置条目，未知 id 直接丢弃（不会产生候选项）。
            let known: HashSet<&str> = ENTRIES.iter().map(|entry| entry.id).collect();
            let filtered: HashSet<String> = enabled
                .into_iter()
                .filter(|id| known.contains(id.as_str()))
                .collect();
            self.settings
                .write()
                .expect("设置状态锁被污染")
                .enabled = filtered;
        }
        Ok(())
    }
}

#[async_trait]
impl DataSource for WindowsSettingsSource {
    async fn fetch_candidates(&self) -> CachedCandidateData {
        // 一次加锁取快照，守卫在 await 前释放（避免同步锁跨 await）。
        let enabled = {
            let state = self.settings.read().expect("设置状态锁被污染");
            state.enabled.clone()
        };
        let locale = host().get_locale().await.unwrap_or_default();
        let catalog = load_catalog(&locale);
        let icon = IconRequest::Path(settings_icon_path());

        let mut result = CachedCandidateData::new();
        for entry in ENTRIES {
            if !enabled.contains(entry.id) {
                continue;
            }
            result.add_candidate(SearchCandidate {
                id: 0,
                name: entry_name(&catalog, entry.id),
                icon: icon.clone(),
                target: ExecutionTarget::Command(entry.command.to_string()),
                keywords: entry.keywords.iter().map(|word| word.to_string()).collect(),
                bias: 0.0,
                trigger_keywords: Vec::new(),
            });
        }
        result
    }
}

/// 读取内嵌语言包（标题字面量与设置项标签共用同一份译文）。
///
/// 中文（含 zh-Hant）回退到简体语言包，其余语言使用英文包。
fn load_catalog(locale: &str) -> serde_json::Map<String, serde_json::Value> {
    let raw = if locale.to_ascii_lowercase().starts_with("zh") {
        include_str!("../i18n/zh-Hans.json")
    } else {
        include_str!("../i18n/en.json")
    };
    serde_json::from_str(raw).expect("内嵌 i18n 语言包必须是合法 JSON")
}

/// 从语言包读取候选标题；缺失时回退到设置项 id（便于发现漏配译文）。
fn entry_name(catalog: &serde_json::Map<String, serde_json::Value>, id: &str) -> String {
    catalog
        .get(&format!("entry.{id}.name"))
        .and_then(|value| value.as_str())
        .unwrap_or(id)
        .to_string()
}

/// 设置应用可执行文件路径，用于候选项图标提取（缺失时前端回退默认图标）。
fn settings_icon_path() -> String {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
    format!(r"{root}\ImmersiveControlPanel\SystemSettings.exe")
}

/// 装配插件应用（骨架 `main()` 调用 `.run()`）：主组件 + 设置数据源组件。
pub fn app() -> PluginApp {
    PluginApp::new(WindowsSettingsPlugin::new()).with_data_source(WindowsSettingsSource::new())
}
