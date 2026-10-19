//! 上游失败语义回归（0.3.9 rc.3；断言迁自主仓 a2a_routes_tests_upstream.rs
//! 里与 HTTP 无关的部分）。
//!
//! 真实 reqwest / mock LLM / ServerContext 部分**不迁**（住主仓 `a2a_gate`
//! 测试）；本文件守 `sanitize_error` 的脱敏口径，以及「失败只回已脱敏描述、
//! 日志同样脱敏 / 不记正文」这两个边界。
use super::tests_support::*;
use super::*;
use crate::sanitize::MAX_ERROR_CHARS;

const SECRET: &str = "sk-super-secret-value";

// ---------------------------------------------------------------- 脱敏

/// 规则 1：`Bearer` 后面的 token 整段替换成 `[redacted]`。
#[test]
fn sanitize_redacts_bearer_token() {
    assert_eq!(sanitize_error("Bearer abc123"), "Bearer [redacted]");
    assert_eq!(sanitize_error("bearer abc123"), "bearer [redacted]");
    assert_eq!(
        sanitize_error(&format!("Bearer {SECRET}")),
        "Bearer [redacted]"
    );
}

/// 规则 2：`sk-` 前缀的 token 整段替换（哪怕没有 Bearer 上下文）。
#[test]
fn sanitize_redacts_sk_prefixed_tokens() {
    let cleaned = sanitize_error(&format!("upstream said {SECRET} invalid api key"));
    assert!(!cleaned.contains(SECRET), "got: {cleaned}");
    assert!(cleaned.contains("[redacted]"), "got: {cleaned}");
    assert!(cleaned.contains("invalid api key"));
}

/// 普通描述原样保留，只做空白裁剪。
#[test]
fn sanitize_keeps_plain_descriptions_and_trims() {
    assert_eq!(
        sanitize_error("  上游对话失败（llm_upstream_401）  "),
        "上游对话失败（llm_upstream_401）"
    );
}

/// 规则 3：超长描述按字符截断到 300 并补一个 `…`。
#[test]
fn sanitize_truncates_at_300_chars_with_ellipsis() {
    let long = "a".repeat(400);
    let out = sanitize_error(&long);
    assert_eq!(
        out.chars().count(),
        MAX_ERROR_CHARS + 1,
        "300 字符 + 省略号"
    );
    assert!(out.ends_with('…'), "got: {out}");
}

/// 规则 4：清洗后为空 → 稳定兜底描述（不编上游原因）。
#[test]
fn sanitize_falls_back_when_input_is_empty() {
    assert_eq!(sanitize_error("   "), "上游对话失败（llm_upstream_failed）");
    assert_eq!(sanitize_error(""), "上游对话失败（llm_upstream_failed）");
}

// ---------------------------------------------------------------- 注入边界

/// 失败：响应与日志都只带 chat 给的**已脱敏**描述，密钥一个字节都不出现。
#[test]
fn upstream_failure_response_and_log_carry_the_sanitized_description() {
    let raw = format!("upstream said Bearer {SECRET}");
    let sanitized = sanitize_error(&raw);
    assert!(!sanitized.contains(SECRET), "脱敏层不得带密钥：{sanitized}");

    let sanitized_for_chat = sanitized.clone();
    let chat = move |_text: &str| -> Result<String, String> { Err(sanitized_for_chat.clone()) };
    let (log, lines) = recording_log();
    let resp = call_logged(
        &test_core(),
        "POST",
        A2A_PATH,
        &rpc_body("message/send", send_params("你好")),
        &chat,
        &log,
    );
    assert_eq!(resp.status, 200, "JSON-RPC 错误也是 HTTP 200");
    assert_eq!(resp.json["error"]["code"], -32603);
    assert_eq!(
        resp.json["error"]["message"], sanitized,
        "chat 的 Err 描述原样回（不改线格式）"
    );
    assert!(!resp.json.to_string().contains(SECRET), "响应不得回显密钥");

    let lines = lines.lock().expect("log lock").clone();
    assert_eq!(lines.len(), 1, "一次失败一条日志：{lines:?}");
    assert_eq!(lines[0].0, LogLevel::Warn);
    assert!(
        lines[0].1.starts_with("code=a2a_upstream_failed"),
        "got: {}",
        lines[0].1
    );
    assert!(lines[0].1.contains(&sanitized), "got: {}", lines[0].1);
    assert!(
        !lines[0].1.contains(SECRET),
        "日志不得回显密钥：{}",
        lines[0].1
    );
}

/// 成功：只有一条 Info 日志，带字符数、不带正文。
#[test]
fn successful_send_logs_info_without_content() {
    let (chat, _) = counting_chat("你好，世界。");
    let (log, lines) = recording_log();
    let resp = call_logged(
        &test_core(),
        "POST",
        A2A_PATH,
        &rpc_body("message/send", send_params("秘密输入")),
        &chat,
        &log,
    );
    assert_eq!(resp.status, 200);
    let lines = lines.lock().expect("log lock").clone();
    assert_eq!(lines.len(), 1, "一次成功一条日志：{lines:?}");
    assert_eq!(lines[0].0, LogLevel::Info);
    assert!(lines[0].1.contains("input_chars=4"), "got: {}", lines[0].1);
    assert!(lines[0].1.contains("output_chars=6"), "got: {}", lines[0].1);
    assert!(
        !lines[0].1.contains("秘密输入"),
        "日志不得记正文：{}",
        lines[0].1
    );
}
