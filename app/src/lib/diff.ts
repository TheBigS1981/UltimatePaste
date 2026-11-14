// Word-level diff for the correction view. Changes can be accepted or rejected one by one.

export type Seg = { kind: "eq"; text: string } | { kind: "chg"; del: string; add: string; on: boolean };

const TOKEN = /\s+|[\p{L}\p{N}_'’-]+|[^\s\p{L}\p{N}_]/gu;
const tokens = (s: string) => s.match(TOKEN) ?? [];

/** Diff of two texts as equal runs and change hunks (removed + added text). */
export function diffWords(a: string, b: string): Seg[] {
  const x = tokens(a);
  const y = tokens(b);
  // Common prefix / suffix keep the LCS table small.
  let pre = 0;
  while (pre < x.length && pre < y.length && x[pre] === y[pre]) pre++;
  let suf = 0;
  while (suf < x.length - pre && suf < y.length - pre && x[x.length - 1 - suf] === y[y.length - 1 - suf]) suf++;
  const xs = x.slice(pre, x.length - suf);
  const ys = y.slice(pre, y.length - suf);
  const ops: { k: "eq" | "del" | "add"; t: string }[] = x.slice(0, pre).map((t) => ({ k: "eq" as const, t }));
  const n = xs.length;
  const m = ys.length;
  if (n * m > 4_000_000) {
    xs.forEach((t) => ops.push({ k: "del", t }));
    ys.forEach((t) => ops.push({ k: "add", t }));
  } else {
    // LCS lengths from the end: L[i][j] = LCS of xs[i..], ys[j..]
    const L = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1));
    for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--) L[i][j] = xs[i] === ys[j] ? L[i + 1][j + 1] + 1 : Math.max(L[i + 1][j], L[i][j + 1]);
    let i = 0;
    let j = 0;
    while (i < n || j < m) {
      if (i < n && j < m && xs[i] === ys[j]) ops.push({ k: "eq", t: xs[i++] }), j++;
      else if (j < m && (i === n || L[i][j + 1] >= L[i + 1][j])) ops.push({ k: "add", t: ys[j++] });
      else ops.push({ k: "del", t: xs[i++] });
    }
  }
  x.slice(x.length - suf).forEach((t) => ops.push({ k: "eq", t }));

  // Group into segments; a change separated from the next one only by whitespace is merged,
  // so "a example" -> "an example" is one clickable change rather than many pieces.
  const segs: Seg[] = [];
  for (const o of ops) {
    const last = segs[segs.length - 1];
    if (o.k === "eq") {
      if (last?.kind === "eq") last.text += o.t;
      else segs.push({ kind: "eq", text: o.t });
    } else {
      let target = last;
      if (last?.kind === "eq" && /^\s+$/.test(last.text) && segs[segs.length - 2]?.kind === "chg") {
        segs.pop();
        target = segs[segs.length - 1];
        if (target.kind === "chg") (target.del += last.text), (target.add += last.text);
      }
      if (target?.kind === "chg") {
        if (o.k === "del") target.del += o.t;
        else target.add += o.t;
      } else segs.push({ kind: "chg", del: o.k === "del" ? o.t : "", add: o.k === "add" ? o.t : "", on: true });
    }
  }
  return segs;
}

/** Text with accepted changes applied (rejected ones keep the original). */
export const applySegs = (segs: Seg[]) => segs.map((s) => (s.kind === "eq" ? s.text : s.on ? s.add : s.del)).join("");

export const changeCount = (segs: Seg[]) => segs.filter((s) => s.kind === "chg").length;
