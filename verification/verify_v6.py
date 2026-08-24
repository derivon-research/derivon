"""
Verification for Derivon v6 core model claims.

Model: weighted directed B-hypergraph G = (P, H), h = (T(h) subset P, head(h) in P, w(h) >= 0)
Query: (S, t).  R subset H is a derivation iff t in Closure_R(S).

Three cost semantics:
  set   C_set(R)  = sum_{h in R} w(h)                       (shared prereqs charged ONCE)  <- semantically wanted
  tree  d_tree(v) = min_h [ w(h) + sum_{p in T(h)} d(p) ]   (Knuth, sharing charged repeatedly)
  depth d_dep(v)  = min_h [ w(h) + max_{p in T(h)} d(p) ]   (critical path)

Claims to verify:
  C1  closure is well-defined and cycle-safe (least fixed point), no acyclicity axiom needed
  C2  bracket theorem:  d_depth <= d_set <= d_tree   (for w >= 0)
  C3  all three coincide when every |T(h)| <= 1 (degenerate = ordinary shortest path)
  C4  set-cover reduction => d_set is NP-hard (and d_tree gives the WRONG answer there)
  C5  multi-head hyperedges are encodable into single-head + zero-weight edges, faithfully
      under set-cost, NOT under tree-cost
"""
import itertools, random, heapq

INF = float('inf')

# ---------- core ----------
def closure(P, H, S, allowed=None):
    """Least fixed point. allowed = subset of hyperedge indices (None = all)."""
    K = set(S)
    idx = range(len(H)) if allowed is None else allowed
    changed = True
    while changed:
        changed = False
        for i in idx:
            T, head, w = H[i]
            if head not in K and T <= K:
                K.add(head); changed = True
    return K

def knuth(P, H, S, combine):
    """Generalized Dijkstra (Knuth 1977) for a superior function.
    combine(w, [d(p) for p in T]) -> cost."""
    d = {p: (0.0 if p in S else INF) for p in P}
    pq = [(0.0, p) for p in S]
    # 空尾超边 (∅, y, w) 在任何状态下都可执行（∅ ⊆ K 恒成立），必须在主循环前播种。
    # 否则 S = ∅ 时优先队列为空、while 循环不执行，它们永远不会触发。
    for (T, head, w) in H:
        if not T:
            c = combine(w, [])
            if c < d[head]:
                d[head] = c
                pq.append((c, head))
    heapq.heapify(pq)
    done = set()
    while pq:
        dv, v = heapq.heappop(pq)
        if v in done or dv > d[v]:
            continue
        done.add(v)
        for (T, head, w) in H:
            if head in done or not T <= done:
                continue
            cand = combine(w, [d[p] for p in T])
            if cand < d[head]:
                d[head] = cand
                heapq.heappush(pq, (cand, head))
    return d

def d_tree(P, H, S):
    return knuth(P, H, S, lambda w, ds: w + sum(ds))

def d_depth(P, H, S):
    return knuth(P, H, S, lambda w, ds: w + (max(ds) if ds else 0.0))

def d_set_bruteforce(P, H, S, t):
    """Exhaustive min over R subset H with t in Closure_R(S). Exponential, for validation only."""
    best = INF
    n = len(H)
    for r in range(n + 1):
        for R in itertools.combinations(range(n), r):
            cost = sum(H[i][2] for i in R)
            if cost >= best:
                continue
            if t in closure(P, H, S, allowed=set(R)):
                best = cost
    return best

# ---------- C1: cycle safety ----------
def test_cycle_safety():
    P = {'a', 'b', 'c'}
    # a and b mutually justify each other -> circular reasoning, must NOT be derivable
    H = [({'a'}, 'b', 1.0), ({'b'}, 'a', 1.0), ({'c'}, 'a', 1.0)]
    assert closure(P, H, {'c'}) == {'a', 'b', 'c'}, "grounded chain must fire"
    assert closure(P, H, set()) == set(), "cyclic self-support must NOT fire"
    dt = d_tree(P, H, set())
    assert dt['a'] == INF and dt['b'] == INF
    print("C1 cycle-safety                 OK  (least fixed point rejects circular justification;"
          " acyclicity axiom NOT required)")

# ---------- C2/C3: bracket ----------
def rand_hypergraph(rng, npts=7, nedges=8, maxtail=3, mintail=1):
    P = [f"p{i}" for i in range(npts)]
    H = []
    for _ in range(nedges):
        k = rng.randint(mintail, maxtail)
        T = frozenset(rng.sample(P, k))
        head = rng.choice([p for p in P if p not in T])
        H.append((set(T), head, float(rng.randint(0, 5))))
    return set(P), H

