//! Rust (syn AST) -> readable Ruby projection.
//!
//! The goal is *readability with faithful navigation*, not executability:
//! every identifier we emit records the Rust span it came from, so an editor
//! can route go-to-definition / hover / references through rust-analyzer.

use crate::out::Out;
use proc_macro2::{LineColumn, Span, TokenStream, TokenTree};
use quote::ToTokens;
use std::collections::HashMap;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::*;

/// Zero-argument Rust methods that vanish from the view (`.iter()`, `.clone()`, `as_*`...):
/// in a garbage-collected, reference-free reading they carry no meaning.
pub const DROPPED_METHODS: &[&str] = &[
    "iter",
    "into_iter",
    "iter_mut",
    "as_ref",
    "as_mut",
    "as_str",
    "as_slice",
    "as_bytes_mut",
    "to_owned",
    "clone",
    "cloned",
    "copied",
    "borrow",
    "borrow_mut",
    "as_deref",
    "as_deref_mut",
    "by_ref",
    "into",
    "deref",
    "to_vec",
    "peekable",
    "as_path",
    "to_path_buf",
    "into_owned",
];

/// Rust keyword/literal -> its Ruby counterpart in the view. Single source of truth, like
/// [`METHOD_RENAMES`].
pub const KEYWORD_RENAMES: &[(&str, &str)] = &[("continue", "next"), ("None", "nil")];

fn keyword_word(rust: &str) -> &'static str {
    KEYWORD_RENAMES.iter().find(|(r, _)| *r == rust).map(|(_, w)| *w).unwrap_or("")
}

/// Rust macro -> the Ruby word that stands in for it (`println!` is shown as `puts`).
/// Single source of truth, like [`METHOD_RENAMES`].
pub const MACRO_RENAMES: &[(&str, &str)] = &[
    ("println", "puts"),
    ("print", "print"),
    ("eprintln", "warn"),
    ("eprint", "warn"),
    ("panic", "raise"),
    ("unreachable", "raise"),
    ("bail", "raise"),
    ("todo", "raise"),
    ("unimplemented", "raise"),
    ("anyhow", "RuntimeError"),
    ("assert", "assert"),
    ("assert_eq", "assert"),
    ("assert_ne", "assert"),
    ("debug_assert", "assert"),
    ("debug_assert_eq", "assert"),
    ("debug_assert_ne", "assert"),
    ("dbg", "p"),
];

fn macro_word(name: &str) -> &'static str {
    MACRO_RENAMES.iter().find(|(r, _)| *r == name).map(|(_, w)| *w).unwrap_or("")
}

/// Rust method -> Ruby method name in the view. The single source of truth: the golden
/// tests' map-accuracy check and `docs/spec/translation.md` both rely on it.
pub const METHOD_RENAMES: &[(&str, &str)] = &[
    ("len", "size"),
    ("is_empty", "empty?"),
    ("push_str", "concat"),
    ("contains", "include?"),
    ("contains_key", "key?"),
    ("starts_with", "start_with?"),
    ("ends_with", "end_with?"),
    ("to_lowercase", "downcase"),
    ("to_ascii_lowercase", "downcase"),
    ("to_uppercase", "upcase"),
    ("to_ascii_uppercase", "upcase"),
    ("trim", "strip"),
    ("trim_start", "lstrip"),
    ("trim_end", "rstrip"),
    ("filter", "select"),
    ("any", "any?"),
    ("all", "all?"),
    ("position", "find_index"),
    ("fold", "inject"),
    ("for_each", "each"),
    ("rev", "reverse"),
    ("extend", "concat"),
    ("retain", "select!"),
    ("sort", "sort!"),
    ("sort_unstable", "sort!"),
    ("sort_by", "sort!"),
    ("sort_unstable_by", "sort!"),
    ("sort_by_key", "sort_by!"),
    ("sort_unstable_by_key", "sort_by!"),
    ("dedup", "uniq!"),
    ("skip", "drop"),
    ("skip_while", "drop_while"),
    ("flatten", "flatten"),
    ("last", "last"),
    ("first", "first"),
    ("is_ok", "ok?"),
    ("is_err", "err?"),
    ("min_by_key", "min_by"),
    ("max_by_key", "max_by"),
    ("min_by", "min"),
    ("max_by", "max"),
    ("chunks", "each_slice"),
    ("windows", "each_cons"),
    ("split_whitespace", "split"),
    ("pop", "pop"),
    ("remove", "delete"),
    ("abs", "abs"),
    ("powi", "pow"),
    ("powf", "pow"),
    ("pow", "pow"),
];

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    Struct,
    Enum,
    Trait,
}

pub struct Gen<'a> {
    pub out: Out,
    /// Crate-wide knowledge (from .rbview/symbols.json + this file): type name -> kind.
    pub kinds: &'a HashMap<String, Kind>,
    local_kinds: HashMap<String, Kind>,
    self_ty: Option<String>,
    /// true while emitting items inside a Ruby `module` (free fns become `def self.x`)
    in_module: bool,
    /// Items declared inside fn bodies; Ruby can't define classes/constants in a
    /// method, so they're shown right after the enclosing method.
    deferred: Vec<Vec<(Item, String)>>,
    fn_names: Vec<String>,
    cur_params: Option<Vec<PatType>>,
    /// token positions src_tokens leaves unmapped (see `src_tokens_except`)
    skip_positions: Vec<LineColumn>,
}

const RUBY_KEYWORDS: &[&str] = &[
    "alias", "and", "begin", "BEGIN", "case", "class", "def", "defined", "do", "else", "elsif", "end", "END", "ensure",
    "module", "next", "nil", "not", "or", "redo", "rescue", "retry", "then", "undef", "unless", "until", "when",
    "yield", "__FILE__", "__LINE__",
];

pub fn rb_ident(name: &str) -> String {
    let n = name.strip_prefix("r#").unwrap_or(name);
    if RUBY_KEYWORDS.contains(&n) { format!("{n}_") } else { n.to_string() }
}

pub fn camel(snake: &str) -> String {
    let s = snake.strip_prefix("r#").unwrap_or(snake);
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// A Ruby constant name for a Rust type name (`timeval` -> `Timeval`, `str` -> `Str`).
pub fn const_name(name: &str) -> String {
    let n = name.strip_prefix("r#").unwrap_or(name);
    if is_capitalized(n) { n.to_string() } else { camel(n) }
}

fn is_capitalized(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_uppercase())
}

fn is_const_case(s: &str) -> bool {
    s.chars().any(|c| c.is_alphabetic()) && s.chars().all(|c| c.is_uppercase() || c.is_ascii_digit() || c == '_')
}

/// Min/max source positions covered by a syntax node's tokens.
pub fn tok_range<T: ToTokens + ?Sized>(t: &T) -> Option<(LineColumn, LineColumn)> {
    fn walk(ts: TokenStream, acc: &mut Option<(LineColumn, LineColumn)>) {
        fn upd(acc: &mut Option<(LineColumn, LineColumn)>, a: LineColumn, b: LineColumn) {
            if a.line == 0 {
                return;
            }
            let lt = |x: &LineColumn, y: &LineColumn| (x.line, x.column) < (y.line, y.column);
            match acc {
                None => *acc = Some((a, b)),
                Some((lo, hi)) => {
                    if lt(&a, lo) {
                        *lo = a;
                    }
                    if lt(hi, &b) {
                        *hi = b;
                    }
                }
            }
        }
        for tt in ts {
            match tt {
                TokenTree::Group(g) => {
                    if g.delimiter() == proc_macro2::Delimiter::None {
                        walk(g.stream(), acc);
                    } else {
                        upd(acc, g.span_open().start(), g.span_open().end());
                        upd(acc, g.span_close().start(), g.span_close().end());
                    }
                }
                other => {
                    let sp = other.span();
                    upd(acc, sp.start(), sp.end());
                }
            }
        }
    }
    let mut acc = None;
    walk(t.to_token_stream(), &mut acc);
    acc
}

/// `%q` delimiters that are safe for `text` (None: needs an escaped "..." string).
fn pct_q_delims(text: &str) -> Option<(String, String)> {
    if text.contains('\\') {
        return None;
    }
    for (open, close) in [('{', '}'), ('(', ')'), ('[', ']')] {
        let mut depth = 0i32;
        let mut ok = true;
        for c in text.chars() {
            if c == open {
                depth += 1
            } else if c == close {
                depth -= 1;
                if depth < 0 {
                    ok = false;
                    break;
                }
            }
        }
        if ok && depth == 0 {
            return Some((format!("%q{open}"), close.to_string()));
        }
    }
    ['~', '^', '|', '!', '`', '@'].into_iter().find(|d| !text.contains(*d)).map(|d| (format!("%q{d}"), d.to_string()))
}

/// `%q` literal with a delimiter that doesn't occur in the text.
#[allow(dead_code)]
fn pct_q(text: &str) -> String {
    for (open, close) in [('{', '}'), ('(', ')'), ('[', ']')] {
        // paired delimiters are fine when balanced
        let mut depth = 0i32;
        let mut ok = true;
        for c in text.chars() {
            if c == open {
                depth += 1
            } else if c == close {
                depth -= 1;
                if depth < 0 {
                    ok = false;
                    break;
                }
            }
        }
        if ok && depth == 0 && !text.contains('\\') {
            return format!("%q{open}{text}{close}");
        }
    }
    for d in ['~', '^', '|', '!', '`', '@'] {
        if !text.contains(d) && !text.contains('\\') {
            return format!("%q{d}{text}{d}");
        }
    }
    format!("\"{}\"", escape_rb(text))
}

fn escape_rb(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            '\n' => o.push_str("\\n"),
            '\t' => o.push_str("\\t"),
            '\r' => o.push_str("\\r"),
            '\0' => o.push_str("\\0"),
            '#' if matches!(chars.peek(), Some('{') | Some('@') | Some('$')) => o.push_str("\\#"),
            c => o.push(c),
        }
    }
    o
}

impl<'a> Gen<'a> {
    pub fn new(src: &str, kinds: &'a HashMap<String, Kind>) -> Self {
        Gen {
            out: Out::new(src),
            kinds,
            local_kinds: HashMap::new(),
            self_ty: None,
            in_module: false,
            deferred: vec![],
            fn_names: vec![],
            cur_params: None,
            skip_positions: vec![],
        }
    }

    fn kind_of(&self, name: &str) -> Option<Kind> {
        self.local_kinds.get(name).or_else(|| self.kinds.get(name)).copied()
    }

    // ---------------------------------------------------------------- tokens

    fn ident(&mut self, id: &Ident) {
        let s = rb_ident(&id.to_string());
        self.span_tok(&s, id.span());
    }

