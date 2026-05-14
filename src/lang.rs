/// Parser and NodeDef builder for the patchlang text syntax.
///
/// # Quick usage
/// ```text
/// let env = parse(r#"
///   bass = patch {
///     osc { shape = saw, freq = freq }
///       | lpf  { cutoff = 800, q = 1.5 }
///       | adsr { attack = 0.02, decay = 0.1, sustain = 0.7, release = 0.15 }
///   }
/// "#);
///
/// // (freq_hz, velocity, duration_secs)
/// let notes = &[(261.63, 0.8, 0.5), (293.66, 0.8, 0.5)];
/// let samples = render_patch(&env, "bass", notes, 44100);
/// ```
use std::collections::HashMap;

use tree_sitter::{Language, Node, Parser};

use crate::envelope::Adsr;
use crate::filter::FilterType;
use crate::graph::{NodeDef, ParamDef, compile};
use crate::oscillator::OscillatorShape;

// ── C binding ─────────────────────────────────────────────────────────────────

unsafe extern "C" {
    fn tree_sitter_patchlang() -> *const tree_sitter::ffi::TSLanguage;
}

fn patchlang_language() -> Language {
    unsafe { Language::from_raw(tree_sitter_patchlang()) }
}

// ── AST ───────────────────────────────────────────────────────────────────────

pub struct PatchEnv {
    pub patches: HashMap<String, PatchDecl>,
    pub effects: HashMap<String, EffectDecl>,
}

pub struct PatchDecl {
    pub name: String,
    #[allow(dead_code)]
    pub voices: VoicesMode,
    pub stmts: Vec<PatchStmt>,
}

#[derive(Clone, Debug)]
pub enum VoicesMode {
    Mono,
    Poly(u32),
    MonoLegato,
}

pub enum PatchStmt {
    Binding { name: String, value: Expr },
    Chain(PipeChain),
}

pub struct EffectDecl {
    pub name: String,
    pub chain: PipeChain,
}

#[derive(Clone, Debug)]
pub struct PipeChain {
    pub head: NodeCall,
    pub segments: Vec<PipeSegment>,
}

#[derive(Clone, Debug)]
pub enum PipeSegment {
    Node(NodeCall),
    Ref(String),
}

#[derive(Clone, Debug)]
pub struct NodeCall {
    pub kind: String,
    pub params: Vec<ParamItem>,
}

#[derive(Clone, Debug)]
pub enum ParamItem {
    Named { key: String, value: Expr },
    Positional(Expr),
}

#[derive(Clone, Debug)]
pub enum Expr {
    Number(f64),
    Ident(String),
    BinOp { left: Box<Expr>, op: BinOpKind, right: Box<Expr> },
    Chain(Box<PipeChain>),
}

#[derive(Clone, Copy, Debug)]
pub enum BinOpKind {
    Add,
    Sub,
    Mul,
    Div,
}

// ── parser (CST → AST) ────────────────────────────────────────────────────────

pub fn parse(src: &str) -> PatchEnv {
    let mut parser = Parser::new();
    parser
        .set_language(&patchlang_language())
        .expect("patchlang language load failed");
    let tree = parser.parse(src, None).expect("parse returned None");
    let root = tree.root_node();

    if root.has_error() {
        eprintln!("warning: patchlang source contains parse errors");
    }

    let src = src.as_bytes();
    let mut patches = HashMap::new();
    let mut effects = HashMap::new();

    let mut cursor = root.walk();
    for decl_node in root.named_children(&mut cursor) {
        // source_file → declaration (choice of patch_decl | effect_decl)
        if decl_node.kind() != "declaration" {
            continue;
        }
        let inner = decl_node.named_child(0).expect("empty declaration");
        match inner.kind() {
            "patch_decl" => {
                let p = parse_patch_decl(inner, src);
                patches.insert(p.name.clone(), p);
            }
            "effect_decl" => {
                let e = parse_effect_decl(inner, src);
                effects.insert(e.name.clone(), e);
            }
            k => eprintln!("warning: unexpected declaration kind '{k}'"),
        }
    }

    PatchEnv { patches, effects }
}

