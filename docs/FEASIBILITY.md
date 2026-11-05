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

> **Update (2026-10-06, gpui-pre port — M1 step 0):** the PoC now builds and
> runs on the `gpui-pre` snapshot family (requirement `"0.3"`; resolved to
> 0.3.8, snapshot zed@279fe07) with `gpui-pre-platform` selecting the Windows
> backend (DirectX/DirectWrite, no features needed). All feasibility checks
> re-ran green: `cargo test` 2/2, `POC_AUTOCLOSE_SECS=4 cargo run` exit 0.
> API drift vs gpui 0.2.2 cost exactly one change in the PoC: the entry point
> is `gpui_platform::application().run(…)` instead of
> `gpui::Application::new().run(…)`; everything else compiled unchanged.
> `SystemBackdrop="Mica"` on the page root maps to
> `WindowBackgroundAppearance::MicaBackdrop` (a `WindowOptions` field,
> gpui-pre 0.3.8 `src/platform.rs`), exercised by demo.xaml. Render evidence
> on gpui-pre: `gpui-pre-screenshot.png` (2026-10-07), captured with
> `scripts/screenshot-window.ps1` — a PrintWindow of the app window itself,
> which needs no focus. The Mica tint is not distinguishable in a
> window-surface capture (DWM composites the backdrop outside the app
> surface); the applied window option is the code-level evidence.

Evidence: `poc-screenshot.png` (rendered window, captured mid-run), exit code 0
on auto-close runs, `cargo test` 2/2.

> **Update (2026-10-07, M1 catalog diagnostics):** `winui-xaml-model` now
> owns the element/property catalog and the WX-coded diagnostics
> (WX0002–WX0007: unknown element/property, property elements, attached
> properties, unknown markup extensions, recognized-but-unimplemented
> properties), each carrying its 1:1 source position, plus a codespan-style
> plain-text emitter. The PoC host resolves the tree against the catalog at
> load: any error exits 1 before a window opens; warnings print and
> rendering continues. A workspace test pins demo.xaml to resolve clean.
> Evidence: `cargo test --workspace` 13/13, `POC_AUTOCLOSE_SECS=12 cargo
> run` exit 0, `m1-catalog-diagnostics.png` (PrintWindow capture; visuals
> unchanged by design).

