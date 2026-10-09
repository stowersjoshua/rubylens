// Position mapping between a Ruby view and its Rust source.
// Pure JS (no vscode import) so it can be unit-tested with plain node.
//
// map.segs:   [rbLine, rbCol, rbLen, rsLine, rsCol, rsLen]  (0-based, UTF-16 columns)
// map.blocks: [rbStart, rbEnd, rsStart, rsEnd]               (0-based lines, inclusive)
'use strict';

class SrcMap {
  constructor(map) {
    this.segs = map.segs || [];
    this.blocks = map.blocks || [];
    this.byRb = groupBy(this.segs, (s) => s[0]);
    this.byRs = groupBy(this.segs, (s) => s[3]);
  }

  /** Ruby position -> Rust position (or null) */
  rbToRs(pos) {
    const seg = pick(this.byRb.get(pos.line), pos.character, 1, 2);
    if (seg) {
      const off = Math.min(Math.max(pos.character - seg[1], 0), seg[5] - 1);
      return { line: seg[3], character: seg[4] + off };
    }
    const b = innermost(this.blocks, pos.line, 0, 1);
    if (b) return { line: Math.min(b[2] + (pos.line - b[0]), b[3]), character: 0 };
    return null;
  }

  /** Rust position -> Ruby position (or null) */
  rsToRb(pos) {
    const seg = pick(this.byRs.get(pos.line), pos.character, 4, 5);
    if (seg) {
      const off = Math.min(Math.max(pos.character - seg[4], 0), seg[2] - 1);
      return { line: seg[0], character: seg[1] + off };
    }
    const b = innermost(this.blocks, pos.line, 2, 3);
    if (b) return { line: Math.min(b[0] + (pos.line - b[2]), b[1]), character: 0 };
    // Nothing on this line (e.g. inside a long doc comment): nearest mapped Rust line,
    // preferring the code that follows; failing that, the top of the view.
    let best = null;
    for (const [l, segs] of this.byRs) {
      const d = l >= pos.line ? l - pos.line : (pos.line - l) * 2 + 1;
      if (!best || d < best.d) best = { d, seg: segs[0] };
    }
    if (best) return { line: best.seg[0], character: best.seg[1] };
    return { line: 0, character: 0 };
  }

  /** The Ruby token range under a position (for highlighting the clicked word). */
  rbTokenRange(pos) {
    const segs = this.byRb.get(pos.line) || [];
    const seg = smallest(segs.filter((s) => pos.character >= s[1] && pos.character < s[1] + s[2]), 2);
    return seg ? { start: { line: seg[0], character: seg[1] }, end: { line: seg[0], character: seg[1] + seg[2] } } : null;
  }

  /** Rust range -> Ruby range */
  rsRangeToRb(range) {
    const start = this.rsToRb(range.start);
    if (!start) return null;
    // Exact token? keep its Ruby length.
    const seg = pick(this.byRs.get(range.start.line), range.start.character, 4, 5, true);
    if (seg && range.start.line === range.end.line && seg[4] === range.start.character) {
      return { start: { line: seg[0], character: seg[1] }, end: { line: seg[0], character: seg[1] + seg[2] } };
    }
    let end = this.rsToRb(range.end) || start;
    if (end.line < start.line || (end.line === start.line && end.character < start.character)) end = start;
    return { start, end };
  }
}

function groupBy(segs, key) {
  const m = new Map();
  for (const s of segs) {
    const k = key(s);
    if (!m.has(k)) m.set(k, []);
    m.get(k).push(s);
  }
  return m;
}

function smallest(cands, lenIdx) {
  let best = null;
  for (const s of cands) if (!best || s[lenIdx] < best[lenIdx]) best = s;
  return best;
}

/** Segment containing `col` (smallest wins), else the nearest one on the line. */
function pick(segs, col, colIdx, lenIdx, exactOnly = false) {
  if (!segs || !segs.length) return null;
  const containing = segs.filter((s) => col >= s[colIdx] && col < s[colIdx] + s[lenIdx]);
  if (containing.length) return smallest(containing, lenIdx);
  if (exactOnly) return null;
  let best = null, bestD = Infinity;
  for (const s of segs) {
    const d = col < s[colIdx] ? s[colIdx] - col : col - (s[colIdx] + s[lenIdx] - 1);
    if (d < bestD) { best = s; bestD = d; }
  }
  return best;
}

function innermost(blocks, line, a, b) {
  let best = null;
  for (const bl of blocks) {
    if (line >= bl[a] && line <= bl[b] && (!best || bl[b] - bl[a] < best[b] - best[a])) best = bl;
  }
  return best;
}

module.exports = { SrcMap };
