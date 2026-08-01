# Dead islands: the algorithm, written for this codebase

How Kotlin Jump finds groups of declarations that reference only each other
(22 verified islands on a 6410-file corpus, zero false positives after audit),
and what it would take to do the same here. Written as a porting guide: every
section ends with where the equivalent lives in this repo.

## 1. The problem with `is_referenced`

`Graph::is_referenced` answers "does any edge point at this node" — a LOCAL
question. Two functions calling each other with no outside caller both have an
incoming edge, so both read as alive, forever. The same goes for a chain
behind a dead entry point, an interface plus its only implementation that
nobody constructs, and a class whose only user is another dead class.

`--deep` already computes reachability from entry points, which is the right
GLOBAL question — but measured against a hand-verified corpus it misses 21 of
22 real islands, for two reasons this document exists to fix:

1. **The root set is blessed too broadly.** `--why-alive` on a class with
   `Incoming references: 0` answers "It is itself an entry point — a retention
   root". Blanket ProGuard keeps (`-keep class **.DO.**`), substring-matched
   inheritance (`CardView` contains `View`), and `contains`-matched annotation
   lists each turn whole swaths of the corpus into roots. One wrong root
   resurrects its entire forward closure.
2. **The guards reintroduce the local question.** `should_skip_declaration`
   and friends consult `is_referenced` — "someone references it" — which is
   the exact negation of transitivity. A zombie's referee is skipped because
   the zombie references it.

## 2. The inversion that makes zero-FP possible

Do not enumerate what is ALIVE (a root set of declarations is never complete,
and every omission manufactures a false positive). Enumerate what is
UNPLACEABLE, and call that life:

> **The invariant.** A group of declarations is dead only if EVERY mention of
> every one of its names, in the whole corpus, lies inside the group's own
> declaration extents. Any mention that cannot be attributed — a token in a
> layout XML, a ProGuard rule, a string literal, a file the parser refuses, a
> generated-name pattern like `DaggerX` — is a ROOT, and a root means life.

The direction of every possible error is then fixed by construction:

- extent too small → mention falls outside → root → alive (false negative);
- unparseable file → all its tokens are roots (false negative);
- unresolvable or ambiguous reference → root (false negative);
- homonym → all bearers share one fate, silence over guessing;
- missing node → never a dropped edge (the silent-drop in
  `Graph::add_reference` when an endpoint is absent is exactly the bug this
  forbids): the mention becomes a root instead.

Errors amplify LIFE, never death. That is the entire zero-FP argument, and it
is why the 57%-FP failure mode of reachability-from-blessed-roots cannot
happen: there is no code path that converts uncertainty into deadness.

## 3. The algorithm, step by step

Working over names (Kotlin Jump has no resolver; with this repo's tree-sitter
AST and import-aware resolution, nodes can be declarations directly, which is
strictly better — keep the error-direction rules above regardless).

1. **Candidates.** Every declaration that survives the per-node guards
   (annotations, framework supertypes, entry conventions, KMP, suppressions).
   A guarded declaration is NOT a node: its extent leaves the pool, so
   everything its body mentions becomes a root. *One rule replaces a whole
   blessing system: the guard that saves a node automatically roots that
   node's dependencies.*
2. **Extents.** For each candidate, an UNDER-approximated source range. When
   the range cannot be delimited safely, refuse the node (its content then
   roots). Never widen: an over-wide extent that swallows a live neighbor's
   mention is the one error that kills live code. (Found live: a bodyless
   `fun`'s extent grabbing the next declaration's `{`; the fix is a hard
   clamp — any extent crossing a following same-or-shallower declaration is
   refused, never trimmed.)
3. **Attributed sweep.** One pass over every token of every file (comments
   stripped, string literals KEPT — reflection and serialization live there).
   A token of a candidate name inside the innermost candidate extent emits an
   edge `owner-name → mentioned-name`. A token anywhere else — XML, ProGuard,
   Gradle, properties, an ineligible declaration's body, a test file (tallied
   separately) — is a root mention. Accessor conventions count both ways
   (`getFoo` reaches `foo`; for ALL_CAPS properties the accessor keeps the
   name verbatim: `getIGNORED_CHILD_CLASSES` reaches `IGNORED_CHILD_CLASSES`
   — a real false positive until mapped). Generated-name conventions root
   their source: a `DaggerX` / `X_Factory` / `XDirections` token with no
   declaration of its own roots `X`.
