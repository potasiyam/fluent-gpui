//! XAML object model -> GPUI element mapping.
//!
//! PoC subset: Page/UserControl, StackPanel, TextBlock, Border, Button.
//! Every element renders bottom-up into an owned `AnyElement`, which proves
//! GPUI can host a UI tree built at runtime from markup (its element API is
//! compile-time-builder-shaped, but `AnyElement` erases that).
//!
//! Unsupported constructs warn on stderr instead of failing silently
//! ("never silently render wrong"); M1 replaces the warnings with real
//! file/line diagnostics (PRD REQ-DIAG-01/02).

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    AnyElement, App, ClickEvent, InteractiveElement, SharedString, StatefulInteractiveElement,
    Styled, WeakEntity, div, prelude::*, px, relative, rgb, rgba,
};

use crate::xaml::XamlElement;

/// Handler logic registered by the host app, keyed by the `Click="Name"`
/// attribute value. Deliberately framework-free: it only touches app state.
pub type HandlerFn = Arc<dyn Fn(&mut AppState)>;

/// Host state the XAML can reach through `{x:Bind PropertyName}`.
/// The real library replaces this with a property registry + derive macro.
#[derive(Default)]
pub struct AppState {
    pub clicks: u32,
}

pub struct XamlContext<'a> {
    pub handlers: &'a HashMap<String, HandlerFn>,
    pub bindings: &'a HashMap<String, String>,
    /// Lets `Click=` handlers mutate the owning view and request a re-render.
    pub view: WeakEntity<crate::DemoApp>,
}

/// Attributes the PoC knows about but does not implement; warned, never
/// silently ignored.
const UNSUPPORTED_ATTRS: &[&str] = &[
    "MinWidth",
    "MinHeight",
    "MaxWidth",
    "MaxHeight",
    "VerticalAlignment",
    "IsEnabled",
];

pub fn render_element(el: &XamlElement, ctx: &XamlContext, path: &str) -> AnyElement {
    warn_unsupported(el);
    match el.name.as_str() {
        "Page" | "UserControl" => container(el, ctx, path, true),
        "StackPanel" => container(el, ctx, path, false),
        "TextBlock" => align_wrap(el, text_block(el, ctx)),
        "Border" => align_wrap(el, border(el, ctx, path)),
        "Button" => align_wrap(el, button(el, ctx, path)),
        other => {
            eprintln!("[xaml] <{other}> not in PoC subset; rendering as plain container");
            align_wrap(el, container(el, ctx, path, false))
        }
    }
}

/// Markup extensions are only resolved on `Text`/`Content`; anywhere else the
/// raw `{…}` value would be silently misparsed downstream (numeric/color
/// coercion fails), so flag it. Known-unsupported attributes are flagged too.
fn warn_unsupported(el: &XamlElement) {
    for attr in UNSUPPORTED_ATTRS {
        if el.attributes.contains_key(*attr) {
            eprintln!("[xaml] attribute \"{attr}\" is not implemented in the PoC subset; ignored");
        }
    }
    for (key, value) in &el.attributes {
        let t = value.trim();
        if key != "Text" && key != "Content" && t.starts_with('{') && t.ends_with('}') {
            eprintln!(
                "[xaml] markup extension on \"{key}\" is only resolved on Text/Content in the PoC subset; ignored"
            );
        }
    }
}

/// WinUI `Visibility` has exactly two values; `Collapsed` removes the element
/// from layout entirely (its children are not rendered at all).
fn is_collapsed(el: &XamlElement) -> bool {
    match el.attributes.get("Visibility").map(|v| v.trim()) {
        None => false,
        Some(v) if v.eq_ignore_ascii_case("Visible") => false,
        Some(v) if v.eq_ignore_ascii_case("Collapsed") => true,
        Some(other) => {
            eprintln!("[xaml] Visibility=\"{other}\" not in PoC subset; rendering as Visible");
            false
        }
    }
}

// --- elements -----------------------------------------------------------

fn container(el: &XamlElement, ctx: &XamlContext, path: &str, is_page: bool) -> AnyElement {
    let vertical = el
        .attributes
        .get("Orientation")
        .map(|o| !o.trim().eq_ignore_ascii_case("Horizontal"))
        .unwrap_or(true);

    let mut d = div().flex();
    d = if vertical { d.flex_col() } else { d.flex_row() };

    if is_page {
        d = d.size_full().font_family("Segoe UI");
    }
    if let Some(sp) = attr_f32(el, "Spacing") {
        d = d.gap(px(sp));
    }
    if let Some((l, t, r, b)) = attr_edges(el, "Padding") {
        d = d.pl(px(l)).pt(px(t)).pr(px(r)).pb(px(b));
    }
    if let Some(bg) = attr_color(el, "Background") {
        d = d.bg(bg);
    }

    let kids: Vec<AnyElement> = el
        .children
        .iter()
        .enumerate()
        .filter(|(_, c)| !is_collapsed(c))
        .map(|(i, c)| render_element(c, ctx, &format!("{path}.{i}")))
        .collect();
    d = d.children(kids);
    apply_common(d, el).into_any_element()
}

