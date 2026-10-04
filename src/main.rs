//! Feasibility PoC: load a WinUI-style .xaml file and render it with GPUI.
//!
//! Run: `cargo run` (or `POC_AUTOCLOSE_SECS=4 cargo run` for a self-closing
//! run used by the automated feasibility check).

mod render;
mod xaml;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gpui::{
    App, Bounds, Context, Render, TitlebarOptions, WindowBounds, WindowOptions, prelude::*, px,
    size,
};
use render::{AppState, HandlerFn, XamlContext};

struct DemoApp {
    tree: xaml::XamlElement,
    state: AppState,
    handlers: HashMap<String, HandlerFn>,
}

/// The text `{x:Bind ClicksText}` resolves to. Shared by the renderer and the
/// tests so the test exercises the real derivation, not its own copy.
fn clicks_binding_text(clicks: u32) -> String {
    format!("Button clicked {} time(s)", clicks)
}

/// Asset paths must not depend on the process CWD: `cargo run` happens to set
/// it to the manifest dir, nothing else does.
fn demo_xaml_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/demo.xaml")
}

impl Render for DemoApp {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Rebuild bindings from state every frame; the real library replaces
        // this with a property registry wired to change notifications.
        let mut bindings = HashMap::new();
        bindings.insert(
            "ClicksText".to_string(),
            clicks_binding_text(self.state.clicks),
        );

        let ctx = XamlContext {
            handlers: &self.handlers,
            bindings: &bindings,
            view: cx.entity().downgrade(),
        };

        gpui::div()
            .size_full()
            .bg(gpui::rgb(0x1F1F1F))
            .child(render::render_element(&self.tree, &ctx, "0"))
    }
}

fn demo_handlers() -> HashMap<String, HandlerFn> {
    HashMap::from([(
        "OnIncrement".to_string(),
        Arc::new(|state: &mut AppState| {
            state.clicks += 1;
        }) as HandlerFn,
    )])
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
    let tree = xaml::parse(&xaml_src).expect("XAML failed to parse");
    let handlers = demo_handlers();

    gpui::Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(720.), px(480.)), cx);
        let _window = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("WinUI XAML on GPUI — feasibility PoC".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| DemoApp {
                    tree,
                    state: AppState::default(),
                    handlers: handlers.clone(),
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
                let _ = cx.update(|cx| cx.quit());
            })
            .detach();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_xaml_parses_into_expected_tree() {
        let src = std::fs::read_to_string(demo_xaml_path()).unwrap();
        let tree = xaml::parse(&src).unwrap();
        assert_eq!(tree.name, "Page");
        let panel = &tree.children[0];
        assert_eq!(panel.name, "StackPanel");
        let kinds: Vec<&str> = panel.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(kinds, ["TextBlock", "TextBlock", "Border", "Button"]);
        let button = &panel.children[3];
        assert_eq!(
            button.attributes.get("Click").map(String::as_str),
            Some("OnIncrement")
        );
    }

    #[test]
    fn click_attr_resolves_and_drives_state() {
        let src = std::fs::read_to_string(demo_xaml_path()).unwrap();
        let tree = xaml::parse(&src).unwrap();
        let handlers = demo_handlers();

        // Resolve exactly as the renderer does: Button's Click attr -> registry.
        let button = tree.children[0]
            .children
            .iter()
            .find(|c| c.name == "Button")
            .expect("demo has a Button");
        let name = button.attributes.get("Click").unwrap();
        let handler = handlers.get(name).expect("handler registered in map");

        let mut state = AppState::default();
        handler(&mut state);
        handler(&mut state);
        assert_eq!(state.clicks, 2);

        // The shared derivation DemoApp::render feeds into the binding map.
        assert_eq!(
            clicks_binding_text(state.clicks),
            "Button clicked 2 time(s)"
        );
    }
}
