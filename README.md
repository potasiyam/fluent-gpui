# fluent-gpui

**WinUI 3–flavored XAML, rendered by [GPUI](https://github.com/zed-industries/zed) — Fluent 2 for Rust, authored in XAML.**

You write XAML:

```xml
<StackPanel Spacing="8" Padding="24">
    <TextBlock Text="Hello" FontSize="32" FontWeight="Bold"/>
    <Button Content="Click me" Click="OnClicked"/>
</StackPanel>
```

…and the library parses it, maps it to GPUI's element tree, and handles
styling, layout, and events. Host code stays Rust; the markup stays
declarative.

## What it will do

- The WinUI 3 control catalog with Fluent 2 design tokens
  (Light / Dark / HighContrast)
- Mica and Acrylic materials, Windows 11 motion
- `{x:Bind}`-style compiled data binding, hot reload, fail-loud diagnostics
- Cross-platform: Windows (DirectX), macOS (Metal), Linux

This is a re-implementation of the XAML authoring surface, not WinUI binary
compatibility.

## Status

Feasibility PoC (M0) complete — a XAML page parses at runtime, renders to a
live window, and `Click=` handlers mutate Rust state through bindings.
**More coming soon.**

## Docs

| Doc | Role |
|---|---|
| [PRD](docs/PRD.md) | Authoritative requirements |
| [MILESTONES](docs/MILESTONES.md) | Feature-level ladder, M0–M7 |
| [PLAN](docs/PLAN.md) | Architecture, dependency decisions |
| [FEASIBILITY](docs/FEASIBILITY.md) | PoC evidence and friction list |

## Run the PoC

```bash
cargo run                       # window stays open; click the button
POC_AUTOCLOSE_SECS=4 cargo run  # renders 4 s, quits, exit 0 = pass
cargo test                      # headless checks
```

## Layout

```
src/                 XAML parser, GPUI renderer, host app
assets/demo.xaml     PoC page
docs/                Project documentation
references/          Vendored win-dev-skills (MIT, pinned @2e8c902)
```

## License

MIT — see [LICENSE](LICENSE). The vendored `references/win-dev-skills` is
also MIT.
