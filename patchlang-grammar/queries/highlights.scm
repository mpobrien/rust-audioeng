; patchlang tree-sitter highlight queries
; For use with nvim-treesitter. See editors/nvim/README.md for setup.

; ── comments ──────────────────────────────────────────────────────────────
(comment) @comment

; ── top-level declaration names ───────────────────────────────────────────
(patch_decl  name: (identifier) @type.definition)
(effect_decl name: (identifier) @type.definition)
(phrase_decl name: (identifier) @type.definition)

; ── structural keywords ───────────────────────────────────────────────────
"patch"   @keyword
"effect"  @keyword
"phrase"  @keyword
"voices"  @keyword
"poly"    @keyword
"mono"    @keyword
"legato"  @keyword

; ── node constructors (osc, adsr, lpf, …) ────────────────────────────────
(node_call kind: (identifier) @function.builtin)

; ── named param keys ──────────────────────────────────────────────────────
(named_param key: (identifier) @property)

; ── named bindings inside a patch body ───────────────────────────────────
(binding_stmt name: (identifier) @variable)

; ── waveform and duration enum values ────────────────────────────────────
((identifier) @string.special
  (#any-of? @string.special
    "sine" "square" "saw" "triangle"
    "eighth" "quarter" "half" "whole"))

; ── implicit patch parameters ────────────────────────────────────────────
((identifier) @variable.builtin
  (#any-of? @variable.builtin "freq" "velocity"))

; ── notes and rests in phrase ─────────────────────────────────────────────
(note) @constant
(rest) @comment

; ── numbers ───────────────────────────────────────────────────────────────
(number)  @number
(integer) @number

; ── operators ─────────────────────────────────────────────────────────────
"|"      @operator
"="      @operator
".."     @operator
(binop)  @operator

; ── punctuation ───────────────────────────────────────────────────────────
"{"  @punctuation.bracket
"}"  @punctuation.bracket
"["  @punctuation.bracket
"]"  @punctuation.bracket
","  @punctuation.delimiter
