//! Logistics vanilla GUI runtime diagnostics
//!
//! Usage: cargo run --example logistics_runtime_diagnostics

use hoi4_ui::vanilla_gui::runtime::{
    vanilla_profile_diagnostics_markdown, VanillaGuiRuntimeContext, COUNTRY_LOGISTICS_DESCRIPTOR,
};

fn main() {
    let context = VanillaGuiRuntimeContext::load(COUNTRY_LOGISTICS_DESCRIPTOR.required_gui_files);
    let report =
        vanilla_profile_diagnostics_markdown(context.as_ref(), &COUNTRY_LOGISTICS_DESCRIPTOR);

    println!("{}", report);

    if let Some(context) = context {
        println!("\n## Additional Details");
        println!("- loaded_gui_files: {}", context.loaded_gui_files());
        println!("- gfx_index_empty: {}", context.gfx_index.is_empty());

        let profile_report = context.profile_report(&COUNTRY_LOGISTICS_DESCRIPTOR);
        println!("\n## Key Templates");
        for template in profile_report.key_templates_present {
            println!("  ✓ {}", template);
        }
        for template in profile_report.key_templates_missing {
            println!("  ✗ {}", template);
        }
    }
}
