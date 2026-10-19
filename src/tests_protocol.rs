//! JSON-RPC 协议回归（0.3.9 rc.3；断言迁自主仓 a2a_routes_tests_protocol.rs）。
//!
//! 只覆盖协议层：解析 / dispatch / 拒绝表 / message/send 形状。Origin /
//! Content-Type / 线程模型 / 真实 HTTP 上游住主仓 `a2a_gate` 测试。
use std::sync::atomic::Ordering;

use serde_json::{Value, json};

use super::tests_support::*;

const REPLY: &str = "你好，世界。";

// ---------------------------------------------------------------- message/send

/// 一次 message/send：一条 completed Task + 只打一下上游。
#[test]
fn message_send_returns_completed_task_and_hits_upstream_once() {
    let (chat, hits) = counting_chat(REPLY);
    let resp = call(
        &test_core(),
        &rpc_body("message/send", send_params("你好")),
        &chat,
    );
    assert_eq!(resp.status, 200);
    let body = &resp.json;
    assert_eq!(body["jsonrpc"], "2.0");
    assert_eq!(body["id"], 7);
    let task = &body["result"];
    assert_eq!(task["kind"], "task");
    assert_eq!(task["status"]["state"], "completed");
    assert!(task["id"].as_str().is_some_and(|s| !s.is_empty()));
    assert!(task["contextId"].as_str().is_some_and(|s| !s.is_empty()));
    // 规范生成物位置：artifacts[0].parts[0].text。
    assert_eq!(task["artifacts"][0]["parts"][0]["kind"], "text");
    assert_eq!(task["artifacts"][0]["parts"][0]["text"], REPLY);
    // 执行单要求的「一条 agent 消息、一个 text part」。
    assert_eq!(task["status"]["message"]["kind"], "message");
    assert_eq!(task["status"]["message"]["role"], "agent");
    assert_eq!(task["status"]["message"]["parts"][0]["text"], REPLY);
    assert_eq!(
        task["status"]["message"]["parts"]
            .as_array()
            .expect("parts 数组")
            .len(),
        1
    );
    assert_eq!(hits.load(Ordering::Relaxed), 1, "一次 send 只打一上游");
}

/// 正文在两处**逐字相同**（status.message 与 artifacts[0]）。
#[test]
fn message_send_puts_the_same_text_in_status_message_and_artifact() {
    let (chat, _) = counting_chat(REPLY);
    let resp = call(
        &test_core(),
        &rpc_body("message/send", send_params("你好")),
        &chat,
    );
    let task = &resp.json["result"];
    assert_eq!(
        task["status"]["message"]["parts"][0]["text"],
        task["artifacts"][0]["parts"][0]["text"],
    );
    assert_eq!(task["status"]["message"]["parts"][0]["text"], REPLY);
}

/// A2A 收到的原文 → 原样交给一路 LLM；task/context id 回填到消息里。
#[test]
fn message_send_passes_the_user_text_to_the_single_chat_call() {
    let (chat, seen) = recording_chat();
    let resp = call(
        &test_core(),
        &rpc_body("message/send", send_params("请用一句话回答")),
        &chat,
    );
    let seen = seen.lock().expect("seen lock").clone();
    assert_eq!(seen, vec!["请用一句话回答".to_string()]);
    assert_eq!(
        resp.json["result"]["status"]["message"]["taskId"],
        resp.json["result"]["id"]
    );
    assert_eq!(
        resp.json["result"]["status"]["message"]["contextId"],
        resp.json["result"]["contextId"]
    );
}

/// 多条 text part 用换行拼成一段再送上游（顺序保持）。
#[test]
fn message_send_joins_multiple_text_parts_with_newlines() {
    let (chat, seen) = recording_chat();
    let params = json!({"message": {
        "kind": "message",
        "messageId": "m-1",
        "role": "user",
        "parts": [
            {"kind": "text", "text": "第一行"},
            {"kind": "text", "text": "第二行"},
        ],
    }});
    let resp = call(&test_core(), &rpc_body("message/send", params), &chat);
    assert_eq!(resp.status, 200);
    assert_eq!(
        seen.lock().expect("seen lock").clone(),
        vec!["第一行\n第二行".to_string()]
    );
}

/// 客户端带来的 contextId 原样回填（多轮上下文的锚点）。
#[test]
fn message_send_echoes_client_context_id() {
    let (chat, _) = counting_chat(REPLY);
    let params = json!({"message": {
        "kind": "message",
        "messageId": "m-1",
        "role": "user",
        "contextId": "ctx-client-1",
        "parts": [{"kind": "text", "text": "你好"}],
    }});
    let resp = call(&test_core(), &rpc_body("message/send", params), &chat);
    assert_eq!(resp.json["result"]["contextId"], "ctx-client-1");
}

/// 空文本 / 空白文本 → invalid params（-32602），且**不打上游**。
#[test]
fn message_send_rejects_blank_text_without_calling_upstream() {
    for text in ["", "   ", "\n\t "] {
        let (chat, hits) = counting_chat(REPLY);
        let resp = call(
            &test_core(),
            &rpc_body("message/send", send_params(text)),
            &chat,
        );
        assert_eq!(resp.status, 200);
        assert_eq!(resp.json["error"]["code"], -32602, "text={text:?}");
        assert_eq!(hits.load(Ordering::Relaxed), 0, "空文本不得打上游");
    }
}

