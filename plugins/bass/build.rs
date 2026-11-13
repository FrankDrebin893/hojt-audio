//! Puts the HØJT badge in the standalone program's exe on Windows, so the taskbar and
//! Explorer show it instead of the default icon. Only the `bass_standalone` program gets it,
//! not the plugin's library.

fn main() {
    println!("cargo:rerun-if-changed=assets/bass.rc");
    println!("cargo:rerun-if-changed=assets/bass.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile_for("assets/bass.rc", ["bass_standalone"], embed_resource::NONE)
            .manifest_optional()
            .unwrap();
    }
}