fn text_block(el: &XamlElement, ctx: &XamlContext) -> AnyElement {
    if !el.children.is_empty() {
        eprintln!(
            "[xaml] <TextBlock> element children are not supported in the PoC subset; dropped"
        );
    }
    // WinUI's content property for TextBlock is Text; the parser maps inner
    // text to Content, so accept both spellings.
    let text = resolve_markup(el, "Text", ctx)
        .or_else(|| resolve_markup(el, "Content", ctx))
        .unwrap_or_default();
    let mut d = div().child(text).line_height(relative(1.25));
    if let Some(fs) = attr_f32(el, "FontSize") {
        d = d.text_size(px(fs));
    }
    if let Some(c) = attr_color(el, "Foreground") {
        d = d.text_color(c);
    }
    if el
        .attributes
        .get("FontWeight")
        .is_some_and(|w| w.trim().eq_ignore_ascii_case("bold"))
    {
        d = d.font_weight(gpui::FontWeight::BOLD);
    }
    apply_common(d, el).into_any_element()
}

fn border(el: &XamlElement, ctx: &XamlContext, path: &str) -> AnyElement {
    let mut d = div().flex().flex_col();
    if let Some(bg) = attr_color(el, "Background") {
        d = d.bg(bg);
    }
    if let Some(cr) = attr_f32(el, "CornerRadius") {
        d = d.rounded(px(cr));
    }
    // PoC approximation: any BorderThickness > 0 renders as 1px.
    if attr_f32(el, "BorderThickness").is_some_and(|t| t > 0.0) {
        d = d.border_1();
        if let Some(bc) = attr_color(el, "BorderBrush") {
            d = d.border_color(bc);
        }
    }
    if let Some((l, t, r, b)) = attr_edges(el, "Padding") {
        d = d.pl(px(l)).pt(px(t)).pr(px(r)).pb(px(b));
    }
    let kids: Vec<AnyElement> = el
        .children
        .iter()
        .enumerate()
        .filter(|(_, c)| !is_collapsed(c))
        .map(|(i, c)| render_element(c, ctx, &format!("{path}.{i}")))
        .collect();
    d = d.children(kids);
    apply_common(d, el).into_any_element()
}

fn button(el: &XamlElement, ctx: &XamlContext, path: &str) -> AnyElement {
    if !el.children.is_empty() {
        eprintln!("[xaml] <Button> element children are not supported in the PoC subset; dropped");
    }
    // WinUI Buttons default to an empty label when no Content is given.
    let label = resolve_markup(el, "Content", ctx).unwrap_or_default();

    let mut b = div()
        .id(SharedString::from(path.to_string()))
        .flex()
        .items_center()
        .justify_center()
        .px(px(14.0))
        .py(px(7.0))
        .bg(rgb(0x0078D4))
        .rounded(px(4.0))
        .text_size(px(14.0))
        .text_color(rgb(0xFFFFFF))
        .font_family("Segoe UI")
        .hover(|s| s.bg(rgb(0x1A86DE)))
        .active(|s| s.bg(rgb(0x006AC1)));

    if let Some(name) = el.attributes.get("Click").map(|s| s.trim().to_string()) {
        if let Some(handler) = ctx.handlers.get(&name).cloned() {
            let view = ctx.view.clone();
            b = b.on_click(
                move |_ev: &ClickEvent, _window: &mut gpui::Window, cx: &mut App| {
                    let _ = view.update(cx, |app, cx| {
                        (handler)(&mut app.state);
                        cx.notify();
                    });
                },
            );
        } else {
            eprintln!("[xaml] Click handler \"{name}\" not registered");
        }
    }

    b = b.child(label);
    apply_common(b, el).into_any_element()
}

// --- attribute helpers --------------------------------------------------

/// Margin, size, Opacity on any element; keeps the builder generic over
/// `Styled`.
fn apply_common<E: Styled>(e: E, el: &XamlElement) -> E {
    let mut e = e;
    if let Some((l, t, r, b)) = attr_edges(el, "Margin") {
        e = e.ml(px(l)).mt(px(t)).mr(px(r)).mb(px(b));
    }
    if let Some(w) = attr_f32(el, "Width") {
        e = e.w(px(w));
    }
    if let Some(h) = attr_f32(el, "Height") {
        e = e.h(px(h));
    }
    if let Some(o) = attr_f32(el, "Opacity") {
        e = e.opacity(o.clamp(0.0, 1.0));
    }
    e
}

