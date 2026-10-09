//! Output buffer that tracks Ruby line/column (UTF-16, like LSP) and records
//! the mapping from Ruby tokens back to Rust source spans.

use proc_macro2::LineColumn;
use serde::Serialize;

/// One mapped token: [rb_line, rb_col, rb_len, rs_line, rs_col, rs_len]
/// All lines are 0-based, all columns/lengths are UTF-16 code units.
pub type Seg = [u32; 6];

/// Coarse mapping of a statement or item: [rb_start, rb_end, rs_start, rs_end] (lines, 0-based, inclusive)
pub type Block = [u32; 4];

#[derive(Serialize, Default)]
pub struct SourceMap {
    pub version: u32,
    pub hash: String,
    pub segs: Vec<Seg>,
    pub blocks: Vec<Block>,
}

pub struct Out {
    pub buf: String,
    pub line: u32,
    pub col: u32,
    pub indent: usize,
    at_line_start: bool,
    pub segs: Vec<Seg>,
    pub blocks: Vec<Block>,
    /// The Rust source and, per line (as `str::lines` splits it), its byte range and whether
    /// it is pure ASCII. On ASCII lines a proc-macro2 char column is also the byte offset and
    /// the UTF-16 column, so the common case needs no scanning.
    src: String,
    src_lines: Vec<(usize, usize, bool)>,
}

impl Out {
    pub fn new(src: &str) -> Self {
        Out {
            buf: String::new(),
            line: 0,
            col: 0,
            indent: 0,
            at_line_start: true,
            segs: vec![],
            blocks: vec![],
            src: src.to_string(),
            src_lines: src
                .lines()
                .map(|l| {
                    let start = l.as_ptr() as usize - src.as_ptr() as usize;
                    (start, start + l.len(), l.is_ascii())
                })
                .collect(),
        }
    }

    /// Text of source line `ln` (1-based, like proc-macro2) and whether it is ASCII.
    fn src_line(&self, ln: usize) -> Option<(&str, bool)> {
        let &(start, end, ascii) = self.src_lines.get(ln.wrapping_sub(1))?;
        Some((&self.src[start..end], ascii))
    }

    /// Byte offset of char column `col` in `text`, clamped to its length.
    fn byte_at(text: &str, ascii: bool, col: usize) -> usize {
        if ascii { col.min(text.len()) } else { text.char_indices().nth(col).map_or(text.len(), |(i, _)| i) }
    }

    fn write_raw(&mut self, s: &str) {
        if s.is_ascii() {
            match s.rfind('\n') {
                None => self.col += s.len() as u32,
                Some(last) => {
                    self.line += s.bytes().filter(|&b| b == b'\n').count() as u32;
                    self.col = (s.len() - last - 1) as u32;
                }
            }
        } else {
            for ch in s.chars() {
                if ch == '\n' {
                    self.line += 1;
                    self.col = 0;
                } else {
                    self.col += ch.len_utf16() as u32;
                }
            }
        }
        self.buf.push_str(s);
    }

    fn ensure_indent(&mut self) {
        if self.at_line_start {
            for _ in 0..self.indent {
                self.buf.push_str("  ");
            }
            self.col += 2 * self.indent as u32;
            self.at_line_start = false;
        }
    }

    /// Write unmapped text. Newlines inside `s` are written verbatim.
    pub fn push(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        self.ensure_indent();
        self.write_raw(s);
        if s.ends_with('\n') {
            self.at_line_start = true;
        }
    }

    /// Write text that corresponds to a Rust span (records a mapping segment).
    pub fn tok(&mut self, s: &str, start: LineColumn, end: LineColumn) {
        self.ensure_indent();
        let rb_line = self.line;
        let rb_col = self.col;
        self.write_raw(s);
        let rb_len = self.col.saturating_sub(rb_col);
        if start.line == 0 {
            return; // no location info
        }
        let rs_line = (start.line - 1) as u32;
        let rs_col = self.col16(start);
        let rs_len = if end.line == start.line { self.col16(end).saturating_sub(rs_col) } else { 1 };
        if rb_len > 0 {
            self.segs.push([rb_line, rb_col, rb_len, rs_line, rs_col, rs_len.max(1)]);
        }
    }

