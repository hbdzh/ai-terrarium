export interface DiffLine {
  type: "same" | "added" | "removed";
  text: string;
}

/**
 * Lightweight line-level LCS diff for highlighting Shared Board changes.
 */
export function computeLineDiff(oldText: string, newText: string): DiffLine[] {
  const a = oldText.split(/\r?\n/);
  const b = newText.split(/\r?\n/);
  const m = a.length;
  const n = b.length;

  // Guard against overly huge inputs by falling back to simple diff
  if (m * n > 250_000) {
    return [
      ...a.map((text): DiffLine => ({ type: "removed", text })),
      ...b.map((text): DiffLine => ({ type: "added", text })),
    ];
  }

  const dp: number[][] = Array.from({ length: m + 1 }, () => new Array(n + 1).fill(0));
  for (let i = m - 1; i >= 0; i--) {
    for (let j = n - 1; j >= 0; j--) {
      if (a[i] === b[j]) {
        dp[i][j] = dp[i + 1][j + 1] + 1;
      } else {
        dp[i][j] = Math.max(dp[i + 1][j], dp[i][j + 1]);
      }
    }
  }

  const out: DiffLine[] = [];
  let i = 0;
  let j = 0;
  while (i < m && j < n) {
    if (a[i] === b[j]) {
      out.push({ type: "same", text: a[i] });
      i++;
      j++;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      out.push({ type: "removed", text: a[i] });
      i++;
    } else {
      out.push({ type: "added", text: b[j] });
      j++;
    }
  }
  while (i < m) {
    out.push({ type: "removed", text: a[i++] });
  }
  while (j < n) {
    out.push({ type: "added", text: b[j++] });
  }
  return out;
}