    /// A field/method name: Ruby keywords are fine here (`:end`, `r.end`), only locals need `end_`.
    fn field_name(&mut self, id: &Ident) {
        let s = id.to_string();
        self.span_tok(s.trim_start_matches("r#"), id.span());
    }

    fn span_tok(&mut self, s: &str, sp: Span) {
        self.out.tok(s, sp.start(), sp.end());
    }

    fn node_tok<T: ToTokens + ?Sized>(&mut self, s: &str, node: &T) {
        match tok_range(node) {
            Some((a, b)) => self.out.tok(s, a, b),
            None => self.out.push(s),
        }
    }

    fn src_of<T: ToTokens + ?Sized>(&self, node: &T) -> String {
        match tok_range(node) {
            Some((a, b)) => self.out.src_text(a, b),
            None => node.to_token_stream().to_string(),
        }
    }

    /// Rust source of `node` on one line, re-spaced as written, with every token mapped to its
    /// own Rust span (so each type in a signature comment is clickable).
    fn src_tokens<T: ToTokens + ?Sized>(&mut self, node: &T) {
        self.src_tokens_except(node, &[]);
    }

    /// Like [`Self::src_tokens`], but tokens starting at `skip` positions are left unmapped:
    /// they're mapped on the code line (e.g. the fn name in `def name`), which must win.
    fn src_tokens_except<T: ToTokens + ?Sized>(&mut self, node: &T, skip: &[LineColumn]) {
        let mut prev = None;
        let saved = std::mem::replace(&mut self.skip_positions, skip.to_vec());
        self.tokens_rec(node.to_token_stream(), &mut prev);
        self.skip_positions = saved;
    }

    fn tokens_rec(&mut self, ts: TokenStream, prev: &mut Option<LineColumn>) {
        for tt in ts {
            match tt {
                TokenTree::Group(g) => {
                    let (open, close) = match g.delimiter() {
                        proc_macro2::Delimiter::Parenthesis => ("(", ")"),
                        proc_macro2::Delimiter::Brace => ("{", "}"),
                        proc_macro2::Delimiter::Bracket => ("[", "]"),
                        proc_macro2::Delimiter::None => ("", ""),
                    };
                    if open.is_empty() {
                        self.tokens_rec(g.stream(), prev);
                        continue;
                    }
                    self.tok_spaced(open, g.span_open(), prev);
                    self.tokens_rec(g.stream(), prev);
                    self.tok_spaced(close, g.span_close(), prev);
                }
                leaf => {
                    let (a, b) = (leaf.span().start(), leaf.span().end());
                    let text =
                        if a.line != 0 && a.line == b.line { self.out.src_exact(a, b) } else { leaf.to_string() };
                    self.tok_spaced(&text, leaf.span(), prev);
                }
            }
        }
    }

    fn tok_spaced(&mut self, text: &str, sp: Span, prev: &mut Option<LineColumn>) {
        let (a, b) = (sp.start(), sp.end());
        // comments are single lines: a multi-line literal must not break out of one
        let flat;
        let text = if text.contains('\n') {
            flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
            flat.as_str()
        } else {
            text
        };
        if self.skip_positions.contains(&a) {
            if prev.is_some_and(|p| a.line != p.line || a.column > p.column) {
                self.out.push(" ");
            }
            self.out.push(text);
            *prev = Some(b);
            return;
        }
        if a.line == 0 {
            // synthesized token without a source position
            if prev.is_some() {
                self.out.push(" ");
            }
            self.out.push(text);
            return;
        }
        if let Some(p) = prev
            && (a.line != p.line || a.column > p.column)
        {
            self.out.push(" ");
        }
        self.out.tok(text, a, b);
        *prev = Some(b);
    }

    /// Raw Rust kept verbatim for constructs we don't translate.
    fn raw<T: ToTokens + ?Sized>(&mut self, node: &T) {
        self.out.push("rust!(");
        self.raw_literal(node);
        self.out.push(")");
    }

    /// Source text of `node` as a %q literal. Single-line source is kept exact (not
    /// whitespace-collapsed) so every character maps back to its Rust column.
    fn raw_literal<T: ToTokens + ?Sized>(&mut self, node: &T) {
        let range = tok_range(node);
        let text = match range {
            Some((a, b)) if a.line == b.line => self.out.src_exact(a, b),
            _ => self.src_of(node),
        };
        match (pct_q_delims(&text), range) {
            (Some((open, close)), Some((a, b))) => {
                self.out.push(&open);
                self.out.tok(&text, a, b);
                self.out.push(&close);
            }
            (_, Some((a, b))) => self.out.tok(&format!("\"{}\"", escape_rb(&text)), a, b),
            (_, None) => self.out.push(&format!("\"{}\"", escape_rb(&text))),
        }
    }

    // ---------------------------------------------------------------- attrs

