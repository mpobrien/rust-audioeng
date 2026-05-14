/// <reference types="tree-sitter-cli/dsl" />
// @ts-check

module.exports = grammar({
  name: 'patchlang',

  extras: $ => [/\s+/, $.comment],

  rules: {
    source_file: $ => repeat($.declaration),

    comment: $ => /#[^\n]*/,

    // ── top-level declarations ─────────────────────────────────────────

    declaration: $ => choice(
      $.patch_decl,
      $.effect_decl,
    ),

    patch_decl: $ => seq(
      field('name', $.identifier),
      '=',
      'patch',
      '{',
      repeat($.patch_stmt),
      '}',
    ),

    effect_decl: $ => seq(
      field('name', $.identifier),
      '=',
      'effect',
      '{',
      $.pipe_chain,
      '}',
    ),

    // ── statements inside a patch block ───────────────────────────────

    patch_stmt: $ => choice(
      $.voices_stmt,
      $.binding_stmt,
      $.pipe_chain,   // unnamed output chain
    ),

    voices_stmt: $ => seq(
      'voices',
      '=',
      $.voices_value,
    ),

    voices_value: $ => choice(
      'mono',
      seq('poly', $.integer),
      seq('mono', 'legato'),
    ),

    // `name = expr`  –  named sub-graph or output alias
    binding_stmt: $ => seq(
      field('name', $.identifier),
      '=',
      field('value', $.expr),
    ),

    // ── expressions ───────────────────────────────────────────────────

    expr: $ => choice(
      $.pipe_chain,
      $.binary_expr,
      $.primary,
    ),

    // node_call (| segment)*
    pipe_chain: $ => seq(
      $.node_call,
      repeat(seq('|', $.pipe_segment)),
    ),

    // after `|`: either a node call or a bare name (effect / binding ref)
    pipe_segment: $ => choice(
      $.node_call,
      $.identifier,
    ),

    // ── node call: `kind { params }` ──────────────────────────────────

    node_call: $ => seq(
      field('kind', $.identifier),
      '{',
      optional($.param_list),
      '}',
    ),

    param_list: $ => seq(
      $.param_item,
      repeat(seq(',', $.param_item)),
      optional(','),
    ),

    param_item: $ => choice(
      $.named_param,
      $.primary,   // positional ref (mix sources)
    ),

    named_param: $ => seq(
      field('key', $.identifier),
      '=',
      field('value', $.param_value),
    ),

    // param values allow arithmetic and bare identifiers
    param_value: $ => choice(
      $.binary_expr,
      $.primary,
    ),

    binary_expr: $ => prec.left(1, seq(
      field('left',  $.primary),
      field('op',    $.binop),
      field('right', $.primary),
    )),

    binop: $ => choice('+', '-', '*', '/'),

    primary: $ => choice(
      $.number,
      $.identifier,
    ),

    // ── terminals ─────────────────────────────────────────────────────

    identifier: $ => /[a-zA-Z_][a-zA-Z0-9_]*/,

    number: $ => /[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?/,

    integer: $ => /[0-9]+/,
  },
});
