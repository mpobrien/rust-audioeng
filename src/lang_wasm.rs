// Hand-rolled patchlang parser for wasm32 builds.
// tree-sitter's C runtime cannot compile for wasm32-unknown-unknown (no stdio.h).
// This pure-Rust implementation covers the grammar subset used in practice.

use std::collections::HashMap;
use crate::lang::{
    Accidental, BinOpKind, EffectDecl, Expr, NoteItem, NoteName,
    ParamItem, PatchDecl, PatchEnv, PatchStmt, PipeChain, PipeSegment,
    NodeCall, PhraseDecl, VoicesMode,
};

// ── lexer ─────────────────────────────────────────────────────────────────────

struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Self { src: src.as_bytes(), pos: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn peek2(&self) -> Option<u8> {
        self.src.get(self.pos + 1).copied()
    }

    fn bump(&mut self) {
        self.pos += 1;
    }

    fn skip_ws(&mut self) {
        loop {
            match self.peek() {
                Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') => { self.bump(); }
                Some(b'/') if self.peek2() == Some(b'/') => {
                    while matches!(self.peek(), Some(c) if c != b'\n') {
                        self.bump();
                    }
                }
                _ => break,
            }
        }
    }

    fn try_byte(&mut self, b: u8) -> bool {
        self.skip_ws();
        if self.peek() == Some(b) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect_byte(&mut self, b: u8) -> Result<(), String> {
        if self.try_byte(b) {
            Ok(())
        } else {
            self.skip_ws();
            Err(format!("expected '{}', found {:?} at pos {}",
                b as char, self.peek().map(|c| c as char), self.pos))
        }
    }

    fn peek_byte(&mut self) -> Option<u8> {
        self.skip_ws();
        self.peek()
    }

    fn read_ident(&mut self) -> Option<String> {
        self.skip_ws();
        let c = self.peek()?;
        if !c.is_ascii_alphabetic() && c != b'_' {
            return None;
        }
        let start = self.pos;
        while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == b'_') {
            self.bump();
        }
        Some(String::from_utf8_lossy(&self.src[start..self.pos]).into_owned())
    }

    fn expect_ident(&mut self) -> Result<String, String> {
        self.read_ident().ok_or_else(|| {
            self.skip_ws();
            let ctx = &self.src[self.pos..self.src.len().min(self.pos + 20)];
            format!("expected identifier, got: {:?}", String::from_utf8_lossy(ctx))
        })
    }

    fn read_number(&mut self) -> Option<f64> {
        self.skip_ws();
        if !matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            return None;
        }
        let start = self.pos;
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.bump();
        }
        // Decimal — but not ".." (range)
        if self.peek() == Some(b'.') && matches!(self.peek2(), Some(c) if c.is_ascii_digit()) {
            self.bump();
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.bump();
            }
        }
        String::from_utf8_lossy(&self.src[start..self.pos]).parse().ok()
    }
}

// ── top-level parse ───────────────────────────────────────────────────────────

pub fn parse(src: &str) -> Result<PatchEnv, String> {
    let mut lex = Lexer::new(src);
    let mut patches: HashMap<String, PatchDecl> = HashMap::new();
    let mut effects: HashMap<String, EffectDecl> = HashMap::new();
    let mut phrases: HashMap<String, PhraseDecl> = HashMap::new();

    loop {
        lex.skip_ws();
        if lex.peek().is_none() {
            break;
        }

        let name = lex.expect_ident()?;
        lex.expect_byte(b'=')?;
        let kw = lex.expect_ident()?;

        match kw.as_str() {
            "patch" => {
                let decl = parse_patch_body(&mut lex, name.clone())?;
                patches.insert(name, decl);
            }
            "effect" => {
                let decl = parse_effect_body(&mut lex, name.clone())?;
                effects.insert(name, decl);
            }
            "phrase" => {
                let decl = parse_phrase_body(&mut lex, name.clone())?;
                phrases.insert(name, decl);
            }
            other => {
                return Err(format!("expected patch/effect/phrase, got '{other}'"));
            }
        }
    }

    Ok(PatchEnv { patches, effects, phrases })
}

