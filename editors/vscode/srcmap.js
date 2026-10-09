// Position mapping between a Ruby view and its Rust source.
// Pure JS (no vscode import) so it can be unit-tested with plain node.
//
// map.segs:   [rbLine, rbCol, rbLen, rsLine, rsCol, rsLen]  (0-based, UTF-16 columns)
// map.blocks: [rbStart, rbEnd, rsStart, rsEnd]               (0-based lines, inclusive)
//
// Every lookup is O(segments on one line) or O(log n): an outline or a reference list asks
// for thousands of positions in one view, and generated files have 100,000+ segments.
'use strict';

class SrcMap {
  constructor(map) {
    this.segs = map.segs || [];
    this.blocks = map.blocks || [];
    this.byRb = groupBy(this.segs, (s) => s[0]);
    this.byRs = groupBy(this.segs, (s) => s[3]);
    this.innerRb = null; // line -> innermost block, built on first use
    this.innerRs = null;
    this.rsLines = null; // mapped Rust lines, sorted, for the nearest-line fallback
  }

  /** Ruby position -> Rust position (or null) */
  rbToRs(pos) {
    const seg = pick(this.byRb.get(pos.line), pos.character, 1, 2);
    if (seg) {
      const off = Math.min(Math.max(pos.character - seg[1], 0), seg[5] - 1);
      return { line: seg[3], character: seg[4] + off };
    }
    this.innerRb = this.innerRb || innermostIndex(this.blocks, 0, 1);
    const b = this.innerRb(pos.line);
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
    this.innerRs = this.innerRs || innermostIndex(this.blocks, 2, 3);
    const b = this.innerRs(pos.line);
    if (b) return { line: Math.min(b[0] + (pos.line - b[2]), b[1]), character: 0 };
    // Nothing on this line (e.g. inside a long doc comment): nearest mapped Rust line,
    // preferring the code that follows; failing that, the top of the view.
    const near = this.nearestRsLine(pos.line);
    if (near !== null) {
      const seg0 = this.byRs.get(near)[0];
      return { line: seg0[0], character: seg0[1] };
    }
    return { line: 0, character: 0 };
  }

  /**
   * The mapped Rust line closest to `line`, where a line below costs its distance and a line
   * above costs twice its distance plus one. Ties go to the line mapped first in the view.
   */
  nearestRsLine(line) {
    if (!this.rsLines) {
      const order = new Map([...this.byRs.keys()].map((l, i) => [l, i]));
      this.rsLines = { sorted: [...this.byRs.keys()].sort((a, b) => a - b), order };
    }
    const { sorted, order } = this.rsLines;
    if (!sorted.length) return null;
    const i = lowerBound(sorted, line); // first mapped line >= line
    const cands = [];
    if (i < sorted.length) cands.push({ l: sorted[i], d: sorted[i] - line });
    if (i > 0) cands.push({ l: sorted[i - 1], d: (line - sorted[i - 1]) * 2 + 1 });
    cands.sort((a, b) => a.d - b.d || order.get(a.l) - order.get(b.l));
    return cands[0].l;
  }

  /** The Ruby token range under a position (for highlighting the clicked word). */
  rbTokenRange(pos) {
    const seg = pick(this.byRb.get(pos.line), pos.character, 1, 2, true);
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
    let list = m.get(k);
    if (!list) m.set(k, (list = []));
    list.push(s);
  }
  return m;
}

/**
 * Segment containing `col` (the shortest; the first of equals), else the nearest one on the
 * line (the first of equals), else null. With `exactOnly`, only a containing segment.
 */
function pick(segs, col, colIdx, lenIdx, exactOnly = false) {
  if (!segs || !segs.length) return null;
  let best = null;
  for (const s of segs) {
    if (col >= s[colIdx] && col < s[colIdx] + s[lenIdx] && (!best || s[lenIdx] < best[lenIdx])) best = s;
  }
  if (best || exactOnly) return best;
  let bestD = Infinity;
  for (const s of segs) {
    const d = col < s[colIdx] ? s[colIdx] - col : col - (s[colIdx] + s[lenIdx] - 1);
    if (d < bestD) { best = s; bestD = d; }
  }
  return best;
}

/**
 * line -> the innermost block containing it (the shortest; the first of equals), over the
 * line range [a, b] of each block. Built once by painting blocks longest first, so shorter
 * ones overwrite: O(total block length), then O(1) per lookup.
 */
function innermostIndex(blocks, a, b) {
  let max = -1;
  for (const bl of blocks) if (bl[b] > max) max = bl[b];
  const owner = new Int32Array(max + 1).fill(-1);
  const order = blocks.map((_, i) => i);
  // longest first; among equal lengths the earliest block paints last, so it wins
  order.sort((i, j) => (blocks[j][b] - blocks[j][a]) - (blocks[i][b] - blocks[i][a]) || j - i);
  for (const i of order) {
    const bl = blocks[i];
    for (let l = Math.max(bl[a], 0); l <= bl[b]; l++) owner[l] = i;
  }
  return (line) => (line >= 0 && line <= max && owner[line] >= 0 ? blocks[owner[line]] : null);
}

function lowerBound(sorted, x) {
  let lo = 0, hi = sorted.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (sorted[mid] < x) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

module.exports = { SrcMap };