/// 只接受 role=user（A2A 的 agent 角色回灌不算用户输入）。
#[test]
fn message_send_rejects_non_user_role() {
    let (chat, hits) = counting_chat(REPLY);
    let params = json!({"message": {
        "kind": "message",
        "messageId": "m-1",
        "role": "agent",
        "parts": [{"kind": "text", "text": "你好"}],
    }});
    let resp = call(&test_core(), &rpc_body("message/send", params), &chat);
    assert_eq!(resp.json["error"]["code"], -32602);
    assert_eq!(hits.load(Ordering::Relaxed), 0);
}

/// parts 只收 kind=text：file / data part 明确 invalid params，不打上游。
#[test]
fn message_send_rejects_file_and_data_parts() {
    for part in [
        json!({"kind": "file", "file": {"uri": "file:///tmp/x"}}),
        json!({"kind": "data", "data": {"a": 1}}),
    ] {
        let (chat, hits) = counting_chat(REPLY);
        let params = json!({"message": {
            "kind": "message",
            "messageId": "m-1",
            "role": "user",
            "parts": [part],
        }});
        let resp = call(&test_core(), &rpc_body("message/send", params), &chat);
        assert_eq!(resp.json["error"]["code"], -32602, "{}", resp.json);
        assert_eq!(hits.load(Ordering::Relaxed), 0);
    }
}

/// 上游失败 → JSON-RPC error（-32603），描述原样来自已脱敏层（不改一个字节）。
#[test]
fn message_send_maps_upstream_failure_to_jsonrpc_error() {
    let (chat, hits) = failing_chat("上游对话失败（llm_upstream_401）");
    let resp = call(
        &test_core(),
        &rpc_body("message/send", send_params("你好")),
        &chat,
    );
    assert_eq!(resp.status, 200, "JSON-RPC 错误也是 HTTP 200");
    assert_eq!(resp.json["error"]["code"], -32603);
    assert_eq!(
        resp.json["error"]["message"],
        "上游对话失败（llm_upstream_401）"
    );
    assert_eq!(hits.load(Ordering::Relaxed), 1);
}

// ---------------------------------------------------------------- 非法请求

/// 未实现的方法 → method not found（-32601），不打上游。
#[test]
fn unknown_method_is_method_not_found() {
    let (chat, hits) = counting_chat(REPLY);
    let resp = call(&test_core(), &rpc_body("foo/bar", json!({})), &chat);
    assert_eq!(resp.status, 200);
    assert_eq!(resp.json["error"]["code"], -32601);
    assert_eq!(hits.load(Ordering::Relaxed), 0);
}

/// 坏 JSON → Parse error（-32700），id = null。
#[test]
fn malformed_json_is_parse_error() {
    let (chat, _) = counting_chat(REPLY);
    let resp = call(&test_core(), "这不是 JSON", &chat);
    assert_eq!(resp.status, 200);
    assert_eq!(resp.json["jsonrpc"], "2.0");
    assert_eq!(resp.json["id"], Value::Null);
    assert_eq!(resp.json["error"]["code"], -32700);
}

/// JSON-RPC id 原样回显：字符串 / 整数 / null 都是合法 id 类型。
#[test]
fn jsonrpc_id_is_echoed_verbatim() {
    for (body, expected) in [
        (
            r#"{"jsonrpc":"2.0","id":"abc","method":"foo/bar","params":{}}"#,
            json!("abc"),
        ),
        (
            r#"{"jsonrpc":"2.0","id":42,"method":"foo/bar","params":{}}"#,
            json!(42),
        ),
        (
            r#"{"jsonrpc":"2.0","id":null,"method":"foo/bar"}"#,
            Value::Null,
        ),
    ] {
        let (chat, _) = counting_chat(REPLY);
        let resp = call(&test_core(), body, &chat);
        assert_eq!(resp.json["id"], expected);
    }
}

/// 连 params 都不给 → invalid params（-32602），不打上游。
#[test]
fn message_send_without_params_is_invalid_params() {
    let (chat, hits) = counting_chat(REPLY);
    let resp = call(
        &test_core(),
        r#"{"jsonrpc":"2.0","id":8,"method":"message/send"}"#,
        &chat,
    );
    assert_eq!(resp.status, 200);
    assert_eq!(resp.json["error"]["code"], -32602);
    assert_eq!(hits.load(Ordering::Relaxed), 0);
}

/// jsonrpc 版本不对 / 缺 method → Invalid Request（-32600），id 原样回显。
#[test]
fn invalid_envelope_is_invalid_request() {
    let (chat, _) = counting_chat(REPLY);
    let resp = call(
        &test_core(),
        r#"{"jsonrpc":"1.0","id":3,"method":"message/send","params":{}}"#,
        &chat,
    );
    assert_eq!(resp.status, 200);
    assert_eq!(resp.json["id"], 3);
    assert_eq!(resp.json["error"]["code"], -32600);

    let resp = call(&test_core(), r#"{"jsonrpc":"2.0","id":4}"#, &chat);
    assert_eq!(resp.json["id"], 4);
    assert_eq!(resp.json["error"]["code"], -32600);
}

/// 非对象 body / id 类型非法 → Invalid Request（-32600），id = null。
#[test]
fn non_object_body_and_bad_id_type_are_invalid_request() {
    let (chat, _) = counting_chat(REPLY);
    let resp = call(&test_core(), "[1,2,3]", &chat);
    assert_eq!(resp.json["id"], Value::Null);
    assert_eq!(resp.json["error"]["code"], -32600);

    let resp = call(
        &test_core(),
        r#"{"jsonrpc":"2.0","id":{"a":1},"method":"message/send","params":{}}"#,
        &chat,
    );
    assert_eq!(resp.json["id"], Value::Null);
    assert_eq!(resp.json["error"]["code"], -32600);
}
