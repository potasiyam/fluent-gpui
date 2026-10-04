# PLAN — WinUI-style XAML rendered by GPUI

> **Product requirements now live in [PRD.md](PRD.md)** (controls, materials,
> effects, motion, performance budgets, milestones). This file keeps the
> architecture, feasibility record, and dependency decisions.

**Vision:** a Rust library that lets you describe UI in WinUI-flavored XAML
and renders it with [GPUI](https://docs.rs/gpui) (Zed's GPU-accelerated UI
framework). You write:

```xml
<StackPanel Spacing="8" Padding="24">
    <TextBlock Text="Hello" FontSize="32" FontWeight="Bold"/>
    <Button Content="Click me" Click="OnClicked"/>
</StackPanel>
```

…and the library parses it, maps it to GPUI's element tree, and handles
styling, layout, and events. Host code stays Rust; the markup stays declarative.

**What this is not:** it is not WinUI. No WinRT, no XAML Islands, no
Composition, no Microsoft.UI.Xaml control templates. It is a *re-implementation
of the XAML authoring surface* (elements, properties, resources, binding
syntax) on a completely different rendering stack. "WinUI-flavored", not
"WinUI-compatible". Deliberately cross-platform: because the backend is GPUI,
the same XAML renders on Windows (DX12/Blade), macOS, and Linux.

---

## 1. Ecosystem facts (verified 2026-10-03)

