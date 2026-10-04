# AGENTS.md

Working rules for agent sessions and humans changing this code.

## What this is

A Rust library that renders WinUI 3 / Fluent 2-flavored XAML with GPUI.
Currently a feasibility PoC (M0). Read in this order: [PRD.md](PRD.md)
(requirements, authoritative) → [PLAN.md](PLAN.md) (architecture, dependency
policy) → [FEASIBILITY.md](FEASIBILITY.md) (PoC evidence, friction list).
The milestone ladder is PRD §15; do not build past the current milestone.

## Prime rules

1. **YAGNI.** Build what the current milestone requires. Nothing "for later".
2. **KISS.** Simplest thing that satisfies the requirement. Stdlib over new
   dependencies; concrete types over generics; a function over a framework.
3. **Plain English.** Docs, comments, commit messages, diagnostics — short
   sentences, active voice, no jargon beyond WinUI/Rust terminology.
4. **No marketing slop.** Banned in all project writing: blazingly, powerful,
   seamless, robust, elegant, world-class, supercharge, unlock, cutting-edge.
   State the fact instead ("parses ≥ 1 MB/s", "launch ≤ 300 ms").
5. **Claims are tagged.** Every capability claim in a doc is [V] verified
   against a source, [D] a decision we own, or [Q] an open question (PRD
   evidence policy). No unmarked claims.

## YAGNI check — run before adding anything

- Which PRD requirement or milestone asks for this? None → don't build it;
  write it in the backlog instead.
- Is there a caller today? A `pub` item with no caller is dead API.
- Would a caller ever pass a different value? No → hardcode it; don't add a
  parameter, config option, or generic.
- Does the second use case exist? No → copy the small thing and extract on
  the third (rule of three). One control does not justify a framework.
- New dependency? Stdlib first. A new crate needs its reason written next to
  the Cargo.toml entry.

## Rust rules

Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
(naming C-CASE/C-GETTER, errors C-GOOD-ERR, docs C-DOC). Project specifics:

- Edition 2024. `cargo fmt`, `cargo clippy --all-targets`, and `cargo test`
  all clean before every commit. No lint allowlists without a written reason.
- Errors: `Result` with descriptive messages; no `unwrap`/`expect` in library
  code — `expect` only in `main`, tests, and startup. XAML failures become
  diagnostics (WX codes from M1), never panics and never silent fallbacks.
- `pub` is an API promise. Default private; expose only what a consumer needs.
- No `unsafe` in our crates — gpui owns the unsafe.
- XAML names (elements, attributes) stay WinUI-exact — that is the
  compatibility surface. Rust-side names are idiomatic Rust, not C# mirrors.
- gpui-pre is caret-tracked `"0.3"` (PLAN §1): periodic `cargo update` + full
  test + window-render check; gpui types stay confined to the backend crate.

## Library structure (how to extend)

- Adding a control = one render function + one dispatch arm in
  `render_element` (the registry replaces the match at M1, PLAN §3). Write
  the third control before extracting shared machinery.
- The model/syntax layers must compile without gpui (backend isolation).
- Fail loud: unknown XAML constructs produce diagnostics with file/line at
  M1 (PRD REQ-DIAG-02); the PoC's stderr warnings are the interim.
- Event plumbing (`cx.notify`) is the library's job; control authors never
  call it by hand (PRD REQ-EVT-01).

## Comments

A comment earns its line only by stating a constraint the code cannot
express. Write the code, mentally delete the comment, and if nothing is
lost, keep it deleted. No comments that:

- narrate what the next line does;
- address a reviewer ("this fixes…", "note that…", "deliberately…");
- record history or future plans (git and the PRD hold those);
- contain commented-out code;
- are TODOs without a milestone tag (`TODO(M1):` if it must exist).

`///` docs on `pub` items are documentation, not comments: one or two
sentences stating the contract, on every pub item. WinUI semantic
constraints (defaults, quirks, crash behaviors) are the comment class worth
keeping — cite the rule ("WinUI defaults `{x:Bind}` to OneTime").

## Docs and claims

- Update the doc that owns the fact in the same change that changes
  behavior; stale claims get corrected the same turn, never left standing.
- Mistakes and incidents get recorded in FEASIBILITY.md's friction list —
  named pattern plus root cause, not just "fixed".
- Verification habit: never state build/test/benchmark status without having
  run it in the current session. Perf budgets (PRD §13) ship benchmarks with
  the feature, not after.

## Commits

Imperative subject ≤ 72 chars ("Add X", "Fix Y"). Body: what changed and why,
in plain sentences. No noise commits.
