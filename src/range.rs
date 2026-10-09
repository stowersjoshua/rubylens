//! Source ranges of syntax nodes, found without re-tokenizing them.
//!
//! [`tok_range`] finds a node's range by printing it back to tokens, which costs time and memory proportional to
//! the node's size. The block map needs the range of every item, statement and match arm, at every nesting level,
//! so doing that for each one is O(size × depth). Here the same two tokens are read straight off the syntax tree
//! instead: the first token down the node's left edge (outer attribute, visibility, keyword, left operand, ...) and
//! the last token down its right edge (closing brace, semicolon, right operand, ...). A shape that isn't covered
//! falls back to re-tokenizing just that sub-node, so the answer is always the same as [`tok_range`]'s; debug builds
//! (every `cargo test`) assert it.
//!
//! The order of tokens is syn's printing order (`ToTokens`), which is source order. If a syn upgrade changes how a
//! node prints, the debug assertion fails in the golden tests.

use crate::emit::tok_range;
use proc_macro2::{LineColumn, Span, TokenStream, TokenTree};
use quote::ToTokens;
use syn::{
    Arm, AttrStyle, Attribute, Block, Expr, ImplItem, Item, Macro, MacroDelimiter, Member, Path, PathArguments,
    RangeLimits, Signature, Stmt, TraitItem, UnOp, Visibility,
};

/// Source range of `node`: the start of its first token to the end of its last, as [`tok_range`] computes it.
pub fn node_range<T: Edges + ToTokens + ?Sized>(node: &T) -> Option<(LineColumn, LineColumn)> {
    let fast = match (node.first(), node.last()) {
        (Some(a), Some(b)) => Some((a.start(), b.end())),
        _ => tok_range(node),
    };
    #[cfg(debug_assertions)]
    {
        let slow = tok_range(node);
        // Tokens syn synthesizes while printing have no source position; they read as 1:0. Only real tokens
        // are compared.
        let synthesized = |r: Option<(LineColumn, LineColumn)>| {
            r.is_some_and(|(a, b)| (a.line, a.column) == (1, 0) || (b.line, b.column) == (1, 0))
        };
        if !synthesized(slow) {
            let show = |r: Option<(LineColumn, LineColumn)>| {
                r.map(|(a, b)| format!("{}:{}-{}:{}", a.line, a.column, b.line, b.column))
            };
            assert_eq!(show(fast), show(slow), "range::node_range disagrees with tok_range");
        }
    }
    fast
}

/// The first and last tokens of a syntax node, in printing (= source) order.
pub trait Edges {
    fn first(&self) -> Option<Span>;
    fn last(&self) -> Option<Span>;
}

/// First and last token of `t` by printing it (the slow, always-right path).
fn printed<T: ToTokens + ?Sized>(t: &T) -> (Option<Span>, Option<Span>) {
    fn walk(ts: TokenStream, first: &mut Option<Span>, last: &mut Option<Span>) {
        for tt in ts {
            match tt {
                TokenTree::Group(g) if g.delimiter() == proc_macro2::Delimiter::None => walk(g.stream(), first, last),
                TokenTree::Group(g) => {
                    first.get_or_insert(g.span_open());
                    *last = Some(g.span_close());
                }
                other => {
                    first.get_or_insert(other.span());
                    *last = Some(other.span());
                }
            }
        }
    }
    let (mut first, mut last) = (None, None);
    walk(t.to_token_stream(), &mut first, &mut last);
    (first, last)
}

fn printed_first<T: ToTokens + ?Sized>(t: &T) -> Option<Span> {
    printed(t).0
}

fn printed_last<T: ToTokens + ?Sized>(t: &T) -> Option<Span> {
    printed(t).1
}

fn outer_attr(attrs: &[Attribute]) -> Option<Span> {
    attrs.iter().find(|a| matches!(a.style, AttrStyle::Outer)).map(|a| a.pound_token.spans[0])
}

fn vis(v: &Visibility) -> Option<Span> {
    match v {
        Visibility::Public(t) => Some(t.span),
        Visibility::Restricted(r) => Some(r.pub_token.span),
        Visibility::Inherited => None,
    }
}

