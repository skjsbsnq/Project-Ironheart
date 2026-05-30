use std::fs;

fn main() {
    let path = r"C:\Users\19180\Documents\999\b1\crates\hoi4-content\content\events\SPA_events.ron";
    let mut bytes = fs::read(path).unwrap();

    // Strip BOM if present
    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        bytes = bytes[3..].to_vec();
    }

    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let mut in_desc = false;
    while i < bytes.len() {
        if !in_desc {
            out.push(bytes[i]);
            // detect start of description: string
            if bytes[i] == b'"' {
                let is_desc_start = || -> bool {
                    let start = if i >= 22 { i - 22 } else { 0 };
                    let slice = &bytes[start..=i];
                    let s = std::str::from_utf8(slice).unwrap_or("");
                    s.ends_with("description: \"") || s.ends_with("title: \"")
                };
                if is_desc_start() { in_desc = true; }
            }
        } else {
            // inside description - replace ASCII " with CJK brackets
            if bytes[i] == b'"' {
                // Could be end of description or literal quote
                // End if followed by \n and not preceded by CJK char
                let mut is_end = false;
                if i + 1 < bytes.len() && bytes[i+1] == b',' {
                    is_end = true;
                } else if i + 2 < bytes.len() && bytes[i+1] == b'\r' && bytes[i+2] == b'\n' {
                    let mut j = i + 3;
                    while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') { j += 1; }
                    if j < bytes.len() && bytes[j] != b'"' {
                        // look ahead for picture/trigger/is_triggered_only etc
                        let peek = &bytes[j..std::cmp::min(j+30, bytes.len())];
                        let ps = std::str::from_utf8(peek).unwrap_or("");
                        if ps.trim_start().starts_with("picture:") || ps.trim_start().starts_with("options:") || ps.trim_start().starts_with("trigger:") || ps.trim_start().starts_with("immediate:") || ps.trim_start().starts_with("is_triggered_only:") || ps.trim_start().starts_with("mean_time_to_happen") {
                            is_end = true;
                        }
                    }
                }
                if is_end {
                    in_desc = false;
                    out.push(b'"');
                } else {
                    // literal quote inside description - replace with CJK brackets
                    if i > 0 && bytes[i-1] > 127 {
                        // odd/even toggle for left/right bracket
                        let mut quote_count = 0;
                        for k in (0..out.len()).rev() {
                            if out[k] == 0xE3 && k+2 < out.len() && out[k+1] == 0x80 {
                                if out[k+2] == 0x8E || out[k+2] == 0x8F { quote_count += 1; }
                            }
                        }
                        if quote_count % 2 == 0 {
                            out.extend_from_slice(&[0xE3, 0x80, 0x8E]); // 『
                        } else {
                            out.extend_from_slice(&[0xE3, 0x80, 0x8F]); // 』
                        }
                    } else {
                        out.push(b'"');
                    }
                }
            } else {
                out.push(bytes[i]);
            }
        }
        i += 1;
    }
    fs::write(path, &out).unwrap();
    println!("Fixed {} bytes -> {} bytes", bytes.len(), out.len());
}
