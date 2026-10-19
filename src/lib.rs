//! live2d-ai-mod-a2a：A2A（Agent2Agent）对外接口的**纯协议核**（0.3.9 rc.3）。
//!
//! # 它是什么
//!
//! 0.3.8 把 A2A（agent card + JSON-RPC 2.0）写在主仓 `web_api/a2a_routes`。
//! 0.3.9 按产品边界（`docs/architecture/CURRENT.md` §1：A2A 是扩展，不是核心）
//! 迁进一个 Mod：id `a2a`，crate `live2d-ai-mod-a2a`，**缺省停用**。本文件
//! 所在层只放**协议**：
//!
//! - agent card（请求时生成：version / url 由注入值决定）；
//! - JSON-RPC 2.0 解析 + dispatch + 拒绝表；
//! - `message/send`（一条 user 文本 → 注入的 chat → completed Task）；
//! - `tasks/get`（进程内任务表，上限 32，**不落盘**）；
//! - 上游调用的失败脱敏（响应 / 日志都不带密钥与上游 body）。
//!
//! # 它不是什么（红线）
//!
//! - **没有 HTTP server**：不依赖 tiny_http、不读 socket、不做 loopback / Origin /
//!   Content-Type 校验——那是宿主门的职责（搬家的全部意义：宿主只留门）；
//! - **不读 `[llm]` / `.env` / 密钥**：上游是注入的
//!   `chat(text) -> Result<text, err>`。密钥只在宿主按 `[llm].api_key_env` 经
//!   `secrets::lookup` 查找，Mod 拿不到密钥值；
//! - **不碰** supervisor / say / 句子装配 / TTS / 口型 / `action_cue`；不写
//!   `[llm]` / `[tts]`；不发 cue；不接 `action_tx`；
//! - **任务表属于实例、不落盘**：进程内共享由 [`core`]（OnceLock 单例）提供，
//!   宿主门与 Mod 运行时薄壳取同一个实例；进程重启即消失。
//!
//! # 线格式（与 0.3.8 完全一致）
//!
//! - card：A2A 0.3.0 字段，`version` = 注入的宿主版本，`url` 只指向本服务
//!   `POST /api/v1/a2a`；`supportedInterfaces` 保留（1.0 字段，取舍见 0.3.8 发布说明）；
//! - `message/send`：成功回 completed Task（`status.message` 与
//!   `artifacts[0].parts[0]` 两处逐字相同）；上游失败回 `-32603` + 已脱敏描述；
//! - `tasks/get`：按 id 取回同一份 Task；未知 id `-32001`；
//! - 取消（`tasks/cancel`）与其它范围外方法整族拒绝、**不打上游**：已知不实现
//!   的方法 `-32004`，推送配置族 `-32003`，完全未知 `-32601`。

mod card;
mod core;
/// 宿主胶水（工厂 + 运行时薄壳）：把协议核挂进 \`live2d-ai-mod-system\` 的静态注册表。
/// 协议层（core / rpc / card / tasks / sanitize）**不 use** 它。
mod factory;
mod rpc;
mod sanitize;
mod tasks;

pub use self::core::{A2aCore, A2aResponse, ChatFn, LogFn, LogLevel, core};
pub use card::{A2A_PATH, AGENT_CARD_PATH, PROTOCOL_VERSION, agent_card};
pub use factory::{A2aFactory, FACTORY};
pub use sanitize::sanitize_error;
pub use tasks::TASK_STORE_CAP;

/// 本 Mod 的静态 id（与宿主 `AVAILABLE_MOD_FACTORIES` 里的注册 id 同一字符串）。
///
/// rc.3a 先只暴露字符串常量；rc.3b 接线时由 `mod-system` 的 `ModDescriptor`
/// 引用它（避免两处各写一份 id）。
pub const MOD_ID: &str = "a2a";

// 回归测试拆到同目录 tests_*.rs（源文件与测试文件分别守住 500 / 800 行）。
#[cfg(test)]
#[path = "tests_card.rs"]
mod tests_card;
#[cfg(test)]
#[path = "tests_protocol.rs"]
mod tests_protocol;
#[cfg(test)]
#[path = "tests_support.rs"]
mod tests_support;
#[cfg(test)]
#[path = "tests_tasks.rs"]
mod tests_tasks;
#[cfg(test)]
#[path = "tests_upstream.rs"]
mod tests_upstream;
