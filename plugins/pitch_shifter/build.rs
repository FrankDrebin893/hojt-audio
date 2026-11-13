//! Puts the HØJT badge in the standalone program's exe on Windows, so the taskbar and
//! Explorer show it instead of the default icon. Only the `pitch_shifter_standalone` program gets it,
//! not the plugin's library.

fn main() {
    println!("cargo:rerun-if-changed=assets/pitch_shifter.rc");
    println!("cargo:rerun-if-changed=assets/pitch_shifter.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile_for(
            "assets/pitch_shifter.rc",
            ["pitch_shifter_standalone"],
            embed_resource::NONE,
        )
        .manifest_optional()
        .unwrap();
    }
}
