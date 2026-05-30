//! 集成测试：验证 mod 链回退 + replace_path 在 [`hoi4_assets::FsAssetDb`] 上的端到端行为。
//!
//! 不需要真实 HOI4 安装：临时目录构造 fake vanilla + 1 个 mod 即可。

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use hoi4_assets::{AssetDb, AssetError, FsAssetDb};
use hoi4_paths::{ModEntry, PathConfig};

fn fixture(tag: &str) -> (PathBuf, PathBuf) {
    let tmp = std::env::temp_dir().join(format!("ironheart_assets_it_{tag}"));
    let _ = fs::remove_dir_all(&tmp);
    let game = tmp.join("game");
    let m = tmp.join("mod_a");
    fs::create_dir_all(game.join("map")).unwrap();
    fs::create_dir_all(game.join("common")).unwrap();
    fs::create_dir_all(game.join("interface")).unwrap();
    fs::create_dir_all(m.join("interface")).unwrap();
    (game, m)
}

#[test]
fn mod_overrides_vanilla_for_same_relative_path() {
    let (game, m) = fixture("override");
    fs::write(game.join("interface/topbar.gui"), b"vanilla").unwrap();
    fs::write(m.join("interface/topbar.gui"), b"modded").unwrap();

    let cfg = PathConfig::with_game_path(&game).with_mods(vec![ModEntry {
        name: "mod_a".into(),
        root: m.clone(),
        replace_paths: vec![],
    }]);
    let db = FsAssetDb::new(cfg);

    let bytes = db.open("interface/topbar.gui").unwrap();
    assert_eq!(&*bytes, b"modded", "mod 应优先于 vanilla");
}

#[test]
fn replace_path_blocks_vanilla_under_prefix() {
    let (game, m) = fixture("replace");
    fs::create_dir_all(game.join("history/units")).unwrap();
    fs::create_dir_all(m.join("history/units")).unwrap();
    fs::write(game.join("history/units/GER_1936.txt"), b"vanilla").unwrap();
    fs::write(m.join("history/units/SOV_1936.txt"), b"mod").unwrap();

    let cfg = PathConfig::with_game_path(&game).with_mods(vec![ModEntry {
        name: "tno".into(),
        root: m.clone(),
        replace_paths: vec![PathBuf::from("history/units")],
    }]);
    let db = FsAssetDb::new(cfg);

    // mod 文件 OK
    let sov = db.open("history/units/SOV_1936.txt").unwrap();
    assert_eq!(&*sov, b"mod");
    // vanilla 文件被屏蔽
    let err = db.open("history/units/GER_1936.txt").unwrap_err();
    assert!(err.is_not_found(), "GER 应被 replace_path 屏蔽");
}

#[test]
fn referrer_chain_in_error_message() {
    let (game, _) = fixture("referrer");
    let cfg = PathConfig::with_game_path(&game);
    let db = FsAssetDb::new(cfg);

    let err = db
        .open_referenced("gfx/nope.dds", "interface/topbar.gui")
        .unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("nope.dds"), "{}", msg);
    assert!(
        msg.contains("referenced by") && msg.contains("topbar.gui"),
        "{}",
        msg
    );
}

#[test]
fn parsed_cache_is_arc_shared() {
    let (game, _) = fixture("parsed_arc");
    fs::write(game.join("interface/x.txt"), b"hello").unwrap();
    let cfg = PathConfig::with_game_path(&game);
    let db = FsAssetDb::new(cfg);

    struct Parsed(Vec<u8>);

    let a: Arc<Parsed> = db
        .parse_or_get("interface/x.txt", |b| {
            Ok::<Parsed, AssetError>(Parsed(b.to_vec()))
        })
        .unwrap();
    let b: Arc<Parsed> = db
        .parse_or_get("interface/x.txt", |_| panic!("cache miss on second call"))
        .unwrap();
    assert_eq!(a.0, b"hello");
    assert!(Arc::ptr_eq(&a, &b), "解析结果应通过 Arc 共享，不应重复克隆");
}

#[test]
fn list_finds_mod_only_paths() {
    let (game, m) = fixture("list_mod");
    fs::write(m.join("interface/extra.gui"), b"m").unwrap();
    let cfg = PathConfig::with_game_path(&game).with_mods(vec![ModEntry {
        name: "x".into(),
        root: m.clone(),
        replace_paths: vec![],
    }]);
    let db = FsAssetDb::new(cfg);

    let v = db.list("interface/extra.gui");
    assert_eq!(v.len(), 1);
    assert_eq!(v[0], m.join("interface/extra.gui"));
}