fn parse_patch_decl(node: Node, src: &[u8]) -> PatchDecl {
    let name = text(node.child_by_field_name("name").unwrap(), src);

    let mut voices = VoicesMode::Mono;
    let mut stmts = Vec::new();

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() != "patch_stmt" {
            continue;
        }
        let inner = child.named_child(0).expect("empty patch_stmt");
        match inner.kind() {
            "voices_stmt" => voices = parse_voices_stmt(inner, src),
            "binding_stmt" => {
                let (bname, value) = parse_binding_stmt(inner, src);
                stmts.push(PatchStmt::Binding { name: bname, value });
            }
            "pipe_chain" => stmts.push(PatchStmt::Chain(parse_pipe_chain(inner, src))),
            k => eprintln!("warning: unexpected patch_stmt content '{k}'"),
        }
    }

    PatchDecl { name, voices, stmts }
}

fn parse_effect_decl(node: Node, src: &[u8]) -> EffectDecl {
    let name = text(node.child_by_field_name("name").unwrap(), src);
    let mut cursor = node.walk();
    let chain = node
        .named_children(&mut cursor)
        .find(|c| c.kind() == "pipe_chain")
        .map(|c| parse_pipe_chain(c, src))
        .expect("effect_decl has no pipe_chain");
    EffectDecl { name, chain }
}

fn parse_voices_stmt(node: Node, src: &[u8]) -> VoicesMode {
    let vv = node.named_child(0).expect("voices_stmt has no voices_value");
    let t = std::str::from_utf8(&src[vv.byte_range()]).unwrap().trim();
    if t == "mono legato" {
        VoicesMode::MonoLegato
    } else if let Some(rest) = t.strip_prefix("poly ") {
        VoicesMode::Poly(rest.trim().parse().unwrap_or(4))
    } else {
        VoicesMode::Mono
    }
}

fn parse_binding_stmt(node: Node, src: &[u8]) -> (String, Expr) {
    let name = text(node.child_by_field_name("name").unwrap(), src);
    let value = parse_expr(node.child_by_field_name("value").unwrap(), src);
    (name, value)
}

fn parse_pipe_chain(node: Node, src: &[u8]) -> PipeChain {
    let mut cursor = node.walk();
    let children: Vec<Node> = node.named_children(&mut cursor).collect();
    assert!(!children.is_empty(), "pipe_chain has no children");

    let head = parse_node_call(children[0], src);
    let segments = children[1..]
        .iter()
        .map(|c| {
            assert_eq!(c.kind(), "pipe_segment", "expected pipe_segment");
            parse_pipe_segment(*c, src)
        })
        .collect();

    PipeChain { head, segments }
}

fn parse_pipe_segment(node: Node, src: &[u8]) -> PipeSegment {
    let child = node.named_child(0).expect("empty pipe_segment");
    match child.kind() {
        "node_call" => PipeSegment::Node(parse_node_call(child, src)),
        "identifier" => PipeSegment::Ref(text(child, src)),
        k => panic!("unexpected pipe_segment child '{k}'"),
    }
}

fn parse_node_call(node: Node, src: &[u8]) -> NodeCall {
    let kind = text(node.child_by_field_name("kind").unwrap(), src);
    let mut params = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "param_list" {
            params = parse_param_list(child, src);
        }
    }
    NodeCall { kind, params }
}

fn parse_param_list(node: Node, src: &[u8]) -> Vec<ParamItem> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .map(|child| {
            assert_eq!(child.kind(), "param_item");
            parse_param_item(child, src)
        })
        .collect()
}

fn parse_param_item(node: Node, src: &[u8]) -> ParamItem {
    let child = node.named_child(0).expect("empty param_item");
    match child.kind() {
        "named_param" => {
            let key = text(child.child_by_field_name("key").unwrap(), src);
            let value = parse_param_value(child.child_by_field_name("value").unwrap(), src);
            ParamItem::Named { key, value }
        }
        "primary" => ParamItem::Positional(parse_primary(child, src)),
        k => panic!("unexpected param_item child '{k}'"),
    }
}

fn parse_param_value(node: Node, src: &[u8]) -> Expr {
    let child = node.named_child(0).expect("empty param_value");
    match child.kind() {
        "binary_expr" => parse_binary_expr(child, src),
        "primary" => parse_primary(child, src),
        k => panic!("unexpected param_value child '{k}'"),
    }
}

