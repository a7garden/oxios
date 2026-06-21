// String similarity utilities, ported from files.md (web/lib/similarity.js).
// Source: https://github.com/zakirullin/files.md
// License: MIT
//
// Used for fuzzy file search in the knowledge base search modal.

/** Return similarity between two strings, 0–100. */
export function similarity(s1: string, s2: string): number {
  const distance = levenshtein(s1.toLowerCase(), s2.toLowerCase())
  const maxLength = Math.max(s1.length, s2.length)
  if (maxLength === 0) return 100
  return Number(((1 - distance / maxLength) * 100).toFixed(2))
}

/** Compute Levenshtein edit distance between two strings. */
export function levenshtein(s1: string, s2: string): number {
  if (s1 === s2) return 0
  const n = s1.length
  const m = s2.length
  if (n === 0 || m === 0) return n + m

  let x = 0
  let y: number
  let a: number
  let b: number
  let c: number
  let d: number
  let g: number
  let h: number
  let k: number
  const p = new Array<number>(n)
  for (y = 0; y < n; ) {
    p[y] = ++y
  }

  for (; x + 3 < m; x += 4) {
    const e1 = s2.charCodeAt(x)
    const e2 = s2.charCodeAt(x + 1)
    const e3 = s2.charCodeAt(x + 2)
    const e4 = s2.charCodeAt(x + 3)
    c = x
    b = x + 1
    d = x + 2
    g = x + 3
    h = x + 4
    for (y = 0; y < n; y++) {
      k = s1.charCodeAt(y)
      a = p[y]!
      if (a < c || b < c) {
        c = a > b ? b + 1 : a + 1
      } else {
        if (e1 !== k) c++
      }
      if (c < b || d < b) {
        b = c > d ? d + 1 : c + 1
      } else {
        if (e2 !== k) b++
      }
      if (b < d || g < d) {
        d = b > g ? g + 1 : b + 1
      } else {
        if (e3 !== k) d++
      }
      if (d < g || h < g) {
        g = d > h ? h + 1 : d + 1
      } else {
        if (e4 !== k) g++
      }
      p[y] = h = g
      g = d
      d = b
      b = c
      c = a
    }
  }

  for (; x < m; ) {
    const e = s2.charCodeAt(x)
    c = x
    d = ++x
    for (y = 0; y < n; y++) {
      a = p[y]!
      if (a < c || d < c) {
        d = a > d ? d + 1 : a + 1
      } else {
        if (e !== s1.charCodeAt(y)) {
          d = c + 1
        } else {
          d = c
        }
      }
      p[y] = d
      c = a
    }
    h = d
  }

  return h!
}
