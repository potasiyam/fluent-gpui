//! GPUI backend for the winui-xaml dialect: maps the parsed XAML tree onto
//! gpui elements. The only crate in the workspace allowed to touch gpui
//! types (PLAN §3).
//!
//! PoC subset: Page/UserControl, StackPanel, TextBlock, Border, Button,
//! rendered bottom-up into owned `AnyElement`s; color attributes resolve
//! `{StaticResource}`/`{ThemeResource}` against the model's `ResourceSet`
//! (REQ-RES-01). Structural unknowns surface as WX diagnostics from
//! winui-xaml-model at load (REQ-DIAG-01/02); the renderer's remaining
//! stderr warnings are value-level.

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    AnyElement, App, ClickEvent, InteractiveElement, SharedString, StatefulInteractiveElement,
    Styled, WeakEntity, div, prelude::*, px, relative, rgb,
};

use winui_xaml_model::{ResourceSet, ResourceValue, Rgba, Theme, parse_color, resource_key};
use winui_xaml_syntax::XamlElement;

/// Host handler keyed by the `Click="Name"` attribute value; it only touches
/// host state. The library owns the `cx.notify()` plumbing (REQ-EVT-01).
pub type HandlerFn<V> = Arc<dyn Fn(&mut V)>;

/// Per-render context the host provides; `V` is the host's root view.
pub struct XamlContext<'a, V: 'static> {
    pub handlers: &'a HashMap<String, HandlerFn<V>>,
    pub bindings: &'a HashMap<String, String>,
    /// Collected page resources (REQ-RES-01); `{StaticResource}` and
    /// `{ThemeResource}` values resolve against it.
    pub resources: &'a ResourceSet,
    /// Active theme for theme-dictionary lookups.
    pub theme: Theme,
    /// Lets `Click=` handlers mutate the owning view and re-render it.
    pub view: WeakEntity<V>,
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

/// Attributes whose markup extensions the renderer resolves; on any other
/// attribute an extension warns instead of resolving.
const RESOLVED_EXTENSION_ATTRS: &[&str] =
    &["Text", "Content", "Background", "Foreground", "BorderBrush"];

pub fn render_element<V>(el: &XamlElement, ctx: &XamlContext<'_, V>, path: &str) -> AnyElement {
    warn_unsupported(el);
    match el.name.as_str() {
        "Page" | "UserControl" => container(el, ctx, path, true),
        "StackPanel" => container(el, ctx, path, false),
        "TextBlock" => align_wrap(el, text_block(el, ctx, path)),
        "Border" => align_wrap(el, border(el, ctx, path)),
        "Button" => align_wrap(el, button(el, ctx, path)),
        other => {
            eprintln!("[xaml] <{other}> not in PoC subset; rendering as plain container");
            align_wrap(el, container(el, ctx, path, false))
        }
    }
}

/// Warns on known-unsupported attributes, on property elements (not in the
/// PoC subset), and on markup extensions outside the attributes the
/// renderer actually resolves.
fn warn_unsupported(el: &XamlElement) {
    for prop in &el.properties {
        // `.Resources` dictionaries are consumed by the model's resource
        // unit, not the renderer.
        if prop.name == format!("{}.Resources", el.name) {
            continue;
        }
        eprintln!(
            "[xaml] property element <{}> is not supported in the PoC subset; ignored",
            prop.name
        );
    }
    for attr in UNSUPPORTED_ATTRS {
        if el.attr(attr).is_some() {
            eprintln!("[xaml] attribute \"{attr}\" is not implemented in the PoC subset; ignored");
        }
    }
    for a in &el.attributes {
        let t = a.value.trim();
        if !RESOLVED_EXTENSION_ATTRS.contains(&a.name.as_str())
            && t.starts_with('{')
            && t.ends_with('}')
        {
            eprintln!(
                "[xaml] markup extension on \"{}\" is only resolved on Text/Content and color attributes in the PoC subset; ignored",
                a.name
            );
        }
    }
}

/// WinUI `Visibility`: `Collapsed` removes the element from layout entirely.
fn is_collapsed(el: &XamlElement) -> bool {
    match el.attr("Visibility").map(|v| v.trim()) {
        None => false,
        Some(v) if v.eq_ignore_ascii_case("Visible") => false,
        Some(v) if v.eq_ignore_ascii_case("Collapsed") => true,
        Some(other) => {
            eprintln!("[xaml] Visibility=\"{other}\" not in PoC subset; rendering as Visible");
            false
        }
    }
}

fn container<V>(
    el: &XamlElement,
    ctx: &XamlContext<'_, V>,
    path: &str,
    is_page: bool,
) -> AnyElement {
    let vertical = el
        .attr("Orientation")
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
    if let Some(bg) = resolve_color(el, "Background", path, ctx) {
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

fn text_block<V>(el: &XamlElement, ctx: &XamlContext<'_, V>, path: &str) -> AnyElement {
    if !el.children.is_empty() {
        eprintln!(
            "[xaml] <TextBlock> element children are not supported in the PoC subset; dropped"
        );
    }
    // WinUI's content property for TextBlock is Text; the parser maps inner
    // text to Content.
    let text = resolve_markup(el, "Text", ctx)
        .or_else(|| resolve_markup(el, "Content", ctx))
        .unwrap_or_default();
    let mut d = div().child(text).line_height(relative(1.25));
    if let Some(fs) = attr_f32(el, "FontSize") {
        d = d.text_size(px(fs));
    }
    if let Some(c) = resolve_color(el, "Foreground", path, ctx) {
        d = d.text_color(c);
    }
    if el
        .attr("FontWeight")
        .is_some_and(|w| w.trim().eq_ignore_ascii_case("bold"))
    {
        d = d.font_weight(gpui::FontWeight::BOLD);
    }
    apply_common(d, el).into_any_element()
}

fn border<V>(el: &XamlElement, ctx: &XamlContext<'_, V>, path: &str) -> AnyElement {
    let mut d = div().flex().flex_col();
    if let Some(bg) = resolve_color(el, "Background", path, ctx) {
        d = d.bg(bg);
    }
    if let Some(cr) = attr_f32(el, "CornerRadius") {
        d = d.rounded(px(cr));
    }
    // PoC approximation: any BorderThickness > 0 renders as 1px.
    if attr_f32(el, "BorderThickness").is_some_and(|t| t > 0.0) {
        d = d.border_1();
        if let Some(bc) = resolve_color(el, "BorderBrush", path, ctx) {
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

fn button<V>(el: &XamlElement, ctx: &XamlContext<'_, V>, path: &str) -> AnyElement {
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

    if let Some(name) = el.attr("Click").map(|s| s.trim().to_string()) {
        if let Some(handler) = ctx.handlers.get(&name).cloned() {
            let view = ctx.view.clone();
            b = b.on_click(
                move |_ev: &ClickEvent, _window: &mut gpui::Window, cx: &mut App| {
                    let _ = view.update(cx, |host, cx| {
                        (handler)(host);
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
    match el.attr("HorizontalAlignment").map(|s| s.trim()) {
        Some("Center") => div().flex().w_full().justify_center().child(inner),
        Some("Right") => div().flex().w_full().justify_end().child(inner),
        _ => return inner,
    }
    .into_any_element()
}

/// `{x:Bind Path[, …]}`: the first comma-separated token is the path, other
/// parameters warn and are ignored. Other `{…}` values pass through verbatim.
fn resolve_markup<V>(
    el: &XamlElement,
    attr: &str,
    ctx: &XamlContext<'_, V>,
) -> Option<SharedString> {
    let raw = el.attr(attr)?;
    let t = raw.trim();
    if let Some(body) = t.strip_prefix("{x:Bind").and_then(|r| r.strip_suffix('}')) {
        if !body.is_empty() && !body.starts_with(char::is_whitespace) {
            // `{x:BindFoo}` is a parse error in real XAML, not a binding.
            eprintln!("[xaml] malformed binding \"{t}\"; rendering as literal text");
            return Some(SharedString::from(raw));
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
    Some(SharedString::from(raw))
}

fn attr_f32(el: &XamlElement, attr: &str) -> Option<f32> {
    let v = el.attr(attr)?.trim();
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
    let v = el.attr(attr)?.trim().to_string();
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

/// Resolves a color-valued attribute: a `{StaticResource …}`/`{ThemeResource
/// …}` value looks up the context resources at the element's render path,
/// anything else parses as a WinUI color literal.
fn resolve_color<V>(
    el: &XamlElement,
    attr: &str,
    path: &str,
    ctx: &XamlContext<'_, V>,
) -> Option<gpui::Rgba> {
    let v = el.attr(attr)?.trim();
    let parsed = resource_key(v, "StaticResource")
        .or_else(|| resource_key(v, "ThemeResource"))
        .map(|key| match ctx.resources.resolve(key, path, ctx.theme) {
            Some(ResourceValue::Color(c)) => Some(to_gpui(*c)),
            None => {
                eprintln!(
                    "[xaml] {attr}=\"{v}\" resolved no resource key \"{key}\"; leaving the property unset"
                );
                None
            }
        })
        .unwrap_or_else(|| parse_color(v).map(to_gpui));
    if parsed.is_none() && !v.starts_with('{') {
        eprintln!(
            "[xaml] color \"{v}\" on \"{attr}\" is not in the PoC subset (6/8-digit #hex, black/white/transparent); ignored"
        );
    }
    parsed
}

/// `Rgba` is `#AARRGGBB`-ordered; gpui's rgba() wants 0xRRGGBBAA.
fn to_gpui(c: Rgba) -> gpui::Rgba {
    gpui::rgba((c.r as u32) << 24 | (c.g as u32) << 16 | (c.b as u32) << 8 | c.a as u32)
}