fn parse_expr(node: Node, src: &[u8]) -> Expr {
    let child = node.named_child(0).expect("empty expr");
    match child.kind() {
        "pipe_chain" => Expr::Chain(Box::new(parse_pipe_chain(child, src))),
        "binary_expr" => parse_binary_expr(child, src),
        "primary" => parse_primary(child, src),
        k => panic!("unexpected expr child '{k}'"),
    }
}

fn parse_binary_expr(node: Node, src: &[u8]) -> Expr {
    let left = parse_primary(node.child_by_field_name("left").unwrap(), src);
    let op_text = text(node.child_by_field_name("op").unwrap(), src);
    let op = match op_text.as_str() {
        "+" => BinOpKind::Add,
        "-" => BinOpKind::Sub,
        "*" => BinOpKind::Mul,
        "/" => BinOpKind::Div,
        s => panic!("unknown binop '{s}'"),
    };
    let right = parse_primary(node.child_by_field_name("right").unwrap(), src);
    Expr::BinOp { left: Box::new(left), op, right: Box::new(right) }
}

fn parse_primary(node: Node, src: &[u8]) -> Expr {
    let child = node.named_child(0).expect("empty primary");
    match child.kind() {
        "number" => Expr::Number(text(child, src).parse().expect("bad number literal")),
        "identifier" => Expr::Ident(text(child, src)),
        k => panic!("unexpected primary child '{k}'"),
    }
}

fn text(node: Node, src: &[u8]) -> String {
    std::str::from_utf8(&src[node.byte_range()])
        .expect("non-utf8 source")
        .to_string()
}

// ── NodeDef builder (AST → NodeDef) ──────────────────────────────────────────

struct BuildCtx<'a> {
    freq: f64,
    velocity: f32,
    duration_secs: f32,
    bindings: HashMap<String, &'a Expr>,
    effects: &'a HashMap<String, EffectDecl>,
}

impl<'a> BuildCtx<'a> {
    fn from_patch(
        freq: f64,
        velocity: f32,
        duration_secs: f32,
        patch: &'a PatchDecl,
        effects: &'a HashMap<String, EffectDecl>,
    ) -> Self {
        let bindings = patch
            .stmts
            .iter()
            .filter_map(|s| match s {
                PatchStmt::Binding { name, value } => Some((name.clone(), value)),
                _ => None,
            })
            .collect();
        Self { freq, velocity, duration_secs, bindings, effects }
    }

    fn build_patch(&self, patch: &PatchDecl) -> NodeDef {
        if let Some(out) = self.bindings.get("out") {
            return self.build_node(out);
        }
        for stmt in &patch.stmts {
            if let PatchStmt::Chain(chain) = stmt {
                return self.build_chain(chain, None);
            }
        }
        panic!("patch '{}' has no output", patch.name);
    }

    fn build_node(&self, expr: &Expr) -> NodeDef {
        match expr {
            Expr::Chain(chain) => self.build_chain(chain, None),
            Expr::Ident(name) => {
                if let Some(binding) = self.bindings.get(name) {
                    self.build_node(binding)
                } else {
                    panic!("unknown binding '{name}'")
                }
            }
            Expr::BinOp { left, op: BinOpKind::Mul, right } => {
                // `signal * scalar` or `scalar * signal` → Amplify
                let (node_side, scalar_side) = self.classify_mul_sides(left, right);
                let level = self.eval_scalar(scalar_side);
                NodeDef::Amplify {
                    level: ParamDef::Const(level),
                    source: Box::new(self.build_node(node_side)),
                }
            }
            Expr::Number(v) => {
                // A bare number used as a node is unusual; treat as a silent oscillator at 0 Hz.
                // In practice this shouldn't appear.
                NodeDef::Oscillator { shape: OscillatorShape::Sine, frequency: *v }
            }
            e => panic!("cannot build NodeDef from expr: {e:?}"),
        }
    }

