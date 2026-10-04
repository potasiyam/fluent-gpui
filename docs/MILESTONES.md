# MILESTONES — complete WinUI 3 feature coverage

**Status:** Draft v1 · 2026-10-04
**Owns:** the feature-level milestone ladder. [PRD §15](PRD.md) stays
authoritative for milestone scope and acceptance; this file expands it so
that **every WinUI 3 feature maps to exactly one milestone — or to an
explicit-out entry with a reason** (§4). Nothing here builds past the
current milestone; it plans the whole surface.
**Evidence policy:** claims are tagged **[V]** (verified against a source in
§7), **[D]** (a decision we own), or **[Q]** (open question), per the PRD
evidence policy. Sources fetched 2026-10-04.

## 1. What this adds beyond PRD §15

The PRD ladder already covers the dialect core (M1), controls wave 1 (M2),
binding (M3), materials/effects (M4), motion/overlays (M5), controls wave 2
(M6), and hardening/DX (M7). This doc slots the remaining WinUI 3 surface
into that ladder and records what the PRD had not yet named. Three facts
were corrected while verifying the WinUI 3 surface:

1. `{x:Static}` is **not** a WinUI 3 markup extension — the official
   supported-extension list enumerates 8 extensions and omits it [V].
   PRD §5.2 listed it at M3; corrected to out-of-dialect.
2. **WrapPanel** is experimental-only in WinUI 3 (2.0-exp) [V]. PRD §10
   ships it at M2 — kept, now tagged as a dialect extension [D].
3. The catalog was missing **ScrollBar** (standalone), **ScrollView /
   ScrollPresenter** (WinAppSDK 1.4), **ListBox** (legacy), **MapControl**
   (1.5), and **SystemBackdropElement** (2.0.1), and said "PullToRefresh"
   where the control names are **RefreshContainer + RefreshVisualizer**.
   PRD §10 rows added/corrected in the same change [V].

## 2. Ladder overview

| M | Theme | Feature areas | Acceptance |
|---|---|---|---|
| M0 ✅ | PoC | parse → render → event loop on 5 elements | done (FEASIBILITY.md) |
| M1 | Dialect foundation | grammar + catalog + diagnostics, tokens (Light/Dark/HC), resources, styles, names, events, window shell | PRD §15 |
| M2 | Controls wave 1 | layout engine, text/input/button families, virtualized lists, DataTemplate, benchmark suite | PRD §15 |
| M3 | Binding/MVVM | `{x:Bind}` codegen, runtime `{Binding}`, commands, deferred/phase | PRD §15 |
| M4 | Materials & effects | backdrops, acrylic, shadows, gradients, **transforms** | PRD §15 |
| M5 | Motion & overlays | motion engine, **VSM**, **Frame/Page navigation**, flyouts/dialogs | PRD §15 |
| M6 | Controls wave 2 | full catalog, **storyboards**, selectors, **advanced scrolling** | PRD §15 |
| M7 | Hardening & DX | hot reload, LSP, lint, **i18n/RTL**, a11y audit, **multi-window, pickers** | PRD §15 |

## 3. Feature checklists per milestone

Legend: `REQ-…` points at the PRD requirement that owns the item. Items
marked **+** are additions this doc introduces (they had no milestone in
PRD §15); they receive PRD REQ-IDs when their milestone starts (YAGNI).

### M1 — Dialect foundation

- Parser (REQ-DX-01): object/property elements, attached properties,
  content property, collection syntax, CDATA, comments, `x:` namespace
  normalization. `xml:space` and `xml:lang` accepted [V — valid in XAML
  per MS Learn]. UTF-8 and UTF-16 input [V]. `d:`/`mc:` designer
  namespaces parsed and ignored [V].
- `x:` directives in M1: `x:Key` (including implicit TargetType keys for
  styles/templates [V]), `x:Name` with per-namescope uniqueness [V],
  `x:Uid` accepted (substitution ships at M7), `x:Null`, intrinsic types
  `x:Boolean`/`x:Double`/`x:String` [V]. `x:Class` parsed and rejected
  with a diagnostic (N1) [D].