fn sig_first(s: &Signature) -> Span {
    s.constness
        .map(|t| t.span)
        .or(s.asyncness.map(|t| t.span))
        .or(s.unsafety.map(|t| t.span))
        .or(s.abi.as_ref().map(|a| a.extern_token.span))
        .unwrap_or(s.fn_token.span)
}

fn path_first(p: &Path) -> Option<Span> {
    match &p.leading_colon {
        Some(c) => Some(c.spans[0]),
        None => p.segments.first().map(|s| s.ident.span()),
    }
}

fn path_last(p: &Path) -> Option<Span> {
    match p.segments.last() {
        Some(s) if matches!(s.arguments, PathArguments::None) => Some(s.ident.span()),
        _ => printed_last(p),
    }
}

fn mac_last(m: &Macro) -> Span {
    match &m.delimiter {
        MacroDelimiter::Paren(d) => d.span.close(),
        MacroDelimiter::Brace(d) => d.span.close(),
        MacroDelimiter::Bracket(d) => d.span.close(),
    }
}

fn block_last(b: &Block) -> Span {
    b.brace_token.span.close()
}

impl Edges for Item {
    fn first(&self) -> Option<Span> {
        let (attrs, rest) = match self {
            Item::Const(i) => (&i.attrs, vis(&i.vis).or(Some(i.const_token.span))),
            Item::Enum(i) => (&i.attrs, vis(&i.vis).or(Some(i.enum_token.span))),
            Item::ExternCrate(i) => (&i.attrs, vis(&i.vis).or(Some(i.extern_token.span))),
            Item::Fn(i) => (&i.attrs, vis(&i.vis).or(Some(sig_first(&i.sig)))),
            Item::ForeignMod(i) => (&i.attrs, i.unsafety.map(|t| t.span).or(Some(i.abi.extern_token.span))),
            Item::Impl(i) => {
                (&i.attrs, i.defaultness.map(|t| t.span).or(i.unsafety.map(|t| t.span)).or(Some(i.impl_token.span)))
            }
            Item::Macro(i) => (&i.attrs, path_first(&i.mac.path)),
            Item::Mod(i) => (&i.attrs, vis(&i.vis).or(i.unsafety.map(|t| t.span)).or(Some(i.mod_token.span))),
            Item::Static(i) => (&i.attrs, vis(&i.vis).or(Some(i.static_token.span))),
            Item::Struct(i) => (&i.attrs, vis(&i.vis).or(Some(i.struct_token.span))),
            Item::Trait(i) => (
                &i.attrs,
                vis(&i.vis)
                    .or(i.unsafety.map(|t| t.span))
                    .or(i.auto_token.map(|t| t.span))
                    .or(Some(i.trait_token.span)),
            ),
            Item::TraitAlias(i) => (&i.attrs, vis(&i.vis).or(Some(i.trait_token.span))),
            Item::Type(i) => (&i.attrs, vis(&i.vis).or(Some(i.type_token.span))),
            Item::Union(i) => (&i.attrs, vis(&i.vis).or(Some(i.union_token.span))),
            Item::Use(i) => (&i.attrs, vis(&i.vis).or(Some(i.use_token.span))),
            other => return printed_first(other),
        };
        outer_attr(attrs).or(rest)
    }

    fn last(&self) -> Option<Span> {
        match self {
            Item::Const(i) => Some(i.semi_token.spans[0]),
            Item::Enum(i) => Some(i.brace_token.span.close()),
            Item::ExternCrate(i) => Some(i.semi_token.spans[0]),
            Item::Fn(i) => Some(block_last(&i.block)),
            Item::ForeignMod(i) => Some(i.brace_token.span.close()),
            Item::Impl(i) => Some(i.brace_token.span.close()),
            Item::Macro(i) => Some(i.semi_token.map_or_else(|| mac_last(&i.mac), |t| t.spans[0])),
            Item::Mod(i) => match (&i.content, &i.semi) {
                (Some((brace, _)), _) => Some(brace.span.close()),
                (None, Some(semi)) => Some(semi.spans[0]),
                (None, None) => printed_last(self),
            },
            Item::Static(i) => Some(i.semi_token.spans[0]),
            Item::Struct(i) => match (&i.fields, &i.semi_token) {
                (syn::Fields::Named(f), _) => Some(f.brace_token.span.close()),
                (_, Some(semi)) => Some(semi.spans[0]),
                _ => printed_last(self),
            },
            Item::Trait(i) => Some(i.brace_token.span.close()),
            Item::TraitAlias(i) => Some(i.semi_token.spans[0]),
            Item::Type(i) => Some(i.semi_token.spans[0]),
            Item::Union(i) => Some(i.fields.brace_token.span.close()),
            Item::Use(i) => Some(i.semi_token.spans[0]),
            other => printed_last(other),
        }
    }
}

