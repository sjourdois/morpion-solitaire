#!/usr/bin/env python3
"""Transcribe a Morpion Solitaire record *grid image* (as published on
morpionsolitaire.com) into a move list our engine can verify.

The site publishes 4T/4D (and some 5D) records only as images — numbered move
circles + drawn lines on a dot lattice, over the initial cross. This recovers a
playable game from one such image:

  1. detect the move circles' numbers (connected-component digits + tesseract);
  2. fit the dot lattice and detect the initial-cross dots;
  3. translate so the detected cross matches our canonical cross for the variant;
  4. reconstruct each move's *line* from the ordered points alone, using the
     variant's overlap rule (T allows sharing one endpoint, D none) with
     backtracking — the drawn lines themselves are never read;
  5. emit a JSON record (`--to msr` + `verify` it with the CLI).

Only points + order are read from the image; the lines are derived, so the
tangle of overlapping segments never has to be parsed. OCR slips are reported
and can be corrected with `--fix "cx,cy=NUM"` overrides.

Usage:
    python3 tools/grid_to_msr.py IMAGE --variant 4T --out game.json \
        [--fix "382,104=11" ...]
Requires: pillow, numpy, tesseract (binary).
"""
import argparse, json, subprocess, tempfile, os, sys
from collections import deque, Counter
import numpy as np
from PIL import Image

# Engine direction deltas (must match morpion_solitaire::game::line::Dir).
DIRS = [("H", (1, 0)), ("V", (0, 1)), ("DP", (1, -1)), ("DN", (1, 1))]


def variant_params(variant):
    n = 5 if variant[0] == "5" else 4
    max_overlap = 1 if variant[1] == "T" else 0  # T: touch (share 1); D: disjoint
    return n, max_overlap


def canonical_cross(n):
    """Centred D4-symmetric hollow Greek cross — matches both crates' setup."""
    arm = n - 1
    w = (2 * n - 1) if n % 2 else (2 * n - 2)
    a = (w - (arm - 1)) // 2
    b = a + arm - 1
    cells = set()
    for x in range(w + 1):
        for y in range(w + 1):
            if (((y in (0, w)) and a <= x <= b)
                    or ((x in (0, w)) and a <= y <= b)
                    or ((x in (a, b)) and (y <= a or y >= b))
                    or ((y in (a, b)) and (x <= a or x >= b))):
                cells.add((x, y))
    return cells


def components(mask):
    H, W = mask.shape
    vis = np.zeros_like(mask)
    nb = ((1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1))
    out = []
    for y0, x0 in np.argwhere(mask):
        if vis[y0, x0]:
            continue
        q = deque([(y0, x0)]); vis[y0, x0] = True; pix = []
        while q:
            y, x = q.popleft(); pix.append((y, x))
            for dy, dx in nb:
                ny, nx = y + dy, x + dx
                if 0 <= ny < H and 0 <= nx < W and mask[ny, nx] and not vis[ny, nx]:
                    vis[ny, nx] = True; q.append((ny, nx))
        xs = [p[1] for p in pix]; ys = [p[0] for p in pix]
        out.append(dict(n=len(pix), cx=sum(xs) / len(pix), cy=sum(ys) / len(pix),
                        x0=min(xs), y0=min(ys), x1=max(xs), y1=max(ys)))
    return out


def ocr(im, box):
    x0, y0, x1, y1 = box; pad = 5
    crop = im.crop((x0 - pad, y0 - pad, x1 + pad, y1 + pad))
    crop = crop.resize(((x1 - x0 + 2 * pad) * 6, (y1 - y0 + 2 * pad) * 6))
    with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as f:
        crop.save(f.name); p = f.name
    r = subprocess.run(["tesseract", p, "-", "--psm", "8",
                        "-c", "tessedit_char_whitelist=0123456789"],
                       capture_output=True, text=True)
    os.unlink(p)
    return r.stdout.strip().replace(" ", "").replace("\n", "")


