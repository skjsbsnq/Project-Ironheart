use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct CleanMapAudit {
    pub renderer: &'static str,
    pub map_backend: &'static str,
    pub audit_stage: &'static str,
    pub old_renderer_removed: bool,
}

impl CleanMapAudit {
    pub const fn plumbing_only() -> Self {
        Self {
            renderer: "clean",
            map_backend: "clean",
            audit_stage: "plumbing_only",
            old_renderer_removed: false,
        }
    }

    pub fn to_json(&self) -> String {
        format!(
            "{{\n  \"renderer\": \"{}\",\n  \"map_backend\": \"{}\",\n  \"audit_stage\": \"{}\",\n  \"old_renderer_removed\": {}\n}}\n",
            self.renderer, self.map_backend, self.audit_stage, self.old_renderer_removed
        )
    }

    pub fn to_text(&self) -> String {
        format!(
            "renderer={}\nmap_backend={}\naudit_stage={}\nold_renderer_removed={}\n",
            self.renderer, self.map_backend, self.audit_stage, self.old_renderer_removed
        )
    }
}

pub fn write_clean_map_audit(output_dir: &Path) -> std::io::Result<PathBuf> {
    let audit = CleanMapAudit::plumbing_only();
    std::fs::create_dir_all(output_dir)?;
    let json_path = output_dir.join("latest.json");
    std::fs::write(output_dir.join("latest.txt"), audit.to_text())?;
    std::fs::write(&json_path, audit.to_json())?;
    Ok(json_path)
}

#[cfg(test)]
mod clean_audit_tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn clean_audit_cli_outputs_renderer_clean() {
        let audit = CleanMapAudit::plumbing_only();
        assert_eq!(audit.renderer, "clean");
        assert_eq!(audit.map_backend, "clean");
    }

    #[test]
    fn clean_audit_cli_uses_clean_schema_stub() {
        let json = CleanMapAudit::plumbing_only().to_json();
        assert!(json.contains("\"renderer\": \"clean\""));
        assert!(json.contains("\"map_backend\": \"clean\""));
        assert!(json.contains("\"audit_stage\": \"plumbing_only\""));
    }

    #[test]
    fn clean_audit_cli_reports_removed_old_renderer_stub() {
        let audit = CleanMapAudit::plumbing_only();
        assert!(!audit.old_renderer_removed);
        assert!(audit.to_json().contains("\"old_renderer_removed\": false"));
    }

    #[test]
    fn clean_audit_cli_does_not_emit_map_baseline_old_keys() {
        let json = CleanMapAudit::plumbing_only().to_json();
        for key in [
            "asset_quality",
            "binding_audit",
            "parity_gate",
            "visual_review_usable",
            "terrain_pdxmap",
            "posteffect_values",
        ] {
            assert!(!json.contains(key), "clean audit emitted legacy key {key}");
        }
    }

    #[test]
    fn clean_audit_cli_does_not_call_map_baseline_audit() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "hoi4_clean_audit_no_legacy_{}_{}",
            std::process::id(),
            unique
        ));

        let path = write_clean_map_audit(&dir).unwrap();
        assert_eq!(path, dir.join("latest.json"));
        assert!(dir.join("latest.txt").is_file());

        for legacy_file in [
            "asset_audit.json",
            "binding_audit.json",
            "posteffect_values.json",
            "terrain_pdxmap.json",
            "parity_gate.txt",
            "parity_gate.json",
        ] {
            assert!(
                !dir.join(legacy_file).exists(),
                "clean audit wrote legacy map baseline file {legacy_file}"
            );
        }

        let _ = std::fs::remove_dir_all(dir);
    }
}
