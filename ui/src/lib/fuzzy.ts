// Small fuzzy matcher for the quick switcher: characters of the query must
// appear in order. Matches at word starts and consecutive runs score higher.

const WORD_SEPARATORS = new Set([" ", "/", "\\", "-", "_", "."]);

/** Score of `query` against `text`, or `null` if it does not match. Higher is better. */
export function fuzzyScore(query: string, text: string): number | null {
  const needle = query.trim().toLowerCase();
  if (needle === "") return 0;
  const haystack = text.toLowerCase();

  let score = 0;
  let from = 0;
  let previous = -2;
  for (const char of needle) {
    const index = haystack.indexOf(char, from);
    if (index === -1) return null;
    const before = index === 0 ? " " : (haystack[index - 1] ?? " ");
    if (WORD_SEPARATORS.has(before)) score += 8;
    if (index === previous + 1) score += 5;
    score -= Math.min(index - from, 10) * 0.5;
    previous = index;
    from = index + 1;
  }
  // Prefer shorter texts when everything else is equal.
  return score - haystack.length * 0.01;
}

/** Items matching `query`, best first. `fields` returns the texts to match; the best field wins. */
export function fuzzyFilter<T>(
  items: readonly T[],
  query: string,
  fields: (item: T) => string[],
): T[] {
  const scored: { item: T; score: number }[] = [];
  for (const item of items) {
    let best: number | null = null;
    for (const field of fields(item)) {
      const score = fuzzyScore(query, field);
      if (score !== null && (best === null || score > best)) best = score;
    }
    if (best !== null) scored.push({ item, score: best });
  }
  return scored.sort((a, b) => b.score - a.score).map(({ item }) => item);
}
