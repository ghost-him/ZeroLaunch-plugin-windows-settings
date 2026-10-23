//! Hello World 示例插件——**业务文件**：不参与模板同步（`.templatesyncignore` 排除 `src/plugin.rs`）。
//!
//! 插件级元数据（id / name / version / description / author / triggerKeywords / supportedOs /
//! priority / mode / hotkey / 图标）由宿主读 `manifest.toml` 构造；本文件只实现行为与**组件级**
//! 描述符（`Configurable::core()`）。
//!
//! 照常实现宿主的 `Plugin` / `Configurable` trait；启动骨架（`src/main.rs`）只负责 `init()` + `app().run()`。
//! 面向用户的文本用 `t_key("key")` 生成命名空间键（`plugin.<插件id>.<key>`），译文放 `i18n/<lang>.json`。

use std::sync::Arc;

use async_trait::async_trait;
use zerolaunch_plugin_api::config::{
    ComponentCore, ComponentType, Configurable, SettingDefinition,
};
use zerolaunch_plugin_api::{
    Plugin, PluginContext, PluginError, PluginHandle,
    Query, QueryResponse, ListItem, ResultAction,
};
use zerolaunch_plugin_sdk_rust::{PluginApp, t_key};

/// Hello World 示例插件 — 演示第三方插件的最小骨架与 i18n 用法。
///
/// 仅作为插件模板使用；文本经 `t_key()` 生成命名空间翻译键
/// （`plugin.com.example.hello-world.<key>`），宿主加载插件目录
/// `i18n/<lang>.json` 语言包后，前端对 key-or-literal 文本自动翻译。
struct HelloWorldPlugin {
    /// 组件级描述符（组件 ID / 名称 / 描述 / 类型 / 优先级），随 `plugin/get_components` 上报；
    /// `Configurable` trait 默认实现委托于此。
    core: ComponentCore,
}

impl HelloWorldPlugin {
    fn new() -> Self {
        Self {
            // 组件 ID 与清单 `[plugin].id` 一致；名称与描述是设置面板里的组件文案。
            core: ComponentCore::new(
                "com.example.hello-world".to_string(),
                "Hello World".to_string(),
                "演示第三方插件最小骨架的示例组件".to_string(),
                ComponentType::Plugin,
                100,
            ),
        }
    }
}

#[async_trait]
impl Configurable for HelloWorldPlugin {
    fn core(&self) -> &ComponentCore {
        &self.core
    }

    /// 本示例无设置项；有设置项时在此声明 schema（label 可用 `t_key!`/`t_key` 形式）。
    fn setting_schema(&self) -> Vec<SettingDefinition> {
        vec![]
    }
}

#[async_trait]
impl Plugin for HelloWorldPlugin {
    async fn init(
        &self,
        _ctx: &PluginContext,
        _handle: Option<Arc<PluginHandle>>,
    ) -> Result<(), PluginError> {
        Ok(())
    }

    async fn query(&self, ctx: &PluginContext, query: &Query) -> Result<QueryResponse, PluginError> {
        // ctx.locale 携带宿主当前界面语言（如 "zh-Hans"），可按语言生成本地化文本；
        // 主动查询可用 host().get_locale().await（经 HostProxy）。
        tracing::debug!(locale = %ctx.locale, "hello query");
        Ok(QueryResponse::List {
            results: vec![ListItem {
                id: 1,
                title: format!("Hello: {}", query.raw_query),
                // key-or-literal：前端命中翻译目录则显示译文，否则回退 key 原文。
                // t_key() 自动带当前插件 id 前缀（plugin.com.example.hello-world.<key>）
                subtitle: t_key("greeting"),
                icon: zerolaunch_plugin_api::services::icon_request::IconRequest::Path(String::new()),
                score: 1.0,
                actions: vec![ResultAction {
                    id: "hello".to_string(),
                    label: t_key("sayHello"),
                    icon: zerolaunch_plugin_api::services::icon_request::IconRequest::Path(String::new()),
                    is_default: true,
                    shortcut_key: String::new(),
                }],
                target_type: "BuiltinCommand".to_string(),
                user_arg_count: 0,
                has_system_params: false,
                trigger_keywords: vec![],
            }],
        })
    }

    async fn execute_action(&self, _ctx: &PluginContext, _action_id: &str, _payload: serde_json::Value) -> Result<(), PluginError> {
        tracing::info!("Hello World action executed!");
        Ok(())
    }
}

/// 装配插件应用（骨架 `main()` 调用 `.run()`）。
pub fn app() -> PluginApp {
    PluginApp::new(HelloWorldPlugin::new())
}