    /// Heuristic: which side of `*` is the signal node and which is the scalar?
    fn classify_mul_sides<'b>(&self, left: &'b Expr, right: &'b Expr) -> (&'b Expr, &'b Expr) {
        let is_scalar = |e: &Expr| match e {
            Expr::Number(_) => true,
            Expr::Ident(n) => n == "velocity" || n == "freq",
            _ => false,
        };
        if is_scalar(left) { (right, left) } else { (left, right) }
    }

    fn build_chain(&self, chain: &PipeChain, inject: Option<NodeDef>) -> NodeDef {
        let mut node = self.build_node_call(&chain.head, inject);
        for seg in &chain.segments {
            node = match seg {
                PipeSegment::Node(call) => self.build_node_call(call, Some(node)),
                PipeSegment::Ref(name) => {
                    if let Some(effect) = self.effects.get(name) {
                        self.build_chain(&effect.chain, Some(node))
                    } else if let Some(binding) = self.bindings.get(name) {
                        // A binding ref in a pipe chain applies that sub-graph to the current node.
                        // Unusual — fall back to building the binding independently.
                        let _ = node;
                        self.build_node(binding)
                    } else {
                        panic!("unknown pipe segment ref '{name}'")
                    }
                }
            };
        }
        node
    }

    fn build_node_call(&self, call: &NodeCall, source: Option<NodeDef>) -> NodeDef {
        match call.kind.as_str() {
            "osc" | "lfo" => {
                let shape = self.param_shape(call, "shape").unwrap_or(OscillatorShape::Sine);
                let freq = self.param_f64(call, "freq").unwrap_or(self.freq);
                NodeDef::Oscillator { shape, frequency: freq }
            }
            "lpf" => self.build_filter(call, FilterType::LowPass, source),
            "hpf" => self.build_filter(call, FilterType::HighPass, source),
            "bpf" => self.build_filter(call, FilterType::BandPass, source),
            "notch" => self.build_filter(call, FilterType::Notch, source),
            "adsr" => self.build_envelope(call, source),
            "delay" => self.build_delay(call, source),
            "mix" => self.build_mix(call),
            "amp" => {
                let level = self.param_f64(call, "level").unwrap_or(1.0);
                NodeDef::Amplify {
                    level: ParamDef::Const(level),
                    source: Box::new(source.expect("amp requires a source")),
                }
            }
            k => panic!("unknown node kind '{k}'"),
        }
    }

    fn build_filter(&self, call: &NodeCall, kind: FilterType, source: Option<NodeDef>) -> NodeDef {
        let cutoff = self.param_f64(call, "cutoff").unwrap_or(1000.0);
        let q = self.param_f64(call, "q").unwrap_or(0.707);
        NodeDef::Filter {
            kind,
            cutoff_hz: ParamDef::Const(cutoff),
            q: ParamDef::Const(q),
            source: Box::new(source.expect("filter requires a source")),
        }
    }

    fn build_envelope(&self, call: &NodeCall, source: Option<NodeDef>) -> NodeDef {
        let attack  = self.param_f64(call, "attack").unwrap_or(0.01) as f32;
        let decay   = self.param_f64(call, "decay").unwrap_or(0.1) as f32;
        let sustain = self.param_f64(call, "sustain").unwrap_or(0.7) as f32;
        let release = self.param_f64(call, "release").unwrap_or(0.1) as f32;
        NodeDef::Envelope {
            adsr: Adsr {
                attack_secs: attack,
                decay_secs: decay,
                sustain_level: sustain,
                release_secs: release,
            },
            duration_secs: self.duration_secs,
            source: Box::new(source.expect("adsr requires a source")),
        }
    }

    fn build_delay(&self, call: &NodeCall, source: Option<NodeDef>) -> NodeDef {
        let time     = self.param_f64(call, "time").unwrap_or(0.25) as f32;
        let feedback = self.param_f64(call, "feedback").unwrap_or(0.0);
        let mix      = self.param_f64(call, "mix").unwrap_or(0.5);
        NodeDef::Delay {
            max_delay_secs: time + 0.01,
            delay_secs: ParamDef::Const(time as f64),
            feedback,
            mix,
            source: Box::new(source.expect("delay requires a source")),
        }
    }

    fn build_mix(&self, call: &NodeCall) -> NodeDef {
        let mut nodes: Vec<NodeDef> = Vec::new();
        for item in &call.params {
            match item {
                ParamItem::Positional(Expr::Ident(name)) => {
                    if let Some(binding) = self.bindings.get(name) {
                        nodes.push(self.build_node(binding));
                    } else {
                        panic!("mix: unknown source '{name}'");
                    }
                }
                _ => {}
            }
        }
        let n = nodes.len() as f64;
        let sources = nodes.into_iter().map(|node| (node, 1.0 / n)).collect();
        NodeDef::Mix { sources }
    }

    fn param_f64(&self, call: &NodeCall, key: &str) -> Option<f64> {
        call.params.iter().find_map(|p| match p {
            ParamItem::Named { key: k, value } if k == key => Some(self.eval_scalar(value)),
            _ => None,
        })
    }

    fn param_shape(&self, call: &NodeCall, key: &str) -> Option<OscillatorShape> {
        call.params.iter().find_map(|p| match p {
            ParamItem::Named { key: k, value: Expr::Ident(shape_name) } if k == key => {
                Some(match shape_name.as_str() {
                    "sine" | "sin" => OscillatorShape::Sine,
                    "saw" | "sawtooth" => OscillatorShape::Sawtooth,
                    "square" | "sq" => OscillatorShape::Square,
                    "triangle" | "tri" => OscillatorShape::Triangle,
                    s => {
                        eprintln!("warning: unknown oscillator shape '{s}', defaulting to sine");
                        OscillatorShape::Sine
                    }
                })
            }
            _ => None,
        })
    }

    fn eval_scalar(&self, expr: &Expr) -> f64 {
        match expr {
            Expr::Number(v) => *v,
            Expr::Ident(name) => match name.as_str() {
                "freq" => self.freq,
                "velocity" => self.velocity as f64,
                other => panic!("cannot evaluate '{other}' as a scalar"),
            },
            Expr::BinOp { left, op, right } => {
                let l = self.eval_scalar(left);
                let r = self.eval_scalar(right);
                match op {
                    BinOpKind::Add => l + r,
                    BinOpKind::Sub => l - r,
                    BinOpKind::Mul => l * r,
                    BinOpKind::Div => l / r,
                }
            }
            e => panic!("cannot evaluate expr as scalar: {e:?}"),
        }
    }
}