| Fact | Value | Source |
|---|---|---|
| `gpui` on crates.io | v0.2.2, published 2025-10-22 | crates.io API |
| Windows renderer in 0.2.x | Blade-graphics (DX12 backend) + cosmic-text + `windows` 0.61 | crates.io dep graph |
| Actively maintained component ecosystem | `gpui-component` 0.7.0 (2026-09-28), 60+ components | crates.io API |
| gpui-component's gpui line | `gpui-pre` 0.3.7 — a third-party republish of a pinned Zed commit (`zed@1a28cff`), i.e. "current Zed main" | crates.io API |
| Existing XAML-on-gpui library | **none found** (searched crates.io + web) | web search |
| Mica / Mica Alt in gpui | official `gpui` 0.2.2: **no** (verified from crate source — only Opaque/Transparent/Blurred via `SetWindowCompositionAttribute`). Zed main since [PR #48340](https://github.com/zed-industries/zed/pull/48340) (merged 2026-09-19): **yes** — `WindowBackgroundAppearance::{MicaBackdrop, MicaAltBackdrop}` via `DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE)`, build ≥ 22621, non-Windows falls back to Transparent | gpui-pre-windows 0.3.7 source |

**The corrected dependency picture (verified 2026-10-03, superseding the
earlier "pin official 0.2" decision):** the official `gpui` crate on crates.io
is a stale snapshot — published once (0.2.2, 2025-10-22) and not maintained;
Zed consumes gpui from its monorepo and does not do crates.io releases. The
live line is the community **`gpui-pre` snapshot family**, republished on a
~weekly cadence (0.3.2 → 0.3.7 across Sept 2026) and split into crates:
`gpui-pre` (core), `gpui-pre-platform` (facade), `gpui-pre-windows` (the
DirectX/DirectWrite/DWM backend, with Mica), `gpui-pre-macos`, `gpui-pre-linux`,
`gpui-pre-wgpu` (Linux renderer), `gpui-pre-web`, plus `-macros`, `-sum-tree`,
`-reqwest(-client)`, `-collections`. The whole ecosystem has moved to it:
gpui-component/gpui-kit 0.7, HeroGPUI, SQLly. Consumers pin snapshots *exactly*
(`=0.3.7`) because any snapshot may change API — gpui-kit documents this and
enforces the pin with a CI script.

**Decision (revised, amended after review):** build on the `gpui-pre` family
with a caret requirement `"0.3"` — deliberately **not** the exact pin
gpui-kit uses. `"0.3"` auto-accepts new snapshots on the 0.3 line (0.3.8,
0.3.9, …) via `cargo update`, while a future family (0.4+, presumed breaking)
requires an explicit bump. The trade-off we accept knowingly: gpui-kit pins
exactly because a downstream snapshot bump once broke their build (#3156),
and their CI enforces the pin; our countermeasures instead are (a) the same
backend-crate isolation — all gpui types confined to `winui-xaml-gpui`, so
drift is a contained port, and (b) a regular `cargo update` + test +
render-check cadence so breakage is caught within days, not discovered
stacked up behind a frozen pin. Note `Cargo.lock` still records the exact
snapshot any given build used — the requirement governs what updates are
*allowed*, not what is silently pulled. The abandoned official 0.2.2 remains
recorded as the PoC's foundation. A `SystemBackdrop="Mica"` XAML attribute
maps 1:1 onto `WindowBackgroundAppearance::MicaBackdrop` — no DWM code of
ours required on Windows 11 22H2+.

---

## 2. Normative reference: the XAML semantic contract

Microsoft's [win-dev-skills](https://github.com/microsoft/win-dev-skills)
package (vendored under [`references/win-dev-skills/`](references/win-dev-skills/README.md),
MIT, pinned to `2e8c902`) defines what *correct* WinUI 3 XAML means. We treat
it as the contract our subset implements:

- **Binding semantics to match (M3):** `{x:Bind}` defaults to **OneTime**;
  `Mode=OneWay/TwoWay` and inherited `x:DefaultBindMode` override it;
  `UpdateSourceTrigger` defaults to LostFocus for `TextBox.Text` specifically;
  `Converter={x:Null}` is a runtime crash in WinUI — our parser rejects it
  with a diagnostic.
- **Theming rules (M1):** `{ThemeResource}` at usage sites, purpose-named
  brushes, explicit Light/Dark/HighContrast dictionaries (never "Default"),
  typography styles (`TitleTextBlockStyle`…), 4px spacing grid,
  `ControlCornerRadius`/`OverlayCornerRadius` tokens in default theme.
- **Diagnostics backlog (M1, from their anti-patterns + review checklist):**
  hardcoded color literals (warn → suggest theme brush), `ScrollViewer`
  wrapped around a virtualizing list, clickable `Border`/`TextBlock` instead
  of a Button, placeholder text as the only label, missing
  `AutomationProperties.Name` on icon-only controls, color-only information.
- **Control map (M2):** no DataGrid — tabular = `ListView` + Grid-based
  item template; `AutoSuggestBox` for search; `InfoBar`/`TeachingTip` for
  feedback; `SelectorBar`/`TabView`/`NavigationView` for chrome. App-shape
  anchors table drives our gallery example pages.
- **Accessibility (M5):** `AutomationProperties.AutomationId`/`Name` as
  attached properties map onto gpui's accesskit surface (gpui already
  depends on accesskit).

Not applicable from that package: the `winapp` CLI toolchain, C#/MVVM
specifics, MSIX packaging, AOT guidance, and the WUI0xxx analyzer IDs (we
mint our own diagnostic codes).

---

## 3. Architecture

```
┌─────────────────────────────────────────────────────────┐
│ your app (Rust): handlers, view-model, assets/*.xaml    │
└──────────────┬──────────────────────────────────────────┘
               │
┌──────────────▼──────────────┐   ┌──────────────────────┐
│ winui-xaml-syntax           │   │ winui-xaml-binding   │
│ XAML → DOM: namespaces,     │   │ {x:Bind} codegen     │
│ property elements, markup   │   │ (build.rs/proc-macro)│
│ extensions, resource lookup │   │ + runtime {Binding}  │
└──────────────┬──────────────┘   └──────────┬───────────┘
               │                             │
┌──────────────▼─────────────────────────────▼───────────┐
│ winui-xaml-model: element/property registry, styles,   │
│ visual tree (resolved attributes, inherited props)     │
└──────────────┬─────────────────────────────────────────┘
               │  backend-agnostic tree
┌──────────────▼──────────────────────────────────────────┐
│ winui-xaml-gpui: visual tree → gpui AnyElement mapping, │
│ theme dictionaries, control visuals, hit-testing glue   │
└──────────────┬──────────────────────────────────────────┘
               │
        ┌──────▼───────┐
        │ gpui-pre     │  "0.3"-tracked snapshot family (DirectX/DWM on
        │              │   Windows, Metal on macOS, wgpu on Linux, web)
        └──────────────┘
```

Crate responsibilities:

- **`winui-xaml-syntax`** — parser. Beyond the PoC's attribute parsing it must
  support: property-element syntax (`<Button.Content>`), attached properties
  (`<Grid.Row>`), markup extensions (`{StaticResource}`, `{ThemeResource}`,
  `{Binding}`, `{x:Bind}`, `{x:Null}`), `x:Key` resources, `<![CDATA[`,
  and clear diagnostics with line/column (roxmltree provides spans).
- **`winui-xaml-model`** — the part Rust makes interesting: there is **no
  runtime reflection**, so every control type registers a property table
  (name → getter/setter/coercion). This registry is what makes `FontSize="34"`
  mean something at runtime; a `derive(XamlBindable)` macro generates it from
  ordinary Rust structs for binding targets.
- **`winui-xaml-controls`** — the control set (see milestone table). Each
  control declares: properties, default property (Content/Text), and its
  visual mapping. Layout semantics follow WinUI: StackPanel spacing, Grid
  star/pixel/auto sizing, alignment, padding/margin shorthand.
- **`winui-xaml-gpui`** — the only crate that touches `gpui` types. Maps the
  resolved visual tree to `AnyElement`s per frame (GPUI re-renders views on
  notify; mapping a tree per notify is cheap relative to the GPU draw).
  Owns theme dictionaries (Light/Dark/HighContrast), control visual states
  mapped to GPUI's hover/active/focus states.
- **`winui-xaml-binding`** — two modes. **`{x:Bind}`**: parsed at build time
  from the app's XAML files, generates typed, compile-checked accessor code —
  mirrors real WinUI's compiled bindings, and dodges the reflection problem.
  **`{Binding}`**: runtime path resolution against the property registry for
  dynamic scenarios (hot reload, tooling), slower but reflective-like.
- **`winui-xaml-hotreload`** (feature-gated) — fs-watch the `.xaml` files,
  re-parse, diff, notify the window.

---

## 4. Milestones

> **Superseded 2026-10-04:** the milestone ladder now lives in
> [PRD.md §15](PRD.md); this table is the historical M0–M5 plan, kept for
> the record.

| M | Scope | Exit criteria |
|---|---|---|
| **M0 — PoC (done)** | Page, StackPanel, Border, TextBlock, Button; Background/Foreground/FontSize/FontWeight/Padding/Spacing/CornerRadius/HorizontalAlignment; `{x:Bind}` one-way; `Click=` handlers; builds and runs on Windows | Window renders `demo.xaml`; button click updates bound text. ✅ see FEASIBILITY.md |
| **M1 — markup surface** | Property elements, `{StaticResource}`/`{ThemeResource}` with ResourceDictionary + ThemeDictionaries, `Style` blocks (Setters), Grid (row/col/star sizing), RelativePanel (defer if low value), Image, shapes (Rectangle/Ellipse), margins shorthand everywhere, error diagnostics with file/line | A nontrivial real layout (settings page) renders correctly in light+dark |
| **M2 — control set** | TextBox, CheckBox, ToggleSwitch, Slider, ProgressBar, ComboBox, ListView (+ virtualization via `uniform_list`), MenuBar, TabView-ish, Icon/Path | Control gallery app ships in-repo; keyboard focus + tab order works |
| **M3 — binding/MVVM** | `derive(XamlObservable)` macro (INotifyPropertyChanged analog), two-way `x:Bind Mode=TwoWay`, `ICommand` analog, converters, `{Binding}` runtime path | Gallery controls drive a view-model without manual wiring |
| **M4 — tooling** | Hot reload (fs-watch → re-parse → notify), `x:Bind` compile-time validation in build, VS Code XML schema for completion/diagnostics | Edit XAML → window updates in <1 s, no recompile |
| **M5 — polish (stretch)** | Visual states/transitions, GPUI animations, accesskit accessibility surface (gpui already depends on accesskit), multi-window/navigation, DPI/font-fidelity audit | — |

## 5. Binding design (the hard part, resolved up front)

Rust has no runtime reflection, so WinUI's reflection-based `{Binding}` cannot
be copied literally. Three options were considered:

1. **Runtime property registry only** — every bindable object registers
   properties by name. Works, but verbose without derive support, and
   type-unsafe at the edges.
2. **Compile-time `{x:Bind}` codegen** — build step parses the app's XAML,
   generates typed Rust that reads the declared paths. Compile errors for bad
   paths. This is what real WinUI does with `x:Bind`, and it fits Rust.
3. Both (registry as fallback for dynamic tooling scenarios).

**Decision: option 3**, with option 2 as the primary developer experience.
M0's `bindings: HashMap` is the placeholder seam where this lands.

## 6. Risks & mitigations

| Risk | Severity | Mitigation |
|---|---|---|
| GPUI API churn; snapshot-line drift (any `gpui-pre` release may change API) | High | All gpui types confined to `winui-xaml-gpui`; caret-tracked `0.3` line (deliberate choice, not exact-pinned) with a regular update+test cadence to catch breakage early; the PoC renderer is ~300 lines — port cost is bounded and known |
| Blade DX12 maturity on Windows (driver variance, GPU feature gaps) | Medium | Test matrix on real machines at M1; if blocked, the backend crate is the only thing that moves (e.g. to a wgpu-flavored gpui line) |
| Typography/visual fidelity vs real WinUI (cosmic-text ≠ DirectWrite) | Medium | Position as "WinUI-flavored" from day one; document deltas; never promise pixel parity |
| XAML breadth (200+ controls, full resource system, x:Uid, …) | High | Strict subset per milestone; unsupported constructs fail loudly at parse time with file/line, never silently render wrong |
| Whole-tree re-render per state change | Low | GPUI is built for this (Zed renders its entire editor UI per notify); lists virtualize at M2; measure before optimizing |
| No reflection (binding) | Medium | Resolved by design above (codegen + registry) |

## 7. Immediate next steps (M1 backlog, in order)

> **Superseded 2026-10-04:** replaced by PRD §15 M1, which includes the
> port below as its first item.

0. **Port the PoC from official `gpui` 0.2.2 to the `gpui-pre` family
   (requirement `"0.3"`, core + `gpui-pre-platform`)**, fix API drift, re-run
   the feasibility checks, then add `WindowBackgroundAppearance::MicaBackdrop`
   to the demo behind a `SystemBackdrop="Mica"` XAML attribute.

1. Split the PoC into the crate layout above (`winui-xaml-syntax` first — it
   is backend-independent and fully unit-testable).
2. Resource dictionaries + `{StaticResource}`/`{ThemeResource}`, Light/Dark.
3. `Style`/`Setter` blocks and property-element syntax in the parser.
4. Grid with `ColumnDefinitions="Auto,*,2x"` star sizing on the gpui side.
5. Diagnostic harness: a `gallery/` example + `xaml-test/` fixture corpus
   (every construct renders an expected screenshot, checked in CI).
