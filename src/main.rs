//! Feasibility PoC host: load a WinUI-style .xaml file and render it with
//! GPUI through the winui-xaml crates.
//!
//! Run: `cargo run` (or `POC_AUTOCLOSE_SECS=4 cargo run` for a self-closing
//! run used by the automated feasibility check).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    App, Bounds, Context, Render, TitlebarOptions, WindowBackgroundAppearance, WindowBounds,
    WindowOptions, prelude::*, px, size,
};
use winui_xaml_gpui::{HandlerFn, XamlContext, render_element};
use winui_xaml_model::{
    Catalog, Diagnostic, ResourceSet, Severity, SourceResolver, Theme, check_resource_references,
    check_style_chains, collect_resources, render_diagnostics, resolve, resolve_styles,
};
use winui_xaml_syntax::{XamlElement, parse};

/// Host state the XAML can reach through `{x:Bind PropertyName}`.
#[derive(Default)]
struct AppState {
    clicks: u32,
}

struct DemoApp {
    tree: XamlElement,
    state: AppState,
    handlers: HashMap<String, HandlerFn<DemoApp>>,
    resources: ResourceSet,
    theme: Theme,
    backdrop: WindowBackgroundAppearance,
}

/// The text `{x:Bind ClicksText}` resolves to; shared by renderer and tests.
fn clicks_binding_text(clicks: u32) -> String {
    format!("Button clicked {} time(s)", clicks)
}

/// Resolved from the manifest dir; the process CWD is not guaranteed.
fn demo_xaml_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/demo.xaml")
}

/// The embedded Fluent 2 token catalog (REQ-TOK-01..04). It merges ahead
/// of app resources through `merge_app_dictionary`, so page and app
/// definitions win over tokens — WinUI's element → page → app lookup.
const FLUENT_TOKENS_XAML: &str =
    include_str!("../crates/winui-xaml-model/assets/fluent-tokens.xaml");

/// PoC theme selection: `POC_THEME=Light|Dark|HighContrast`. Dark is the
/// default until OS theme detection lands (REQ-TOK-01); an unknown value
/// fails the run loudly.
fn theme_from_name(name: &str) -> Result<Theme, String> {
    match name.trim() {
        "Light" => Ok(Theme::Light),
        "Dark" => Ok(Theme::Dark),
        "HighContrast" => Ok(Theme::HighContrast),
        other => Err(format!(
            "POC_THEME=\"{other}\" is not Light, Dark, or HighContrast"
        )),
    }
}

fn poc_theme() -> Theme {
    match std::env::var("POC_THEME") {
        Err(_) => Theme::Dark,
        Ok(name) => match theme_from_name(&name) {
            Ok(theme) => theme,
            Err(message) => {
                eprintln!("[xaml] {message}");
                std::process::exit(1);
            }
        },
    }
}

impl Render for DemoApp {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut bindings = HashMap::new();
        bindings.insert(
            "ClicksText".to_string(),
            clicks_binding_text(self.state.clicks),
        );

        let ctx = XamlContext {
            handlers: &self.handlers,
            bindings: &bindings,
            resources: &self.resources,
            theme: self.theme,
            view: cx.entity().downgrade(),
        };

        // Opaque windows need an explicit base color; a Mica/Acrylic backdrop
        // shows through only when the root stays transparent.
        let mut root = gpui::div().size_full();
        if self.backdrop == WindowBackgroundAppearance::Opaque {
            root = root.bg(gpui::rgb(0x1F1F1F));
        }
        root.child(render_element(&self.tree, &ctx, "0"))
    }
}

fn demo_handlers() -> HashMap<String, HandlerFn<DemoApp>> {
    HashMap::from([(
        "OnIncrement".to_string(),
        Arc::new(|app: &mut DemoApp| {
            app.state.clicks += 1;
        }) as HandlerFn<DemoApp>,
    )])
}

/// The PoC's interim surface for the window backdrop, read off the page
/// root; the real XAML `Window` element is REQ-WIN-01 (M2).
fn backdrop_from_xaml(tree: &XamlElement) -> WindowBackgroundAppearance {
    match tree.attr("SystemBackdrop").map(|v| v.trim()) {
        None => WindowBackgroundAppearance::Opaque,
        Some("Mica") => WindowBackgroundAppearance::MicaBackdrop,
        Some("MicaAlt") => WindowBackgroundAppearance::MicaAltBackdrop,
        Some("Acrylic") => WindowBackgroundAppearance::Blurred,
        Some(other) => {
            eprintln!("[xaml] SystemBackdrop=\"{other}\" not in the PoC subset; using Opaque");
            WindowBackgroundAppearance::Opaque
        }
    }
}

/// Resolves ResourceDictionary `Source` values against demo.xaml's
/// directory; the PoC's `Source` form is a plain relative path.
struct DemoSourceResolver {
    base: PathBuf,
}