impl Edges for ImplItem {
    fn first(&self) -> Option<Span> {
        let (attrs, rest) = match self {
            ImplItem::Const(i) => {
                (&i.attrs, vis(&i.vis).or(i.defaultness.map(|t| t.span)).or(Some(i.const_token.span)))
            }
            ImplItem::Fn(i) => (&i.attrs, vis(&i.vis).or(i.defaultness.map(|t| t.span)).or(Some(sig_first(&i.sig)))),
            ImplItem::Type(i) => (&i.attrs, vis(&i.vis).or(i.defaultness.map(|t| t.span)).or(Some(i.type_token.span))),
            ImplItem::Macro(i) => (&i.attrs, path_first(&i.mac.path)),
            other => return printed_first(other),
        };
        outer_attr(attrs).or(rest)
    }

    fn last(&self) -> Option<Span> {
        match self {
            ImplItem::Const(i) => Some(i.semi_token.spans[0]),
            ImplItem::Fn(i) => Some(block_last(&i.block)),
            ImplItem::Type(i) => Some(i.semi_token.spans[0]),
            ImplItem::Macro(i) => Some(i.semi_token.map_or_else(|| mac_last(&i.mac), |t| t.spans[0])),
            other => printed_last(other),
        }
    }
}

impl Edges for TraitItem {
    fn first(&self) -> Option<Span> {
        let (attrs, rest) = match self {
            TraitItem::Const(i) => (&i.attrs, Some(i.const_token.span)),
            TraitItem::Fn(i) => (&i.attrs, Some(sig_first(&i.sig))),
            TraitItem::Type(i) => (&i.attrs, Some(i.type_token.span)),
            TraitItem::Macro(i) => (&i.attrs, path_first(&i.mac.path)),
            other => return printed_first(other),
        };
        outer_attr(attrs).or(rest)
    }

    fn last(&self) -> Option<Span> {
        match self {
            TraitItem::Const(i) => Some(i.semi_token.spans[0]),
            TraitItem::Fn(i) => match (&i.default, &i.semi_token) {
                (Some(b), _) => Some(block_last(b)),
                (None, Some(semi)) => Some(semi.spans[0]),
                (None, None) => printed_last(self),
            },
            TraitItem::Type(i) => Some(i.semi_token.spans[0]),
            TraitItem::Macro(i) => Some(i.semi_token.map_or_else(|| mac_last(&i.mac), |t| t.spans[0])),
            other => printed_last(other),
        }
    }
}

impl Edges for Stmt {
    fn first(&self) -> Option<Span> {
        match self {
            Stmt::Local(l) => outer_attr(&l.attrs).or(Some(l.let_token.span)),
            Stmt::Item(i) => i.first(),
            Stmt::Expr(e, _) => e.first(),
            Stmt::Macro(m) => outer_attr(&m.attrs).or(path_first(&m.mac.path)),
        }
    }

    fn last(&self) -> Option<Span> {
        match self {
            Stmt::Local(l) => Some(l.semi_token.spans[0]),
            Stmt::Item(i) => i.last(),
            Stmt::Expr(_, Some(semi)) => Some(semi.spans[0]),
            Stmt::Expr(e, None) => e.last(),
            Stmt::Macro(m) => Some(m.semi_token.map_or_else(|| mac_last(&m.mac), |t| t.spans[0])),
        }
    }
}

