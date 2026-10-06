# PRD — winui-xaml: the WinUI 3 / Fluent 2 surface, implemented on GPUI

**Status:** Draft v1 · 2026-10-03
**Supersedes:** PLAN.md §2, §4, and §7 — the semantic contract, the historical milestone ladder, and the M1 backlog (§15 below replaces that ladder). PLAN.md §1 (ecosystem facts), §3 (architecture), §5 (binding design), and §6 (risks) remain canonical; PRD §16 extends PLAN §6.
**Evidence policy:** every capability claim below is either **[V]** verified against a primary source (listed in Appendix E) or **[D]** a design decision we own / **[Q]** an open question. No unmarked claims.

---

## 1. Vision

Rust developers write UI in **WinUI 3-flavored XAML** and get a **Fluent 2** app:
correct controls, correct design tokens, the real materials (Mica, Mica Alt,
Acrylic), the real motion system (Windows 11 easings, springs, page
transitions, connected animations) — rendered by GPUI at 120 FPS on Windows,
macOS, Linux, and wasm.

The library is a *faithful re-implementation of the XAML authoring surface*,
not a WinRT interop. It adopts Microsoft's WinUI 3 semantics as its
normative contract — including the ones that are easy to get wrong
(`x:Bind` defaults to OneTime; `UpdateSourceTrigger` for `TextBox.Text` is
LostFocus; `Converter={x:Null}` is rejected, not crashed on) — because the
vendored [win-dev-skills](../references/win-dev-skills/README.md) define those
semantics precisely and we treat them as spec.

**One-line positioning:** "Fluent 2 for Rust, authored in XAML, rendered by
GPUI."

## 2. Users and scenarios

| Persona | Scenario | What they need from us |
|---|---|---|
| Rust product team | Internal desktop tool (settings, dashboards, admin) | Declarative UI that looks native-Win11 without C#/WinRT; hot reload; MVVM-lite binding |
| XAML-literate designer/dev | Porting a WinUI 3 app concept to Rust | Familiar element names, property names, resources, `x:Bind`; familiar failure messages |
| GPUI app developer | Wants Fluent components/materials inside an existing gpui app | Opt-in crates; controls usable without XAML; materials as plain API |
| OSS contributor | Extends control set | Registry architecture: one trait + one mapping file per control; golden-image tests |

Non-scenario (explicit): pixel-perfect porting of an existing WinUI 3 app
with identical C# behavior. We are "WinUI-flavored", not "WinUI-compatible"
(fidelity deltas are documented per feature in this PRD).

## 3. Goals and non-goals

**Goals**
1. G1 — Full Fluent 2 design-token system (color brushes, typography, spacing,
   corner radius, elevation, motion) as runtime theme dictionaries:
   Light / Dark / **HighContrast**.
