//! tasks/get + 任务表 + 范围外方法回归（0.3.9 rc.3；断言迁自主仓
//! a2a_routes_tests_tasks.rs）。真实 HTTP / ServerContext 部分不迁。
use std::sync::atomic::Ordering;

use serde_json::{Value, json};

use super::tests_support::*;
use super::*;
use crate::tasks::TaskStore;

/// 发一条 message/send，返回 result Task。
fn send(core: &A2aCore, chat: &ChatFn<'_>, text: &str) -> Value {
    let resp = call(core, &rpc_body("message/send", send_params(text)), chat);
    assert_eq!(resp.status, 200);
    let body = &resp.json;
    assert!(body.get("error").is_none(), "send 不该失败：{body}");
    body["result"].clone()
}

// ---------------------------------------------------------------- tasks/get

/// send 之后能按 id 取回**同一份** completed Task（内存、不落盘）。
#[test]
fn tasks_get_returns_the_same_completed_task() {
    let core = test_core();
    let (chat, _) = counting_chat("你好，世界。");
    let task = send(&core, &chat, "你好");
    let task_id = task["id"].as_str().expect("task id").to_string();

    let got = call(&core, &rpc_body("tasks/get", json!({"id": task_id})), &chat);
    assert_eq!(got.status, 200);
    assert_eq!(got.json["result"], task, "tasks/get 必须回同一份 Task");
    assert_eq!(got.json["result"]["status"]["state"], "completed");
    assert_eq!(
        got.json["result"]["artifacts"][0]["parts"][0]["text"],
        "你好，世界。"
    );
}

/// 未知 id → -32001 TaskNotFound（不是 invalid params）。
#[test]
fn tasks_get_unknown_id_is_task_not_found() {
    let core = test_core();
    let (chat, hits) = counting_chat("不会用到");
    let resp = call(
        &core,
        &rpc_body("tasks/get", json!({"id": "task-does-not-exist"})),
        &chat,
    );
    assert_eq!(resp.status, 200);
    assert_eq!(resp.json["error"]["code"], -32001);
    assert_eq!(hits.load(Ordering::Relaxed), 0, "tasks/get 不打上游");
}

/// 缺 id / 空 id / 非字符串 id → -32602 invalid params。
#[test]
fn tasks_get_missing_id_is_invalid_params() {
    let core = test_core();
    let (chat, _) = counting_chat("不会用到");
    for params in [json!({}), json!({"id": ""}), json!({"id": 7})] {
        let resp = call(&core, &rpc_body("tasks/get", params), &chat);
        assert_eq!(resp.json["error"]["code"], -32602, "{}", resp.json);
    }
}

/// 上限钉死 32（执行单 §3），且第 33 条会淘汰最旧的一条。
#[test]
fn task_store_keeps_at_most_32_and_evicts_oldest() {
    assert_eq!(TASK_STORE_CAP, 32, "执行单 §3：上限 32 条");
    let core = test_core();
    let (chat, _) = counting_chat("好。");
    let mut ids = Vec::new();
    for i in 0..33 {
        let task = send(&core, &chat, &format!("第 {i} 条"));
        ids.push(task["id"].as_str().expect("task id").to_string());
    }
    // 最新的 32 条读得到，最旧的第 1 条已被淘汰。
    for id in &ids[1..] {
        let got = call(&core, &rpc_body("tasks/get", json!({"id": id})), &chat);
        assert!(
            got.json["result"].is_object(),
            "应能取回 {id}：{}",
            got.json
        );
    }
    let gone = call(&core, &rpc_body("tasks/get", json!({"id": ids[0]})), &chat);
    assert_eq!(
        gone.json["error"]["code"], -32001,
        "最旧任务应被淘汰：{}",
        gone.json
    );
}

