fn main() {
    let grammar_src = std::path::Path::new("patchlang-grammar/src");
    cc::Build::new()
        .file(grammar_src.join("parser.c"))
        .include(grammar_src)
        .compile("patchlang");
    println!("cargo:rerun-if-changed=patchlang-grammar/src/parser.c");
    println!("cargo:rerun-if-changed=patchlang-grammar/grammar.js");
}
