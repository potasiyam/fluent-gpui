# Vendored: microsoft/win-dev-skills (winui-design + winui-code-review)

Upstream: https://github.com/microsoft/win-dev-skills
Pinned at commit `2e8c902e9ba675e4c57550e066fc7e48b64c49b0` (2026-10).
License: MIT (see `LICENSE`, `THIRD_PARTY_NOTICES.md` in this folder).

## Why this is vendored

These two Agent Skills are Microsoft's authoritative, distilled statement of
what *correct* WinUI 3 XAML looks like — binding semantics, theming rules,
accessibility requirements, control selection, and a review checklist. For
this project they serve as the **semantic contract** that our XAML subset
must honor, and the source of our parser-diagnostics backlog. They are
reference material for library design, not build tooling.

## What applies to us vs. what doesn't

| Applies to winui-xaml (this project) | Does not apply |
|---|---|
| `x:Bind` mode semantics (default **OneTime**, `x:DefaultBindMode`, `Mode=`) | `winapp` CLI / `find-ui` / `find-api` (real WinUI toolchain) |
| `UpdateSourceTrigger` behavior (`TextBox.Text` defaults to LostFocus) | C# / MVVM / CommunityToolkit specifics (`[ObservableProperty]`, `ObservableObject`) |
| `{ThemeResource}` vs `{StaticResource}` rules; Light/Dark/**HighContrast** dictionaries; purpose-named brushes | Native AOT / trimming guidance |
| Typography styles (`TitleTextBlockStyle`…), 4px spacing grid, `ControlCornerRadius` tokens | MSIX packaging / Store / signing |
| Attached-property syntax (`Grid.Row`, `AutomationProperties.*`) | Windows Sandbox UI testing (we drive gpui windows directly) |
| Accessibility: `AutomationProperties.AutomationId`/`Name`, semantic controls, no color-only meaning | WPF migration tables (loosely useful; our control map comes from the design skill) |
| Anti-pattern table → our linter warnings (hardcoded colors, `ScrollViewer` around `ListView`, clickable `Border`, placeholder-as-label…) | Analyzer WUI0xxx–WUI4xxx rule IDs (we'll define our own `XX` diagnostic codes, using their categories as inspiration) |
| App-shape anchors table → our gallery/example apps | Window sizing C# snippets (we'll implement the *rubric* in Rust) |

The `winui-design/references/` subfolder (brushes-and-icons, theme-accessibility,
layout-review) is loaded on demand when designing our theme dictionaries and
control styles; `winui-code-review/references/quality-rules.md` feeds the M1
diagnostics work.
