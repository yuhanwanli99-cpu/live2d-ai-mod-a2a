//! Agent card（发现文档）：A2A 0.3.0 形状，与 0.3.8 主仓实现逐字段一致。
//!
//! 只在内存里生成，不读盘、不读配置：`version` 与 `port` 都是构造
//! [`crate::A2aCore`] 时注入的（宿主告诉 Mod「我是谁」），因此卡片里
//! **不可能**出现上游 base_url、密钥、`.env` 路径。

use serde_json::{Value, json};

/// 发现文档路径（A2A 0.3.0 起；**不是**旧的 `/.well-known/agent.json`）。
pub const AGENT_CARD_PATH: &str = "/.well-known/agent-card.json";

/// JSON-RPC 端点路径。
pub const A2A_PATH: &str = "/api/v1/a2a";

/// 本实现声明的 A2A 协议版本（JSON-RPC 绑定）。
pub const PROTOCOL_VERSION: &str = "0.3.0";

/// 唯一的 skill id（0.3.8 执行单 §3 钉死，线格式不能变）。
const SKILL_ID: &str = "llm-chat";

/// 请求时生成 agent card。
///
/// - `version` = 宿主版本（生产传 `env!("CARGO_PKG_VERSION")` 的克隆）；
/// - `url` / `supportedInterfaces` **只**指向本服务 `POST /api/v1/a2a`；
/// - streaming / pushNotifications 都是 false（本版不支持）；
/// - 一个 skill：`llm-chat`。
pub fn agent_card(version: &str, port: u16) -> Value {
    let url = format!("http://127.0.0.1:{port}{A2A_PATH}");
    json!({
        "name": "Live2D-Ai",
        "description": "Live2D-Ai 本地服务：把当前已配置的 OpenAI-compatible 对话模型暴露成 A2A agent（纯文本进、纯文本出）。",
        "version": version,
        "protocolVersion": PROTOCOL_VERSION,
        "url": url.clone(),
        "supportedInterfaces": [{
            "url": url,
            "protocolBinding": "JSONRPC",
            "protocolVersion": PROTOCOL_VERSION,
        }],
        "capabilities": {
            "streaming": false,
            "pushNotifications": false,
        },
        "defaultInputModes": ["text/plain"],
        "defaultOutputModes": ["text/plain"],
        "skills": [{
            "id": SKILL_ID,
            "name": "LLM chat",
            "description": "把文本交给当前已配置的对话模型，回文本",
            "tags": ["chat", "llm", "text"],
        }],
    })
}