// ── public API ────────────────────────────────────────────────────────────────

/// Build a [`NodeDef`] for one note from a named patch in the environment.
///
/// `duration_secs` is used as the gate length for any `adsr` node in the patch.
pub fn build_note(
    env: &PatchEnv,
    patch_name: &str,
    freq: f64,
    velocity: f32,
    duration_secs: f32,
) -> NodeDef {
    let patch = env.patches.get(patch_name).expect("patch not found");
    let ctx = BuildCtx::from_patch(freq, velocity, duration_secs, patch, &env.effects);
    ctx.build_patch(patch)
}

/// Render a sequence of notes from a named patch to a flat sample buffer.
///
/// Each note is `(freq_hz, velocity, duration_secs)`.  Notes are rendered
/// sequentially; the buffer contains their audio concatenated end-to-end.
///
/// This is a **throwaway test helper** — not meant for production use.
pub fn render_patch(
    env: &PatchEnv,
    patch_name: &str,
    notes: &[(f64, f32, f32)],
    sample_rate: u32,
) -> Vec<f64> {
    let mut all_samples = Vec::new();

    for &(freq, velocity, duration_secs) in notes {
        let node = build_note(env, patch_name, freq, velocity, duration_secs);
        let mut source = compile(node, sample_rate);

        // Safety ceiling: render at most duration + 4 s to avoid infinite loops
        // when a patch has no terminating envelope.
        let max_samples = ((duration_secs + 4.0) * sample_rate as f32) as usize;
        let mut rendered = 0;

        const CHUNK: usize = 256;
        while !source.is_done() && rendered < max_samples {
            let n = CHUNK.min(max_samples - rendered);
            let mut chunk = vec![0.0f64; n];
            source.next_samples(&mut chunk);
            all_samples.extend_from_slice(&chunk);
            rendered += n;
        }
    }

    all_samples
}