/// HorizontalAlignment is approximated by wrapping in a full-width flex line,
/// which works inside any column/row flex parent.
fn align_wrap(el: &XamlElement, inner: AnyElement) -> AnyElement {
    match el.attributes.get("HorizontalAlignment").map(|s| s.trim()) {
        Some("Center") => div().flex().w_full().justify_center().child(inner),
        Some("Right") => div().flex().w_full().justify_end().child(inner),
        _ => return inner,
    }
    .into_any_element()
}

/// Supports `{x:Bind Path[, Mode=…]}` against the host's binding map: the
/// first comma-separated token is the path, remaining parameters are ignored
/// with a warning. Other `{…}` values pass through verbatim (rendered as
/// literal text).
fn resolve_markup(el: &XamlElement, attr: &str, ctx: &XamlContext) -> Option<SharedString> {
    let raw = el.attributes.get(attr)?;
    let t = raw.trim();
    if let Some(body) = t.strip_prefix("{x:Bind").and_then(|r| r.strip_suffix('}')) {
        if !body.is_empty() && !body.starts_with(char::is_whitespace) {
            // `{x:BindFoo}` is a parse error in real XAML, not a binding.
            eprintln!("[xaml] malformed binding \"{t}\"; rendering as literal text");
            return Some(raw.clone().into());
        }
        let path = body.trim().split(',').next().unwrap_or("").trim();
        if body.trim().contains(',') {
            eprintln!("[xaml] binding parameters on \"{t}\" are ignored in the PoC subset");
        }
        if path.is_empty() {
            eprintln!("[xaml] empty binding path in \"{t}\"; rendering empty text");
            return Some(SharedString::from(""));
        }
        return match ctx.bindings.get(path) {
            Some(value) => Some(value.clone().into()),
            None => {
                eprintln!(
                    "[xaml] binding path \"{path}\" not found in the binding map; rendering empty"
                );
                Some(SharedString::from(""))
            }
        };
    }
    Some(raw.clone().into())
}

fn attr_f32(el: &XamlElement, attr: &str) -> Option<f32> {
    let v = el.attributes.get(attr)?.trim();
    match v.parse() {
        Ok(n) => Some(n),
        Err(_) => {
            eprintln!("[xaml] attribute {attr}=\"{v}\" is not a number; ignored");
            None
        }
    }
}

/// WinUI edge format: "12", "leftRight,topBottom" (2 values), or
/// "left,top,right,bottom" (4 values).
fn attr_edges(el: &XamlElement, attr: &str) -> Option<(f32, f32, f32, f32)> {
    let v = el.attributes.get(attr)?.trim().to_string();
    let parts: Vec<f32> = v.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    let parsed = match parts.len() {
        1 => Some((parts[0], parts[0], parts[0], parts[0])),
        2 => Some((parts[0], parts[1], parts[0], parts[1])),
        4 => Some((parts[0], parts[1], parts[2], parts[3])),
        _ => None,
    };
    if parsed.is_none() {
        eprintln!(
            "[xaml] edge value \"{v}\" on \"{attr}\" is not 1, 2, or 4 comma-separated numbers; ignored"
        );
    }
    parsed
}

fn attr_color(el: &XamlElement, attr: &str) -> Option<gpui::Rgba> {
    let v = el.attributes.get(attr)?.trim();
    let parsed = if let Some(hex) = v.strip_prefix('#') {
        match hex.len() {
            6 => u32::from_str_radix(hex, 16).ok().map(rgb),
            8 => {
                // WinUI hex is #AARRGGBB; gpui rgba() wants 0xRRGGBBAA.
                let aa = u32::from_str_radix(&hex[0..2], 16).ok();
                let val = u32::from_str_radix(&hex[2..], 16).ok();
                match (aa, val) {
                    (Some(aa), Some(val)) => Some(rgba(val << 8 | aa)),
                    _ => None,
                }
            }
            _ => None,
        }
    } else {
        match v.to_ascii_lowercase().as_str() {
            "black" => Some(rgb(0x000000)),
            "white" => Some(rgb(0xFFFFFF)),
            "transparent" => Some(gpui::transparent_black().into()),
            _ => None,
        }
    };
    if parsed.is_none() && !v.starts_with('{') {
        eprintln!(
            "[xaml] color \"{v}\" on \"{attr}\" is not in the PoC subset (6/8-digit #hex, black/white/transparent); ignored"
        );
    }
    parsed
}
