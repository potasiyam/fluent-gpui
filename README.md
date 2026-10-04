# WinUI-GPUI-XAML

**WinUI 3 / Fluent 2-flavored XAML, rendered by [GPUI](https://github.com/zed-industries/zed) (Zed's GPU UI framework).**
"Fluent 2 for Rust, authored in XAML, rendered by GPUI."

Status: **M0 (feasibility PoC) complete.** The PoC parses a WinUI-flavored
XAML page at runtime (StackPanel / TextBlock / Border / Button, `{x:Bind}`,
`Click=` handlers) and renders it on Windows via gpui 0.2. M1 — the port to
the `gpui-pre` `"0.3"` snapshot family and the full markup surface — has not
started.

## Documents

| Doc | Role |
|---|---|
| [PRD.md](PRD.md) | **Authoritative requirements** — XAML dialect, tokens, materials, effects, motion, controls, binding, performance budgets, milestone ladder M0–M7 |
| [PLAN.md](PLAN.md) | Architecture, dependency decisions (the `gpui-pre` `"0.3"` policy), feasibility record |
| [FEASIBILITY.md](FEASIBILITY.md) | PoC test protocol, results, friction list |

## Run the PoC

```bash
cargo run                       # window stays open; click the button
POC_AUTOCLOSE_SECS=4 cargo run  # renders 4 s, quits, exit 0 = pass
cargo test                      # headless checks
```

## Layout

```
src/xaml.rs                  XAML -> DOM parser (roxmltree)
src/render.rs                DOM -> GPUI element mapping
src/main.rs                  Host app + tests
assets/demo.xaml             PoC page
references/win-dev-skills/   Microsoft's WinUI agent skills (MIT, pinned @2e8c902)
```

License: not yet chosen for this project (the vendored win-dev-skills is MIT).
