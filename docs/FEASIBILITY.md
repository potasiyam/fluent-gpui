# FEASIBILITY — WinUI-style XAML on GPUI (PoC results)

**Date:** 2026-10-03 · **Machine:** Windows 11 (build 26300), Rust 1.97.1 ·
**gpui:** 0.2.2 (crates.io) · **Backend:** Blade DX12 + cosmic-text

## Question under test

Can a WinUI-flavored XAML file be parsed at runtime and mapped to GPUI's
element API — including dynamic trees, `{x:Bind}`-style data, and `Click=`
event handlers — with acceptable effort on Windows?

## Test protocol

`assets/demo.xaml` exercises the riskiest mechanics first (each one is a
potential feasibility blocker for the whole idea):

| # | Mechanic | Why it could have failed |
|---|---|---|
| 1 | GPUI builds & opens a window on this machine via plain cargo | Blade/DX12 driver variance; crate's Windows maturity |
| 2 | XAML DOM rendered as a **runtime** element tree | GPUI's API is compile-time-builder-shaped; `AnyElement` erasure must carry nested dynamic trees |
| 3 | `StackPanel` spacing/padding → flex layout | Layout semantics mapping |
| 4 | `TextBlock` FontSize/FontWeight/Foreground, `{x:Bind}` | Text styling through cosmic-text; markup extension hook |
| 5 | `Border` Background/CornerRadius/BorderBrush | Container styling |
| 6 | `Button` with `Click="OnIncrement"` → mutates `AppState` → bound text re-renders | The full loop: markup → event → Rust state → re-render through markup |
| 7 | `#AARRGGBB` WinUI colors → gpui color spaces | Color format conversion |

Run:
```
cargo run                      # manual: window stays open, click the button
POC_AUTOCLOSE_SECS=4 cargo run # automated: renders 4s, quits, exit 0 = pass
cargo test                     # headless: parse tree shape + click->state->binding loop
```

## Results

| # | Mechanic | Result |
|---|---|---|
| 1 | Build + window | ✅ builds with plain `cargo build`, window opens and renders (`poc-screenshot.png`) |
| 2 | Runtime element tree | ✅ nested `AnyElement` tree from XAML DOM |
| 3 | Flex layout mapping | ✅ StackPanel spacing/padding verified in screenshot |
| 4 | Text styling + `{x:Bind}` | ✅ system font ("Segoe UI") resolved by cosmic-text; bound text renders |
| 5 | Border styling | ✅ Background/CornerRadius/BorderBrush visible in screenshot |
| 6 | Event → state → re-render loop | ✅ library segment unit-tested (`cargo test`): `Click="OnIncrement"` resolves from the registry, mutates `AppState`, binding text derives `"Button clicked 2 time(s)"`. The OS-level click delivery is GPUI's own machinery, not this library's surface |
| 7 | Color conversion | ✅ #AARRGGBB → gpui rgba |

**Verdict: FEASIBLE.** See `PLAN.md` for the library architecture this de-risks.

> **Update (2026-10-03, post-PoC research):** the PoC above runs on the
> official `gpui` 0.2.2 crate, which research subsequently showed to be a
> stale one-off snapshot (no Mica, no further publishes). The project
> foundation is now the `gpui-pre` snapshot family (requirement `"0.3"`,
> tracking the 0.3 line) — the line
> Zed's ecosystem (gpui-component/gpui-kit) actually uses. The PoC's findings
> remain valid (the element/binding/event mechanics are the same), and porting
> the PoC to `gpui-pre` is step 0 of the M1 backlog. Mica/Mica Alt turn out to
> be **first-class in that line** (`WindowBackgroundAppearance::MicaBackdrop`
> → `DWMWA_SYSTEMBACKDROP_TYPE`, Windows 11 22H2+), so the window-backdrop
> story needs no custom DWM code from us.

Evidence: `poc-screenshot.png` (rendered window, captured mid-run), exit code 0
on auto-close runs, `cargo test` 2/2.

## Friction found (goes into M1 backlog)

- A `Click` handler that mutates state must call `cx.notify()` inside the
  entity update or the view never re-renders — caught during review, fixed in
  the PoC; the real library must own this in its event plumbing so control
  authors can't forget it.
- An attempt to prove the click loop with a simulated OS mouse click was
  abandoned: an unrelated foreground app won the focus race (the machine was
  in active use). The library-side segment is covered by unit tests instead —
  the right boundary anyway, since raw input delivery belongs to gpui.
- gpui 0.2.2's API is already a moving target vs Zed main (`Application` entry,
  `Option<WindowOptions>` fields) — confirms the "isolate gpui in one backend
  crate, pin exact version" decision in PLAN.md.
- `BorderThickness` rendered as binary (any >0 → 1px) — need per-edge widths.
- `HorizontalAlignment` approximated via flex wrappers — real library should
  implement WinUI's alignment semantics in the model, not ad-hoc wrappers.
- Button keyboard activation (Space/Enter) and tab stops are absent —
  correctly scoped to M2 (REQ-CTRL-03); recorded so it isn't lost.
- Diagnostics are stderr warnings only. After the 2026-10-04 review fixes
  they cover: unknown elements, dropped element children, unsupported
  attributes (Min/Max size, VerticalAlignment, IsEnabled), markup extensions
  on non-Text/Content attributes, unparseable color/edge/number values, and
  binding-path misses/malformations. The parser still must carry roxmltree
  spans for real file/line diagnostics at M1.

## Scope of the PoC

590 lines total (`src/xaml.rs` 67, `src/render.rs` 360, `src/main.rs` 163;
refreshed 2026-10-04 after the review fixes).
Subset: Page, StackPanel, TextBlock, Border, Button, plus post-review
handling for `Visibility="Collapsed"`, `Opacity`, `Width`/`Height` and
2-value edge shorthand. Everything else in the
plan (Grid, resources, styles, two-way binding, control set) is layered
architecture, not new feasibility questions — the two genuinely uncertain
mechanics (runtime trees, event/state loop) are proven.