    fn attrs(&mut self, attrs: &[Attribute]) {
        let mut doc = vec![];
        for a in attrs {
            if a.path().is_ident("doc") {
                if let Meta::NameValue(nv) = &a.meta
                    && let Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) = &nv.value
                {
                    doc.push(s.value());
                }
            } else {
                if !doc.is_empty() {
                    let text: Vec<String> = doc.iter().map(|l| l.strip_prefix(' ').unwrap_or(l).to_string()).collect();
                    self.out.comment(&text.join("\n"));
                    doc.clear();
                }
                self.out.nl();
                self.out.push("# ");
                self.src_tokens(a);
                self.out.nl();
            }
        }
        if !doc.is_empty() {
            let text: Vec<String> = doc.iter().map(|l| l.strip_prefix(' ').unwrap_or(l).to_string()).collect();
            self.out.comment(&text.join("\n"));
        }
    }

    // ---------------------------------------------------------------- file / items

    pub fn file(&mut self, f: &File, module_path: &[String]) {
        // Pre-scan local type kinds
        fn scan(items: &[Item], m: &mut HashMap<String, Kind>) {
            for it in items {
                match it {
                    Item::Struct(s) => {
                        m.insert(s.ident.to_string(), Kind::Struct);
                    }
                    Item::Enum(e) => {
                        m.insert(e.ident.to_string(), Kind::Enum);
                    }
                    Item::Trait(t) => {
                        m.insert(t.ident.to_string(), Kind::Trait);
                    }
                    Item::Mod(ItemMod { content: Some((_, items)), .. }) => scan(items, m),
                    _ => {}
                }
            }
        }
        scan(&f.items, &mut self.local_kinds);

        self.out.push("# frozen_string_literal: true\n");
        self.out.push("# rbview: read-only Ruby projection of Rust source. Not executable.\n");
        self.attrs(&f.attrs);
        if !module_path.is_empty() {
            self.out.push("module ");
            self.out.push(&module_path.iter().map(|m| camel(m)).collect::<Vec<_>>().join("::"));
            self.out.nl();
            self.out.indent += 1;
            self.in_module = true;
        }
        self.items(&f.items);
        if !module_path.is_empty() {
            self.out.indent -= 1;
            self.out.nl();
            self.out.push("end");
        }
        self.out.nl();
    }

    fn items(&mut self, items: &[Item]) {
        let mut first = true;
        let mut prev_use = false;
        for it in items {
            let is_use = matches!(it, Item::Use(_));
            if !first && !(is_use && prev_use) {
                self.out.blank();
            }
            first = false;
            prev_use = is_use;
            self.out.nl();
            let rb_start = self.out.line;
            self.item(it);
            self.out.nl();
            self.out.block(rb_start, tok_range(it));
        }
    }

    fn item(&mut self, it: &Item) {
        match it {
            Item::Use(u) => {
                self.attrs(&u.attrs);
                self.out.push("# use ");
                self.use_tree(&u.tree);
                self.out.nl();
            }
            Item::Fn(f) => {
                self.attrs(&f.attrs);
                let as_self = self.in_module;
                self.fn_def(&f.sig, Some(&f.block), as_self);
            }
            Item::Struct(s) => self.struct_def(s),
            Item::Enum(e) => self.enum_def(e),
            Item::Trait(t) => self.trait_def(t),
            Item::Impl(i) => self.impl_def(i),
            Item::Mod(m) => {
                self.attrs(&m.attrs);
                match &m.content {
                    Some((_, items)) => {
                        self.out.push("module ");
                        self.span_tok(&camel(&m.ident.to_string()), m.ident.span());
                        self.out.nl();
                        self.out.indent += 1;
                        let prev = std::mem::replace(&mut self.in_module, true);
                        self.items(items);
                        self.in_module = prev;
                        self.out.indent -= 1;
                        self.out.nl();
                        self.out.push("end");
                    }
                    None => {
                        self.out.push("# mod ");
                        self.ident(&m.ident);
                        self.out.push("  (separate file — follow with go-to-definition)");
                    }
                }
            }
            Item::Const(c) => {
                self.attrs(&c.attrs);
                self.ident(&c.ident);
                self.out.push(" = ");
                self.expr(&c.expr);
                self.out.push("  # ");
                self.src_tokens(&c.ty);
            }
            Item::Static(s) => {
                self.attrs(&s.attrs);
                self.ident(&s.ident);
                self.out.push(" = ");
                self.expr(&s.expr);
                self.out.push("  # static ");
                self.src_tokens(&s.ty);
            }
            Item::Type(t) => {
                self.attrs(&t.attrs);
                self.ident(&t.ident);
                self.out.push(" = ");
                self.type_const(&t.ty);
                self.out.push("  # type alias: ");
                self.src_tokens(&t.ty);
            }
            Item::Macro(m) => {
                self.attrs(&m.attrs);
                if m.mac.path.is_ident("macro_rules") {
                    // Kept verbatim (as comments) so jumps to items defined inside macros —
                    // common in std, e.g. integer methods — land on the exact token.
                    match tok_range(&m.mac) {
                        Some((a, b)) => {
                            self.out.comment("macro definition (Rust, verbatim):");
                            self.out.verbatim(a, b, "#   ");
                        }
                        None => {
                            self.out.push("# macro_rules! ");
                            if let Some(id) = &m.ident {
                                self.ident(id);
                            }
                        }
                    }
                } else {
                    self.macro_stmt(&m.mac);
                }
            }
            other => {
                // extern crate, foreign mods, unions, trait aliases, `pub macro`...: verbatim
                match tok_range(other) {
                    Some((a, b)) => self.out.verbatim(a, b, "# "),
                    None => self.out.push("# (unsupported item)"),
                }
            }
        }
    }

    fn use_tree(&mut self, t: &UseTree) {
        match t {
            UseTree::Path(p) => {
                self.span_tok(&p.ident.to_string(), p.ident.span());
                self.out.push("::");
                self.use_tree(&p.tree);
            }
            UseTree::Name(n) => self.span_tok(&n.ident.to_string(), n.ident.span()),
            UseTree::Rename(r) => {
                self.span_tok(&r.ident.to_string(), r.ident.span());
                self.out.push(" as ");
                self.span_tok(&r.rename.to_string(), r.rename.span());
            }
            UseTree::Glob(_) => self.out.push("*"),
            UseTree::Group(g) => {
                self.out.push("{");
                for (i, it) in g.items.iter().enumerate() {
                    if i > 0 {
                        self.out.push(", ");
                    }
                    self.use_tree(it);
                }
                self.out.push("}");
            }
        }
    }

    /// `def name(params)` with the original Rust signature as a comment above it.
    fn fn_def(&mut self, sig: &Signature, body: Option<&Block>, force_self: bool) {
        let mut skip = vec![sig.ident.span().start()];
        for arg in &sig.inputs {
            if let FnArg::Typed(t) = arg
                && let Pat::Ident(pi) = &*t.pat
            {
                skip.push(pi.ident.span().start());
            }
        }
        self.out.nl();
        self.out.push("# ");
        self.src_tokens_except(sig, &skip);
        self.out.nl();

        let has_receiver = matches!(sig.inputs.first(), Some(FnArg::Receiver(_)));
        self.out.push("def ");
        if !has_receiver && (self.self_ty.is_some() || force_self) {
            self.out.push("self.");
        }
        let name = sig.ident.to_string();
        let name = name.strip_prefix("r#").unwrap_or(&name).to_string();
        self.fn_names.push(name.clone());
        self.span_tok(&name, sig.ident.span());
        let params: Vec<&PatType> = sig
            .inputs
            .iter()
            .filter_map(|a| match a {
                FnArg::Typed(t) => Some(t),
                _ => None,
            })
            .collect();
        if !params.is_empty() {
            self.out.push("(");
            for (i, p) in params.iter().enumerate() {
                if i > 0 {
                    self.out.push(", ");
                }
                match &*p.pat {
                    Pat::Ident(pi) => self.ident(&pi.ident),
                    Pat::Wild(w) => self.node_tok("_", w),
                    _ => {
                        // destructuring param: Ruby can't, show as raw name
                        self.node_tok(&format!("arg{i}"), &p.pat);
                    }
                }
            }
            self.out.push(")");
        }
        self.cur_params = Some(params.iter().map(|p| (*p).clone()).collect());
        self.fn_body(body);
        self.fn_names.pop();
    }

    fn fn_body(&mut self, body: Option<&Block>) {
        let params: Vec<PatType> = self.cur_params.take().unwrap_or_default();
        let params: Vec<&PatType> = params.iter().collect();
        match body {
            None => {
                self.out.push(" = raise NotImplementedError  # required");
            }
            Some(b) => {
                self.out.nl();
                self.out.indent += 1;
                // destructuring params become pattern matches at the top of the body
                for (i, p) in params.iter().enumerate() {
                    if !matches!(&*p.pat, Pat::Ident(_) | Pat::Wild(_)) {
                        self.out.nl();
                        self.out.push(&format!("arg{i} => "));
                        self.pat(&p.pat);
                    }
                }
                self.deferred.push(vec![]);
                self.block_stmts(b);
                let hoisted = self.deferred.pop().unwrap_or_default();
                self.out.indent -= 1;
                self.out.nl();
                self.out.push("end");
                for (it, fn_name) in hoisted {
                    let fn_name: String = fn_name;
                    self.out.blank();
                    self.out.comment(&format!("(declared inside fn {fn_name})"));
                    let rb_start = self.out.line;
                    let prev = std::mem::replace(&mut self.in_module, true);
                    self.item(&it);
                    self.in_module = prev;
                    self.out.nl();
                    self.out.block(rb_start, tok_range(&it));
                }
            }
        }
    }

    fn generics_comment(&mut self, g: &Generics) {
        if !g.params.is_empty() || g.where_clause.is_some() {
            self.out.push("  # <");
            self.src_tokens(&g.params);
            self.out.push(">");
            if let Some(w) = &g.where_clause {
                self.out.push(" ");
                self.src_tokens(w);
            }
        }
    }

    fn const_ident(&mut self, id: &Ident) {
        self.span_tok(&const_name(&id.to_string()), id.span());
    }

    fn struct_def(&mut self, s: &ItemStruct) {
        self.attrs(&s.attrs);
        self.out.push("class ");
        self.const_ident(&s.ident);
        self.generics_comment(&s.generics);
        match &s.fields {
            Fields::Unit => {
                self.out.push("; end");
                return;
            }
            Fields::Named(n) => {
                self.out.indent += 1;
                for f in &n.named {
                    self.out.nl();
                    self.attrs(&f.attrs);
                    self.out.push("attr_accessor :");
                    self.field_name(f.ident.as_ref().unwrap());
                    self.out.push("  # ");
                    self.src_tokens(&f.ty);
                }
                self.out.indent -= 1;
            }
            Fields::Unnamed(u) => {
                self.out.indent += 1;
                self.out.nl();
                self.out.push("# tuple struct: self[0]");
                for (i, f) in u.unnamed.iter().enumerate() {
                    if i > 0 {
                        self.out.push(&format!(", self[{i}]"));
                    }
                    self.out.push(": ");
                    self.src_tokens(&f.ty);
                }
                self.out.indent -= 1;
            }
        }
        self.out.nl();
        self.out.push("end");
    }

    fn enum_def(&mut self, e: &ItemEnum) {
        self.attrs(&e.attrs);
        self.out.push("module ");
        self.const_ident(&e.ident);
        self.out.push("  # enum");
        self.generics_comment(&e.generics);
        self.out.indent += 1;
        let enum_name = const_name(&e.ident.to_string());
        for v in &e.variants {
            self.out.nl();
            self.attrs(&v.attrs);
            self.const_ident(&v.ident);
            self.out.push(" = Data.define");
            match &v.fields {
                Fields::Unit => {}
                Fields::Named(n) => {
                    self.out.push("(");
                    for (i, f) in n.named.iter().enumerate() {
                        if i > 0 {
                            self.out.push(", ");
                        }
                        self.out.push(":");
                        self.field_name(f.ident.as_ref().unwrap());
                    }
                    self.out.push(")");
                }
                Fields::Unnamed(u) => {
                    self.out.push("(");
                    for (i, f) in u.unnamed.iter().enumerate() {
                        if i > 0 {
                            self.out.push(", ");
                        }
                        self.node_tok(&format!(":_{i}"), &f.ty);
                    }
                    self.out.push(")");
                }
            }
            self.out.push(&format!(" {{ include {enum_name} }}"));
            if !matches!(v.fields, Fields::Unit) {
                self.out.push("  # ");
                // field names are mapped on `Data.define(:name)`; the comment only adds types
                let skip: Vec<LineColumn> =
                    v.fields.iter().filter_map(|f| f.ident.as_ref().map(|i| i.span().start())).collect();
                self.src_tokens_except(&v.fields, &skip);
            }
            if let Some((_, d)) = &v.discriminant {
                self.out.push(if matches!(v.fields, Fields::Unit) { "  # = " } else { " = " });
                self.src_tokens(d);
            }
        }
        self.out.indent -= 1;
        self.out.nl();
        self.out.push("end");
    }

    fn trait_def(&mut self, t: &ItemTrait) {
        self.attrs(&t.attrs);
        self.out.push("module ");
        self.const_ident(&t.ident);
        self.out.push("  # trait");
        if !t.supertraits.is_empty() {
            self.out.push(": ");
            self.src_tokens(&t.supertraits);
        }
        self.generics_comment(&t.generics);
        self.out.indent += 1;
        let prev = self.self_ty.replace(t.ident.to_string());
        for (idx, it) in t.items.iter().enumerate() {
            if idx > 0 {
                self.out.blank();
            } else {
                self.out.nl();
            }
            let rb_start = self.out.line;
            match it {
                TraitItem::Fn(f) => {
                    self.attrs(&f.attrs);
                    self.fn_def(&f.sig, f.default.as_ref(), false);
                }
                TraitItem::Type(ty) => {
                    self.attrs(&ty.attrs);
                    self.out.push("# associated type ");
                    self.ident(&ty.ident);
                    if !ty.bounds.is_empty() {
                        self.out.push(": ");
                        self.src_tokens(&ty.bounds);
                    }
                }
                TraitItem::Const(c) => {
                    self.attrs(&c.attrs);
                    self.ident(&c.ident);
                    self.out.push(" = ");
                    match &c.default {
                        Some((_, e)) => self.expr(e),
                        None => self.out.push("nil  # required"),
                    }
                    self.out.push("  # ");
                    self.src_tokens(&c.ty);
                }
                other => {
                    self.out.push("# ");
                    self.src_tokens(other);
                }
            }
            self.out.nl();
            self.out.block(rb_start, tok_range(it));
        }
        self.self_ty = prev;
        self.out.indent -= 1;
        self.out.nl();
        self.out.push("end");
    }

    fn type_name(ty: &Type) -> Option<(String, &Ident)> {
        match ty {
            Type::Path(tp) => tp.path.segments.last().map(|s| (s.ident.to_string(), &s.ident)),
            Type::Reference(r) => Self::type_name(&r.elem),
            Type::Paren(p) => Self::type_name(&p.elem),
            Type::Group(g) => Self::type_name(&g.elem),
            Type::Ptr(p) => Self::type_name(&p.elem),
            Type::TraitObject(t) => t.bounds.iter().find_map(|b| match b {
                TypeParamBound::Trait(tb) => tb.path.segments.last().map(|s| (s.ident.to_string(), &s.ident)),
                _ => None,
            }),
            _ => None,
        }
    }

    fn type_const(&mut self, ty: &Type) {
        match Self::type_name(ty) {
            Some((n, id)) => self.span_tok(&const_name(&n), id.span()),
            None => match ty {
                Type::Slice(_) | Type::Array(_) | Type::Tuple(_) => self.node_tok("Array", ty),
                _ => self.out.push("Object"),
            },
        }
    }

    fn impl_def(&mut self, i: &ItemImpl) {
        self.attrs(&i.attrs);
        let (name, kw) = match Self::type_name(&i.self_ty) {
            Some((n, _)) => {
                let kw = match self.kind_of(&n) {
                    Some(Kind::Enum) | Some(Kind::Trait) => "module",
                    _ => "class",
                };
                (n, kw)
            }
            None => ("Object".to_string(), "class"),
        };
        self.out.push(kw);
        self.out.push(" ");
        self.type_const(&i.self_ty);
        self.out.push("  # ");
        self.node_tok("impl", &i.impl_token);
        if !i.generics.params.is_empty() {
            self.out.push("<");
            self.src_tokens(&i.generics.params);
            self.out.push(">");
        }
        self.out.push(" ");
        if let Some((bang, p, _)) = &i.trait_ {
            if bang.is_some() {
                self.out.push("!");
            }
            self.src_tokens(p);
            self.out.push(" for ");
        }
        self.src_tokens(&i.self_ty);
        if let Some(w) = &i.generics.where_clause {
            self.out.push(" ");
            self.src_tokens(w);
        }
        self.out.indent += 1;
        if let Some((neg, p, _)) = &i.trait_ {
            self.out.nl();
            self.out.push(if neg.is_some() { "# !" } else { "include " });
            self.path_const(p);
        }
        let prev = self.self_ty.replace(name);
        for (idx, it) in i.items.iter().enumerate() {
            if idx > 0 || i.trait_.is_some() {
                self.out.blank();
            } else {
                self.out.nl();
            }
            let rb_start = self.out.line;
            self.impl_item(it);
            self.out.nl();
            self.out.block(rb_start, tok_range(it));
        }
        self.self_ty = prev;
        self.out.indent -= 1;
        self.out.nl();
        self.out.push("end");
    }

    fn impl_item(&mut self, it: &ImplItem) {
        match it {
            ImplItem::Fn(f) => {
                self.attrs(&f.attrs);
                self.fn_def(&f.sig, Some(&f.block), false);
            }
            ImplItem::Const(c) => {
                self.attrs(&c.attrs);
                self.ident(&c.ident);
                self.out.push(" = ");
                self.expr(&c.expr);
                self.out.push("  # ");
                self.src_tokens(&c.ty);
            }
            ImplItem::Type(t) => {
                self.attrs(&t.attrs);
                self.out.push("# type ");
                self.ident(&t.ident);
                self.out.push(" = ");
                self.src_tokens(&t.ty);
            }
            ImplItem::Macro(m) => {
                self.attrs(&m.attrs);
                // `cfg_net! { pub async fn connect(..) {..} }` inside an impl: translate the methods
                let parsed = m.mac.parse_body_with(|input: parse::ParseStream| {
                    let mut v = Vec::new();
                    while !input.is_empty() {
                        v.push(input.parse::<ImplItem>()?);
                    }
                    Ok(v)
                });
                match parsed {
                    Ok(items) if !items.is_empty() => {
                        let path = self.src_of(&m.mac.path);
                        self.out.push("# ");
                        self.node_tok(&format!("{path}! {{"), &m.mac.path);
                        for (i, sub) in items.iter().enumerate() {
                            if i > 0 {
                                self.out.blank();
                            } else {
                                self.out.nl();
                            }
                            let rb_start = self.out.line;
                            self.impl_item(sub);
                            self.out.nl();
                            self.out.block(rb_start, tok_range(sub));
                        }
                        self.out.nl();
                        self.out.push("# }");
                    }
                    _ => self.macro_stmt(&m.mac),
                }
            }
            other => match tok_range(other) {
                Some((a, b)) => self.out.verbatim(a, b, "# "),
                None => self.out.push("# (unsupported impl item)"),
            },
        }
    }

    /// A path used as a constant/type: `a::b::Foo` -> `A::B::Foo`
    fn path_const(&mut self, p: &Path) {
        let n = p.segments.len();
        let mut first = true;
        for (i, seg) in p.segments.iter().enumerate() {
            let s = seg.ident.to_string();
            if (s == "crate" || s == "self") && i + 1 < n {
                continue;
            }
            if !first {
                self.out.push("::");
            }
            first = false;
            let rb = if s == "Self" {
                self.self_ty.clone().unwrap_or_else(|| "self.class".into())
            } else if is_capitalized(&s) {
                s
            } else {
                camel(&s)
            };
            self.span_tok(&rb, seg.ident.span());
        }
    }

    // ---------------------------------------------------------------- statements

    fn block_stmts(&mut self, b: &Block) {
        for s in &b.stmts {
            self.out.nl();
            let rb_start = self.out.line;
            self.stmt(s);
            self.out.nl();
            self.out.block(rb_start, tok_range(s));
        }
    }

    /// `begin ... end` / indented body used where a block is an expression.
    fn body(&mut self, b: &Block) {
        self.out.indent += 1;
        self.block_stmts(b);
        self.out.indent -= 1;
        self.out.nl();
    }

    fn stmt(&mut self, s: &Stmt) {
        match s {
            Stmt::Local(l) => self.local(l),
            Stmt::Item(it) => {
                let keep_in_place =
                    matches!(it, Item::Use(_)) || matches!(it, Item::Macro(m) if m.mac.path.is_ident("macro_rules"));
                if keep_in_place || self.deferred.is_empty() {
                    return self.item(it);
                }
                let (kind, name) = item_label(it);
                let fn_name = self.fn_names.last().cloned().unwrap_or_default();
                self.out.push(&format!("# local {kind} "));
                match item_ident(it) {
                    Some(id) => self.span_tok(&name, id.span()),
                    None => self.out.push(&name),
                }
                self.out.push(" — shown after this method");
                self.deferred.last_mut().unwrap().push((it.clone(), fn_name));
            }
            Stmt::Expr(Expr::Macro(m), _) => {
                self.attrs(&m.attrs);
                self.macro_stmt(&m.mac);
            }
            Stmt::Expr(e, _) => self.expr(e),
            Stmt::Macro(m) => {
                self.attrs(&m.attrs);
                self.macro_stmt(&m.mac);
            }
        }
    }

    fn some_inner(p: &Pat) -> Option<&Pat> {
        if let Pat::TupleStruct(ts) = p
            && ts.path.is_ident("Some")
            && ts.elems.len() == 1
        {
            return ts.elems.first();
        }
        None
    }

    /// `(a, b)` / `[a, b]` made only of identifiers/wildcards -> their list.
    fn flat_idents(p: &Pat) -> Option<Vec<&Pat>> {
        let elems: Vec<&Pat> = match Self::strip_pat(p) {
            Pat::Tuple(t) => t.elems.iter().collect(),
            Pat::Slice(s) => s.elems.iter().collect(),
            _ => return None,
        };
        elems
            .iter()
            .all(|e| matches!(Self::strip_pat(e), Pat::Ident(PatIdent { subpat: None, .. }) | Pat::Wild(_)))
            .then(|| elems.into_iter().map(Self::strip_pat).collect())
    }

    fn strip_pat(p: &Pat) -> &Pat {
        match p {
            Pat::Type(t) => Self::strip_pat(&t.pat),
            Pat::Reference(r) => Self::strip_pat(&r.pat),
            Pat::Paren(p) => Self::strip_pat(&p.pat),
            other => other,
        }
    }

    fn local(&mut self, l: &Local) {
        self.attrs(&l.attrs);
        let pat = Self::strip_pat(&l.pat);
        let ty_comment = match &l.pat {
            Pat::Type(t) => Some(t.ty.clone()),
            _ => None,
        };
        match &l.init {
            None => {
                let names = Self::flat_idents(pat);
                match names {
                    Some(list) if !list.is_empty() => {
                        for (i, p) in list.iter().enumerate() {
                            if i > 0 {
                                self.out.push(" = ");
                            }
                            self.pat(p);
                        }
                    }
                    _ => self.pat(pat),
                }
                self.out.push(" = nil");
            }
            Some(init) => {
                if let Some((_, else_expr)) = &init.diverge {
                    // let PAT = e else { ... };
                    if let Some(inner @ Pat::Ident(_)) = Self::some_inner(pat).map(Self::strip_pat) {
                        self.pat(inner);
                        self.out.push(" = ");
                        self.expr(&init.expr);
                        self.out.nl();
                        self.out.push("if ");
                        self.pat(inner);
                        self.out.push(".nil?  # let-else");
                    } else {
                        self.out.push("unless ");
                        self.expr(&init.expr);
                        self.out.push(" in ");
                        self.pat(pat);
                        self.out.push("  # let-else");
                    }
                    self.out.nl();
                    self.out.indent += 1;
                    match &**else_expr {
                        Expr::Block(b) => self.block_stmts(&b.block),
                        e => self.expr(e),
                    }
                    self.out.indent -= 1;
                    self.out.nl();
                    self.out.push("end");
                    return;
                }
                match pat {
                    Pat::Ident(_) | Pat::Wild(_) => {
                        self.pat(pat);
                        self.out.push(" = ");
                        self.expr(&init.expr);
                    }
                    _ if Self::flat_idents(pat).is_some_and(|l| l.is_empty()) => {
                        // let () = expr;
                        self.expr(&init.expr);
                    }
                    _ if Self::flat_idents(pat).is_some() => {
                        for (i, e) in Self::flat_idents(pat).unwrap().iter().enumerate() {
                            if i > 0 {
                                self.out.push(", ");
                            }
                            self.pat(e);
                        }
                        self.out.push(" = ");
                        self.expr(&init.expr);
                    }
                    _ => {
                        // rightward pattern assignment
                        self.expr(&init.expr);
                        self.out.push(" => ");
                        self.pat(pat);
                    }
                }
            }
        }
        if let Some(ty) = ty_comment {
            self.out.push("  # ");
            self.src_tokens(&ty);
        }
    }

    // ---------------------------------------------------------------- patterns

    fn pat(&mut self, p: &Pat) {
        match p {
            Pat::Ident(pi) if pi.ident == "None" && pi.subpat.is_none() => {
                self.span_tok(keyword_word("None"), pi.ident.span())
            }
            Pat::Ident(pi) => {
                if let Some((_, sub)) = &pi.subpat {
                    self.pat(sub);
                    self.out.push(" => ");
                }
                self.ident(&pi.ident);
            }
            Pat::Wild(_) => self.out.push("_"),
            Pat::Rest(_) => self.out.push("*"),
            Pat::Lit(ExprLit { lit: Lit::ByteStr(b), .. }) => {
                let t = format!("\"{}\"", escape_rb(&String::from_utf8_lossy(&b.value())));
                self.span_tok(&t, b.span());
            }
            Pat::Lit(ExprLit { lit: Lit::Byte(b), .. }) => self.span_tok(&b.value().to_string(), b.span()),
            Pat::Lit(l) => self.lit(&l.lit),
            Pat::Paren(pp) => self.pat(&pp.pat),
            Pat::Reference(r) => self.pat(&r.pat),
            Pat::Type(t) => self.pat(&t.pat),
            Pat::Or(o) => {
                for (i, c) in o.cases.iter().enumerate() {
                    if i > 0 {
                        self.out.push(" | ");
                    }
                    self.pat(c);
                }
            }
            Pat::Tuple(t) => {
                self.out.push("[");
                self.pat_list(&t.elems);
                self.out.push("]");
            }
            Pat::Slice(s) => {
                self.out.push("[");
                for (i, e) in s.elems.iter().enumerate() {
                    if i > 0 {
                        self.out.push(", ");
                    }
                    match e {
                        Pat::Ident(PatIdent { ident, subpat: Some((_, sub)), .. }) if matches!(**sub, Pat::Rest(_)) => {
                            self.out.push("*");
                            self.ident(ident);
                        }
                        e => self.pat(e),
                    }
                }
                self.out.push("]");
            }
            Pat::Path(pp) => {
                if pp.path.is_ident("None") {
                    self.span_tok(keyword_word("None"), pp.path.span());
                } else {
                    self.path_const(&pp.path);
                }
            }
            Pat::TupleStruct(ts) => {
                if let Some(inner) = Self::some_inner(p) {
                    return self.pat(inner);
                }
                self.path_const(&ts.path);
                self.out.push("[");
                self.pat_list(&ts.elems);
                self.out.push("]");
            }
            Pat::Struct(ps) => {
                self.path_const(&ps.path);
                self.out.push("(");
                for (i, f) in ps.fields.iter().enumerate() {
                    if i > 0 {
                        self.out.push(", ");
                    }
                    match &f.member {
                        Member::Named(id) => {
                            if f.colon_token.is_some() {
                                self.span_tok(&id.to_string(), id.span());
                                self.out.push(": ");
                                self.pat(&f.pat);
                            } else {
                                // shorthand `{ a }` binds `a`; a Ruby keyword can't be a local,
                                // so `{ end }` binds explicitly: `end: end_`
                                let raw = id.to_string();
                                let raw = raw.trim_start_matches("r#");
                                if rb_ident(raw) == raw {
                                    self.ident(id);
                                    self.out.push(":");
                                } else {
                                    self.field_name(id);
                                    self.out.push(": ");
                                    self.ident(id);
                                }
                            }
                        }
                        Member::Unnamed(idx) => {
                            self.span_tok(&format!("_{}", idx.index), idx.span);
                            self.out.push(": ");
                            self.pat(&f.pat);
                        }
                    }
                }
                if ps.rest.is_some() {
                    if !ps.fields.is_empty() {
                        self.out.push(", ");
                    }
                    self.out.push("**");
                }
                self.out.push(")");
            }
            Pat::Range(r) => {
                let num = |e: &Option<Box<Expr>>| match e.as_deref() {
                    Some(Expr::Lit(ExprLit { lit: Lit::Byte(b), .. })) => Some(b.value().to_string()),
                    _ => None,
                };
                if num(&r.start).is_some() || num(&r.end).is_some() {
                    self.out.push("(");
                    if let Some(x) = &r.start {
                        match num(&r.start) {
                            Some(n) => self.node_tok(&n, x),
                            None => self.expr(x),
                        }
                    }
                    self.out.push(match r.limits {
                        RangeLimits::HalfOpen(_) => "...",
                        RangeLimits::Closed(_) => "..",
                    });
                    if let Some(x) = &r.end {
                        match num(&r.end) {
                            Some(n) => self.node_tok(&n, x),
                            None => self.expr(x),
                        }
                    }
                    self.out.push(")");
                } else {
                    self.range(r)
                }
            }
            other => self.raw(other),
        }
    }

    fn pat_list(&mut self, elems: &Punctuated<Pat, Token![,]>) {
        for (i, e) in elems.iter().enumerate() {
            if i > 0 {
                self.out.push(", ");
            }
            self.pat(e);
        }
    }

    /// Pattern for a block param list: `|a, (b, c)|`
    fn closure_params(&mut self, inputs: &Punctuated<Pat, Token![,]>) {
        if inputs.is_empty() {
            return;
        }
        self.out.push("|");
        for (i, p) in inputs.iter().enumerate() {
            if i > 0 {
                self.out.push(", ");
            }
            self.closure_param(p);
        }
        self.out.push("| ");
    }

    fn closure_param(&mut self, p: &Pat) {
        match Self::strip_pat(p) {
            Pat::Tuple(t) if t.elems.is_empty() => self.out.push("_"),
            Pat::Tuple(t) => {
                self.out.push("(");
                for (i, e) in t.elems.iter().enumerate() {
                    if i > 0 {
                        self.out.push(", ");
                    }
                    self.closure_param(e);
                }
                self.out.push(")");
            }
            Pat::Ident(pi) => self.ident(&pi.ident),
            Pat::Wild(_) => self.out.push("_"),
            other => {
                self.node_tok("arg", other);
            }
        }
    }

    // ---------------------------------------------------------------- expressions

    fn lit(&mut self, l: &Lit) {
        match l {
            Lit::Str(s) => {
                let t = format!("\"{}\"", escape_rb(&s.value()));
                self.span_tok(&t, s.span());
            }
            Lit::ByteStr(s) => {
                let t = format!("\"{}\".b", escape_rb(&String::from_utf8_lossy(&s.value())));
                self.span_tok(&t, s.span());
            }
            Lit::Char(c) => {
                let t = format!("\"{}\"", escape_rb(&c.value().to_string()));
                self.span_tok(&t, c.span());
            }
            Lit::Byte(b) => {
                let v = b.value();
                let t = if v.is_ascii_graphic() || v == b' ' {
                    format!("\"{}\".ord", escape_rb(&(v as char).to_string()))
                } else {
                    format!("0x{v:02x}")
                };
                self.span_tok(&t, b.span())
            }
            Lit::Int(i) => {
                let repr = i.to_string();
                let t = repr.strip_suffix(i.suffix()).unwrap_or(&repr).trim_end_matches('_').to_string();
                self.span_tok(&t, i.span());
            }
            Lit::Float(f) => {
                let repr = f.to_string();
                let mut t = repr.strip_suffix(f.suffix()).unwrap_or(&repr).trim_end_matches('_').to_string();
                if t.ends_with('.') {
                    t.push('0');
                }
                if !t.contains('.') && !t.contains('e') && !t.contains('E') {
                    t.push_str(".0");
                }
                self.span_tok(&t, f.span());
            }
            Lit::Bool(b) => self.span_tok(if b.value { "true" } else { "false" }, b.span),
            other => self.raw(other),
        }
    }

    fn range(&mut self, r: &ExprRange) {
        if r.start.is_none() && r.end.is_none() {
            return self.out.push("(0..)");
        }
        self.out.push("(");
        if let Some(s) = &r.start {
            self.expr(s);
        }
        self.out.push(match r.limits {
            RangeLimits::HalfOpen(_) => "...",
            RangeLimits::Closed(_) => "..",
        });
        if let Some(e) = &r.end {
            self.expr(e);
        }
        self.out.push(")");
    }

    /// A path in expression position.
    fn path_expr(&mut self, p: &ExprPath) {
        let path = &p.path;
        let segs: Vec<&PathSegment> = path.segments.iter().collect();
        if segs.len() == 1 {
            let s = segs[0].ident.to_string();
            match s.as_str() {
                "None" => return self.span_tok(keyword_word("None"), segs[0].ident.span()),
                "self" => return self.span_tok("self", segs[0].ident.span()),
                "Self" => {
                    let n = self.self_ty.clone().unwrap_or_else(|| "self.class".into());
                    return self.span_tok(&n, segs[0].ident.span());
                }
                _ => {}
            }
            return self.ident(&segs[0].ident);
        }
        // a::b::func -> A::B.func ; A::B::CONST / Type::Variant -> A::B::X
        let last = segs.last().unwrap();
        let last_s = last.ident.to_string();
        let mut prefix = path.clone();
        prefix.segments.pop();
        // drop trailing punct
        let prefix_path =
            Path { leading_colon: None, segments: prefix.segments.into_pairs().map(|p| p.into_value()).collect() };
        self.path_const(&prefix_path);
        if is_capitalized(&last_s) || is_const_case(&last_s) {
            self.out.push("::");
            self.span_tok(&last_s, last.ident.span());
        } else {
            self.out.push(".");
            let n = last_s.strip_prefix("r#").unwrap_or(&last_s).to_string();
            self.span_tok(&n, last.ident.span());
        }
    }

    fn args(&mut self, args: &Punctuated<Expr, Token![,]>, block_last: bool) {
        let n = args.len();
        let (plain, block): (Vec<&Expr>, Option<&ExprClosure>) = match args.last() {
            Some(Expr::Closure(c)) if block_last => (args.iter().take(n - 1).collect(), Some(c)),
            _ => (args.iter().collect(), None),
        };
        if !plain.is_empty() {
            self.out.push("(");
            for (i, a) in plain.iter().enumerate() {
                if i > 0 {
                    self.out.push(", ");
                }
                self.expr(a);
            }
            self.out.push(")");
        }
        if let Some(c) = block {
            self.out.push(" ");
            self.closure_block(c);
        }
    }

    fn closure_block(&mut self, c: &ExprClosure) {
        match &*c.body {
            Expr::Block(b) if b.block.stmts.len() > 1 || b.block.stmts.iter().any(|s| matches!(s, Stmt::Local(_))) => {
                self.out.push("do ");
                self.closure_params(&c.inputs);
                self.out.nl();
                self.body(&b.block);
                self.out.push("end");
            }
            Expr::Block(b) if b.block.stmts.len() == 1 => {
                self.out.push("{ ");
                self.closure_params(&c.inputs);
                self.stmt(&b.block.stmts[0]);
                self.out.push(" }");
            }
            body => {
                self.out.push("{ ");
                self.closure_params(&c.inputs);
                self.expr(body);
                self.out.push(" }");
            }
        }
    }

    fn lambda(&mut self, c: &ExprClosure) {
        self.out.push("->");
        if !c.inputs.is_empty() {
            self.out.push("(");
            for (i, p) in c.inputs.iter().enumerate() {
                if i > 0 {
                    self.out.push(", ");
                }
                self.closure_param(p);
            }
            self.out.push(")");
        }
        self.out.push(" ");
        match &*c.body {
            Expr::Block(b) if b.block.stmts.len() > 1 => {
                self.out.push("do");
                self.out.nl();
                self.body(&b.block);
                self.out.push("end");
            }
            body => {
                self.out.push("{ ");
                match body {
                    Expr::Block(b) if b.block.stmts.len() == 1 => self.stmt(&b.block.stmts[0]),
                    e => self.expr(e),
                }
                self.out.push(" }");
            }
        }
    }

    fn call(&mut self, c: &ExprCall) {
        if matches!(&*c.func, Expr::Path(p) if p.path.is_ident("self")) {
            self.expr(&c.func);
            self.out.push(".(");
            for (i, a) in c.args.iter().enumerate() {
                if i > 0 {
                    self.out.push(", ");
                }
                self.expr(a);
            }
            self.out.push(")");
            return;
        }
        if let Expr::Path(p) = &*c.func {
            let segs: Vec<String> = p.path.segments.iter().map(|s| s.ident.to_string()).collect();
            let joined = segs.join("::");
            let last = segs.last().cloned().unwrap_or_default();
            // unwrapping constructors / common std helpers
            match joined.as_str() {
                "Some" | "Box::new" | "Rc::new" | "Arc::new" | "String::from" | "PathBuf::from" | "Cow::Borrowed"
                | "Cow::Owned"
                    if c.args.len() == 1 =>
                {
                    return self.expr(&c.args[0]);
                }
                "String::new" | "String::with_capacity" => return self.node_tok("\"\"", &c.func),
                "Vec::new" | "Vec::with_capacity" | "VecDeque::new" => return self.node_tok("[]", &c.func),
                "HashMap::new" | "BTreeMap::new" | "HashMap::with_capacity" | "IndexMap::new" => {
                    return self.node_tok("{}", &c.func);
                }
                "HashSet::new" | "BTreeSet::new" => return self.node_tok("Set.new", &c.func),
                "Default::default" => return self.node_tok("default", &c.func),
                _ => {}
            }
            if is_capitalized(&last) && segs.len() <= 3 {
                // tuple struct / tuple variant constructor
                if segs.len() == 1 {
                    self.ident(&p.path.segments[0].ident);
                } else {
                    self.path_const(&p.path);
                }
                self.out.push("[");
                for (i, a) in c.args.iter().enumerate() {
                    if i > 0 {
                        self.out.push(", ");
                    }
                    self.expr(a);
                }
                self.out.push("]");
                return;
            }
            self.path_expr(p);
            if c.args.is_empty() {
                self.out.push("()");
            }
            self.args(&c.args, true);
            return;
        }
        // calling a closure / fn value
        self.expr(&c.func);
        self.out.push(".(");
        for (i, a) in c.args.iter().enumerate() {
            if i > 0 {
                self.out.push(", ");
            }
            self.expr(a);
        }
        self.out.push(")");
    }

    fn method_call(&mut self, m: &ExprMethodCall) {
        let name = m.method.to_string();
        let argc = m.args.len();
        if argc == 0 && DROPPED_METHODS.contains(&name.as_str()) {
            return self.expr(&m.receiver); // whole expression
        }
        match (name.as_str(), argc) {
            ("is_some", 0) => {
                self.out.push("!");
                self.expr_recv(&m.receiver);
                self.span_tok(".nil?", m.method.span());
                return;
            }
            ("is_none", 0) => {
                self.expr_recv(&m.receiver);
                self.span_tok(".nil?", m.method.span());
                return;
            }
            ("unwrap_or", 1) => {
                self.out.push("(");
                self.expr(&m.receiver);
                self.span_tok(" || ", m.method.span());
                self.expr(&m.args[0]);
                self.out.push(")");
                return;
            }
            ("cmp", 1) | ("partial_cmp", 1) | ("total_cmp", 1) => {
                self.out.push("(");
                self.expr_recv(&m.receiver);
                self.span_tok(" <=> ", m.method.span());
                self.expr(&m.args[0]);
                self.out.push(")");
                return;
            }
            ("get", 1) => {
                self.expr_recv(&m.receiver);
                self.span_tok("[", m.method.span());
                self.expr(&m.args[0]);
                self.out.push("]");
                return;
            }
            ("nth", 1) => {
                self.expr_recv(&m.receiver);
                self.span_tok("[", m.method.span());
                self.expr(&m.args[0]);
                self.out.push("]");
                return;
            }
            ("collect", 0) => {
                let target = m.turbofish.as_ref().map(|t| t.to_token_stream().to_string()).unwrap_or_default();
                self.expr_recv(&m.receiver);
                // `.collect::<HashMap<..>>()` -> `.to_h`, mapped over the method *and* its turbofish
                let start = m.method.span().start();
                let end = m
                    .turbofish
                    .as_ref()
                    .and_then(tok_range)
                    .filter(|(a, _)| a.line == start.line)
                    .map(|(_, b)| b)
                    .unwrap_or(m.method.span().end());
                let conv = if target.contains("Map") {
                    ".to_h"
                } else if target.contains("Set") {
                    ".to_set"
                } else if target.contains("String") {
                    ".join"
                } else {
                    ""
                };
                if !conv.is_empty() {
                    self.out.tok(conv, start, end);
                }
                return;
            }
            ("to_string", 0) | ("to_owned_string", 0) => {
                self.expr_recv(&m.receiver);
                self.span_tok(".to_s", m.method.span());
                return;
            }
            _ => {}
        }
        let rb = METHOD_RENAMES.iter().find(|(r, _)| *r == name).map(|(_, rb)| *rb).unwrap_or("");
        let rb = if rb.is_empty() { name.strip_prefix("r#").unwrap_or(&name).to_string() } else { rb.to_string() };
        self.expr_recv(&m.receiver);
        self.out.push(".");
        self.span_tok(&rb, m.method.span());
        self.args(&m.args, true);
    }

    fn expr(&mut self, e: &Expr) {
        match e {
            Expr::Lit(l) => self.lit(&l.lit),
            Expr::Path(p) => self.path_expr(p),
            Expr::Paren(p) => {
                self.out.push("(");
                self.expr(&p.expr);
                self.out.push(")");
            }
            Expr::Group(g) => self.expr(&g.expr),
            Expr::Reference(r) => self.expr(&r.expr),
            Expr::Unary(u) => match u.op {
                UnOp::Deref(_) => self.expr(&u.expr),
                UnOp::Not(_) => {
                    self.out.push("!");
                    self.expr(&u.expr);
                }
                UnOp::Neg(_) => {
                    self.out.push("-");
                    self.expr(&u.expr);
                }
                _ => self.expr(&u.expr),
            },
            Expr::Binary(b) => {
                let op = b.op.to_token_stream().to_string().replace(' ', "");
                let compound = op.len() >= 2 && op.ends_with('=') && !["==", "!=", "<=", ">="].contains(&op.as_str());
                if compound {
                    let target = Self::strip_deref(&b.left);
                    if let Some((map, key, default, entry_id, or_id)) = Self::entry_or_insert(target) {
                        // *map.entry(k).or_insert(v) += x   =>   map[k] = (map[k] || v) + x
                        self.expr_recv(map);
                        self.span_tok("[", entry_id.span());
                        self.expr(key);
                        self.out.push("] = (");
                        self.expr_recv(map);
                        self.out.push("[");
                        self.expr(key);
                        self.out.push("] ");
                        self.span_tok("||", or_id.span());
                        self.out.push(" ");
                        match default {
                            Some(d) => self.expr(d),
                            None => self.out.push("default"),
                        }
                        self.out.push(") ");
                        self.node_tok(op.trim_end_matches('='), &b.op);
                        self.out.push(" ");
                        self.expr(&b.right);
                        return;
                    }
                    let is_self = matches!(target, Expr::Path(p) if p.path.is_ident("self"));
                    if is_self || !matches!(target, Expr::Path(_) | Expr::Field(_) | Expr::Index(_)) {
                        // `*cell.borrow_mut() += 1` — not assignable in Ruby; show as a mutating call
                        let name = match op.as_str() {
                            "+=" => "add!",
                            "-=" => "sub!",
                            "*=" => "mul!",
                            "/=" => "div!",
                            "%=" => "rem!",
                            "|=" => "bitor!",
                            "&=" => "bitand!",
                            "^=" => "bitxor!",
                            "<<=" => "shl!",
                            ">>=" => "shr!",
                            _ => "assign!",
                        };
                        self.expr_recv(target);
                        self.out.push(".");
                        self.node_tok(name, &b.op);
                        self.out.push("(");
                        self.expr(&b.right);
                        self.out.push(")");
                        return;
                    }
                }
                self.expr(&b.left);
                self.out.push(" ");
                self.node_tok(&op, &b.op);
                self.out.push(" ");
                self.expr(&b.right);
            }
            Expr::Assign(a) => {
                let target = Self::strip_deref(&a.left);
                let is_self = matches!(target, Expr::Path(p) if p.path.is_ident("self"));
                if is_self
                    || !matches!(
                        target,
                        Expr::Path(_) | Expr::Field(_) | Expr::Index(_) | Expr::Tuple(_) | Expr::Infer(_)
                    )
                {
                    // `*self = x` / `*ptr() = x`: show as an in-place replace
                    self.expr_recv(target);
                    self.out.push(".");
                    self.node_tok("replace", &a.eq_token);
                    self.out.push("(");
                    self.expr(&a.right);
                    self.out.push(")");
                } else {
                    self.expr(&a.left);
                    self.out.push(" = ");
                    self.expr(&a.right);
                }
            }
            Expr::Cast(c) => {
                let ty = c.ty.to_token_stream().to_string();
                self.expr_recv(&c.expr);
                let conv = match ty.as_str() {
                    "f32" | "f64" => ".to_f",
                    "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128"
                    | "usize" => ".to_i",
                    "char" => ".chr",
                    _ => "",
                };
                if !conv.is_empty() {
                    self.node_tok(conv, &c.ty);
                }
            }
            Expr::Field(f) => {
                let is_self = matches!(&*f.base, Expr::Path(p) if p.path.is_ident("self"));
                match &f.member {
                    Member::Named(id) => {
                        if is_self {
                            let n = format!("@{}", id.to_string().trim_start_matches("r#"));
                            self.span_tok(&n, id.span());
                        } else {
                            self.expr_recv(&f.base);
                            self.out.push(".");
                            self.span_tok(id.to_string().trim_start_matches("r#"), id.span());
                        }
                    }
                    Member::Unnamed(idx) => {
                        self.expr_recv(&f.base);
                        self.span_tok(&format!("[{}]", idx.index), idx.span);
                    }
                }
            }
            Expr::Index(i) if matches!(&*i.index, Expr::Range(r) if r.start.is_none() && r.end.is_none()) => {
                self.expr(&i.expr);
            }
            Expr::Index(i) => {
                self.expr_recv(&i.expr);
                self.out.push("[");
                self.expr(&i.index);
                self.out.push("]");
            }
            Expr::Call(c) => self.call(c),
            Expr::MethodCall(m) => self.method_call(m),
            Expr::Closure(c) => self.lambda(c),
            Expr::Try(t) => {
                self.expr_recv(&t.expr);
                self.span_tok(".try!", t.question_token.span);
            }
            Expr::Await(a) => {
                self.expr_recv(&a.base);
                self.out.push(".await");
            }
            Expr::Tuple(t) => {
                if t.elems.is_empty() {
                    self.node_tok("nil", t);
                } else {
                    self.out.push("[");
                    for (i, x) in t.elems.iter().enumerate() {
                        if i > 0 {
                            self.out.push(", ");
                        }
                        self.expr(x);
                    }
                    self.out.push("]");
                }
            }
            Expr::Array(a) => {
                self.out.push("[");
                for (i, x) in a.elems.iter().enumerate() {
                    if i > 0 {
                        self.out.push(", ");
                    }
                    self.expr(x);
                }
                self.out.push("]");
            }
            Expr::Repeat(r) => {
                self.out.push("Array.new(");
                self.expr(&r.len);
                self.out.push(", ");
                self.expr(&r.expr);
                self.out.push(")");
            }
            Expr::Range(r) => self.range(r),
            Expr::Struct(s) => {
                self.path_const(&s.path);
                self.out.push("[");
                for (i, f) in s.fields.iter().enumerate() {
                    if i > 0 {
                        self.out.push(", ");
                    }
                    match &f.member {
                        Member::Named(id) => self.span_tok(id.to_string().trim_start_matches("r#"), id.span()),
                        Member::Unnamed(idx) => self.span_tok(&format!("_{}", idx.index), idx.span),
                    }
                    self.out.push(": ");
                    self.expr(&f.expr);
                }
                if let Some(rest) = &s.rest {
                    if !s.fields.is_empty() {
                        self.out.push(", ");
                    }
                    self.out.push("**");
                    self.expr(rest);
                    self.out.push(".to_h");
                }
                self.out.push("]");
            }
            Expr::Return(r) => {
                self.span_tok("return", r.return_token.span);
                if let Some(x) = &r.expr {
                    self.out.push(" ");
                    self.expr(x);
                }
            }
            Expr::Break(b) => {
                self.span_tok("break", b.break_token.span);
                if let Some(x) = &b.expr {
                    self.out.push(" ");
                    self.expr(x);
                }
                if let Some(l) = &b.label {
                    self.out.push(&format!("  # break {}", l.to_token_stream()));
                }
            }
            Expr::Continue(c) => {
                self.span_tok(keyword_word("continue"), c.continue_token.span);
                if let Some(l) = &c.label {
                    self.out.push(&format!("  # continue {}", l.to_token_stream()));
                }
            }
            Expr::Let(l) => self.let_cond(l),
            Expr::If(i) => self.if_expr(i, false),
            Expr::Match(m) => self.match_expr(m),
            Expr::Block(b) => {
                self.out.push("begin");
                if let Some(l) = &b.label {
                    self.out.push(&format!("  # {}", l.name.to_token_stream()));
                }
                self.out.nl();
                self.body(&b.block);
                self.out.push("end");
            }
            Expr::Unsafe(u) => {
                self.out.push("begin  # unsafe");
                self.out.nl();
                self.body(&u.block);
                self.out.push("end");
            }
            Expr::Async(a) => {
                self.out.push("async do");
                self.out.nl();
                self.body(&a.block);
                self.out.push("end");
            }
            Expr::Loop(l) => {
                self.out.push("loop do");
                if let Some(lb) = &l.label {
                    self.out.push(&format!("  # {}", lb.name.to_token_stream()));
                }
                self.out.nl();
                self.body(&l.body);
                self.out.push("end");
            }
            Expr::While(w) => {
                self.out.push("while ");
                self.expr(&w.cond);
                if let Some(lb) = &w.label {
                    self.out.push(&format!("  # {}", lb.name.to_token_stream()));
                }
                self.out.nl();
                self.body(&w.body);
                self.out.push("end");
            }
            Expr::ForLoop(f) => {
                self.expr(&f.expr);
                self.out.push(".each do ");
                // for (i, x) in ... => |i, x|
                match Self::strip_pat(&f.pat) {
                    Pat::Tuple(t) => {
                        self.out.push("|");
                        for (i, e) in t.elems.iter().enumerate() {
                            if i > 0 {
                                self.out.push(", ");
                            }
                            self.closure_param(e);
                        }
                        self.out.push("|");
                    }
                    p => {
                        self.out.push("|");
                        self.closure_param(p);
                        self.out.push("|");
                    }
                }
                if let Some(lb) = &f.label {
                    self.out.push(&format!("  # {}", lb.name.to_token_stream()));
                }
                self.out.nl();
                self.body(&f.body);
                self.out.push("end");
            }
            Expr::Macro(m) => self.macro_call(&m.mac),
            Expr::Infer(_) => self.out.push("_"),
            Expr::Yield(y) => {
                self.out.push("yield");
                if let Some(x) = &y.expr {
                    self.out.push(" ");
                    self.expr(x);
                }
            }
            other => self.raw(other),
        }
    }

    fn strip_deref(e: &Expr) -> &Expr {
        match e {
            Expr::Unary(ExprUnary { op: UnOp::Deref(_), expr, .. }) => Self::strip_deref(expr),
            Expr::Reference(r) => Self::strip_deref(&r.expr),
            Expr::Paren(p) => Self::strip_deref(&p.expr),
            other => other,
        }
    }

    /// `map.entry(k).or_insert(v)` / `.or_default()` / `.or_insert_with(f)` -> (map, k, default, entry, or_insert)
    fn entry_or_insert(e: &Expr) -> Option<(&Expr, &Expr, Option<&Expr>, &Ident, &Ident)> {
        let Expr::MethodCall(outer) = e else { return None };
        let m = outer.method.to_string();
        if !matches!(m.as_str(), "or_insert" | "or_default" | "or_insert_with") {
            return None;
        }
        let Expr::MethodCall(entry) = &*outer.receiver else { return None };
        if entry.method != "entry" || entry.args.len() != 1 {
            return None;
        }
        Some((
            &entry.receiver,
            &entry.args[0],
            if m == "or_insert" { outer.args.first() } else { None },
            &entry.method,
            &outer.method,
        ))
    }

    /// Would this expression's Ruby rendering need parentheses when used as a receiver?
    fn needs_parens(e: &Expr) -> bool {
        match e {
            Expr::Binary(_)
            | Expr::Assign(_)
            | Expr::Range(_)
            | Expr::Closure(_)
            | Expr::Let(_)
            | Expr::Unary(ExprUnary { op: UnOp::Not(_) | UnOp::Neg(_), .. }) => true,
            Expr::Unary(u) => Self::needs_parens(&u.expr),
            Expr::Reference(r) => Self::needs_parens(&r.expr),
            Expr::Group(g) => Self::needs_parens(&g.expr),
            Expr::Macro(m) => {
                let n = m.mac.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
                matches!(
                    n.as_str(),
                    "write"
                        | "writeln"
                        | "matches"
                        | "assert"
                        | "assert_eq"
                        | "assert_ne"
                        | "println"
                        | "print"
                        | "eprintln"
                        | "eprint"
                        | "panic"
                )
            }
            Expr::MethodCall(m) => m.args.is_empty() && m.method == "is_some",
            Expr::If(_) | Expr::Match(_) | Expr::Loop(_) | Expr::While(_) | Expr::ForLoop(_) => true,
            _ => false,
        }
    }

    /// Emit an expression that is about to receive `.method` / `[i]` / `.try!`.
    fn expr_recv(&mut self, e: &Expr) {
        if Self::needs_parens(e) {
            self.out.push("(");
            self.expr(e);
            self.out.push(")");
        } else {
            self.expr(e);
        }
    }

    fn let_cond(&mut self, l: &ExprLet) {
        let pat = Self::strip_pat(&l.pat);
        if let Some(inner @ Pat::Ident(_)) = Self::some_inner(pat).map(Self::strip_pat) {
            self.out.push("(");
            self.pat(inner);
            self.out.push(" = ");
            self.expr(&l.expr);
            self.out.push(")");
        } else {
            self.expr(&l.expr);
            self.out.push(" in ");
            self.pat(pat);
        }
    }

    fn if_expr(&mut self, i: &ExprIf, is_elsif: bool) {
        self.out.push(if is_elsif { "elsif " } else { "if " });
        self.expr(&i.cond);
        self.out.nl();
        self.body(&i.then_branch);
        match &i.else_branch {
            None => self.out.push("end"),
            Some((_, e)) => match &**e {
                Expr::If(ei) => self.if_expr(ei, true),
                Expr::Block(b) => {
                    self.out.push("else");
                    self.out.nl();
                    self.body(&b.block);
                    self.out.push("end");
                }
                other => {
                    self.out.push("else");
                    self.out.nl();
                    self.out.indent += 1;
                    self.expr(other);
                    self.out.indent -= 1;
                    self.out.nl();
                    self.out.push("end");
                }
            },
        }
    }

    fn match_expr(&mut self, m: &ExprMatch) {
        if m.arms.is_empty() {
            self.expr(&m.expr);
            self.out.push(" => _never  # empty match: the type has no values");
            return;
        }
        self.out.push("case ");
        self.expr(&m.expr);
        self.out.nl();
        for arm in &m.arms {
            let rb_start = self.out.line;
            self.out.push("in ");
            let pat = Self::strip_pat(&arm.pat);
            let some_ident = Self::some_inner(pat).map(Self::strip_pat).and_then(|p| match p {
                Pat::Ident(pi) => Some(pi.ident.clone()),
                _ => None,
            });
            self.pat(pat);
            match (&some_ident, &arm.guard) {
                (Some(id), None) => {
                    self.out.push(" unless ");
                    self.out.push(&rb_ident(&id.to_string()));
                    self.out.push(".nil?");
                }
                (Some(id), Some((_, g))) => {
                    self.out.push(" if !");
                    self.out.push(&rb_ident(&id.to_string()));
                    self.out.push(".nil? && ");
                    self.expr(g);
                }
                (None, Some((_, g))) => {
                    self.out.push(" if ");
                    self.expr(g);
                }
                (None, None) => {}
            }
            if arm.pat.to_token_stream().into_iter().any(|t| t.to_string().starts_with("b'")) {
                let src = self.src_of(&arm.pat);
                self.out.push("  # ");
                self.node_tok(&src, &arm.pat);
            }
            self.out.nl();
            self.out.indent += 1;
            match &*arm.body {
                Expr::Block(b) if b.label.is_none() => {
                    if b.block.stmts.is_empty() {
                        self.out.push("nil");
                    } else {
                        self.block_stmts(&b.block);
                    }
                }
                e => self.expr(e),
            }
            self.out.indent -= 1;
            self.out.nl();
            self.out.block(rb_start, tok_range(arm));
        }
        self.out.push("end");
    }

    // ---------------------------------------------------------------- macros

    fn macro_call(&mut self, mac: &Macro) {
        let name = mac.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
        let span = mac.path.span();
        let parse_args = || mac.parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated).ok();
        match name.as_str() {
            "println" | "print" | "eprintln" | "eprint" | "format" | "panic" | "write" | "writeln" | "format_args"
            | "unreachable" | "todo" | "unimplemented" | "bail" | "anyhow" => {
                let Some(args) = parse_args() else { return self.raw_macro(mac) };
                let mut it = args.iter();
                let mut target = None;
                if name == "write" || name == "writeln" {
                    target = it.next();
                }
                let rest: Vec<&Expr> = it.collect();
                let word = macro_word(&name);
                let kw = match name.as_str() {
                    "format" | "format_args" | "write" | "writeln" => String::new(),
                    "anyhow" => format!("{word}.new("),
                    "todo" | "unimplemented" => format!("{word} NotImplementedError"),
                    _ => format!("{word} "),
                };
                let kw = kw.as_str();
                if let Some(t) = target {
                    self.expr(t);
                    self.span_tok(" << ", span);
                } else if !kw.is_empty() {
                    self.span_tok(kw.trim_end_matches(' '), span);
                    if kw.ends_with(' ') && !rest.is_empty() {
                        self.out.push(" ");
                    }
                    if (name == "todo" || name == "unimplemented") && !rest.is_empty() {
                        self.out.push(", ");
                    }
                }
                if rest.is_empty() {
                    if target.is_some() {
                        self.out.push(if name == "writeln" { "\"\\n\"" } else { "\"\"" });
                    } else if name == "println" {
                        self.out.push("\"\"");
                    } else if name == "unreachable" {
                        self.out.push(" \"unreachable\"");
                    }
                } else {
                    let open = matches!(name.as_str(), "format" | "format_args").then(|| span);
                    self.format_string(&rest, name == "writeln", open);
                }
                if name == "anyhow" {
                    self.out.push(")");
                }
            }
            "vec" => {
                // vec![x; n] or vec![a, b]
                if let Ok((x, n)) = mac.parse_body_with(|input: parse::ParseStream| {
                    let x: Expr = input.parse()?;
                    input.parse::<Token![;]>()?;
                    let n: Expr = input.parse()?;
                    Ok((x, n))
                }) {
                    self.span_tok("Array.new", span);
                    self.out.push("(");
                    self.expr(&n);
                    self.out.push(", ");
                    self.expr(&x);
                    self.out.push(")");
                } else if let Some(args) = parse_args() {
                    self.out.push("[");
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            self.out.push(", ");
                        }
                        self.expr(a);
                    }
                    self.out.push("]");
                } else {
                    self.raw_macro(mac)
                }
            }
            "assert" | "debug_assert" | "assert_eq" | "debug_assert_eq" | "assert_ne" | "debug_assert_ne" => {
                let Some(args) = parse_args() else { return self.raw_macro(mac) };
                let a: Vec<&Expr> = args.iter().collect();
                self.span_tok(macro_word(&name), span);
                self.out.push(" ");
                if a.is_empty() {
                    return;
                }
                self.expr(a[0]);
                let mut extra = 1;
                if name.ends_with("_eq") || name.ends_with("_ne") {
                    if let Some(b) = a.get(1) {
                        self.out.push(if name.ends_with("_eq") { " == " } else { " != " });
                        self.expr(b);
                    }
                    extra = 2;
                }
                if a.len() > extra {
                    self.out.push(", ");
                    self.format_string(&a[extra..], false, None);
                }
            }
            "matches" => {
                let parsed = mac.parse_body_with(|input: parse::ParseStream| {
                    let e: Expr = input.parse()?;
                    input.parse::<Token![,]>()?;
                    let p = Pat::parse_multi_with_leading_vert(input)?;
                    let guard = if input.peek(Token![if]) {
                        input.parse::<Token![if]>()?;
                        Some(input.parse::<Expr>()?)
                    } else {
                        None
                    };
                    let _ = input.parse::<Option<Token![,]>>();
                    Ok((e, p, guard))
                });
                match parsed {
                    Ok((e, p, g)) => {
                        self.out.push("(");
                        self.expr(&e);
                        self.span_tok(" in ", span);
                        self.pat(&p);
                        if let Some(g) = g {
                            self.out.push(" if ");
                            self.expr(&g);
                        }
                        self.out.push(")");
                    }
                    Err(_) => self.raw_macro(mac),
                }
            }
            "dbg" => {
                let Some(args) = parse_args() else { return self.raw_macro(mac) };
                self.span_tok(macro_word("dbg"), span);
                self.out.push("(");
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        self.out.push(", ");
                    }
                    self.expr(a);
                }
                self.out.push(")");
            }
            _ => {
                // Generic: if the body parses as call args, show it as a bang method call
                if let Some(args) = parse_args() {
                    self.span_tok(&format!("{}!", rb_ident(&name)), span);
                    self.out.push("(");
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            self.out.push(", ");
                        }
                        self.expr(a);
                    }
                    self.out.push(")");
                } else {
                    self.raw_macro(mac)
                }
            }
        }
    }

    /// A macro invocation on its own line (item or statement position).
    fn macro_stmt(&mut self, mac: &Macro) {
        const KNOWN: &[&str] = &[
            "println",
            "print",
            "eprintln",
            "eprint",
            "format",
            "panic",
            "write",
            "writeln",
            "format_args",
            "unreachable",
            "todo",
            "unimplemented",
            "bail",
            "anyhow",
            "vec",
            "assert",
            "debug_assert",
            "assert_eq",
            "debug_assert_eq",
            "assert_ne",
            "debug_assert_ne",
            "matches",
            "dbg",
        ];
        let name = mac.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
        if KNOWN.contains(&name.as_str()) {
            return self.macro_call(mac);
        }
        // `cfg_rt! { pub mod x; fn y() {} }` — a body of ordinary items: translate them
        let items = mac.parse_body_with(|input: parse::ParseStream| {
            let mut v = Vec::new();
            while !input.is_empty() {
                v.push(input.parse::<Item>()?);
            }
            Ok(v)
        });
        if let Ok(items) = items
            && !items.is_empty()
        {
            let path = self.src_of(&mac.path);
            self.out.push("# ");
            self.node_tok(&format!("{path}! {{"), &mac.path);
            self.out.nl();
            if self.deferred.is_empty() {
                self.items(&items);
            } else {
                // inside a fn: same hoisting as any item declared in a function body
                for it in items {
                    self.out.nl();
                    self.stmt(&Stmt::Item(it));
                }
            }
            self.out.nl();
            self.out.push("# }");
            return;
        }
        // body parses as call arguments: show as a bang call
        if mac.parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated).is_ok() {
            return self.macro_call(mac);
        }
        // anything else (tokio::select!, DSLs): verbatim, mapped line by line
        match tok_range(mac) {
            Some((a, b)) if b.line > a.line => self.out.verbatim(a, b, "# "),
            _ => self.raw_macro(mac),
        }
    }

    fn raw_macro(&mut self, mac: &Macro) {
        let name = mac.path.segments.last().map(|s| s.ident.to_string()).unwrap_or_default();
        self.span_tok(&format!("{}!", rb_ident(&name)), mac.path.span());
        self.out.push("(");
        self.raw_literal(&mac.tokens);
        self.out.push(")");
    }

    /// "{} has {count:?}" + args -> "#{a} has #{count.inspect}"
    fn format_string(&mut self, args: &[&Expr], newline: bool, open: Option<Span>) {
        let Some(Expr::Lit(ExprLit { lit: Lit::Str(fmt), .. })) = args.first() else {
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    self.out.push(", ");
                }
                self.expr(a);
            }
            return;
        };
        let mut positional: Vec<&Expr> = vec![];
        let mut named: HashMap<String, &Expr> = HashMap::new();
        for a in &args[1..] {
            match a {
                Expr::Assign(asg) => {
                    if let Expr::Path(p) = &*asg.left
                        && let Some(id) = p.path.get_ident()
                    {
                        named.insert(id.to_string(), &asg.right);
                        continue;
                    }
                    positional.push(a)
                }
                _ => positional.push(a),
            }
        }
        let s = fmt.value();
        let chars: Vec<char> = s.chars().collect();
        let mut text = String::new();
        let mut next_pos = 0usize;
        let mut i = 0;
        self.span_tok("\"", open.unwrap_or_else(|| fmt.span()));
        while i < chars.len() {
            let c = chars[i];
            if c == '{' && chars.get(i + 1) == Some(&'{') {
                text.push('{');
                i += 2;
                continue;
            }
            if c == '}' && chars.get(i + 1) == Some(&'}') {
                text.push('}');
                i += 2;
                continue;
            }
            if c == '{' {
                let end = chars[i..].iter().position(|&x| x == '}').map(|p| p + i);
                let Some(end) = end else {
                    text.push(c);
                    i += 1;
                    continue;
                };
                let inner: String = chars[i + 1..end].iter().collect();
                let (arg, spec) = match inner.split_once(':') {
                    Some((a, s)) => (a.trim().to_string(), s.to_string()),
                    None => (inner.trim().to_string(), String::new()),
                };
                self.out.push(&escape_rb(&text));
                text.clear();
                self.out.push("#{");
                let precision =
                    spec.split_once('.').map(|(_, p)| p.trim_end_matches(|c: char| !c.is_ascii_digit()).to_string());
                if let Some(p) = &precision
                    && !p.is_empty()
                {
                    self.out.push(&format!("format('%.{p}f', "));
                }
                if arg.is_empty() {
                    if let Some(e) = positional.get(next_pos) {
                        self.expr(e);
                    }
                    next_pos += 1;
                } else if let Ok(n) = arg.parse::<usize>() {
                    if let Some(e) = positional.get(n) {
                        self.expr(e);
                    }
                } else if let Some(e) = named.get(&arg) {
                    self.expr(e);
                } else {
                    // inline captured identifier: {name}
                    self.out.push(&rb_ident(&arg));
                }
                if precision.as_ref().is_some_and(|p| !p.is_empty()) {
                    self.out.push(")");
                }
                if spec.contains('?') {
                    self.out.push(".inspect");
                }
                self.out.push("}");
                i = end + 1;
                continue;
            }
            text.push(c);
            i += 1;
        }
        if newline {
            text.push('\n');
        }
        self.out.push(&escape_rb(&text));
        self.out.push("\"");
    }
}

