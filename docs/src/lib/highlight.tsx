// phogbank patchlang syntax highlighter — regex-based, light touch.
import { Fragment, type ReactNode } from 'react';

type TokenType =
  | 'kw'    // keyword: patch, effect, phrase, voices, poly, mono
  | 'glob'  // node constructors: osc, adsr, lpf, hpf, delay, mix …
  | 'val'   // enum values: sine, square, saw, triangle, eighth, quarter …
  | 'num'   // numeric literal
  | 'cm'    // comment (# or --)
  | 'punct' // { } [ ] ( ) , =
  | 'op'    // | * + - /
  | 'id'    // identifier / param name
  | 'ws';   // whitespace

interface Token {
  t: TokenType;
  v: string;
}

// top-level structural keywords
const PB_KEYWORDS: ReadonlySet<string> = new Set([
  'patch', 'effect', 'phrase', 'voices', 'poly', 'mono', 'out',
]);

// node constructors — appear before { ... }
const PB_NODES: ReadonlySet<string> = new Set([
  'osc', 'adsr', 'lpf', 'hpf', 'bpf', 'notch', 'delay', 'mix', 'amp', 'lfo',
]);

// enum / value literals (waveforms, durations, filter types)
const PB_VALUES: ReadonlySet<string> = new Set([
  'sine', 'square', 'saw', 'triangle',
  'eighth', 'quarter', 'half', 'whole',
]);

/** Tokenize a single line. */
export function pbTokens(line: string): Token[] {
  const out: Token[] = [];
  let i = 0;
  const n = line.length;

  while (i < n) {
    const c = line[i];

    // line comment: //
    if (c === '/' && line[i + 1] === '/') {
      out.push({ t: 'cm', v: line.slice(i) });
      return out;
    }

    // numbers (incl. decimals and negatives already handled as op + num)
    if (/[0-9]/.test(c)) {
      let j = i + 1;
      while (j < n && /[0-9.]/.test(line[j])) j += 1;
      out.push({ t: 'num', v: line.slice(i, j) });
      i = j;
      continue;
    }

    // identifiers / keywords / nodes / values
    if (/[A-Za-z_]/.test(c)) {
      let j = i + 1;
      while (j < n && /[A-Za-z_0-9]/.test(line[j])) j += 1;
      const w = line.slice(i, j);
      let cls: TokenType = 'id';
      if (PB_KEYWORDS.has(w)) cls = 'kw';
      else if (PB_NODES.has(w))   cls = 'glob';
      else if (PB_VALUES.has(w))  cls = 'val';
      out.push({ t: cls, v: w });
      i = j;
      continue;
    }

    // whitespace
    if (/\s/.test(c)) {
      let j = i + 1;
      while (j < n && /\s/.test(line[j])) j += 1;
      out.push({ t: 'ws', v: line.slice(i, j) });
      i = j;
      continue;
    }

    // punctuation vs operators
    if (/[{}()[\],]/.test(c)) out.push({ t: 'punct', v: c });
    else if (/[|*+\-/]/.test(c)) out.push({ t: 'op', v: c });
    else if (c === '=') out.push({ t: 'punct', v: c });
    else out.push({ t: 'op', v: c });
    i += 1;
  }

  return out;
}

/** Render highlighted code to React nodes given a code string. */
export function highlightPb(code: string): ReactNode {
  const lines = code.split('\n');
  return lines.map((line, li) => {
    const toks = pbTokens(line);
    return (
      <Fragment key={li}>
        {toks.map((tk, ti) =>
          tk.t === 'ws'
            ? tk.v
            : <span key={ti} className={tk.t}>{tk.v}</span>,
        )}
        {li < lines.length - 1 ? '\n' : ''}
      </Fragment>
    );
  });
}