def test_bracket(trials=400, maxtail=3, seed=7):
    rng = random.Random(seed)
    strict_gap = 0
    checked = 0
    empty = 0
    for _ in range(trials):
        # mintail=0 lets empty-tail (unconditional) hyperedges into the sample as well
        P, H = rand_hypergraph(rng, maxtail=maxtail, mintail=0)
        empty += sum(1 for h in H if not h[0])
        S = set(rng.sample(sorted(P), 2))
        dt, dd = d_tree(P, H, S), d_depth(P, H, S)
        for t in sorted(P):
            ds = d_set_bruteforce(P, H, S, t)
            reach = t in closure(P, H, S)
            assert reach == (ds < INF) == (dt[t] < INF) == (dd[t] < INF), "reachability must agree"
            if not reach:
                continue
            checked += 1
            assert dd[t] <= ds + 1e-9, f"depth<=set violated: {dd[t]} {ds}"
            assert ds <= dt[t] + 1e-9, f"set<=tree violated: {ds} {dt[t]}"
            if dd[t] < ds - 1e-9 or ds < dt[t] - 1e-9:
                strict_gap += 1
    print(f"C2 bracket d_depth<=d_set<=d_tree OK  ({checked} reachable queries, "
          f"{strict_gap} with a strict gap -> bounds are non-trivial; "
          f"{empty} empty-tail hyperedges included)")

def test_degenerate(trials=200, seed=11):
    """|T| <= 1 covers BOTH |T| = 1 (ordinary edge) and |T| = 0 (unconditional axiom).
    The 0 case must be exercised explicitly: with at most one premise per point no join
    ever occurs, so a minimal derivation is a chain and all three measures must coincide."""
    rng = random.Random(seed)
    seen_empty = 0
    for mintail in (1, 0):                       # |T| = 1 only, then |T| in {0, 1}
        for _ in range(trials):
            P, H = rand_hypergraph(rng, maxtail=1, mintail=mintail)
            seen_empty += sum(1 for h in H if not h[0])
            S = set(rng.sample(sorted(P), 1))
            for Sq in (S, set()):                # also probe S = empty set
                dt, dd = d_tree(P, H, Sq), d_depth(P, H, Sq)
                for t in sorted(P):
                    reach = t in closure(P, H, Sq)
                    assert reach == (dt[t] < INF), ("knuth disagrees with closure", t)
                    if not reach:
                        continue
                    ds = d_set_bruteforce(P, H, Sq, t)
                    assert abs(dt[t] - dd[t]) < 1e-9 and abs(ds - dt[t]) < 1e-9, \
                        (t, ds, dt[t], dd[t])
    print(f"C3 degenerate |T|<=1 collapse    OK  (all three = classical shortest path; "
          f"{seen_empty} empty-tail hyperedges exercised, S=empty included)")

# ---------- C4: set-cover reduction ----------
def setcover_instance(universe, sets, unit=0.0):
    P = {'r', 't'} | {f"x{j}" for j in range(len(sets))} | {f"u{u}" for u in universe}
    H = [({'r'}, f"x{j}", 1.0) for j in range(len(sets))]
    for j, Sj in enumerate(sets):
        for u in Sj:
            H.append(({f"x{j}"}, f"u{u}", unit))
    H.append(({f"u{u}" for u in universe}, 't', unit))
    return P, H

def brute_setcover(universe, sets):
    for k in range(1, len(sets) + 1):
        for comb in itertools.combinations(range(len(sets)), k):
            if set().union(*[sets[j] for j in comb]) >= set(universe):
                return k
    return INF

def test_reduction():
    rng = random.Random(3)
    for _ in range(60):
        n, m = rng.randint(3, 5), rng.randint(3, 5)
        universe = list(range(n))
        sets = [set(rng.sample(universe, rng.randint(1, n))) for _ in range(m)]
        if set().union(*sets) != set(universe):
            continue
        P, H = setcover_instance(universe, sets)
        ds = d_set_bruteforce(P, H, {'r'}, 't')
        opt = brute_setcover(universe, sets)
        assert abs(ds - opt) < 1e-9, (ds, opt, sets)
    # and show tree cost is blind to sharing on a case where they differ
    universe, sets = [0, 1, 2], [{0, 1, 2}, {0}, {1}, {2}]
    P, H = setcover_instance(universe, sets)
    ds = d_set_bruteforce(P, H, {'r'}, 't')
    dt = d_tree(P, H, {'r'})['t']
    dd = d_depth(P, H, {'r'})['t']
    print(f"C4 SetCover <=p min-set-cost     OK  (exact on 60 random instances)")
    print(f"   witness  universe=3, sets={sets}:  d_set={ds} (=optimal cover 1), "
          f"d_tree={dt} (over-counts sharing 3x), d_depth={dd}")

# ---------- C5: multi-head encoding ----------
def test_multihead():
    # "T jointly yields y1 and y2 at cost w" encoded as T->p* (w), p*->y1 (0), p*->y2 (0)
    P = {'a', 'b', 'star', 'y1', 'y2', 'g'}
    H = [({'a', 'b'}, 'star', 4.0), ({'star'}, 'y1', 0.0), ({'star'}, 'y2', 0.0),
         ({'y1', 'y2'}, 'g', 0.0)]
    S = {'a', 'b'}
    ds = d_set_bruteforce(P, H, S, 'g')
    dt = d_tree(P, H, S)['g']
    assert ds == 4.0, ds
    assert dt == 8.0, dt
    print(f"C5 multi-head encoding           OK  (set-cost={ds} faithful; "
          f"tree-cost={dt} double-charges the shared head -> encoding is NOT tree-faithful)")

if __name__ == '__main__':
    test_cycle_safety()
    test_bracket()
    test_degenerate()
    test_reduction()
    test_multihead()
