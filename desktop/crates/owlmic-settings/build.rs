//! Generates the settings table from design/settings.json, the model both apps share.

use serde_json::Value;
use std::fmt::Write;
use std::path::PathBuf;

fn main() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../shared/design/settings.json");
    println!("cargo:rerun-if-changed={}", path.display());
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("can't read {}: {e}", path.display()));
    let model: Value = serde_json::from_str(&text).expect("design/settings.json is not valid JSON");

    let mut out = String::from("pub const SETTINGS: &[SettingDef] = &[\n");
    for s in model["settings"]
        .as_array()
        .expect("`settings` must be a list")
    {
        let values: Vec<&str> = s["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let shown: Vec<&str> = s["shownOn"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let scope = match s["scope"].as_str().unwrap() {
            "shared" => "Shared",
            "phone" => "Phone",
            "pc" => "Pc",
            other => panic!("unknown scope `{other}`"),
        };
        writeln!(
            out,
            "    SettingDef {{ id: {:?}, values: &{:?}, default: {:?}, scope: Scope::{scope}, on_phone: {}, on_pc: {} }},",
            s["id"].as_str().unwrap(),
            values,
            s["default"].as_str().unwrap(),
            shown.contains(&"phone"),
            shown.contains(&"pc"),
        )
        .unwrap();
    }
    out.push_str("];\n");
    std::fs::write(
        PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("settings.rs"),
        out,
    )
    .unwrap();
}