// ── patch ─────────────────────────────────────────────────────────────────────

fn parse_patch_body(lex: &mut Lexer, name: String) -> Result<PatchDecl, String> {
    lex.expect_byte(b'{')?;
    let mut voices = VoicesMode::Mono;
    let mut stmts = Vec::new();

    loop {
        lex.skip_ws();
        if matches!(lex.peek(), Some(b'}') | None) {
            break;
        }

        // Read the leading identifier — could be a keyword, a node kind, or a binding name.
        let id = lex.expect_ident()?;

        match lex.peek_byte() {
            Some(b'=') if id == "voices" => {
                lex.bump(); // consume '='
                voices = parse_voices_mode(lex)?;
            }
            Some(b'=') => {
                lex.bump(); // consume '='
                let value = parse_binding_value(lex)?;
                stmts.push(PatchStmt::Binding { name: id, value });
            }
            Some(b'{') => {
                // Anonymous node call at the head of a chain
                let head = parse_node_call_after_kind(lex, id)?;
                let chain = parse_chain_tail(lex, head)?;
                stmts.push(PatchStmt::Chain(chain));
            }
            Some(b'|') => {
                // Bare identifier reference as chain head
                lex.bump(); // consume '|'
                let seg = parse_pipe_segment(lex)?;
                let mut segments = vec![seg];
                while lex.peek_byte() == Some(b'|') {
                    lex.bump();
                    segments.push(parse_pipe_segment(lex)?);
                }
                // Construct as a degenerate pipe chain with a ref head.
                // We model it as Chain(PipeChain { head: Ref("id"), segments }).
                // But PipeChain requires NodeCall at head, so use a dummy pattern:
                // wrap the ref in a special way.
                // In practice this pattern (bare ref | ...) isn't used in examples.
                // Fall back to a chain starting with a built "ref" node.
                let dummy = NodeCall { kind: "__ref__".to_string(), params: vec![
                    ParamItem::Named { key: "name".to_string(), value: Expr::Ident(id) }
                ]};
                stmts.push(PatchStmt::Chain(PipeChain { head: dummy, segments }));
            }
            _ => {
                return Err(format!("unexpected token after identifier '{id}' in patch body"));
            }
        }
    }

    lex.expect_byte(b'}')?;
    Ok(PatchDecl { name, voices, stmts })
}

fn parse_voices_mode(lex: &mut Lexer) -> Result<VoicesMode, String> {
    let kw = lex.expect_ident()?;
    Ok(match kw.as_str() {
        "mono" => {
            // Check for optional "legato"
            let saved = lex.pos;
            lex.skip_ws();
            if let Some(w) = lex.read_ident() {
                if w == "legato" {
                    VoicesMode::MonoLegato
                } else {
                    lex.pos = saved;
                    VoicesMode::Mono
                }
            } else {
                VoicesMode::Mono
            }
        }
        "poly" => {
            lex.skip_ws();
            let n = lex.read_number().unwrap_or(4.0) as u32;
            VoicesMode::Poly(n)
        }
        _ => VoicesMode::Mono,
    })
}

// ── effect ────────────────────────────────────────────────────────────────────

fn parse_effect_body(lex: &mut Lexer, name: String) -> Result<EffectDecl, String> {
    lex.expect_byte(b'{')?;
    let chain = parse_chain(lex)?;
    lex.expect_byte(b'}')?;
    Ok(EffectDecl { name, chain })
}

// ── phrase ────────────────────────────────────────────────────────────────────