    /// Newline (no-op if we're already at the start of a line).
    pub fn nl(&mut self) {
        if !self.at_line_start {
            self.write_raw("\n");
            self.at_line_start = true;
        }
    }

    /// Force a blank line.
    pub fn blank(&mut self) {
        self.nl();
        if !self.buf.ends_with("\n\n") && !self.buf.is_empty() {
            self.write_raw("\n");
        }
    }

    pub fn comment(&mut self, text: &str) {
        for l in text.lines() {
            self.nl();
            let l = l.trim_end();
            if l.is_empty() {
                self.push("#");
            } else {
                self.push("# ");
                self.push(l);
            }
            self.nl();
        }
    }

    pub fn block(&mut self, rb_start: u32, rs: Option<(LineColumn, LineColumn)>) {
        if let Some((a, b)) = rs {
            if a.line == 0 {
                return;
            }
            let rb_end = if self.at_line_start && self.line > rb_start { self.line - 1 } else { self.line };
            self.blocks.push([rb_start, rb_end, (a.line - 1) as u32, (b.line.max(1) - 1) as u32]);
        }
    }

    /// Emit source lines `from.line..=to.line` verbatim behind `prefix` (e.g. "# "),
    /// common indentation removed, each line mapped character-for-character.
    pub fn verbatim(&mut self, from: LineColumn, to: LineColumn, prefix: &str) {
        if from.line == 0 {
            return;
        }
        let lines: Vec<(usize, &str)> =
            (from.line..=to.line).filter_map(|ln| self.src_line(ln).map(|(t, _)| (ln, t))).collect();
        let dedent = lines
            .iter()
            .filter(|(_, t)| t.chars().any(|ch| !ch.is_whitespace()))
            .map(|(_, t)| t.chars().take_while(|ch| ch.is_whitespace()).count())
            .min()
            .unwrap_or(0);
        let lines: Vec<(usize, String)> = lines
            .into_iter()
            .map(|(ln, t)| (ln, t[t.char_indices().nth(dedent).map_or(t.len(), |(i, _)| i)..].trim_end().to_string()))
            .collect();
        for (ln, body) in lines {
            self.nl();
            if body.is_empty() {
                self.push(prefix.trim_end());
            } else {
                self.push(prefix);
                let start = LineColumn { line: ln, column: dedent };
                let end = LineColumn { line: ln, column: dedent + body.chars().count() };
                self.tok(&body, start, end);
            }
            self.nl();
        }
    }

    /// Convert a proc-macro2 char column to a UTF-16 column for the given source line.
    pub fn col16(&self, lc: LineColumn) -> u32 {
        match self.src_line(lc.line) {
            Some((text, true)) => lc.column.min(text.len()) as u32,
            Some((text, false)) => text.chars().take(lc.column).map(|c| c.len_utf16() as u32).sum(),
            None => lc.column as u32,
        }
    }

    /// Exact source text between two positions on the same line (char columns).
    pub fn src_exact(&self, a: LineColumn, b: LineColumn) -> String {
        match self.src_line(a.line) {
            Some((text, ascii)) if a.line == b.line => {
                let (i, j) = (Self::byte_at(text, ascii, a.column), Self::byte_at(text, ascii, b.column));
                text[i..j.max(i)].to_string()
            }
            _ => self.src_text(a, b),
        }
    }

    /// Source text between two positions (char columns), whitespace collapsed.
    pub fn src_text(&self, a: LineColumn, b: LineColumn) -> String {
        let mut s = String::new();
        if a.line == 0 {
            return s;
        }
        for ln in a.line..=b.line {
            let Some((text, ascii)) = self.src_line(ln) else { break };
            let from = if ln == a.line { Self::byte_at(text, ascii, a.column) } else { 0 };
            let to = if ln == b.line { Self::byte_at(text, ascii, b.column) } else { text.len() };
            if from < to {
                s.push_str(&text[from..to]);
            }
            if ln != b.line {
                s.push(' ');
            }
        }
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}