> **Update (2026-10-07, M1 resources):** the model collects `.Resources`
> dictionaries — plain entries plus `ThemeDictionaries` for
> Light/Dark/HighContrast — into a `ResourceSet`, and the renderer resolves
> `{StaticResource}`/`{ThemeResource}` in color attributes against it:
> nearest element scope wins, and within a scope the active theme's
> dictionary wins over plain entries. Unresolvable `{StaticResource}` keys
> are WX1001 errors at load (matching WinUI's parse exception);
> unparsable/unsupported entries are WX1002, unknown theme keys WX1003 —
> WinUI's `"Default"` theme key is rejected per the vendored authoring
> rules. demo.xaml now colors everything through theme-keyed brushes and
> renders identically to the previous literal-hex version. Evidence:
> `cargo test --workspace` 18/18, `POC_AUTOCLOSE_SECS=12` exit 0 with an
> empty stderr, `m1-resources-themeresource.png`. Not yet from REQ-RES-01:
> `{ThemeResource}` re-resolution on theme switch (rides with the
> token/theme engine).

> **Update (2026-10-07, M1 styles):** `Style`/`Setter`/`BasedOn` land in
> the model (REQ-RES-02): styles are resource values — keyed, or implicit
> keyed by TargetType — `style_setters` flattens the BasedOn chain (base
> first, derived last), and `resolve_styles` produces the resolved tree:
> setter values become attributes wherever no local value exists (WinUI
> precedence: local value beats style setter). The renderer is unchanged;
> it renders the resolved tree. `check_style_chains` validates TargetType
> presence/knownness, setter properties against the catalog (the first
> type-level diagnostics — WX0003 on a misspelled Setter property),
> BasedOn cycles, and TargetType mismatches (WX1004). demo.xaml styles its
> text through two keyed styles (one BasedOn) plus an implicit TextBlock
> style and renders pixel-identical to the literal-hex version. Evidence:
> `cargo test --workspace` 23/23, `POC_AUTOCLOSE_SECS=18` exit 0 with an
> empty stderr, `m1-styles.png`. Not yet: library default styles as
> embedded XAML (rides with the M2 control catalog) and WinUI's implicit
> where-used scoping nuance — our implicit styles follow the normal
> nearest-scope order, a documented approximation.

> **Update (2026-11-05, M1 merged dictionaries + Source files):** the last
> two REQ-RES-01 items land. `collect_resources` now consumes
> `ResourceDictionary.MergedDictionaries` and the `Source` attribute —
> including `Source` on theme dictionaries and inside merged files — with
> lookup precedence per MS Learn (xaml-resource-dictionary): a dictionary's
> own entries are checked before its merged dictionaries, and multiple
> merged dictionaries are checked last-added first. `Source` files load
> through a host-supplied `SourceResolver` trait, so the model layer stays
> gpui-free and I/O-free; the PoC host resolves plain relative paths
> against demo.xaml's directory. An unresolvable or unparsable `Source` is
> a WX1005 error at the dictionary's position, as is a file whose root is
> not a `ResourceDictionary`, a `Source`/inline-content combination, and a
> merge cycle (cycle loads are also depth-capped at 32 so pathological
> inputs can't overflow the stack). demo.xaml now keeps only
> `<ResourceDictionary Source="demo-resources.xaml"/>` — its whole
> dictionary moved to `assets/demo-resources.xaml` — and renders
> pixel-identical to the previous screenshot. One coverage note: catalog
> checks (WX0002/3) and `{StaticResource}` reference checks (WX1001) walk
> the page tree only, so constructs inside a `Source` file are checked by
> the resource collector (WX1002/1003/1005) but not yet by those two
> passes. Evidence: `cargo test --workspace` 36/36 (9 merged/Source tests
> new), `cargo clippy --all-targets --workspace` clean, `cargo fmt` clean,
> `POC_AUTOCLOSE_SECS=6` and `POC_AUTOCLOSE_SECS=12` `cargo run` exit 0
> with an empty app stderr, `m1-merged-dictionaries-source.png`.

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
- Diagnostics were stderr warnings only. Now split by layer (2026-10-07;
  deduplicated after the issue #1 review): structural unknowns (elements,
  properties, property elements, markup extension names) come from the
  `winui-xaml-model` catalog as WX-coded diagnostics with file/line at
  load time, and the renderer no longer re-reports them. The renderer's
  remaining stderr warnings are value-level (unparseable color/edge/number
  values, binding-path misses) plus the unresolved-extension note for
  attributes it cannot resolve; both get absorbed into WX codes as M1
  continues. Still missing for REQ-DIAG-01/02: type checks on property
  values (Setter property names are checked; values are not) and
  renderer-side resolution diagnostics.
- Screenshot evidence has two environment failure modes (2026-10-06): an
  actively used desktop wins the focus race (app renders behind other
  windows), and a locked session makes GDI `CopyFromScreen` fail with "The
  handle is invalid" entirely (observed: capture worked 22:51, failed 22:55
  with the unchanged script; LogonUI running). Resolution (2026-10-07):
  `scripts/screenshot-window.ps1` uses PrintWindow with
  PW_RENDERFULLCONTENT — no focus needed, works regardless of what is in
  the foreground. Use it as the default evidence capture; it cannot show
  the DWM backdrop tint (composited outside the app surface), and
  `screenshot-focused.ps1` stays for whole-desktop shots when that is
  explicitly wanted. Window-open latency also varies by seconds between
  runs; poll for the process window in a retry loop instead of sleeping a
  fixed interval before capturing (2026-10-07).
- Docs incident (2026-10-06): one app's hand-rolled Fluent layer was
  weighted as [V] "field evidence" and promoted into PRD requirements; the
  owner corrected — it is one anecdotal data point, not a validated example.
  Root cause: verification tags applied to provenance rather than to claims.
  Fixed same turn: MILESTONES §7 is non-normative, the PRD items became
  M1/M2 evaluations. Anecdotes are inputs to evaluate, never requirement
  sources.

## Scope of the PoC

590 lines total (`src/xaml.rs` 67, `src/render.rs` 360, `src/main.rs` 163;
refreshed 2026-10-04 after the review fixes).
Subset: Page, StackPanel, TextBlock, Border, Button, plus post-review
handling for `Visibility="Collapsed"`, `Opacity`, `Width`/`Height` and
2-value edge shorthand. Everything else in the
plan (Grid, resources, styles, two-way binding, control set) is layered
architecture, not new feasibility questions — the two genuinely uncertain
mechanics (runtime trees, event/state loop) are proven.

> **Update (2026-11-05, M1 token catalog):** the Fluent 2 token set ships
> as embedded XAML (`include_str!` of
> `crates/winui-xaml-model/assets/fluent-tokens.xaml`) merged ahead of app
> resources through the new `ResourceSet::merge_app_dictionary` — lookup
> now follows WinUI's element → page → app order, so page and app
> definitions override tokens. Brush values are WinUI's
> `Common_themeresources_any.xaml` (fetched 2026-11-05); that file keys
> its dark values "Default", which the vendored rules ban, so they ship
> re-keyed "Dark". All five vendored families land (text, control fill,
> stroke, surface/layer, accent — 58 brushes plus the SystemAccentColor
> ramp as `Color` resources); the Acrylic family rides with the materials
> milestone (flyouts/menus, M6). HighContrast maps every catalog key onto
> the `SystemColor*` pairing exactly as WinUI's own HighContrast
> dictionary does, ships the `SystemColor*Brush` family, and carries no
> opacity; because the PoC reads no live OS colors, the `SystemColor*`
> and `SystemAccentColor` defaults are the classic palette (Uno's
> `SystemResources.xaml` mirror of the native placeholders). Accent fill
> secondary/tertiary bake the official Opacity 0.9/0.8 into the color
> alpha. REQ-TOK-02 lands as eight keyed ramp styles (Caption 12, Body 14,
> BodyStrong 14 SemiBold, BodyLarge 18, Subtitle 20, Title 28, TitleLarge
> 40, Display 68); their Foreground setter stands in for TextBlock's
> platform default style until the M2 control catalog. REQ-TOK-03 ships
> `ControlCornerRadius`=4 / `OverlayCornerRadius`=8 as `CornerRadius`
> resources (a new `ResourceValue::Text` form) and the renderer now
> resolves `CornerRadius` through them; the 4px spacing grid is an
> authoring rule, applied to demo.xaml (spacing 14 → 16). REQ-TOK-04's
> `SymbolThemeFontFamily` indirection ships; its consumer (`FontIcon`)
> is M2. demo.xaml is restyled onto token keys (card, strokes, text,
> accent via `AccentTextFillColorPrimaryBrush`) and renders themed in
> both modes. The window surface itself is not token-driven yet — the
> host paints only its opaque fallback, so in the Light capture the dark
> Mica surface sits behind light-themed brushes (the `Window` element
> owns that in M2). **Accent evaluation (MILESTONES §7.3, outcome):**
> static catalog values ship for M1. The anecdotal app's fill convention
> (AccentDark1-based in Light, AccentLight2-based in Dark) is verified
> against the official `Common_themeresources_any.xaml` — it matches
> exactly, so it is no longer unverified. Reading the live accent from
> `UISettings.GetColorValue` stays a candidate rider for the M2 control
> wave, when a control first consumes `AccentFillColorDefaultBrush`; a
> live reader must also cover the `SystemColor*` set for real
> HighContrast. Evidence: `cargo test --workspace` 45/45 (token brushes
> per theme, HC SystemColor pairs, ramp sizes, corner/font tokens,
> page-over-tokens precedence, theme-name parsing), `cargo clippy
> --all-targets --workspace` clean, `cargo fmt` clean,
> `POC_AUTOCLOSE_SECS=6` (default Dark) and `POC_THEME=Light|Dark
> POC_AUTOCLOSE_SECS=12` runs all exit 0 with an empty app stderr,
> `m1-token-catalog-dark.png`, `m1-token-catalog-light.png`.

> **Update (2026-11-05, M1 live theme switch):** REQ-TOK-05 lands. The
> active theme now lives in host view state: `{ThemeResource}` values in
> color and number attributes re-resolve against it on every re-render
> (theme change → update + notify, no window recreation), while style
> selection (`Style=` keys, implicit styles) still resolves once at load —
> unobservable today because no styles come from theme dictionaries and
> demo.xaml keys styles with `{StaticResource}`. `{StaticResource}`
> resolves once against the load theme and never follows a switch (WinUI
> semantics; PRD §5.2 "resolve once at tree build"). `RequestedTheme` on any subtree
> root overrides the active theme for that subtree only (renderer walks
> with an effective theme; `RequestedTheme` joined the common catalog
> properties). The initial theme comes from the OS: `App::window_appearance()`
> at startup, and OS appearance changes drive the same re-resolution
> through `Window::observe_window_appearance` — step 2's `POC_THEME` env
> seam is removed. Two platform notes: gpui-pre 0.3.8's
> `WindowAppearance` has no HighContrast variant, so OS HC detection is
> not reachable through gpui — HC rendering goes through
> `RequestedTheme="HighContrast"` on a subtree over the token catalog's HC
> dictionary. That value is a dialect extension: WinUI's `ElementTheme` is
> Default/Light/Dark only (MS Learn), so accepting `HighContrast` is
> recorded as [D] in PRD §6 REQ-TOK-05; a live OS HC reader would need a
> registry read (new dependency, deferred), and the HC capture rides the
> PRD §15 "Light+Dark+HC" gate at M1 exit. And the window surface itself is still not
> token-driven (M2 `Window` element), so light brushes render against the
> dark Mica surface in the captures. Evidence: `cargo test --workspace`
> 48/48 — new renderer tests pin that a `StaticResource` stays at its
> load-theme value across a switch, a `ThemeResource` re-resolves, and a
> `RequestedTheme` subtree overrides without leaking to siblings; a host
> test pins the appearance→theme mapping. `POC_AUTOCLOSE_SECS=6 cargo
> run` exits 0 with an empty stderr. The acceptance flip run
> (`POC_FLIP_THEME_SECS=10`, autoclose 30s) captured before/after
> screenshots from the same process with no restart —
> `m1-live-theme-switch-before.png` (Dark) and
> `m1-live-theme-switch-after.png` (the same window 11 seconds later,
> re-resolved to Light brushes). `cargo fmt` and `cargo clippy
> --all-targets --workspace` clean.