fn parse_phrase_body(lex: &mut Lexer, name: String) -> Result<PhraseDecl, String> {
    // phrase { dur=eighth, tempo=120, [c d e] } | patch_name
    lex.expect_byte(b'{')?;

    let mut tempo = 120.0f64;
    let mut default_dur = "quarter".to_string();
    let mut notes: Vec<Option<NoteItem>> = Vec::new();

    loop {
        lex.skip_ws();
        match lex.peek() {
            Some(b'}') | None => break,
            Some(b'[') => {
                lex.bump();
                notes = parse_note_list(lex)?;
                lex.expect_byte(b']')?;
            }
            Some(b',') => { lex.bump(); }
            _ => {
                let key = lex.expect_ident()?;
                lex.expect_byte(b'=')?;
                match key.as_str() {
                    "tempo" => {
                        tempo = lex.read_number()
                            .ok_or("expected number after tempo=")?;
                    }
                    "dur" => {
                        default_dur = lex.expect_ident()?;
                    }
                    _ => {
                        // skip unknown param (consume until ',' or '}')
                        while !matches!(lex.peek_byte(), Some(b',') | Some(b'}') | Some(b'[') | None) {
                            lex.bump();
                        }
                    }
                }
            }
        }
    }

    lex.expect_byte(b'}')?;

    // Consume `| patch_name`
    lex.expect_byte(b'|')?;
    let patch_name = lex.expect_ident()?;

    Ok(PhraseDecl { name, tempo, default_dur, notes, patch_name })
}

fn parse_note_list(lex: &mut Lexer) -> Result<Vec<Option<NoteItem>>, String> {
    let mut notes = Vec::new();
    loop {
        lex.skip_ws();
        match lex.peek() {
            None | Some(b']') => break,
            Some(b'_') => {
                lex.bump();
                notes.push(None);
            }
            Some(c) if is_note_char(c) => {
                notes.push(Some(parse_note(lex)?));
            }
            _ => break,
        }
    }
    Ok(notes)
}

fn is_note_char(c: u8) -> bool {
    matches!(c, b'a'..=b'g' | b'A'..=b'G')
}

fn parse_note(lex: &mut Lexer) -> Result<NoteItem, String> {
    let c = lex.peek().ok_or("expected note")?;
    let name = match c.to_ascii_lowercase() {
        b'c' => NoteName::C,
        b'd' => NoteName::D,
        b'e' => NoteName::E,
        b'f' => NoteName::F,
        b'g' => NoteName::G,
        b'a' => NoteName::A,
        b'b' => NoteName::B,
        _ => return Err(format!("invalid note '{}'", c as char)),
    };
    lex.bump();

    let accidental = match lex.peek() {
        Some(b'#') => { lex.bump(); Some(Accidental::Sharp) }
        Some(b'b') => {
            // 'b' after a note char is flat, but only if not followed by a non-digit.
            // (e.g. "bb" = B-flat, "b4" = B in octave 4, "b" = B natural)
            // Since we're in a note list context, 'b' by itself is B-natural.
            // 'b' followed by a digit is B in that octave.
            // 'b' following [a-g] is always treated as flat here.
            // The "eb" case: e followed by 'b' — is this E-flat or E followed by B?
            // In standard music notation, 'b' immediately after a note letter = flat.
            // Note list items are space-separated, so "eb" is one token = E-flat.
            lex.bump();
            Some(Accidental::Flat)
        }
        _ => None,
    };

    // Parse optional octave number (default 4)
    let octave = if matches!(lex.peek(), Some(b'0'..=b'9')) {
        let d = lex.peek().unwrap() - b'0';
        lex.bump();
        d
    } else {
        4
    };

    Ok(NoteItem { name, accidental, octave })
}

// ── pipe chains ───────────────────────────────────────────────────────────────

fn parse_chain(lex: &mut Lexer) -> Result<PipeChain, String> {
    let id = lex.expect_ident()?;
    let head = parse_node_call_after_kind(lex, id)?;
    parse_chain_tail(lex, head)
}

