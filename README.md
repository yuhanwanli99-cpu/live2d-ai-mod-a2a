# live2d-ai-mod-a2a

A2A 协议核 Mod（稳定 id a2a；工厂 `FACTORY`（`A2aFactory`））的实现 crate。本仓库是 2026-10-19 从主仓 `/home/skystar/Live2D-Ai` 拆出来的独立本地仓库：**仓库根 = crate 根**（`Cargo.toml` 在根），许可证 AGPL-3.0-only（见 `LICENSE`）。

依赖方向只有「Mod → 主仓」：`live2d-ai-mod-system` 走 path 指向同父目录的 `../Live2D-Ai/crates/...`；没有指向旧第二仓 `../Live2D-Ai-mods` 的 path，也没有 Mod→Mod 的 path 依赖。本仓是独立 workspace（`Cargo.toml` 末尾空的 `[workspace]`），可以 `cd` 进来单独跑门禁。

## 挂回主仓（维护者手动，**没有动态加载器**）

1. 主仓根 `Cargo.toml` 的 `[workspace.dependencies]` 加一行：`live2d-ai-mod-a2a` = { path = "../live2d-ai-mod-a2a" }；
2. `crates/live2d-ai-desktop/Cargo.toml` 加同名依赖（`live2d-ai-mod-a2a` = { workspace = true }）；
3. 把 `FACTORY`（`A2aFactory`） 加进 `AVAILABLE_MOD_FACTORIES`；
4. 宿主侧 `/api/v1/a2a` 门与路由（主仓 `crates/live2d-ai-desktop/src/web_api/a2a_gate.rs`）也要一并加回；缺省停用，且拿不到密钥值。

## 门禁

```bash
cargo test --all-targets
cargo test --doc
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
```

公开仓库：<https://github.com/yuhanwanli99-cpu/live2d-ai-mod-a2a>（分支 `main`）。
