//! CR-2.2 — naga static validation of `counter_v3.wgsl`.
//!
//! 把 `counter_v3.wgsl` 走一遍 naga 前端 + 后续 validator，确保
//! shader 没有 typo / 类型错配 / 未定义符号——不依赖 GPU。
//!
//! 与 `terrain_wgsl.rs` 不同：counter_v3.wgsl **不**经过 `compose_shader`
//! 注入（独立 pass，自管 group(0) uniform 与 group(1) atlas）。

#[test]
fn counter_v3_wgsl_parses_cleanly() {
    let source = include_str!("../src/passes/counter_v3.wgsl");
    let module = match naga::front::wgsl::parse_str(source) {
        Ok(m) => m,
        Err(err) => {
            eprintln!(
                "[shader-validate] counter_v3.wgsl parse failed:\n{}",
                err.emit_to_string(source)
            );
            panic!("counter_v3.wgsl parse failed");
        }
    };

    // 跑一遍 validator（不限制 capabilities，因为 counter_v3.wgsl 仅用 1.0 基础特性）。
    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );
    if let Err(err) = validator.validate(&module) {
        eprintln!(
            "[shader-validate] counter_v3.wgsl validation failed:\n{}",
            err.emit_to_string(source)
        );
        panic!("counter_v3.wgsl validation failed");
    }
}