impl Edges for Arm {
    fn first(&self) -> Option<Span> {
        // arms print every attribute (they can only be outer)
        self.attrs.first().map(|a| a.pound_token.spans[0]).or_else(|| printed_first(&self.pat))
    }

    fn last(&self) -> Option<Span> {
        match &self.comma {
            Some(c) => Some(c.spans[0]),
            None => self.body.last(),
        }
    }
}

impl Edges for Expr {
    fn first(&self) -> Option<Span> {
        let (attrs, rest) = match self {
            Expr::Array(e) => (&e.attrs, Some(e.bracket_token.span.open())),
            Expr::Assign(e) => (&e.attrs, e.left.first()),
            Expr::Async(e) => (&e.attrs, Some(e.async_token.span)),
            Expr::Await(e) => (&e.attrs, e.base.first()),
            Expr::Binary(e) => (&e.attrs, e.left.first()),
            Expr::Block(e) => (
                &e.attrs,
                Some(e.label.as_ref().map_or_else(|| e.block.brace_token.span.open(), |l| l.name.apostrophe)),
            ),
            Expr::Break(e) => (&e.attrs, Some(e.break_token.span)),
            Expr::Call(e) => (&e.attrs, e.func.first()),
            Expr::Cast(e) => (&e.attrs, e.expr.first()),
            Expr::Closure(e) => (
                &e.attrs,
                e.lifetimes
                    .as_ref()
                    .map(|l| l.for_token.span)
                    .or(e.constness.map(|t| t.span))
                    .or(e.movability.map(|t| t.span))
                    .or(e.asyncness.map(|t| t.span))
                    .or(e.capture.map(|t| t.span))
                    .or(Some(e.or1_token.spans[0])),
            ),
            Expr::Const(e) => (&e.attrs, Some(e.const_token.span)),
            Expr::Continue(e) => (&e.attrs, Some(e.continue_token.span)),
            Expr::Field(e) => (&e.attrs, e.base.first()),
            Expr::ForLoop(e) => (&e.attrs, Some(e.label.as_ref().map_or(e.for_token.span, |l| l.name.apostrophe))),
            Expr::Group(e) => (&e.attrs, e.expr.first()),
            Expr::If(e) => (&e.attrs, Some(e.if_token.span)),
            Expr::Index(e) => (&e.attrs, e.expr.first()),
            Expr::Infer(e) => (&e.attrs, Some(e.underscore_token.spans[0])),
            Expr::Let(e) => (&e.attrs, Some(e.let_token.span)),
            Expr::Lit(e) => (&e.attrs, Some(e.lit.span())),
            Expr::Loop(e) => (&e.attrs, Some(e.label.as_ref().map_or(e.loop_token.span, |l| l.name.apostrophe))),
            Expr::Macro(e) => (&e.attrs, path_first(&e.mac.path)),
            Expr::Match(e) => (&e.attrs, Some(e.match_token.span)),
            Expr::MethodCall(e) => (&e.attrs, e.receiver.first()),
            Expr::Paren(e) => (&e.attrs, Some(e.paren_token.span.open())),
            Expr::Path(e) => (&e.attrs, e.qself.as_ref().map(|q| q.lt_token.spans[0]).or_else(|| path_first(&e.path))),
            Expr::Range(e) => (
                &e.attrs,
                match (&e.start, &e.limits) {
                    (Some(s), _) => s.first(),
                    (None, RangeLimits::HalfOpen(t)) => Some(t.spans[0]),
                    (None, RangeLimits::Closed(t)) => Some(t.spans[0]),
                },
            ),
            Expr::RawAddr(e) => (&e.attrs, Some(e.and_token.spans[0])),
            Expr::Reference(e) => (&e.attrs, Some(e.and_token.spans[0])),
            Expr::Repeat(e) => (&e.attrs, Some(e.bracket_token.span.open())),
            Expr::Return(e) => (&e.attrs, Some(e.return_token.span)),
            Expr::Struct(e) => {
                (&e.attrs, e.qself.as_ref().map(|q| q.lt_token.spans[0]).or_else(|| path_first(&e.path)))
            }
            Expr::Try(e) => (&e.attrs, e.expr.first()),
            Expr::TryBlock(e) => (&e.attrs, Some(e.try_token.span)),
            Expr::Tuple(e) => (&e.attrs, Some(e.paren_token.span.open())),
            Expr::Unary(e) => (
                &e.attrs,
                match e.op {
                    UnOp::Deref(t) => Some(t.spans[0]),
                    UnOp::Not(t) => Some(t.spans[0]),
                    UnOp::Neg(t) => Some(t.spans[0]),
                    _ => return printed_first(self),
                },
            ),
            Expr::Unsafe(e) => (&e.attrs, Some(e.unsafe_token.span)),
            Expr::While(e) => (&e.attrs, Some(e.label.as_ref().map_or(e.while_token.span, |l| l.name.apostrophe))),
            Expr::Yield(e) => (&e.attrs, Some(e.yield_token.span)),
            other => return printed_first(other),
        };
        outer_attr(attrs).or(rest)
    }

