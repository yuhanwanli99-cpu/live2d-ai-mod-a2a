//! 协议核实例：一次请求 → 响应 JSON + HTTP 状态码（0.3.9 rc.3）。
//!
//! 0.3.8 把「路径分流 + 安全门 + 协议」写在 `web_api/a2a_routes.rs`；0.3.9 之后
//! 本层**只做协议**：不校验 Origin / Content-Type / loopback（那是宿主
//! `a2a_gate` 的职责），也不依赖 tiny_http。
//!
//! 任务表属于实例：`A2aCore` 内持 `Mutex<TaskStore>`；进程内单例由 [`core`]
//! 提供（宿主门与 Mod 运行时薄壳取同一个实例，任务跨请求存活；进程重启即消失）。

use std::sync::{Mutex, OnceLock};

use serde_json::{Value, json};

use crate::card::{A2A_PATH, AGENT_CARD_PATH, agent_card};
use crate::rpc;
use crate::tasks::{TASK_STORE_CAP, TaskStore};

/// 上游对话：一段文本 → 一段正文；`Err` = **已脱敏**的失败描述。
pub type ChatFn<'a> = dyn Fn(&str) -> Result<String, String> + 'a;

/// 日志级别（协议核不直接依赖 tracing 的宿主 target）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// 正常完成。
    Info,
    /// 上游失败等可恢复异常。
    Warn,
}

/// 宿主日志注入：level + 一行文本（宿主决定 target / 结构化字段）。
pub type LogFn<'a> = dyn Fn(LogLevel, &str) + 'a;

/// 协议核响应：宿主负责把 `json` 序列化并补 Content-Type / Cache-Control。
#[derive(Debug, Clone)]
pub struct A2aResponse {
    /// HTTP 状态码：200 / 404 / 405（**JSON-RPC 错误仍是 200**，与 0.3.8 一致）。
    pub status: u16,
    /// 响应体。
    pub json: Value,
    /// 405 且来自 agent card 时为 true：宿主据此补 `Allow: GET`。
    pub allow_get: bool,
}

impl A2aResponse {
    /// 200 + JSON 体。
    fn ok(json: Value) -> Self {
        Self {
            status: 200,
            json,
            allow_get: false,
        }
    }

    /// 非 200（404 / 405）+ JSON 体。
    fn status(status: u16, json: Value) -> Self {
        Self {
            status,
            json,
            allow_get: false,
        }
    }
}

/// 协议核：一次请求 → 响应 JSON + HTTP 状态码。任务表属于实例。
pub struct A2aCore {
    /// agent card 回显的版本（宿主通常是 `env!("CARGO_PKG_VERSION")`）。
    version: String,
    /// 进程内任务表（上限见 [`TASK_STORE_CAP`]，不落盘）。
    tasks: Mutex<TaskStore>,
}

impl A2aCore {
    /// 生产构造：任务表上限 [`TASK_STORE_CAP`]。
    pub fn new(version: String) -> Self {
        Self::with_capacity(version, TASK_STORE_CAP)
    }

    /// 测试构造：任务表用更小的 `cap`（淘汰语义可断言）。
    pub fn with_tasks(cap: usize) -> Self {
        Self::with_capacity(env!("CARGO_PKG_VERSION").to_string(), cap)
    }

    fn with_capacity(version: String, cap: usize) -> Self {
        Self {
            version,
            tasks: Mutex::new(TaskStore::new(cap)),
        }
    }

    /// GET card / POST JSON-RPC 的统一入口（**只做协议**，不校验 Origin / CT）。
    ///
    /// - `path == AGENT_CARD_PATH`：`GET` → card；其它方法 → 405 + `allow_get`；
    /// - `path == A2A_PATH`：`POST` → 解析 + dispatch；其它方法 → 405；
    /// - 其它 path：404 形状（宿主不会调用）。
    pub fn handle(
        &self,
        method: &str,
        path: &str,
        body: &str,
        port: u16,
        chat: &ChatFn<'_>,
        log: &LogFn<'_>,
    ) -> A2aResponse {
        if path == AGENT_CARD_PATH {
            if method != "GET" {
                let mut response = A2aResponse::status(
                    405,
                    api_error("method_not_allowed", "agent card 只支持 GET"),
                );
                response.allow_get = true;
                return response;
            }
            return A2aResponse::ok(agent_card(&self.version, port));
        }
        if path != A2A_PATH {
            return A2aResponse::status(404, api_error("not_found", "未知的 A2A 路径"));
        }
        if method != "POST" {
            return A2aResponse::status(
                405,
                api_error("method_not_allowed", "POST /api/v1/a2a 仅支持 POST"),
            );
        }
        let response = match rpc::parse_rpc_request(body) {
            Ok(rpc) => rpc::dispatch_rpc(&self.tasks, rpc, chat, log),
            Err(error) => error,
        };
        A2aResponse::ok(response)
    }
}

/// 进程内唯一协议核实例（OnceLock 单例）：任务表随实例持久。
///
/// 版本用本 crate 的 `env!("CARGO_PKG_VERSION")`（与宿主同为 0.3.1），
/// 宿主测试断言 `card.version == env!("CARGO_PKG_VERSION")`。
pub fn core() -> &'static A2aCore {
    static CORE: OnceLock<A2aCore> = OnceLock::new();
    CORE.get_or_init(|| A2aCore::new(env!("CARGO_PKG_VERSION").to_string()))
}

/// 传输层（非 JSON-RPC）错误体：与外部注入端点同款。
fn api_error(code: &str, message: &str) -> Value {
    json!({"error": {"code": code, "message": message}})
}