2. G2 — The **complete WinUI 3 control catalog** (all 50+ controls from
   Microsoft's controls index) delivered across milestones, each meeting the
   Fluent visual spec — or explicitly absent with a documented reason.
3. G3 — **All Fluent materials**: Mica, Mica Alt, background (desktop)
   acrylic, in-app acrylic — with the documented degradation matrix
   (battery saver, transparency off, window deactivated, HighContrast).
4. G4 — **The Windows 11 motion system**: every animation class Microsoft
   specifies — entrances, exits, point-to-point, fades, elastic entrances,
   page transitions, connected animations, springs, micro-interactions,
   animated icons — with the exact Fluent easings/durations as tokens.
5. G5 — Performance as a first-class requirement (budgets in §13, enforced
   by benchmarks in CI, not aspiration).
6. G6 — Accessibility is part of "done" for every control (accesskit, keyboard,
   HighContrast, reduced-motion), per the vendored code-review checklist.
7. G7 — Developer experience: hot reload, typed `{x:Bind}` codegen, editor
   schema, gallery app, fail-loud diagnostics with file/line.

**Non-goals**
- N1 — WinRT/WinUI binary compatibility, `x:Class` code-behind in C#, or
  XAML Islands. Event handling binds to Rust host types (§5.5).
- N2 — Full `ControlTemplate` re-templating engine in v1. Restyling is
  brush/spacing-token-driven (Microsoft's own review guidance prefers this);
  a template engine is a v2 evaluation. [D]
- N3 — `WebView2` (no Chromium embedding in v1; the gpui ecosystem's
  `gpui-wry` is the designated future path [Q]).
- N4 — Inking (InkCanvas/InkToolbar): unavailable even in stable WinUI 3
  (experimental in WinAppSDK 2.0-exp) — out of scope.
- N5 — ExpressionAnimation as an authoring paradigm. Fluent's implicit
  property-driven animation is expressible with our transition/animation
  engine (§9); raw expression strings are not accepted in XAML. [D]
- N6 — 3D transforms, perspective, Composition custom geometries beyond
  what gpui paths/SVG support.
- N7 — MapControl: in WinUI 3 since WinAppSDK 1.5 [V], but it needs an
  online tile service and a geo stack. Out of scope v1. [D]
- N8 — Printing: WinUI 3 does ship it (`Microsoft.UI.Xaml.Printing.PrintDocument`
  + `PrintManagerInterop`, since 0.8) [V], but the pipeline is Windows-only
  and gpui has no print path. Out of scope v1; a v2 PDF-emission route is
  the designated evaluation. [D]

## 4. Platform foundation — verified capability matrix

Foundation: **`gpui-pre` snapshot family, requirement `"0.3"`** (0.3.7 at
writing), the line Zed's ecosystem uses (see PLAN.md §1 for the full
verified dependency picture). What Fluent needs vs. what gpui provides:

| Fluent primitive | gpui-pre 0.3.7 primitive | Status | Evidence |
|---|---|---|---|
| Mica / Mica Alt window backdrop | `WindowBackgroundAppearance::{MicaBackdrop, MicaAltBackdrop}` → `DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE)`, build ≥ 22621 | ✅ native | gpui-pre-windows 0.3.7 `src/window.rs:929-934, 1601-1615` [V] |
| Background (desktop) acrylic | `Blurred` → `SetWindowCompositionAttribute` accent 4 (ACRYLICBLURBEHIND), build ≥ 17763 | ✅ native (Win10+) | same file, `set_background_appearance` [V] |
| Win11 system acrylic (`DWMSBT_TRANSIENTWINDOW`) | **no enum variant** (only Mica/MicaAlt go through DWM) | ⚠️ upstream gap — optional contribution | same file, match arms [V] |
| Opaque / transparent windows | `Opaque` (accent 0) / `Transparent` (accent 2) | ✅ native | same file [V] |
| Multi-layer shadows | `BoxShadow { blur_radius, spread_radius, offset, color }` | ✅ | gpui-pre `src/style.rs:349+` [V] |
| Opacity | `Styled::opacity(f32)` | ✅ | `src/styled.rs:746` [V] |
| Linear gradients | `linear_gradient()` as background | ✅ | `src/color.rs:858` [V] |
| Radial gradients | not found in 0.3.7 source | ⚠️ gap — emulate (§8.2) or upstream | source grep [V] |
| Clipping | `overflow_hidden()` | ✅ | `src/styled.rs:140,148` [V] |
| Tween animation, pluggable easing | animation element + `with_easing(impl Fn(f32)->f32)`, built-in easings incl. `ease_in_out` | ✅ | `src/elements/animation.rs:60,516` [V] |
| Physics springs | `SpringConfig { stiffness, damping, mass }` | ✅ | `src/spring.rs:13-28` [V] |
| Keyframe/multi-stage animation | no first-class primitive | ⚠️ build a keyframe sequencer on the easing closure (§9.3) | source grep [V] |
| Backdrop blur *inside* the app (true in-app acrylic blur) | not exposed (window-level `blur` exists; element backdrop-blur does not) | ⚠️ gap — approximation + upstream ask (§7.3) | source grep [V] |
| Noise texture (acrylic recipe) | image/paint layering | ✅ composable | [D] |
| Text shaping | **DirectWrite on Windows** (`direct_write.rs`), CoreText on macOS, cosmic-text elsewhere (rustybuzz: complex scripts incl. Bengali/Indic) | ✅ | gpui-pre-windows `src/direct_write.rs`; dep graph [V] |
| GPU renderer | DirectX 12 + DirectComposition (Win), Metal (mac), wgpu (Linux, `gpui-pre-wgpu`), wasm (`gpui-pre-web`) | ✅ | gpui-pre-platform 0.3.7 facade [V] |
| Accessibility | accesskit dependency in gpui line | ✅ surface exists | dep graph [V] |
| Headless test platform | `platform/test` module (render real components headless, drive input) | ✅ enables golden-image CI | gpui-pre `src/platform/test.rs`; gpui-kit advertises the same pattern [V] |

**Custom shaders:** not exposed by the 0.3.7 core API we inspected [V].
The `runtime_shaders` feature exists on the Linux platform facade [V] —
tracked as an open question for the exclusion-blend acrylic layer (§7.3).

## 5. The XAML dialect (product surface)

### 5.1 Grammar
- REQ-DX-01 (Must, M1): parse object elements, property elements
  (`<Button.Content>`), attached properties (`Grid.Row`), implicit
  `Content` (element text), CDATA, comments, `x:` namespace normalization.
  roxmltree spans carried end-to-end.
- REQ-DX-02 (Must, M1): every file resolves against a **catalog**: elements,
  properties (per type, with value coercion), attached properties, markup
  extensions. Unknown name ⇒ diagnostic with file/line/col, never silent.
- REQ-DX-03 (Should, M7): strict-mode flag (`xaml-lint`) that also flags
  perf/a11y anti-patterns (§13.3, §14). Basic fail-loud parsing is
  REQ-DIAG-02 at M1.

### 5.2 Markup extensions
| Extension | v1 semantics | Milestone |
|---|---|---|
| `{StaticResource Key}` | resolve once at tree build; resolution order: element scope → page → app → theme dictionary | M1 |
| `{ThemeResource Key}` | resolve per active theme; re-resolve on theme switch | M1 |
| `{x:Bind Path[, Mode=…][, Converter=…][, FallbackValue=…]}` | compile-time typed codegen (§11); **defaults to OneTime** per WinUI semantics | M3 |
| `{Binding Path}` | runtime path binding via property registry (tooling/dynamic scenarios) | M3 |
| `{x:Null}` | valid; **rejected with diagnostic where WinUI crashes** (`Converter={x:Null}`) — parity-with-the-docs exception [D] | M1 |
| `{TemplateBinding}` | not in v1 (N2) | — |
| `{x:Static}` | **not a WinUI 3 extension** [V — the official supported-extension list enumerates 8 and omits it]; Rust constants are reached through `{x:Bind}` paths instead | out [D] |

### 5.3 Resources and styles
- REQ-RES-01 (Must, M1): `ResourceDictionary`, `x:Key`, merged dictionaries,
  `ThemeDictionaries` with **Light, Dark, HighContrast** keys (a "Default"
  key is a diagnostic warning, per the vendored theming rules).
- REQ-RES-02 (Must, M1): `Style`/`Setter` blocks, `BasedOn` inheritance;
  library default styles ship as embedded XAML compiled into the crate.
- REQ-RES-03 (Should, M2): `DataTemplate` + `x:DataType` (typed item
  templates for collections).
- REQ-RES-04 (Could, v2): full `ControlTemplate` (see N2).

### 5.4 Diagnostics
- REQ-DIAG-01 (Must, M1): codes `WXnnnn` (parse WX0xxx, resource WX1xxx,
  binding WX2xxx, layout WX3xxx, a11y WX4xxx, perf-lint WX5xxx) with
  file/line/col, rendered by a codespan-reporting-style emitter.
- REQ-DIAG-02 (Must, M1): fail-loud policy — unknown construct stops the
  build/run with a diagnostic; "render something plausible" is never the
  fallback (PoC's stderr warnings are retired at M1).

### 5.5 Events & commands
- REQ-EVT-01 (Must, M1): `Click="HandlerName"` resolves against a host
  trait (`XamlHost`) provided by the app; the library owns `cx.notify()`
  plumbing (PoC lesson, FEASIBILITY.md).
- REQ-EVT-02 (Must, M3): `[RelayCommand]`-analog via derive: `#[derive(XamlViewModel)]`
  generates property table + change notifications + command thunks.

### 5.6 Localization & naming
- REQ-I18N-01 (Should, M7): `x:Uid` + resource lookup for strings; RTL
  `FlowDirection` supported on all layout containers (taffy direction
  properties), verified with mirrored gallery pages.

## 6. Design tokens (Fluent 2) — the full inventory

Source of truth for names/values: vendored `winui-design/references/brushes-and-icons.md`
(Microsoft's XAML theme resources, pinned @2e8c902) + Fluent motion
(§9.1, primary source) + Fluent UI shadow tokens (§8.2). All ship as
theme dictionaries.

- REQ-TOK-01 (Must, M1): **brush catalog** — every brush family in the
  vendored catalog, implemented as theme-resolvable brushes:
  Text (Primary/Secondary/Tertiary/Disabled/OnAccent…), Control fills
  (Default/Secondary/Tertiary/Disabled/InputActive/Strong/Alt×5),
  Strokes (Control/Strong/OnAccent/Focus outer+inner/Card/Divider/Surface/Flyout),
  Surfaces & layers (Layer, SolidBackground×5, Card×2, Subtle×4),
  Accent (Default/Secondary/Tertiary/Disabled/SelectedTextBackground,
  `SystemAccentColor` + Light1-3/Dark1-3), Acrylic family. One anecdotal
  precedent (MILESTONES §7) reads the accent live from the OS
  (`UISettings.GetColorValue` on Windows); whether we do that instead of
  shipping catalog values is an M1 evaluation, not a requirement.
  Constant-width numeric ramp values imported from the WinUI theme resource
  dump; Light/Dark/HighContrast values per catalog; HighContrast maps to
  the `SystemColor*` pairing table (never `Opacity` on HC brushes).
- REQ-TOK-02 (Must, M1): **typography ramp** — `TitleLarge`(40) `Title`(28)
  `Subtitle`(20) `BodyLarge`(18) `BodyStrong`(14 semi-bold) `Body`(14)
  `Caption`(12), `Display`(68) — plus `SymbolThemeFontFamily` indirection
  for icons.
  (Sizes per Fluent 2 type ramp [V — Fluent 2 public spec]; values in
  tokens.rs with a single override point.) [D on exact px audit at M2]
- REQ-TOK-03 (Must, M1): **spacing** 4px grid, **corner radius** tokens
  (`ControlCornerRadius`=4, `OverlayCornerRadius`=8) [V — vendored rules],
  **elevation** shadow tokens (§8.2).
- REQ-TOK-04 (Must, M2): icons — `FontIcon` (glyph font), `SymbolIcon`
  (enumerated), `PathIcon` (svg/path), `BitmapIcon`, `ImageIcon` (the
  elements ship with the control waves per §10). Windows ships **Segoe
  Fluent Icons** if present; fallback icon font (Lucide subset, as
  gpui-kit does) for cross-platform parity. [D] Segoe MDL2 as a pre-Win11
  fallback is a candidate from one anecdotal precedent (MILESTONES §7),
  evaluated at M2. The
  `SymbolThemeFontFamily` indirection ships with the M1 token
  dictionaries. `AnimatedIcon` in §9.2 (REQ-MOT-08).
- REQ-TOK-05 (Must, M1): theme switch at runtime (`RequestedTheme` on any
  subtree root) re-resolves `{ThemeResource}` without window recreation;
  `{StaticResource}` stays put — matching WinUI semantics.

## 7. Materials

### 7.1 Window backdrops (XAML: `Window.SystemBackdrop="…"`)
| Value | WinUI equivalent | Our implementation | Floor |
|---|---|---|---|
| `Mica` | `MicaBackdrop` | `WindowBackgroundAppearance::MicaBackdrop` → DWMSBT_MAINWINDOW | Win11 22H2 |
| `MicaAlt` | `MicaBackdrop{Kind=BaseAlt}` | `MicaAltBackdrop` → DWMSBT_TABBEDWINDOW | Win11 22H2 |
| `Acrylic` | `DesktopAcrylicBackdrop` (background acrylic) | `Blurred` (accent ACRYLICBLURBEHIND) | Win10 1809+ |
| `None`/default | — | `Opaque` | all |

- REQ-MAT-01 (Must, M4): the four values above, applied at window
  creation **and** live-switchable via `Window::set_background_appearance`.
- REQ-MAT-02 (Must, M4): on pre-22621 Windows, Mica falls back to
  `Opaque`+`SolidBackgroundFillColorBaseBrush`; on non-Windows: macOS
  vibrancy / Linux compositor blur where available, else opaque [V —
  upstream behavior "behave like transparent" for Mica variants; we
  override to theme-appropriate opaque for legibility]. [D]
- REQ-MAT-03 (Must, M4): **deactivated-window rule** — background acrylic
  swaps to solid when the window loses focus (system behavior we mirror in
  fallback paths) [V — acrylic doc].
- REQ-MAT-04 (Should, M4): dark-mode DWM attribute sync
  (`DWMWA_USE_IMMERSIVE_DARK_MODE`) so system-drawn Mica tints match the
  active theme. [D — small DWM call in our backend crate]
- REQ-MAT-05 (Could, upstream): `SystemBackdrop="AcrylicTransient"`
  mapping to `DWMSBT_TRANSIENTWINDOW` — requires a new
  `WindowBackgroundAppearance` variant upstream; file issue + PR. [Q]

### 7.2 Content layers above materials
- REQ-MAT-06 (Must, M4): `LayerFillColorDefaultBrush` overlay above Mica;
  `LayerOnMicaBaseAltFillColorDefaultBrush` above Mica Alt — the Fluent
  layering recipe, baked into our default page/chrome styles [V — brush
  catalog].

### 7.3 In-app acrylic (`AcrylicBrush`)
Microsoft's recipe [V — acrylic doc]: background, blur, **exclusion blend**,
tint, noise.
- REQ-MAT-07 (Must, M4): `AcrylicBrush` with `TintColor`, `TintOpacity`,
  `TintLuminosityOpacity`, `FallbackColor`, `AlwaysUseFallback` — rendered
  as: translucent tint + tiling noise texture + exclusion-style contrast
  layer (approximated as a soft dark/light overlay) over a blurred backdrop
  where the platform provides it (window-blur regions).
- REQ-MAT-08 (Must, M4): **degradation matrix** — solid `FallbackColor`
  when: Transparency effects off (registry query, Windows), Battery Saver
  active, HighContrast theme, window deactivated, remote session, or
  low-end GPU heuristics. Mirrors system behavior [V — acrylic doc].
- REQ-MAT-09 (Should, M5): **true backdrop blur of in-app content**
  requires an element-level backdrop-blur primitive gpui does not expose
  [V]. Plan: (a) ship the approximation (REQ-MAT-07) which covers menus/
  flyouts visually; (b) prototype a two-pass render path in our backend
  crate (render content beneath → blur texture → composite); (c) engage
  upstream. Gated by perf budget: blurred area × radius caps (§13.4). [Q]

### 7.4 Materials rules (enforced as perf-lint WX5xxx)
No acrylic on large opaque background surfaces; no edge-to-edge acrylic
panes (seam artifact); no accent text on acrylic (contrast failure)
[V — acrylic doc "do's and don'ts"].

## 8. Effects

### 8.1 Elevation & shadows
- Fluent tokens (multi-layer box shadows) [V — Fluent UI shadow tokens]:
  `Shadow2, Shadow4, Shadow8, Shadow16, Shadow28, Shadow64`; e.g.
  `Shadow16 = 0 0 2px rgba(0,0,0,.12) + 0 8px 16px rgba(0,0,0,.14)`.
- Control elevation convention: popups/tooltips 16, flyouts 32, dialogs 128
  (z-convention [V — vendored design skill]); `ThemeShadow.Receivers`
  concept → our `Shadow` attached property takes a token; receivers
  resolved automatically by layer order in the gpui paint stack.
- REQ-FX-01 (Must, M4): `Shadow="Shadow16"` on any element (and per-control
  defaults in styles); maps 1:1 to gpui `BoxShadow` layers.
- REQ-FX-02 (Must, M4): shadows inherit `RequestedTheme`; disabled in
  HighContrast [D per HC rules].

### 8.2 Gradients, opacity, clip
- REQ-FX-03 (Must, M4): `LinearGradientBrush` with `GradientStop` children,
  `StartPoint`/`EndPoint` (normalized), default Fluent diagonal; native
  gpui support [V].
- REQ-FX-04 (Should, M4): `RadialGradientBrush` — verify against 0.3.7
  (`linear_gradient` confirmed; radial not found [V]); if absent:
  emulate with concentric quad layers for ≤ N stops, and upstream ask.
- REQ-FX-05 (Must, M4): `Opacity`, `Clip`/rounded clip
  (`overflow_hidden` + corner radius) on any element [V].
- REQ-FX-06 (Must, M1): **focus visuals** — 2px outer ring
  `FocusStrokeColorOuterBrush` + 1px inner `FocusStrokeColorInnerBrush`
  [V — brush catalog], rendered by the focus system, not per-control code.

### 8.3 Explicitly out (with reasons)
Reveal highlight (Fluent 1 legacy, retired in Fluent 2) [D]; 3D/perspective
(N6); arbitrary pixel shaders (no surface in gpui 0.3.7 core [V]).

## 9. Motion — the full Windows 11 animation system

### 9.1 Motion tokens (primary source: MS Learn "Motion in Windows") [V]
| Token | Curve | Duration | Use |
|---|---|---|---|
| `Motion.DirectEntrance` | cubic-bezier(0,0,0,1) | 167 / 250 / 333 ms | position, scale, rotation (entrance) |
| `Motion.PointToPoint` | cubic-bezier(0.55,0.55,0,1) | 167 / 250 / 333 ms | in-view movement |
| `Motion.DirectExit` | cubic-bezier(0,0,0,1) | 167 ms | exit — **always combined with fade-out** |
| `Motion.GentleExit` | cubic-bezier(1,0,1,1) | 167 ms | position, scale |
| `Motion.Fade` | linear | 83 ms | opacity only |
| `Motion.ElasticEntrance` | 3 keyframes: CB(0.85,0,0,1)@167 → CB(0.85,0,0.75,1)@167 → CB(0.85,0,0,1)@333 | 667 ms total | strong entrance |
| Springs | `SpringConfig` (stiffness/damping/mass) [V] | continuous | toggles, dragging, implicit layout |

Principles [V]: connected, consistent, responsive, delightful, resourceful
("avoid custom animations where possible — use the platform's").

### 9.2 Animation engine
- REQ-MOT-01 (Must, M4): fluent-token API in Rust and XAML:
  `Animations="Entrance(Fade+SlideUp, DirectEntrance-250)"`,
  `Transition="Page(NavigationSlide)"`, etc. Token table is data, not code.
- REQ-MOT-02 (Must, M4): **keyframe sequencer** on top of the gpui
  animation element's easing closure [V]: multi-keyframe tracks (Elastic
  Entrance), property targets (offset/scale/opacity/rotation), duration
  mapping. This is the one engine piece we author; springs and easing
  come from gpui. [D]
- REQ-MOT-03 (Must, M4): **stagger** (per-child delay, capped: max 20 items
  × 16.7ms before collapsing to uniform delay) for list/collection
  entrances. [D, perf-driven]
- REQ-MOT-04 (Must, M5): **page transitions** — NavigationView/Frame-style:
  entrance slide-up for top-level, slide left/right for peer navigation,
  drill-in for forward; matches OS Settings conventions [V — motion doc].
  Implemented as view-level animation choreography on navigation events.
- REQ-MOT-05 (Must, M5): **connected animation** — shared-element
  transition between two trees (list item → detail hero). Two-phase:
  capture source bounds → animate transform of target from source bounds
  with PointToPoint curve. [D — buildable from verified primitives]
- REQ-MOT-06 (Must, M5): **micro-interactions** in default control styles:
  button hover/press (fill token transitions + 1px scale press via spring),
  ToggleSwitch thumb travel (spring), CheckBox check draw (opacity/scale),
  ComboBox open (GentleExit reverse + fade), flyout/dialog entrances with
  correct elevation ramps.
- REQ-MOT-07 (Must, M5): **continuous animations** — indeterminate
  ProgressBar/ProgressRing (repeat), InfoBadge pulse, skeleton shimmer
  (animated gradient [V — linear_gradient]).
- REQ-MOT-08 (Should, M6): **AnimatedIcon** — Lottie-class vector
  animation. Strategy: keyframe/vector renderer over gpui paths; source
  format decision (Lottie JSON via a Rust parser vs. our compact format)
  is an M6 spike. Fallback: static glyph. [Q]
- REQ-MOT-09 (Must, M4): **reduced motion** — OS "show animations" setting
  (SPI_GETCLIENTAREAANIMATION on Windows) and `Animations="None"` override
  disable all non-essential motion; fades reduced to ≤ 83ms opacity only.
- REQ-MOT-10 (Should, M5): **theme switch transition** — 83ms cross-fade
  of color layers (Fade token), never layout animation.

### 9.3 Motion performance rules (hard requirements — §13 enforcement)
Animate **transform + opacity only** on the per-frame path (gpui quads are
GPU-composited; no re-layout) [D grounded in verified gpui animation model].
Layout-affecting animation (width/height/margin) requires explicit opt-in
and never runs on lists. Blur/tint animation on materials is capped
(§13.4). Concurrent animation budget per window: 64 active tracks
(measured, tunable) [D].

## 10. Controls — the complete WinUI 3 catalog

Source: Microsoft's controls index (2026-09-19) [V]. "Strategy" = gpui
mapping. Tier: **M2** (wave 1: shell of every app), **M6** (wave 2:
complex/auxiliary). Every control ships with: properties per WinUI docs,
visual states (default/hover/pressed/disabled/focused/selected/error) via
tokens, keyboard support, accesskit role/name, animations per §9.2.

**Layout & containers**
| Control | Tier | Strategy |
|---|---|---|
| Grid (`*`/px/Auto rows+cols) | M2 | taffy grid or flex emulation + WinUI star-sizing pre-pass [D] |
| StackPanel, WrapPanel†, UniformGrid? (via ItemsRepeater layouts) | M2 | flex row/col + wrap; †WrapPanel is experimental-only (2.0-exp) in real WinUI 3 [V] — we ship it as a dialect extension [D] |
| Border, Viewbox, Canvas (absolute) | M2 | styled container; taffy position:absolute |
| ScrollViewer (pan, zoom), ScrollBar (standalone) | M2 | gpui scrollable + scrollbars styled to Fluent; zoom modes, snap points, per-axis chaining, deferred scrolling at M6 [V — ScrollViewer API] |
| ScrollView / ScrollPresenter (modern API, WinAppSDK 1.4 [V]) | M6 (Should) | same engine, second surface |
| SplitView, Expander, TwoPaneView | M2 / M6 | split panes; expander animates height (opt-in) per §9.3 |
| RelativePanel | M6 | constraint solver pre-pass → absolute layout |
| ItemsRepeater (+ layouts: Stack/Grid/Uniform) | M2 | virtualized list engine core |
| AnnotatedScrollbar, SemanticZoom | M6 | scrollbar annotations; zoom-overview pair (Tier C if low demand) [D] |

**Text**
| Control | Tier | Strategy |
|---|---|---|
| TextBlock (wraps, TextTrimming, TextWrapping, inline runs) | M2 | DirectWrite-backed text element |
| TextBox (multi-line, AcceptsReturn, placeholder, clear button) | M2 | editable text (gpui editor lineage), `UpdateSourceTrigger` semantics per WinUI [V — vendored rule] |
| PasswordBox (reveal peek) | M2 | masked editing |
| AutoSuggestBox (QueryIcon, suggestions flyout) | M6 | TextBox + acrylic flyout list |
| NumberBox (spin, validation, algebraic evaluation?) | M6 | validation + up/down; expression eval opt-in [D] |
| RichTextBlock / RichEditBox | M6 | markdown/HTML-backed rich text (gpui-kit precedent) — full ITextDocument parity out of scope [D] |

**Buttons & commanding**
| Control | Tier | Strategy |
|---|---|---|
| Button, RepeatButton, HyperlinkButton, ToggleButton | M2 | styled interactive div; hover/press tokens + micro-animations |
| DropDownButton, SplitButton, ToggleSplitButton | M6 | button + flyout anchor |
| AppBarButton, AppBarToggleButton, AppBarSeparator, CommandBar, CommandBarFlyout | M6 | overflow logic (More flyout) per design-skill rules |

**Collections**
| Control | Tier | Strategy |
|---|---|---|
| ListView / GridView (+ headers, selection modes) | M2 | **virtualized** (uniform_list + variable-height engine); no `ScrollViewer` wrap allowed (lint WX5001) |
| ListBox (legacy) | M6-C | ListView alias — MS guidance recommends ListView [V] |
| ItemsView (+ layouts) | M6 | same engine, layout abstraction |
| TreeView | M6 | virtualized tree (sum-tree lineage in gpui) |
| FlipView, PipsPager | M6 | paged container + pips |
| SwipeControl, RefreshContainer + RefreshVisualizer | M6/C | gesture-driven; touch-first; desktop-low priority [D] |

**Input & selection**
| Control | Tier | Strategy |
|---|---|---|
| CheckBox, RadioButton(s) + RadioButtons group | M2 | stateful visuals + keyboard groups |
| ToggleSwitch | M2 | spring thumb (§9.2) |
| Slider (+ ticks, ranges) | M2 | drag + keyboard + tooltip value |
| ComboBox (editable?) | M6 | virtualized popup list, acrylic flyout |
| RatingControl, ColorPicker | M6 | star interaction; HSV spectrum (gradient effects [V]) |
| CalendarDatePicker, CalendarView, DatePicker, TimePicker | M6 | calendar grid virtualization; locales from chrono |

**Dialogs, flyouts, status**
| Control | Tier | Strategy |
|---|---|---|
| ContentDialog (modal, scrim) | M6 | overlay layer + elevation Shadow64 + entrance animation |
| Flyout, MenuFlyout, MenuBar, Popup, ToolTip | M6 | **overlay/floating layer** with acrylic surfaces (§7.3), light dismiss, correct anchor positioning |
| TeachingTip | M6 | content-rich flyout w/ tail |
| InfoBar, InfoBadge, ProgressBar, ProgressRing, PersonPicture | M2/M6 | status set; ring indeterminate per §9.2 |

**Navigation & chrome**
| Control | Tier | Strategy |
|---|---|---|
| NavigationView (Left/Top/Compact/Minimal, Settings item) | M6 | pane + content + back stack; pane acrylic in Compact/Minimal per design rules [V — acrylic doc] |
| TabView (+ document tabs, tear-out?) | M6 | tab strip + content; tear-out = multi-window move [D, Tier C] |
| BreadcrumbBar, SelectorBar, Pivot (legacy) | M6 | bar controls; Pivot = SelectorBar compat alias |
| TitleBar | M2 | custom titlebar via gpui TitlebarOptions; drag region + caption buttons |
| Window (SystemBackdrop, min size, DPI) | M2 | §7.1 |
| SystemBackdropElement | M4 | in-app backdrop host element; shipped in WinAppSDK 2.0.1 [V] |

**Media & shapes**
| Control | Tier | Strategy |
|---|---|---|
| Image (+ ImageBrush, nine-grid?) | M2 | async decode, atlas cache |
| Shapes (Rectangle, Ellipse, Line, Path, Polygon, Polyline) + Geometry | M2 | gpui Path/SVG tessellation |
| Icons (Font/Symbol/Path/Bitmap/Image + AnimatedIcon) | M2/M6 | §6, §9.2 |
| MediaPlayerElement | **out of scope v1** (media stack; revisit) [D] — shipped in real WinUI 3 at 1.2 [V] | — |
| MapControl | out (N7) | tile service + geo stack [D]; shipped in real WinUI 3 at 1.5 [V] |
| SwapChainPanel | out v1 | exists in WinUI 3 [V]; no foreign-swapchain hosting in gpui [D] |
| InkCanvas | out (N4) — experimental-only in stable WinUI 3 [V] | — |

**Cross-cutting control requirements**
- REQ-CTRL-01 (Must): virtualization mandatory for ListView/GridView/
  ItemsView/TreeView/ComboBox lists — a collection control renders only
  the viewport (+overscan); memory bounded.
- REQ-CTRL-02 (Must): every interactive control has the **full visual-state
  set** (rest/hover/pressed/focused/disabled/selected/error) driven by
  tokens — the vendored "build custom UI only if…" checklist is our bar.
- REQ-CTRL-03 (Must): keyboard complete: tab order, arrow navigation in
  lists/menus/radios, accelerators (KeyStroke → action), focus trap in
  dialogs.
- REQ-CTRL-04 (Should): `ItemsControl`-family shares one item-source
  contract (observable vector + incremental render state).

## 11. Data binding & MVVM

- REQ-BIND-01 (Must, M3): **`{x:Bind}` = compiled bindings**: build step
  parses XAML → generates typed accessor code against app view-models.
  Semantics per WinUI: **OneTime default**, `Mode=` and inherited
  `x:DefaultBindMode` override [V — vendored rule]; nullable paths require
  `FallbackValue` (lint WX2001).
- REQ-BIND-02 (Must, M3): `#[derive(XamlViewModel)]` — property registry +
  change notifications (the `INotifyPropertyChanged` analog); partial
  struct introspection via macro; collections get an observable-vector
  analog with granular diffs (never whole-list reset for paged updates
  [V — vendored MVVM rules re ObservableCollection]).
- REQ-BIND-03 (Must, M3): two-way with **`UpdateSourceTrigger`**
  (`TextBox.Text` = LostFocus default, others PropertyChanged) [V].
- REQ-BIND-04 (Must, M3): converters as **static functions preferred**
  (`{x:Bind local:MainPage.BoolToVisibility(Vm.IsLoading)}` pattern [V]),
  `IValueConverter`-style trait for parity; `Converter={x:Null}` rejected
  (WX2xxx, it crashes real WinUI [V — vendored rule]).
- REQ-BIND-05 (Should, M3): `{Binding}` runtime path engine for tooling/
  hot-reload paths (slower; documented cost).
- REQ-BIND-06 (Must, M3): commands (`ICommand` analog) + async commands
  (re-entrancy guard), CanExecute re-query on property change.

## 12. Windowing & system integration

- REQ-WIN-01 (Must, M2): Window options surface in XAML: size/min-size,
  TitleBar customization, SystemBackdrop (§7), icon, startup position;
  **window sizing rubric** (width = widest row + 48, height sum + 48,
  rounded to 20 — vendored rule) implemented as a gallery helper [V].
- REQ-WIN-02 (Must, M2): per-monitor DPI correctness (epx units internal;
  physical-pixel APIs documented as scaled, mirroring the `AppWindow.Resize`
  DPI landmine [V — vendored rule]).
- REQ-WIN-03 (Should, M7): multi-window; windows carry their own theme +
  backdrop; taskbar/jump-list via gpui destination_list [V — backend file].
- REQ-WIN-04 (Could, M7): drag & drop between windows and OS; clipboard
  (rich text = RTF/HTML subset [Q]).

## 13. Performance requirements

> Performance is a requirement with budgets and CI gates, not a hope. The
> host frame budget at 120 Hz is **8.33 ms**; gpui redraws retained views
> on notify, and our tree rebuild must fit inside the app-side slice.

### 13.1 Budgets (Must, enforced by CI benchmarks from M2)
| Budget | Target | Measured by |
|---|---|---|
| Launch → first frame visible | ≤ 300 ms (cold, gallery app) | bench + instrumented run |
| XAML parse throughput | ≥ 1 MB/s single-thread | parse bench |
| Hot-reload reparse+rebuild (≤ 5k lines) | ≤ 50 ms to first frame | reload bench |
| Frame: input dispatch | ≤ 0.5 ms | tracing spans |
| Frame: style/property resolution | ≤ 0.5 ms | tracing |
| Frame: layout (taffy pass) | ≤ 3.0 ms (typical settings page) | tracing |
| Frame: element build (XAML tree → gpui elements) | ≤ 2.0 ms | tracing |
| List scroll, 100k items | ≥ 90 fps sustained, bounded memory | headless scroll bench [V — gpui test platform] |
| Static-text shaping | cached; atlas hit-rate ≥ 95% | text bench |
| Memory: gallery, 10k-row list open | ≤ 150 MB RSS | bench harness |
| Memory: idle baseline | ≤ 80 MB RSS | bench harness |
| Diagnostics: debug overlay (fps, frame breakdown) | always available (debug builds) | — |

### 13.2 Architecture rules (Must)
1. **Re-render scoping:** `cx.notify()` on the *notified subtree's* entity;
   a button press never rebuilds an unobserved list (PoC lesson
   generalized; verified mechanism [V — gpui entity model]).
2. **Virtualization everywhere** (REQ-CTRL-01). A `StackPanel` with > 200
   children from a collection triggers WX5002 lint.
3. **Measure/cache:** property/style resolution is a hash-consed lookup;
   brush/theme resolution memoized per theme generation; element subtree
   rebuild memoized on unchanged inputs (identity-keyed).
4. **Fonts/text:** shape once per (family, size, weight, script, string);
   LRU shaping cache; glyph atlas maintained by gpui [V].
5. **Images:** decode off-thread, atlas upload, downsample to draw size.

### 13.3 XAML-side performance (mirrors WinUI guidance [V — vendored rules])
- `x:Load`-analog: `Deferred="true"` on subtrees (dialog/panel content)
  builds elements on first show.
- `x:Phase`-analog: staged item-template binding for long lists.
- Compiled `{x:Bind}` is the default path; `{Binding}` is the opt-in slow
  path (§11).
- Perf lints (WX5xxx): hardcoded color where a token exists; blur radius ×
  area over budget; unvirtualized collection; `Deferred` missing on
  below-fold heavy content; animation on layout properties; stacked
  acrylic panes; > 5 concurrent continuous animations per window.

### 13.4 Materials & effects budget (Must, M4)
- Per-frame blurred surface cap: **≤ 1 viewport at r=30** or **≤ 4 panes at
  r=10** (auto-degrade beyond, per §7.3 matrix) [D, tunable; grounded in
  acrylic GPU-cost guidance V].
- Shadows: ≤ 8 elevated surfaces visible; shadow layers are quads (cheap)
  [V — BoxShadow model].
- Indeterminate progress ≤ 4 concurrent per window before collapse to
  shared timer [D].

### 13.5 Benchmark suite (Must, M2 onward)
Headless (gpui test platform [V]): parse corpus, settings-page layout,
100k-row scroll, theme-switch storm, animation load (32 springs +
16 fades). Golden-image snapshots for the visual set. Run on CI Windows +
Linux; regression = budget table violation blocks merge.

## 14. Accessibility (continuous requirement, audit at M7)

- REQ-A11Y-01 (Must): every control emits accesskit nodes with correct
  role, name, state; `AutomationProperties.AutomationId/Name` attached
  properties map 1:1 [V — accesskit in gpui deps; vendored checklist].
- REQ-A11Y-02 (Must): icon-only controls require names (WX4xxx lint).
- REQ-A11Y-03 (Must): HighContrast theme correctness — `SystemColor*`
  pairing table only, no opacity on HC brushes, no color-only information
  [V — vendored theming reference].
- REQ-A11Y-04 (Must): contrast ≥ 4.5:1 for text on materials; accent-on-
  acrylic forbidden by default style lint [V — acrylic doc].
- REQ-A11Y-05 (Must): hit targets ≥ effective 32px; focus visuals per
  REQ-FX-06; reduced-motion per REQ-MOT-09.

## 15. Milestones & acceptance criteria

_REQ milestone tags were reconciled against this table on 2026-10-04 during
the project review; where they ever disagree, this table is authoritative._

| M | Scope (PRD refs) | Acceptance |
|---|---|---|
| **M0 — PoC** ✅ | dialect skeleton, 5 elements, event loop (FEASIBILITY.md) | done, on gpui 0.2.2 |
| **M1 — Foundation** | port to gpui-pre `"0.3"`; crate split; grammar+catalog+diagnostics (§5); tokens Light/Dark/HC (§6); resources/styles; StaticResource/ThemeResource; focus visuals; events (REQ-EVT-01); host window shell (the XAML `Window` element is REQ-WIN-01 at M2) | settings-page XAML renders in Light+Dark+HC with zero silent errors; parse bench ≥ 1 MB/s; theme switch live |
| **M2 — Controls wave 1** | Grid star sizing; ScrollViewer; TextBox/PasswordBox; button family; CheckBox/RadioButton/ToggleSwitch; Slider; ProgressBar/Ring; Image; shapes; FontIcon; ListView virtualized; DataTemplate/`x:DataType` (REQ-RES-03); benchmark suite live (§13.5) | gallery page per control; 100k-row scroll ≥ 90 fps; budgets green in CI |
| **M3 — Binding/MVVM** | §11 complete | gallery drives a view-model with no manual wiring; `x:Bind` codegen type-errors fail the build |
| **M4 — Materials & effects** | §7.1–7.2, §7.3 (approx), §8.1–8.3; degradation matrix; motion tokens + engine (§9.1–9.3), entrances/exits/stagger | Mica/MicaAlt/Acrylic demo on Win11 22H2 + fallbacks verified on Win10-class and HC; shadow/gradient/focus-visual golden images |
| **M5 — Motion & overlays** | page transitions; connected animation; micro-interactions; continuous; flyouts/dialogs/tooltips with acrylic; reduced-motion | transition gallery matches OS-convention choreography; 60+ fps under animation load bench |
| **M6 — Controls wave 2** | NavigationView, TabView, ComboBox, menus, ContentDialog, InfoBar/TeachingTip, TreeView, ItemsView, pickers/calendar, ColorPicker, NumberBox, AutoSuggestBox, RichTextBlock, SelectorBar/Breadcrumb, Expander, Swipe/PTR (C) | full-catalog gallery ships; per-control a11y + visual-state checklist passes |
| **M7 — Hardening & DX** | hot reload; editor schema/LSP; `xaml-lint`; i18n/RTL; a11y audit (§14); multi-window; docs | edit-XAML → update < 1 s; audit scorecard 100% Must-items |

Feature-level breakdown — every WinUI 3 feature assigned to a milestone or
to an explicit-out entry with a reason — lives in
[MILESTONES.md](MILESTONES.md); it expands this table, never replaces it.

## 16. Risks & open questions (new/changed only — PLAN.md §6 holds the rest)

| # | Risk / question | Impact | Handling |
|---|---|---|---|
| R1 | Element-level backdrop blur absent in gpui → in-app acrylic is approximate | Visual fidelity of menus/flyouts | Two-pass prototype in backend crate; upstream proposal; approximation ships meanwhile (§7.3) |
| R2 | Radial gradient not found in 0.3.7 | ColorPicker, some fills | Emulation + upstream ask (§8.2) |
| R3 | Keyframe animation not first-class | Elastic Entrance, choreography | Our sequencer on easing closures (§9.2) — low complexity |
| R4 | `runtime_shaders` surface (Linux facade only) unclear | Exclusion-blend fidelity in acrylic | Spike in M4; fallback contrast overlay [Q] |
| R5 | Grid star-sizing on taffy | All layout fidelity | Pre-pass expansion before taffy; benchmark with pathological grids |
| R6 | AnimatedIcon source format | M6 | Spike: Lottie-subset vs custom [Q] |
| R7 | Snapshot-line drift (0.3 caret policy) | Build breakage | Weekly update+test cadence; gpui isolated in backend crate (PLAN §6) |
| Q1 | Does `Blurred` accent acrylic survive DX12 alpha swapchain on all drivers? | Materials tier 1 | M4 first task: empirical matrix (this was FEASIBILITY §"unverified link" for 0.2.2 — re-verify on gpui-pre) |

## Appendix E — Verification log (primary sources)

1. MS Learn, "Motion in Windows" (updated 2026-07-14) — motion token table,
   principles, page/connected/animated-icon guidance. [V]
2. MS Learn, "Acrylic material" (updated 2026-10-02) — recipe layers,
   usage rules, degradation matrix, contrast guidance. [V]
3. MS Learn, "Windows Controls and patterns" (updated 2026-09-19) — full
   control catalog (50+). [V]
4. Vendored win-dev-skills @`2e8c902` — brush catalog, theming/HC rules,
   binding landmines, window-sizing rubric, layout review template. [V]
5. Fluent UI shadow tokens (Fluent UI React Storybook theme) — Shadow
   multi-layer definitions. [V]
6. gpui-pre 0.3.7 source: `src/style.rs` (BoxShadow), `src/styled.rs`
   (opacity, overflow_hidden), `src/color.rs` (linear_gradient),
   `src/elements/animation.rs` (easing), `src/spring.rs` (SpringConfig),
   `src/platform.rs` (WindowBackgroundAppearance incl. Mica variants). [V]
7. gpui-pre-windows 0.3.7 source: `src/window.rs` (backdrop mapping,
   DWM build gates), `src/direct_write.rs` (text stack). [V]
8. gpui-pre-platform 0.3.7 facade (per-OS backend selection, wgpu on
   Linux). [V]
9. THEME decision record: PLAN.md §1/§6 (gpui-pre `"0.3"` policy, PoC
   history), FEASIBILITY.md (M0 evidence incl. screenshot, exit codes,
   tests). [V]
10. MS Learn, "XAML overview" (2026-07-27) — the supported markup-extension
    list (8 extensions; `x:Static` absent); WinUI 3 controls index
    (2026-09-19) re-verified 2026-10-04 against WASDK 1.0–2.0 release notes
    for version attributions; x:Uid / x:Load / VSM / ScrollViewer / Frame /
    PrintDocument / ElementSoundPlayer / SwapChainPanel API pages. [V] —
    full URL list in MILESTONES.md §8.