fn parse_chain_tail(lex: &mut Lexer, head: NodeCall) -> Result<PipeChain, String> {
    let mut segments = Vec::new();
    while lex.peek_byte() == Some(b'|') {
        lex.bump();
        segments.push(parse_pipe_segment(lex)?);
    }
    Ok(PipeChain { head, segments })
}

fn parse_pipe_segment(lex: &mut Lexer) -> Result<PipeSegment, String> {
    let id = lex.expect_ident()?;
    if lex.peek_byte() == Some(b'{') {
        let nc = parse_node_call_after_kind(lex, id)?;
        Ok(PipeSegment::Node(nc))
    } else {
        Ok(PipeSegment::Ref(id))
    }
}

// ── binding value (RHS of `name = ...`) ──────────────────────────────────────

fn parse_binding_value(lex: &mut Lexer) -> Result<Expr, String> {
    // Three cases:
    // (a) ident { ... } (| segment)* → Expr::Chain  (pipe_chain starting with node_call)
    // (b) primary op primary          → Expr::BinOp
    // (c) primary                     → Expr::Ident or Expr::Number
    //
    // We need 1-token lookahead past the first identifier/number.

    let saved = lex.pos;
    lex.skip_ws();

    // Try a number primary
    if let Some(n) = lex.read_number() {
        // Check for range ..
        lex.skip_ws();
        if lex.peek() == Some(b'.') && lex.peek2() == Some(b'.') {
            lex.bump(); lex.bump();
            let hi = lex.read_number()
                .ok_or("expected number after '..'")?;
            return Ok(Expr::Range { lo: n, hi });
        }
        // Check for binary op
        if let Some(op) = try_binop(lex) {
            let right = parse_primary_expr(lex)?;
            return Ok(Expr::BinOp {
                left: Box::new(Expr::Number(n)),
                op,
                right: Box::new(right),
            });
        }
        return Ok(Expr::Number(n));
    }

    // Try an identifier
    if let Some(id) = lex.read_ident() {
        lex.skip_ws();
        match lex.peek() {
            Some(b'{') => {
                // Node call — could be start of pipe chain
                let head = parse_node_call_after_kind(lex, id)?;
                let chain = parse_chain_tail(lex, head)?;
                if chain.segments.is_empty() {
                    // Single node call — wrap as chain anyway for consistency with tree-sitter parser
                    return Ok(Expr::Chain(Box::new(chain)));
                }
                return Ok(Expr::Chain(Box::new(chain)));
            }
            Some(b'*') | Some(b'+') | Some(b'-') | Some(b'/') => {
                let op = try_binop(lex).unwrap();
                let right = parse_primary_expr(lex)?;
                return Ok(Expr::BinOp {
                    left: Box::new(Expr::Ident(id)),
                    op,
                    right: Box::new(right),
                });
            }
            Some(b'|') => {
                // Identifier reference followed by pipe — chain starting with ref
                let dummy = NodeCall { kind: "__ref__".to_string(), params: vec![
                    ParamItem::Named { key: "name".to_string(), value: Expr::Ident(id) }
                ]};
                let chain = parse_chain_tail(lex, dummy)?;
                return Ok(Expr::Chain(Box::new(chain)));
            }
            _ => {
                // Just an identifier
                return Ok(Expr::Ident(id));
            }
        }
    }

    lex.pos = saved;
    Err("could not parse binding value".to_string())
}

fn parse_primary_expr(lex: &mut Lexer) -> Result<Expr, String> {
    lex.skip_ws();
    if let Some(n) = lex.read_number() {
        return Ok(Expr::Number(n));
    }
    if let Some(id) = lex.read_ident() {
        return Ok(Expr::Ident(id));
    }
    Err("expected primary expression (number or identifier)".to_string())
}

