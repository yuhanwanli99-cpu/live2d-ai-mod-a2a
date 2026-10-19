//! 上游失败的**脱敏**（0.3.9：从主仓 `a2a_routes/a2a_llm.rs` 迁入）。
//!
//! 生产口径：宿主注入的 chat 应只回**稳定错误码**（如
//! `上游对话失败（llm_upstream_401）`）。这一层是**防御性兜底**：协议核在把
//! 错误写进 JSON-RPC 响应 / 日志之前再查一遍，保证即使注入实现把上游 body
//! 或 Authorization 原样带出来，密钥也不会出现在 A2A 响应与日志里。
//!
//! 规则（逐条可测）：
//! 1. `Bearer` / `bearer` 后面的那个 token 整段替换成 `[redacted]`；
//! 2. 以 `sk-` 开头的 token 整段替换成 `[redacted]`（OpenAI 风格密钥前缀）；
//! 3. 只保留前 [`MAX_ERROR_CHARS`] 个字符，超出截断并补一个 `…`
//!    （上游 body 可能很长，不把整段回给客户端）；
//! 4. 清洗后为空 → 回一句稳定的兜底描述（不编上游原因）。

/// 单条错误描述上限（字符数，不是字节；中文也算一个字符）。
pub const MAX_ERROR_CHARS: usize = 300;

/// 兜底描述（清洗后为空时用；不含上游信息）。
const FALLBACK: &str = "上游对话失败（llm_upstream_failed）";

/// 把一条上游失败描述变成**可回给客户端 / 可写日志**的脱敏文本。
pub fn sanitize_error(raw: &str) -> String {
    let redacted = redact_secret_tokens(raw.trim());
    let mut out: String = redacted.chars().take(MAX_ERROR_CHARS).collect();
    if redacted.chars().count() > MAX_ERROR_CHARS {
        out.push('…');
    }
    if out.is_empty() {
        return FALLBACK.to_string();
    }
    out
}

/// 把 `Bearer` 后的 token 与 `sk-` 前缀 token 替换成 `[redacted]`。
///
/// 按空白切分再拼回（错误描述不是正文，不需要保留原始换行 / 缩进）。
fn redact_secret_tokens(input: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    let mut redact_next = false;
    for token in input.split_whitespace() {
        let is_bearer = token.eq_ignore_ascii_case("bearer")
            || token.eq_ignore_ascii_case("bearer:")
            || token.eq_ignore_ascii_case("authorization:");
        if redact_next || token.starts_with("sk-") {
            parts.push("[redacted]");
            redact_next = false;
        } else if is_bearer {
            parts.push(token);
            redact_next = true;
        } else {
            parts.push(token);
        }
    }
    parts.join(" ")
}