    fn last(&self) -> Option<Span> {
        match self {
            Expr::Array(e) => Some(e.bracket_token.span.close()),
            Expr::Assign(e) => e.right.last(),
            Expr::Async(e) => Some(block_last(&e.block)),
            Expr::Await(e) => Some(e.await_token.span),
            Expr::Binary(e) => e.right.last(),
            Expr::Block(e) => Some(block_last(&e.block)),
            Expr::Break(e) => match (&e.expr, &e.label) {
                (Some(x), _) => x.last(),
                (None, Some(l)) => Some(l.ident.span()),
                (None, None) => Some(e.break_token.span),
            },
            Expr::Call(e) => Some(e.paren_token.span.close()),
            Expr::Cast(e) => printed_last(&e.ty),
            Expr::Closure(e) => e.body.last(),
            Expr::Const(e) => Some(block_last(&e.block)),
            Expr::Continue(e) => Some(e.label.as_ref().map_or(e.continue_token.span, |l| l.ident.span())),
            Expr::Field(e) => Some(match &e.member {
                Member::Named(i) => i.span(),
                Member::Unnamed(i) => i.span,
            }),
            Expr::ForLoop(e) => Some(block_last(&e.body)),
            Expr::Group(e) => e.expr.last(),
            Expr::If(e) => match &e.else_branch {
                Some((_, els)) => els.last(),
                None => Some(block_last(&e.then_branch)),
            },
            Expr::Index(e) => Some(e.bracket_token.span.close()),
            Expr::Infer(e) => Some(e.underscore_token.spans[0]),
            Expr::Let(e) => e.expr.last(),
            Expr::Lit(e) => Some(e.lit.span()),
            Expr::Loop(e) => Some(block_last(&e.body)),
            Expr::Macro(e) => Some(mac_last(&e.mac)),
            Expr::Match(e) => Some(e.brace_token.span.close()),
            Expr::MethodCall(e) => Some(e.paren_token.span.close()),
            Expr::Paren(e) => Some(e.paren_token.span.close()),
            Expr::Path(e) => path_last(&e.path),
            Expr::Range(e) => match (&e.end, &e.limits) {
                (Some(x), _) => x.last(),
                (None, RangeLimits::HalfOpen(t)) => Some(t.spans[1]),
                (None, RangeLimits::Closed(t)) => Some(t.spans[2]),
            },
            Expr::RawAddr(e) => e.expr.last(),
            Expr::Reference(e) => e.expr.last(),
            Expr::Repeat(e) => Some(e.bracket_token.span.close()),
            Expr::Return(e) => e.expr.as_ref().map_or(Some(e.return_token.span), |x| x.last()),
            Expr::Struct(e) => Some(e.brace_token.span.close()),
            Expr::Try(e) => Some(e.question_token.spans[0]),
            Expr::TryBlock(e) => Some(block_last(&e.block)),
            Expr::Tuple(e) => Some(e.paren_token.span.close()),
            Expr::Unary(e) => e.expr.last(),
            Expr::Unsafe(e) => Some(block_last(&e.block)),
            Expr::While(e) => Some(block_last(&e.body)),
            Expr::Yield(e) => e.expr.as_ref().map_or(Some(e.yield_token.span), |x| x.last()),
            other => printed_last(other),
        }
    }
}
