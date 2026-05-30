use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn grep_hc2_cash_rm_deductions_only_in_treasury_gateways() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf();
    let crates = root.join("crates");
    let mut violations = Vec::new();
    visit_rs_files(&crates, &mut |path| {
        let content = fs::read_to_string(path).expect("read rust source");
        let mut current_fn: Option<String> = None;
        for (idx, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") || trimmed.starts_with("///") || trimmed.starts_with("*") {
                continue;
            }
            if let Some(name) = fn_name(trimmed) {
                current_fn = Some(name.to_owned());
            }
            if !line.contains("cash_rm -=") {
                continue;
            }
            let normalized = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            if normalized == "crates/hoi4-logic/tests/grep_hc2.rs" {
                continue;
            }
            let allowed_file = normalized == "crates/hoi4-state/src/finance.rs";
            let allowed_fn = matches!(
                current_fn.as_deref(),
                Some("gov_buy" | "gov_buy_for" | "gov_buy_on_credit_for" | "pay")
            );
            if !allowed_file || !allowed_fn {
                violations.push(format!("{}:{}: {}", normalized, idx + 1, line.trim()));
            }
        }
    });

    assert!(
        violations.is_empty(),
        "HC-2 violation: cash_rm direct deduction outside Treasury cash gateways:\n{}",
        violations.join("\n")
    );
}

fn fn_name(line: &str) -> Option<&str> {
    let rest = line
        .strip_prefix("pub fn ")
        .or_else(|| line.strip_prefix("fn "))?;
    let end = rest.find('(')?;
    Some(&rest[..end])
}

fn visit_rs_files(dir: &Path, f: &mut dyn FnMut(&Path)) {
    for entry in fs::read_dir(dir).expect("read dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) == Some("target") {
                continue;
            }
            visit_rs_files(&path, f);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            f(&path);
        }
    }
}