- Markup extensions (§5.2): `{StaticResource}` (element → page → app →
  theme resolution order; unresolvable key = WX diagnostic, matching
  WinUI's parse exception [V]); `{ThemeResource}` re-resolves on theme
  switch [V]; `{x:Null}` rejection rules (REQ §5.2).
- Resources (REQ-RES-01): ResourceDictionary, MergedDictionaries
  (last-added wins [V]), ThemeDictionaries **Light/Dark/HighContrast**.
  WinUI also accepts a `"Default"` theme key [V]; our lint still warns on
  it per the vendored authoring rules [D]. ResourceDictionary loaded from
  a separate file via `Source` [V]. **+**
- Styles (REQ-RES-02): `Style`/`Setter`/`BasedOn`; implicit styles keyed
  by TargetType [V]. Library default styles as embedded XAML.
- **+** Property value inheritance (FontFamily/FontSize/FontWeight/
  Foreground/FlowDirection down the tree) — WinUI semantics [D].
- Tokens (REQ-TOK-01..05), focus visuals (REQ-FX-06), events (REQ-EVT-01),
  diagnostics (REQ-DIAG-01/02), host window shell.

### M2 — Controls wave 1

- Every PRD §10 row tagged M2 (≈35 controls: Grid, StackPanel, Border,
  Viewbox, Canvas, ScrollViewer, TextBlock, TextBox, PasswordBox, button
  family, CheckBox/RadioButton/ToggleSwitch, Slider, ProgressBar/Ring,
  Image, shapes, FontIcon, ListView, TitleBar, Window), plus:
- **+** `ScrollBar` standalone control (ships with the ScrollViewer
  visuals) [V].
- WrapPanel as a dialect extension (not in stable WinUI 3) [D].
- Layout engine: Grid star sizing (R5), WinUI alignment semantics (M0
  friction list), Canvas absolute positioning, Viewbox stretch modes [D].
- Items engine: `ItemsControl`/`ContentControl`/`ContentPresenter`
  contracts [D]; `ItemsPanelTemplate` [V]; DataTemplate + `x:DataType`
  (REQ-RES-03); virtualization (REQ-CTRL-01) with uniform and
  variable-height layout modes (`ItemsStackPanel`/`ItemsWrapGrid`
  equivalents) [D].
- **+** TextBlock inline content: `Run`, `Span`, `Bold`/`Italic`,
  `Hyperlink`, `LineBreak` [D].
- **+** Custom font loading (`FontFamily="path"`) [D].
- Keyboard accelerators, tab order, full visual-state set (REQ-CTRL-02/03).
- Benchmark suite live (§13.5).

### M3 — Binding/MVVM

- §11 complete (REQ-BIND-01..06), plus these WinUI binding semantics:
- **+** `TargetNullValue` alongside `FallbackValue` on both binding kinds
  [V].
- **+** `ElementName` bindings via `{Binding}` [V]; with `{x:Bind}` the
  named element is referenced in the Path instead [V].
- **+** `RelativeSource` **Self** mode [V]. `TemplatedParent` waits on N2;
  `FindAncestor`/`PreviousData` do not exist in WinUI 3 [V].
- **+** Mode set is OneTime/OneWay/TwoWay — no OneWayToSource [V].
- **+** `UpdateSourceTrigger` = Default/LostFocus/PropertyChanged;
  `Explicit` exists for `{Binding}` only and `{x:Bind}` rejects it [V].
- **+** Built-in bool→Visibility converter (WinUI ≥ 1607 behavior) [V].
- **+** `x:Phase` staged item-template binding [V]; `x:Load` analog
  (`Deferred="true"`, §13.3) realized through our tree, not FindName [D].
- **+** `x:FieldModifier` maps to the visibility of generated `x:Name`
  accessors [D].
- **+** CollectionViewSource analog: sorted/grouped/filtered views feeding
  ItemsControls [D].

### M4 — Materials & effects

- §7.1–7.3 backdrops and acrylic, §8 effects (REQ-FX-01..05), degradation
  matrix, motion tokens + engine core (REQ-MOT-01..03, REQ-MOT-09), plus:
- **+** `RenderTransform` family: Rotate/Scale/Skew/Translate/
  Composite/TransformGroup/MatrixTransform + `RenderTransformOrigin` — all
  present in WinUI 3 [V]; mapped to gpui styled transforms [D]. Usable as
  animation targets by the §9.2 engine [D].
- **+** `SystemBackdropElement` analog (in-app backdrop host element;
  shipped in WinAppSDK 2.0.1) [V] (Should).
- Note: `SlideUpThemeAnimation` is absent from WinUI 3 (its API page 404s;
  present in UWP) [V]. Our motion token set keeps slide-up entrances as
  tokens regardless [D].

### M5 — Motion & overlays

- Motion engine (REQ-MOT-01..07, 09, 10), page transitions, connected
  animation, micro-interactions, continuous animations, flyouts/dialogs/
  tooltips overlay layer, plus:
- **+** WinUI theme-transition names as the compatibility surface —
  Entrance, Popup, EdgeUI, Pane, AddDelete, Content, Reorder, Reposition,
  NavigationThemeTransition, all present in WinUI 3 [V] — mapped onto §9.1
  tokens [D].
- **+** WinUI theme-animation names likewise: FadeIn/Out, PopIn/Out,
  PointerDown/Up, DrillIn/Out, SplitOpen/Close, SwipeBack/Hint, Drag item
  animations [V] → token mapping [D].
- **+** VisualStateManager authoring surface: `VisualStateManager.
  VisualStateGroups` attached property, `VisualStateGroup`/`VisualState`/
  `VisualTransition` (with storyboard), `AdaptiveTrigger`
  (MinWindowWidth/Height) and custom triggers — all present since WASDK
  0.8 [V]; the convenience `StateTrigger` class is unverified [Q].
  `GoToState` callable from host code and from bindings [D].
- **+** Frame/Page navigation model: `Frame` with Navigate/GoBack/
  GoForward/CanGoBack, BackStack/ForwardStack (PageStackEntry),
  `NavigationCacheMode`, Frame.CacheSize, Navigating/Navigated events —
  all present in WinUI 3 [V]. Rust API: Frame element + registered page
  factories [D]; NavigationThemeTransition as the default [D].

### M6 — Controls wave 2

- Every PRD §10 row tagged M6/C (ComboBox, menus/CommandBar, ContentDialog,
  InfoBar/InfoBadge/TeachingTip, NavigationView, TabView, TreeView,
  ItemsView, FlipView, pickers/calendar, ColorPicker, NumberBox,
  AutoSuggestBox, RichTextBlock/Editor, SelectorBar, BreadcrumbBar,
  Expander, SplitButton family, SemanticZoom, AnnotatedScrollBar,
  SwipeControl, RefreshContainer + RefreshVisualizer, PersonPicture,
  icons incl. AnimatedIcon), plus:
- **+** Storyboard subset mapped onto the §9.2 keyframe sequencer:
  `Storyboard`, `DoubleAnimation`/`ColorAnimation`/`PointAnimation`, the
  `*UsingKeyFrames` forms with Discrete/Linear/Spline/Easing keyframes,
  and the EasingFunctionBase family (Back/Bounce/Circle/Cubic/Elastic/
  Exponential/Power/Quadratic/Quartic/Quintic/Sine) — all present in
  WinUI 3 [V]. Host API for Begin/Stop/Pause/Resume [D].
- **+** DataTemplateSelector (SelectTemplate analog) [V].
- **+** Grouped collection rendering with group headers [Q — verify the
  GroupStyle surface at milestone start].
- **+** ScrollViewer advanced semantics: ZoomMode/ZoomFactor/
  ZoomSnapPoints, Horizontal/VerticalSnapPointsType + Alignment, per-axis
  scroll chaining (IsHorizontal/Vertical/ZoomScrollChainingEnabled),
  IsDeferredScrollingEnabled — all present in WinUI 3 [V]. PanningMode
  does not exist in WinUI 3 [V] and not for us.
- **+** ScrollView/ScrollPresenter modern surface analog (WinAppSDK 1.4)
  [V] (Should).
- **+** ListBox as a ListView alias (legacy; MS guidance recommends
  ListView) [V] (Could).
- **+** Custom-control authoring API: a Rust control struct registers in
  the catalog, styleable and usable from XAML; UserControl analog
  (XAML-composed reusable element) [D].
- Validation visual states (error) for TextBox/NumberBox (REQ-CTRL-02).
- Gamepad/XY focus [D] (Could).

### M7 — Hardening & DX

- Hot reload, editor schema/LSP, `xaml-lint` (REQ-DX-03), perf-lint suite
  (WX5xxx).
- **+** i18n: `x:Uid` substitution against string resources [V — x:Uid +
  resw via MRT Core in WinUI]; our store format decision (resw-compatible
  vs. flat table) [Q]; `xml:lang` semantics; RTL FlowDirection on all
  containers with mirrored gallery pages (REQ-I18N-01); OS text-scaling
  factor honored [D].
- A11y audit (§14): full `AutomationProperties` set (Name, AutomationId,
  AcceleratorKey, …) [V], accesskit roles, HighContrast, reduced motion.
- Multi-window (REQ-WIN-03).
- **+** File/folder pickers via native dialogs [D — OS APIs; a crate like
  rfd only if cross-platform parity is demanded].
- **+** Window presenter modes (fullscreen, size limits) [D].
- Drag & drop + clipboard (REQ-WIN-04, Could).

## 4. Explicit-out table (in WinUI 3, out of our v1)

| Feature | In WinUI 3? | Why out | Tag |
|---|---|---|---|
| `x:Class` code-behind (C#/C++) | Yes | N1 — the Rust host owns wiring; `x:Class` rejected with a diagnostic | D |
| `ControlTemplate` + `{TemplateBinding}` | Yes | N2 — v1 restyles via tokens; template engine is a v2 evaluation | D |
| WebView2 control | Yes (0.8+) | N3 — `gpui-wry` is the designated future path | D |
| InkCanvas / InkToolbar | **Experimental only** (2.0-exp; not in any stable channel) | N4 — not even stable in real WinUI 3 | V |
| ExpressionAnimation (expression strings) | Yes | N5 — authoring paradigm excluded; the token API covers Fluent motion | D |
| 3D Projection / perspective | Yes | N6 | D |
| MediaPlayerElement + MediaTransportControls | Yes (1.2+) | media stack out of v1 | D |
| MapControl | Yes (1.5+) | needs an online tile service and a geo stack | D |
| Printing (`PrintDocument` + `PrintManagerInterop`) | Yes (0.8+) | Windows-only pipeline; no print path in gpui; v2 PDF-emission evaluation | D |
| SwapChainPanel | Yes (0.8+) | gpui composites its own surfaces; no foreign-swapchain hosting | D |
| ElementSoundPlayer | Yes (0.8+) | gpui audio surface unverified [Q]; sound is optional polish; revisit v2 | D |
| `{CustomResource}` | Yes | host-loader hook; v2 Could | D |
| `x:Array` | Not documented for WinUI (WPF/MAUI only) | out — unverified in WinUI 3 | V |
| `{Binding} RelativeSource=TemplatedParent`; FindAncestor/PreviousData | TemplatedParent yes; others no | rides with N2 | V/D |
| OneWayToSource binding mode | No | not in WinUI 3; our mode set matches | V |
| `UpdateSourceTrigger=Explicit` on `{x:Bind}` | No (Binding only) | parity | V |
| Behaviors SDK (Microsoft.Xaml.Behaviors) | NuGet package, not core | ecosystem, not the XAML surface | D |
| WinAppSDK app platform (app lifecycle, background tasks, push, app-to-app, share, MSIX) | Yes (non-XAML) | not the XAML/UI surface this library reimplements | D |

## 5. Coverage accounting

Counted from PRD §10 after this update (91 named controls/patterns):

- ~35 ship at M2, ~50 across M4–M6 (3 of those Could-tier), ~6 explicit-out
  (MapControl, MediaPlayerElement, SwapChainPanel, InkCanvas, InkToolbar,
  WebView2 via N3).
- Markup extensions: WinUI 3 supports exactly 8 [V]. We cover 7 by M3 —
  StaticResource, ThemeResource, x:Null (M1); x:Bind, Binding,
  RelativeSource=Self (M3); TemplateBinding waits on N2 — and
  `{CustomResource}` is v2-Could. `{x:Static}` is correctly absent.
- `x:` directives: all 8 supported ones (Key, Name, Uid, DataType, Load,
  Phase, DefaultBindMode, FieldModifier) covered by M7; x:Class rejected (N1).
- Binding feature set (Mode/Trigger/Converter/FallbackValue/TargetNullValue/
  ElementName/RelativeSource): complete at M3.

## 6. Ladder rules

- PRD §15 stays the authoritative milestone table; when this file and §15
  ever disagree, §15 wins and this file gets fixed the same turn.
- A milestone starts by promoting its `+` items into PRD REQ-IDs.
- New WinUI features Microsoft ships (WinAppSDK releases) get a row here in
  the same change that adds them to the catalog — coverage stays provable.

## 7. Evidence sources (fetched 2026-10-04)

1. MS Learn, "XAML overview" (2026-07-27) — the supported markup-extension
   table (8 extensions; x:Static absent), x: namespace table, namescopes,
   xml:lang, encodings. https://learn.microsoft.com/en-us/windows/apps/develop/platform/xaml/xaml-overview
2. MS Learn, "Controls and patterns for Windows apps" index (2026-09-19),
   re-verified with the UWP→WinUI migration table ("What's supported").
   https://learn.microsoft.com/en-us/windows/apps/design/controls/index ·
   https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/migrate-to-windows-app-sdk/what-is-supported
3. WinAppSDK release notes 1.0, 1.2, 1.4, 1.5, 1.6, 1.7, 2.0 — version
   attributions (Expander/BreadcrumbBar/PipsPager 1.0; InfoBadge and
   MediaPlayerElement 1.2; ItemsView/AnnotatedScrollBar/ScrollView 1.4;
   SelectorBar/MapControl 1.5; TitleBar 1.7; WrapPanel and InkCanvas
   experimental in 2.0; SystemBackdropElement 2.0.1).
   https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/release-notes/
4. MS Learn API pages: StaticResource/ThemeResource/Binding/TemplateBinding/
   RelativeSource/x:Null/x:Bind markup extensions; x-uid-directive;
   x-load-attribute; x-fieldmodifier-attribute; xaml-resource-dictionary;
   VisualStateManager/AdaptiveTrigger; Storyboard + Microsoft.UI.Xaml.Media.
   Animation namespace; ConnectedAnimation; Frame; Page; ScrollViewer;
   PrintDocument; ElementSoundPlayer; SwapChainPanel; DataTemplateSelector;
   ListBox; MapControl; TitleBar. (Under
   https://learn.microsoft.com/en-us/windows/apps/develop/platform/xaml/ and
   https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/)
5. MS Learn, "Data binding in depth" — UpdateSourceTrigger values, Explicit
   restriction, FallbackValue/TargetNullValue, mode comparison.
   https://learn.microsoft.com/en-us/windows/apps/develop/data-binding/data-binding-in-depth
6. MS Learn, "Print from your app" — WinUI 3 printing via PrintManagerInterop.
   https://learn.microsoft.com/en-us/windows/apps/develop/devices-sensors/print-from-your-app
