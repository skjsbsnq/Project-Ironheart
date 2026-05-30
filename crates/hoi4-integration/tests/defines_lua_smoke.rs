//! Phase 3.12.14: defines.lua CI 校验。
//!
//! 加载 vanilla `common/defines.lua`，对比项目镜像常量，
//! 输出 banner 供 CI smoke 检测。

use hoi4_integration::defines_lua_banner;

#[test]
fn defines_lua_diff_against_vanilla() {
    let banner = defines_lua_banner();
    println!("{}", banner);
    if banner.contains("skipping") {
        eprintln!("HOI4 install not found — skipping defines.lua diff test");
        return;
    }
}