impl SourceResolver for DemoSourceResolver {
    fn resolve(&self, source: &str) -> Result<String, String> {
        let path = self.base.join(source);
        std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// All load-time diagnostics for one parsed tree: resource collection
/// (including `Source` files next to demo.xaml), the merged Fluent token
/// catalog, style-chain validation, catalog resolution, and
/// resource-reference checks. Also returns the collected resources for
/// rendering.
fn startup_diagnostics(tree: &XamlElement, theme: Theme) -> (ResourceSet, Vec<Diagnostic>) {
    let resolver = DemoSourceResolver {
        base: demo_xaml_path()
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default(),
    };
    let (mut resources, mut diags) = collect_resources(tree, &resolver);
    let tokens = parse(FLUENT_TOKENS_XAML).expect("embedded Fluent token catalog failed to parse");
    diags.extend(resources.merge_app_dictionary(&tokens, &resolver));
    diags.extend(check_style_chains(&resources, &Catalog::poc_subset()));
    diags.extend(resolve(tree, &Catalog::poc_subset()));
    diags.extend(check_resource_references(tree, &resources, theme));
    (resources, diags)
}

fn main() {
    let xaml_path = demo_xaml_path();
    let xaml_src = match std::fs::read_to_string(&xaml_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to read {}: {e}", xaml_path.display());
            std::process::exit(1);
        }
    };
    let tree = parse(&xaml_src).expect("XAML failed to parse");
    let theme = poc_theme();
    // REQ-DIAG-01/02: load-time diagnostics with file/line. Errors stop the
    // run before a window opens; warnings print and rendering continues.
    let (resources, diags) = startup_diagnostics(&tree, theme);
    if !diags.is_empty() {
        eprint!(
            "{}",
            render_diagnostics(&xaml_path.display().to_string(), &xaml_src, &diags)
        );
        if diags.iter().any(|d| d.severity == Severity::Error) {
            std::process::exit(1);
        }
    }
    let backdrop = backdrop_from_xaml(&tree);
    // Setter values join the attribute set here (local values win); the
    // renderer renders the resolved tree unchanged.
    let resolved = resolve_styles(&tree, &resources, theme);
    let handlers = demo_handlers();

    gpui_platform::application().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(720.), px(480.)), cx);
        let _window = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_background: backdrop,
                titlebar: Some(TitlebarOptions {
                    title: Some("WinUI XAML on GPUI — feasibility PoC".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| DemoApp {
                    tree: resolved,
                    state: AppState::default(),
                    handlers: handlers.clone(),
                    resources,
                    theme,
                    backdrop,
                })
            },
        );
        cx.activate(true);

        if let Ok(secs) = std::env::var("POC_AUTOCLOSE_SECS")
            && let Ok(secs) = secs.parse::<u64>()
        {
            cx.spawn(async move |cx| {
                cx.background_executor()
                    .timer(Duration::from_secs(secs))
                    .await;
                cx.update(|cx| cx.quit());
            })
            .detach();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_resolves_clean_against_the_catalog() {
        let src = std::fs::read_to_string(demo_xaml_path()).unwrap();
        let tree = parse(&src).unwrap();
        // Also covers the merged Fluent token catalog: startup_diagnostics
        // merges it into the same resource set.
        let (_, diags) = startup_diagnostics(&tree, Theme::Dark);
        assert!(
            diags.is_empty(),
            "demo.xaml must stay inside the PoC dialect: {diags:?}"
        );
    }

    #[test]
    fn theme_names_parse() {
        assert_eq!(theme_from_name("Light"), Ok(Theme::Light));
        assert_eq!(theme_from_name(" Dark "), Ok(Theme::Dark));
        assert_eq!(theme_from_name("HighContrast"), Ok(Theme::HighContrast));
        assert!(theme_from_name("midnight").is_err());
    }

    #[test]
    fn click_attr_resolves_and_drives_state() {
        let src = std::fs::read_to_string(demo_xaml_path()).unwrap();
        let tree = parse(&src).unwrap();

        let button = tree.children[0]
            .children
            .iter()
            .find(|c| c.name == "Button")
            .expect("demo has a Button");
        let name = button.attr("Click").unwrap();
        assert_eq!(name, "OnIncrement");
        assert!(
            demo_handlers().contains_key(name),
            "handler registered in map"
        );

        // The handler seam is generic over the host view; driving a real
        // DemoApp exercises it without a window.
        let mut app = DemoApp {
            tree,
            state: AppState::default(),
            handlers: demo_handlers(),
            resources: ResourceSet::default(),
            theme: Theme::Dark,
            backdrop: WindowBackgroundAppearance::Opaque,
        };
        // Arc clone: the call takes &mut app while the handler stays alive.
        let handler = app.handlers.get("OnIncrement").unwrap().clone();
        handler(&mut app);
        handler(&mut app);
        assert_eq!(app.state.clicks, 2);

        assert_eq!(
            clicks_binding_text(app.state.clicks),
            "Button clicked 2 time(s)"
        );
    }
}