fn try_binop(lex: &mut Lexer) -> Option<BinOpKind> {
    lex.skip_ws();
    let op = match lex.peek()? {
        b'*' => BinOpKind::Mul,
        b'+' => BinOpKind::Add,
        b'-' => BinOpKind::Sub,
        b'/' => BinOpKind::Div,
        _ => return None,
    };
    lex.bump();
    Some(op)
}

// ── node calls ────────────────────────────────────────────────────────────────

fn parse_node_call_after_kind(lex: &mut Lexer, kind: String) -> Result<NodeCall, String> {
    lex.expect_byte(b'{')?;
    let params = parse_param_list(lex)?;
    lex.expect_byte(b'}')?;
    Ok(NodeCall { kind, params })
}

fn parse_param_list(lex: &mut Lexer) -> Result<Vec<ParamItem>, String> {
    let mut params = Vec::new();
    loop {
        lex.skip_ws();
        match lex.peek() {
            Some(b'}') | None => break,
            Some(b',') => { lex.bump(); }
            _ => {
                params.push(parse_param_item(lex)?);
            }
        }
    }
    Ok(params)
}

fn parse_param_item(lex: &mut Lexer) -> Result<ParamItem, String> {
    // Could be:
    //   ident = value   → Named param
    //   ident           → Positional (identifier reference, e.g. in mix { osc1, osc2 })
    //   number          → Positional number (unlikely but handle it)

    let saved = lex.pos;
    lex.skip_ws();

    // Try number first
    if let Some(n) = lex.read_number() {
        lex.skip_ws();
        if lex.peek() == Some(b'.') && lex.peek2() == Some(b'.') {
            lex.bump(); lex.bump();
            let hi = lex.read_number().ok_or("expected number after '..'")?;
            return Ok(ParamItem::Named { key: "__range__".to_string(), value: Expr::Range { lo: n, hi } });
        }
        return Ok(ParamItem::Positional(Expr::Number(n)));
    }

    let id = lex.expect_ident()?;
    lex.skip_ws();

    if lex.peek() == Some(b'=') {
        lex.bump();
        let value = parse_param_value(lex)?;
        Ok(ParamItem::Named { key: id, value })
    } else {
        // Positional identifier reference (e.g. `mix { osc1, osc2 }`)
        Ok(ParamItem::Positional(Expr::Ident(id)))
    }
}

fn parse_param_value(lex: &mut Lexer) -> Result<Expr, String> {
    // param values: node_call | range_literal | binary_expr | primary
    lex.skip_ws();

    // Check for range literal (number..number)
    let saved = lex.pos;
    if let Some(lo) = lex.read_number() {
        lex.skip_ws();
        if lex.peek() == Some(b'.') && lex.peek2() == Some(b'.') {
            lex.bump(); lex.bump();
            let hi = lex.read_number().ok_or("expected number after '..'")?;
            return Ok(Expr::Range { lo, hi });
        }
        // Check binary op
        if let Some(op) = try_binop(lex) {
            let right = parse_primary_expr(lex)?;
            return Ok(Expr::BinOp {
                left: Box::new(Expr::Number(lo)),
                op,
                right: Box::new(right),
            });
        }
        return Ok(Expr::Number(lo));
    }
    lex.pos = saved;

    if let Some(id) = lex.read_ident() {
        lex.skip_ws();
        match lex.peek() {
            Some(b'{') => {
                // Node call (e.g. osc { ... }, adsr { ... })
                let nc = parse_node_call_after_kind(lex, id)?;
                return Ok(Expr::Call(nc));
            }
            Some(b'*') | Some(b'+') | Some(b'-') | Some(b'/') => {
                let op = try_binop(lex).unwrap();
                let right = parse_primary_expr(lex)?;
                return Ok(Expr::BinOp {
                    left: Box::new(Expr::Ident(id)),
                    op,
                    right: Box::new(right),
                });
            }
            _ => {
                return Ok(Expr::Ident(id));
            }
        }
    }

    lex.pos = saved;
    Err("could not parse param value".to_string())
}