/// 任务表本体：cap 生效 + 同 id 覆盖不重复占位（直接测 TaskStore）。
#[test]
fn task_store_directly_evicts_oldest_and_overwrites_same_id() {
    let mut store = TaskStore::new(2);
    store.insert("t1".to_string(), json!({"id": "t1"}));
    store.insert("t2".to_string(), json!({"id": "t2"}));
    store.insert("t2".to_string(), json!({"id": "t2", "rev": 2}));
    assert_eq!(store.len(), 2, "同 id 覆盖不重复占位");
    store.insert("t3".to_string(), json!({"id": "t3"}));
    assert_eq!(store.len(), 2, "cap=2");
    assert!(store.get("t1").is_none(), "最旧的被淘汰");
    assert_eq!(store.get("t2").expect("t2")["rev"], 2);
    assert!(store.get("t3").is_some());
}

/// 小 cap 实例（with_tasks）同样 FIFO 淘汰。
#[test]
fn with_tasks_honours_small_capacity() {
    let core = A2aCore::with_tasks(2);
    let (chat, _) = counting_chat("好。");
    let first = send(&core, &chat, "一")["id"]
        .as_str()
        .expect("id")
        .to_string();
    let second = send(&core, &chat, "二")["id"]
        .as_str()
        .expect("id")
        .to_string();
    let third = send(&core, &chat, "三")["id"]
        .as_str()
        .expect("id")
        .to_string();
    let gone = call(&core, &rpc_body("tasks/get", json!({"id": first})), &chat);
    assert_eq!(gone.json["error"]["code"], -32001);
    for id in [second, third] {
        let got = call(&core, &rpc_body("tasks/get", json!({"id": id})), &chat);
        assert!(got.json["result"].is_object(), "{id} 应还在：{}", got.json);
    }
}

// ---------------------------------------------------------------- 范围外方法

/// 范围外方法一律回 JSON-RPC 错误、**不打上游**（chat 调用次数 = 0）。
#[test]
fn out_of_scope_methods_are_rejected_without_calling_upstream() {
    let core = test_core();
    let (chat, hits) = counting_chat("不会用到");
    let cases = [
        // 已知 A2A 方法，但本服务不实现（streaming=false / 不做取消 / 不做列表）。
        ("message/stream", -32004),
        ("tasks/cancel", -32004),
        ("tasks/resubscribe", -32004),
        ("tasks/list", -32004),
        ("agent/getAuthenticatedExtendedCard", -32004),
        // 推送配置整族不支持（pushNotifications=false）。
        ("tasks/pushNotificationConfig/set", -32003),
        ("tasks/pushNotificationConfig/get", -32003),
        ("tasks/pushNotificationConfig/list", -32003),
        ("tasks/pushNotificationConfig/delete", -32003),
        // 完全未知的方法。
        ("foo/bar", -32601),
    ];
    for (method, expected) in cases {
        let resp = call(&core, &rpc_body(method, json!({})), &chat);
        assert_eq!(resp.status, 200, "{method}: JSON-RPC 错误是 HTTP 200");
        assert_eq!(
            resp.json["error"]["code"], expected,
            "{method}: {}",
            resp.json
        );
    }
    assert_eq!(hits.load(Ordering::Relaxed), 0, "范围外方法不得打上游");
}

// ---------------------------------------------------------------- 单例

/// core() 是 Send + Sync 的进程内单例；任务表随实例持久（跨请求同一张）。
#[test]
fn core_singleton_is_static_send_sync_and_shares_one_task_store() {
    fn assert_send_sync<T: Send + Sync>(_: &T) {}
    let first = crate::core();
    assert_send_sync(first);
    assert!(
        std::ptr::eq(first, crate::core()),
        "core() 必须回同一个实例"
    );

    let (chat, _) = counting_chat("好。");
    let task = send(first, &chat, "单例");
    let id = task["id"].as_str().expect("task id").to_string();
    let got = call(first, &rpc_body("tasks/get", json!({"id": id})), &chat);
    assert_eq!(got.json["result"], task, "任务表随单例持久");
}