4. **Liveness = least fixpoint.** Seed with every root-mentioned name; then
   propagate: if `N` is alive, every name mentioned inside the extents of
   `N`'s bearers is alive. Worklist, O(V+E), each name enqueued once.
   **Do not compute deadness by peeling from proven-dead seeds** — a mutual
   pair never qualifies (each one's mentions sit in the not-yet-dead other)
   and the headline case is lost. Deadness is the COMPLEMENT of the liveness
   fixpoint; equivalently the greatest fixpoint of "all my mentions are
   covered by dead extents", which is the co-induction that kills mutual
   recursion.
5. **Islands = weakly connected components** of the mention relation
   restricted to dead names. WCC, not SCC: the deletable unit must close in
   both directions, because deleting a referee without its referencer breaks
   the build and users will not respect a topological order. Soundness: every
   mention of an island name is attributed to a node of the same component
   (anything else would have made it alive), so the discount set equals the
   deletion set — deleting the island leaves no dangling reference except
   imports, which are collected and deleted with it.
6. **Reporting discipline.** Subsume islands wholly covered by the per-symbol
   detectors (one finding per root cause). An island with any test-side root
   is `testOnly`: reported, never auto-deleted (removing tests is a human
   call). A component larger than a small cap (default 8) is withheld whole —
   a forty-symbol component is more likely an analysis gap than a corpse.
   The fix is island-atomic through a reviewed multi-file change, outer
   extents winning over nested ones (deleting members while the dead class
   shell stays leaves orphan references — found by a red audit build).

## 4. Mapping onto this repo

| Piece | Kotlin Jump | Here |
|---|---|---|
| Node table | name-level candidates | `Graph` declarations (better: real resolution) |
| Edges | offset-attributed token mentions | already resolved references — but **keep ambiguous edges for liveness propagation only, never as the sole life of a name** (`kill_list::forward_closure` already skips them; reachability follows them; unify on: follow for LIFE, never for death) |
| Roots | unattributable mentions | replace the blessing lists: manifest/XML/ProGuard/strings stay roots (the soft-refs machinery in `analysis/risk.rs` already collects most of this — promote it from "risk demotion" to "root") |
| Blessings | none — guards remove nodes, which roots their contents | `ENTRY_ANNOTATIONS` contains-matching, inheritance substring matching, blanket keeps: each should either name a REAL runtime caller or become "node removed ⇒ contents root" |
| Fixpoint | liveness lfp from root mentions | `deep.rs::find_reachable_strict` is already a worklist — the change is the seed set and deleting the `is_referenced` shortcuts from the skip logic |
| Components | union-find over dead names | `kill_list.rs::dead_clusters` already does BFS components over the dead set — it inherits correctness automatically once the dead set is sound |
| Truncation | any unreadable file ⇒ report nothing | no equivalent today; an incomplete parse currently yields findings |

The shortest path here is not a new detector: it is (1) making every root
NAMEABLE and conservative, (2) removing `is_referenced` from every death
decision, (3) treating missing nodes and unresolved references as roots, and
(4) letting the existing reachability + clusters machinery do what it already
does over a sound dead set.

## 5. The validation protocol that earned the numbers

Four tiers, each catching what the previous cannot; a finding ships only when
it survives all four. (1) Adversarial unit suites with anti-silence controls —
every guard has a twin fixture proving it can stay off. (2) 100% manual corpus
audit with whole-word greps, recorded in a committed ground-truth file with a
precision floor (no verified-alive symbol ever reported) and a recall floor
(no verified corpse ever lost) as a permanent CI ratchet. (3) Oracles: the R8
`usage.txt` diff — with the caveat that R8-dead is not always
source-removable (const inlining, `by`-delegation merging) — and cross-tool
triangulation. (4) Delete-and-build: every fixable island actually deleted on
a throwaway branch, `assembleDebug` plus unit tests green, and every red
build converted into a guard plus a regression test named after the real
finding that forced it. Both false positives the audits caught (a duplicated
`@Component` hidden behind guard ordering; the ALL_CAPS accessor) exist today
as guards with tests.
