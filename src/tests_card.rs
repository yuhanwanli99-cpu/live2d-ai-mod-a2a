//! agent card 回归（0.3.9 rc.3；断言迁自主仓 a2a_routes_tests_protocol.rs）。
//!
//! 只覆盖协议核的 card 形状 / 404 / 405；Origin / Content-Type / 真实 HTTP 住
//! 主仓 `a2a_gate` 测试。
use std::sync::atomic::Ordering;

use serde_json::json;

use super::tests_support::*;
use super::*;

/// 卡片请求时生成：version = 注入版本，url 只指向本服务端点。
#[test]
fn agent_card_is_generated_with_version_and_self_url() {
    let core = test_core();
    let (chat, _) = counting_chat("不会用到");
    let resp = call_logged(&core, "GET", AGENT_CARD_PATH, "", &chat, &discard_log());
    assert_eq!(resp.status, 200);
    let card = &resp.json;
    assert_eq!(card["name"], "Live2D-Ai");
    assert_eq!(card["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(card["protocolVersion"], "0.3.0");
    assert_eq!(card["url"], "http://127.0.0.1:18080/api/v1/a2a");
    // 执行单 §3：supportedInterfaces 也只指向本服务端点。
    assert_eq!(card["supportedInterfaces"][0]["url"], card["url"]);
    assert_eq!(card["supportedInterfaces"][0]["protocolBinding"], "JSONRPC");
    assert_eq!(card["supportedInterfaces"][0]["protocolVersion"], "0.3.0");
    // 流式与推送都是 false（本版不实现）。
    assert_eq!(card["capabilities"]["streaming"], false);
    assert_eq!(card["capabilities"]["pushNotifications"], false);
    assert_eq!(card["defaultInputModes"], json!(["text/plain"]));
    assert_eq!(card["defaultOutputModes"], json!(["text/plain"]));
}

/// 0.3.0 schema 的必填项一个都不能少（agent card 是发现契约）。
#[test]
fn agent_card_has_every_required_field_of_a2a_0_3_0() {
    let core = test_core();
    let (chat, _) = counting_chat("不会用到");
    let card = call_logged(&core, "GET", AGENT_CARD_PATH, "", &chat, &discard_log()).json;
    for key in [
        "protocolVersion",
        "name",
        "description",
        "url",
        "version",
        "capabilities",
        "defaultInputModes",
        "defaultOutputModes",
        "skills",
    ] {
        assert!(
            card.get(key).is_some(),
            "agent card 缺少必填项 {key}: {card}"
        );
    }
    let skills = card["skills"].as_array().expect("skills 数组");
    assert_eq!(skills.len(), 1, "本版只有一个 skill：{card}");
    let skill = &skills[0];
    assert_eq!(skill["id"], "llm-chat");
    assert_eq!(
        skill["description"],
        "把文本交给当前已配置的对话模型，回文本"
    );
    for key in ["id", "name", "description", "tags"] {
        assert!(skill.get(key).is_some(), "skill 缺少必填项 {key}");
    }
}

/// 卡片不泄露配置：不写上游 base_url、不写密钥、不写 .env。
#[test]
fn agent_card_never_contains_secrets_or_upstream_base_url() {
    let core = test_core();
    let (chat, _) = counting_chat("不会用到");
    let card = call_logged(&core, "GET", AGENT_CARD_PATH, "", &chat, &discard_log()).json;
    let text = card.to_string();
    assert!(!text.contains("api_key"), "卡片不得出现 api_key：{text}");
    assert!(!text.contains("sk-"), "卡片不得出现密钥：{text}");
    assert!(!text.contains(".env"), "卡片不得出现 .env：{text}");
    assert!(!text.contains("base_url"), "卡片不得出现 base_url：{text}");
}

/// card 是只读发现端点：非 GET → 405，且要求宿主补 Allow: GET。
#[test]
fn agent_card_rejects_non_get_with_allow_get() {
    let core = test_core();
    let (chat, hits) = counting_chat("不会用到");
    let resp = call_logged(&core, "POST", AGENT_CARD_PATH, "", &chat, &discard_log());
    assert_eq!(resp.status, 405);
    assert!(resp.allow_get, "405 必须让宿主补 Allow: GET");
    assert_eq!(resp.json["error"]["code"], "method_not_allowed");
    assert_eq!(hits.load(Ordering::Relaxed), 0, "card 不打上游");
}

/// JSON-RPC 端点只收 POST；两条路径之外一律 404（宿主只按精确路径转发）。
#[test]
fn rpc_path_only_accepts_post_and_other_paths_are_404() {
    let core = test_core();
    let (chat, hits) = counting_chat("不会用到");
    let body = rpc_body("message/send", send_params("你好"));

    let get = call_logged(&core, "GET", A2A_PATH, "", &chat, &discard_log());
    assert_eq!(get.status, 405);
    assert!(!get.allow_get, "JSON-RPC 端点不补 Allow: GET");
    assert_eq!(get.json["error"]["code"], "method_not_allowed");

    for path in [
        "/",
        "/api/v1/a2a/",
        "/api/v1/chat",
        "/.well-known/agent.json",
    ] {
        let resp = call_logged(&core, "POST", path, &body, &chat, &discard_log());
        assert_eq!(resp.status, 404, "{path}");
        assert_eq!(resp.json["error"]["code"], "not_found", "{path}");
    }
    assert_eq!(hits.load(Ordering::Relaxed), 0, "路径不匹配不打上游");
}
