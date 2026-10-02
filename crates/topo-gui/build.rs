fn main() {
    println!("cargo:rerun-if-changed=../../assets/branding/topo.ico");
    println!("cargo:rerun-if-changed=resources/topo.rc");
    #[cfg(target_os = "windows")]
    embed_resource::compile_for("resources/topo.rc", &["topo-gui"], embed_resource::NONE)
        .manifest_required()
        .expect("failed to embed the topo application icon");
}
