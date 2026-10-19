//! 宿主胶水（0.3.9 rc.3b）：工厂 + 运行时**薄壳**。
//!
//! 协议核住 \`core.rs\` / \`rpc.rs\`（不依赖宿主）。本文件只把协议核挂进
//! \`live2d-ai-mod-system\` 的静态注册表，让主仓 \`AVAILABLE_MOD_FACTORIES\` 能收录
//! id \`a2a\`、缺省停用。
//!
//! # 薄壳边界（红线）
//!
//! - **不订阅任何事件**、不注册设置 schema、不发 cue、不碰 \`action_tx\`；
//! - \`start\` 只做一件事：把协议核单例建起来（\`core()\`，任务表随实例持久）；
//! - \`state_json\` 只回「本 Mod 是谁 + 任务表上限」，**不含**请求 / 任务内容；
//! - 不读 \`[llm]\` / \`.env\` / 密钥：上游由**宿主门**注入（见主仓 \`web_api/a2a_gate\`）。

use live2d_ai_mod_system::*;

use crate::{MOD_ID, TASK_STORE_CAP, core};

/// 本 Mod 的静态描述符（与宿主 \`AVAILABLE_MOD_FACTORIES\` 的注册 id 同一字符串）。
pub const DESCRIPTOR: ModDescriptor = ModDescriptor {
    id: MOD_ID,
    name: "A2A 对外接口",
    version: "0.3.1",
    api_version: MOD_API_VERSION,
};

/// 运行时：只有「把协议核建起来」这一个副作用。
pub struct A2aRuntime;

impl ModRuntime for A2aRuntime {
    fn start(&mut self, _registrar: &mut dyn ModRegistrar) -> Result<(), ModError> {
        // 建单例即完成接线：宿主门经 \`live2d_ai_mod_a2a::core()\` 取同一个实例，
        // 因此任务表在多次 HTTP 请求之间是同一张（进程内、上限 32、不落盘）。
        let _ = core();
        Ok(())
    }

    fn state_json(&mut self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "mod": MOD_ID,
            "tasks_cap": TASK_STORE_CAP,
        }))
    }

    fn shutdown(&mut self) -> Result<(), ModError> {
        Ok(())
    }
}

/// 静态工厂（注册进 \`AVAILABLE_MOD_FACTORIES\` 时用 \`&FACTORY\`）。
pub struct A2aFactory;

impl ModFactory for A2aFactory {
    fn descriptor(&self) -> &'static ModDescriptor {
        &DESCRIPTOR
    }

    /// 本 Mod **缺省停用**、也没有用户可填的配置（协议核不吃配置）：
    /// 返回 \`None\`，不给前端多一张空卡。
    fn settings_spec(&self) -> Option<ModSettingsSpec> {
        None
    }

    fn create(
        &self,
        _services: ModServices,
        _config: serde_json::Value,
    ) -> Result<Box<dyn ModRuntime>, ModError> {
        Ok(Box::new(A2aRuntime))
    }
}

/// 工厂单例。
pub const FACTORY: A2aFactory = A2aFactory;