fn item_ident(it: &Item) -> Option<&Ident> {
    match it {
        Item::Fn(f) => Some(&f.sig.ident),
        Item::Struct(s) => Some(&s.ident),
        Item::Enum(e) => Some(&e.ident),
        Item::Trait(t) => Some(&t.ident),
        Item::Const(c) => Some(&c.ident),
        Item::Static(s) => Some(&s.ident),
        Item::Type(t) => Some(&t.ident),
        Item::Mod(m) => Some(&m.ident),
        Item::Union(u) => Some(&u.ident),
        Item::Macro(m) => m.ident.as_ref(),
        _ => None,
    }
}

fn item_label(it: &Item) -> (&'static str, String) {
    let kind = match it {
        Item::Fn(_) => "fn",
        Item::Struct(_) => "struct",
        Item::Enum(_) => "enum",
        Item::Trait(_) => "trait",
        Item::Impl(_) => "impl",
        Item::Const(_) => "const",
        Item::Static(_) => "static",
        Item::Type(_) => "type",
        Item::Mod(_) => "mod",
        _ => "item",
    };
    let name = match it {
        Item::Impl(i) => i.self_ty.to_token_stream().to_string().replace(' ', ""),
        _ => item_ident(it).map(|i| i.to_string()).unwrap_or_default(),
    };
    (kind, name)
}
