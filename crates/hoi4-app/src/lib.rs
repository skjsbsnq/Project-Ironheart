//! `hoi4-app` 既是主二进制（`hoi4-app` bin）也是一个 library，
//! 后者把 `SystemSchedule` 等"非渲染、非引擎"调度组件暴露给集成测试 / 子工具。
//!
//! 二进制入口在 `src/main.rs`；本文件只 re-export 公共模块。

pub mod ai;
pub mod script;
pub mod systems;
pub mod vanilla_resource_views;
