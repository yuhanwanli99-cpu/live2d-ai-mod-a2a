//! a2a 协议核测试共享脚手架（0.3.9 rc.3）。
//!
//! 只给同目录 tests_*.rs 用：假上游闭包 / 日志记录 / 请求构造。**不搬**主仓的
//! mock LLM HTTP 脚手架——真实 reqwest + ServerContext 的上游回归住主仓
//! `a2a_gate` 测试；本 crate 只有注入闭包，不碰网络。
#![allow(dead_code)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use crate::{A2A_PATH, A2aCore, A2aResponse, ChatFn, LogFn, LogLevel};

/// card 的 url 用的端口（与主仓 0.3.8 回归同一固定值）。
pub(super) const PORT: u16 = 18080;

/// 生产形状的协议核（任务表 cap = TASK_STORE_CAP）。
pub(super) fn test_core() -> A2aCore {
    A2aCore::new(env!("CARGO_PKG_VERSION").to_string())
}

/// 一次 `POST /api/v1/a2a` 调用，不记日志。
pub(super) fn call(core: &A2aCore, body: &str, chat: &ChatFn<'_>) -> A2aResponse {
    call_logged(core, "POST", A2A_PATH, body, chat, &discard_log())
}

/// 指定 method / path / 日志的原始调用（宿主门只做这些转发）。
pub(super) fn call_logged(
    core: &A2aCore,
    method: &str,
    path: &str,
    body: &str,
    chat: &ChatFn<'_>,
    log: &LogFn<'_>,
) -> A2aResponse {
    core.handle(method, path, body, PORT, chat, log)
}

/// 丢弃日志的注入实现。
pub(super) fn discard_log() -> impl Fn(LogLevel, &str) {
    |_level: LogLevel, _line: &str| {}
}

/// 记录日志行的注入实现（断言级别 / 脱敏）。
#[allow(clippy::type_complexity)]
pub(super) fn recording_log() -> (impl Fn(LogLevel, &str), Arc<Mutex<Vec<(LogLevel, String)>>>) {
    let lines = Arc::new(Mutex::new(Vec::new()));
    let lines_for_log = Arc::clone(&lines);
    let log = move |level: LogLevel, line: &str| {
        lines_for_log
            .lock()
            .expect("log lock")
            .push((level, line.to_string()));
    };
    (log, lines)
}

/// 构造 JSON-RPC 请求体（id 固定 7，与 0.3.8 回归一致）。
pub(super) fn rpc_body(method: &str, params: Value) -> String {
    json!({"jsonrpc": "2.0", "id": 7, "method": method, "params": params}).to_string()
}

/// 一条最小 user 文本消息的 params。
pub(super) fn send_params(text: &str) -> Value {
    json!({
        "message": {
            "kind": "message",
            "messageId": "client-msg-1",
            "role": "user",
            "parts": [{"kind": "text", "text": text}],
        }
    })
}

/// 注入点假上游：固定回复 + 调用计数。
pub(super) fn counting_chat(
    reply: &'static str,
) -> (impl Fn(&str) -> Result<String, String>, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_for_chat = Arc::clone(&hits);
    let chat = move |_text: &str| -> Result<String, String> {
        hits_for_chat.fetch_add(1, Ordering::Relaxed);
        Ok(reply.to_string())
    };
    (chat, hits)
}

/// 注入点假上游：记录收到的文本（证明原文原样交给上游）。
#[allow(clippy::type_complexity)]
pub(super) fn recording_chat() -> (
    impl Fn(&str) -> Result<String, String>,
    Arc<Mutex<Vec<String>>>,
) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seen_for_chat = Arc::clone(&seen);
    let chat = move |text: &str| -> Result<String, String> {
        seen_for_chat
            .lock()
            .expect("seen lock")
            .push(text.to_string());
        Ok("收到。".to_string())
    };
    (chat, seen)
}

/// 注入点假上游：恒失败（描述按生产路径的脱敏口径）。
pub(super) fn failing_chat(
    message: &'static str,
) -> (impl Fn(&str) -> Result<String, String>, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_for_chat = Arc::clone(&hits);
    let chat = move |_text: &str| -> Result<String, String> {
        hits_for_chat.fetch_add(1, Ordering::Relaxed);
        Err(message.to_string())
    };
    (chat, hits)
}
