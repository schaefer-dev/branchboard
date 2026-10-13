import type { Token, TokenKind } from "../../lib/api";

export interface Segment {
  text: string;
  kind: TokenKind | null;
}

/** Splits a line into plain and highlighted segments along its tokens. */
export function segments(text: string, tokens: readonly Token[]): Segment[] {
  if (tokens.length === 0) return [{ text, kind: null }];
  const result: Segment[] = [];
  let position = 0;
  for (const token of tokens) {
    const start = Math.max(token.start, position);
    const end = Math.min(token.end, text.length);
    if (start >= end) continue;
    if (start > position) result.push({ text: text.slice(position, start), kind: null });
    result.push({ text: text.slice(start, end), kind: token.kind });
    position = end;
  }
  if (position < text.length) result.push({ text: text.slice(position), kind: null });
  return result;
}
