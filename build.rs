fn main() {
    // The patchlang parser C code only compiles for native targets.
    // wasm32 builds use the hand-rolled Rust parser in src/lang_wasm.rs.
    let target = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if target == "wasm32" {
        return;
    }

    let grammar_src = std::path::Path::new("patchlang-grammar/src");
    cc::Build::new()
        .file(grammar_src.join("parser.c"))
        .include(grammar_src)
        .compile("patchlang");
    println!("cargo:rerun-if-changed=patchlang-grammar/src/parser.c");
    println!("cargo:rerun-if-changed=patchlang-grammar/grammar.js");
}
