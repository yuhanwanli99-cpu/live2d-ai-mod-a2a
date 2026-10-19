//! 进程内 A2A 任务表（0.3.8 rc.2 口径，0.3.9 原样迁入）。
//!
//! 0.3.8 执行单 §3 的 `tasks/get` 要求：**按本进程内存里的 task id 取回，上限
//! 32 条，不落盘；进程重启后任务消失**。因此这里刻意不接持久化、不接磁盘：
//! - 环形淘汰：第 33 条任务进来时丢最旧的一条（FIFO）；
//! - 任务表属于 [`crate::A2aCore`] 实例（不再有进程级全局单例）：生产由 Mod
//!   胶水持有一个 `Arc<A2aCore>`，测试各自造自己的表。

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, MutexGuard};

use serde_json::Value;

/// 任务表上限（0.3.8 执行单 §3 钉死 32；线格式 / 语义不得变）。
pub const TASK_STORE_CAP: usize = 32;

/// 进程内任务表：按 task id 取回 completed Task 的 JSON。
pub(crate) struct TaskStore {
    cap: usize,
    /// FIFO 顺序（淘汰用）。
    order: VecDeque<String>,
    tasks: HashMap<String, Value>,
}

impl TaskStore {
    /// 新建任务表（生产 cap = [`TASK_STORE_CAP`]；测试可造更小 cap）。
    pub(crate) fn new(cap: usize) -> Self {
        Self {
            cap,
            order: VecDeque::new(),
            tasks: HashMap::new(),
        }
    }

    /// 存一条任务；超过上限先淘汰最旧的一条（同 id 覆盖不重复占位）。
    pub(crate) fn insert(&mut self, id: String, task: Value) {
        if let std::collections::hash_map::Entry::Occupied(mut existing) =
            self.tasks.entry(id.clone())
        {
            existing.insert(task);
            return;
        }
        while self.order.len() >= self.cap {
            if let Some(oldest) = self.order.pop_front() {
                self.tasks.remove(&oldest);
            }
        }
        self.order.push_back(id.clone());
        self.tasks.insert(id, task);
    }

    /// 按 id 取回（克隆；调用方不持有内部引用）。
    pub(crate) fn get(&self, id: &str) -> Option<Value> {
        self.tasks.get(id).cloned()
    }

    /// 当前条数（测试断言淘汰 / 上限）。
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.tasks.len()
    }
}

/// 加锁读任务表；锁中毒不 panic（A2A 状态面不能因此整个失效）。
pub(crate) fn lock_tasks(store: &Mutex<TaskStore>) -> MutexGuard<'_, TaskStore> {
    store
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