def cluster1d(vals, gap):
    s = sorted(vals); cl = [[s[0]]]
    for v in s[1:]:
        (cl[-1] if v - cl[-1][-1] <= gap else cl.append([v]) or cl[-1]).append(v)
    return [sum(c) / len(c) for c in cl]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("image")
    ap.add_argument("--variant", required=True, choices=["4T", "4D", "5T", "5D"])
    ap.add_argument("--out", required=True)
    ap.add_argument("--fix", action="append", default=[],
                    help='OCR override "cx,cy=NUM" (cx,cy = printed move centre)')
    ap.add_argument("--author", default="")
    ap.add_argument("--source", default="")
    ap.add_argument("--description", default="")
    ap.add_argument("--ink", type=int, default=140,
                    help="gray threshold for move-number ink (raise for light/"
                         "coloured numbers, e.g. 220)")
    args = ap.parse_args()
    n, max_overlap = variant_params(args.variant)
    fixes = {}
    for f in args.fix:
        pos, num = f.split("="); cx, cy = pos.split(","); fixes[(int(cx), int(cy))] = int(num)

    im = Image.open(args.image).convert("L")
    a = np.array(im)
    dark = a < 110          # filled cross dots (always black)
    ink = a < args.ink      # move-number ink (may be light/coloured)

    # Digits → numbers (move circles).
    small = [c for c in components(ink)
             if 6 <= c["n"] < 90 and (c["x1"] - c["x0"]) < 22 and (c["y1"] - c["y0"]) < 24]
    small.sort(key=lambda c: (round(c["cy"] / 12), c["cx"]))
    used = [False] * len(small); groups = []
    for i, c in enumerate(small):
        if used[i]:
            continue
        g = [c]; used[i] = True
        for j, d in enumerate(small):
            if not used[j] and abs(d["cy"] - c["cy"]) < 10 and abs(d["cx"] - c["cx"]) < 18:
                g.append(d); used[j] = True
        groups.append(g)
    moves = []
    for g in groups:
        box = (min(c["x0"] for c in g), min(c["y0"] for c in g),
               max(c["x1"] for c in g), max(c["y1"] for c in g))
        cx = round(sum(c["cx"] for c in g) / len(g))
        cy = round(sum(c["cy"] for c in g) / len(g))
        num = fixes.get((cx, cy))
        if num is None:
            t = ocr(im, box); num = int(t) if t.isdigit() else None
        moves.append([num, cx, cy])

    nums = [m[0] for m in moves]
    cnt = Counter(x for x in nums if x is not None)
    bad = [m for m in moves if m[0] is None or m[0] < 1 or cnt[m[0]] > 1]
    if bad or len(moves) != max(cnt) if cnt else True:
        N = len(moves)
        got = set(x for x in nums if x is not None and 1 <= x <= N)
        miss = sorted(set(range(1, N + 1)) - got)
        print(f"[detect] {len(moves)} move circles; missing {miss}", file=sys.stderr)
        for m in bad:
            print(f"  fix needed near ({m[1]},{m[2]}): OCR={m[0]}", file=sys.stderr)
        if bad:
            print("Re-run with --fix \"cx,cy=NUM\" for each.", file=sys.stderr)
            sys.exit(2)

    # Lattice from move centres.
    sx = np.median(np.diff(cluster1d([m[1] for m in moves], 18)))
    sy = np.median(np.diff(cluster1d([m[2] for m in moves], 16)))
    ox = min(m[1] for m in moves); oy = min(m[2] for m in moves)
    lat = lambda px, py: (round((px - ox) / sx), round((py - oy) / sy))
    mv = {m[0]: lat(m[1], m[2]) for m in moves}
    moveset = set(mv.values())
    assert len(moveset) == len(moves), "two moves landed on one lattice cell"

    # Cross dots on the lattice (filled centres, not move circles).
    cmin = min(c for c, _ in moveset); cmax = max(c for c, _ in moveset)
    rmin = min(r for _, r in moveset); rmax = max(r for _, r in moveset)
    cross_img = set()
    for c in range(cmin - 2, cmax + 3):
        for r in range(rmin - 2, rmax + 3):
            px, py = ox + c * sx, oy + r * sy
            win = dark[int(py) - 3:int(py) + 4, int(px) - 3:int(px) + 4]
            if win.size and win.mean() > 0.75 and (c, r) not in moveset:
                cross_img.add((c, r))

    OUR = canonical_cross(n)
    dx = min(x for x, _ in OUR) - min(x for x, _ in cross_img)
    dy = min(y for _, y in OUR) - min(y for _, y in cross_img)
    tcross = {(x + dx, y + dy) for x, y in cross_img}
    if tcross != OUR:
        print(f"[cross] detected {len(cross_img)} cross dots; does NOT match the "
              f"canonical {len(OUR)}-point cross after translation.", file=sys.stderr)
        print(f"        extra={sorted(tcross - OUR)} missing={sorted(OUR - tcross)}",
              file=sys.stderr)
        sys.exit(3)
    mv = {k: (x + dx, y + dy) for k, (x, y) in mv.items()}

    # Reconstruct each move's line from the ordered points (backtracking).
    order = [mv[k] for k in range(1, len(mv) + 1)]

    def overlaps(win, segs):  # pairwise overlap vs existing same-line segments
        return max((len(win & s) for s in segs), default=0)

    sol = [None]

    def dfs(k, placed, segs, acc):
        if sol[0]:
            return
        if k == len(order):
            sol[0] = list(acc); return
        P = order[k]; placed.add(P)
        for dn, (ddx, ddy) in DIRS:
            for s in range(-(n - 1), 1):
                win = [(P[0] + (s + i) * ddx, P[1] + (s + i) * ddy) for i in range(n)]
                if P not in win or not all(c in placed for c in win):
                    continue
                ws = frozenset(win)
                if overlaps(ws, segs.get(dn, [])) > max_overlap:
                    continue
                segs.setdefault(dn, []).append(ws)
                acc.append({"x": P[0], "y": P[1], "dir": dn, "pos": -s})
                dfs(k + 1, placed, segs, acc)
                acc.pop(); segs[dn].pop()
                if sol[0]:
                    return
        placed.discard(P)

    dfs(0, set(OUR), {}, [])
    if not sol[0]:
        print("[reconstruct] no legal line assignment found — check the move "
              "order/positions.", file=sys.stderr)
        sys.exit(4)

    game = {"version": 1, "variant": args.variant, "score": len(order),
            "available_moves": 0, "terminal": True, "bbox": [0, 0, 0, 0],
            "moves": sol[0]}
    for k, v in (("author", args.author), ("source", args.source),
                 ("description", args.description)):
        if v:
            game[k] = v
    json.dump(game, open(args.out, "w"))
    print(f"[ok] {len(order)} moves -> {args.out}")


if __name__ == "__main__":
    main()
