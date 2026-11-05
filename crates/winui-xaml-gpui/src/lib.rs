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

/// Attributes whose markup extensions the renderer or the model's load
/// pass resolves; on any other attribute an extension warns instead.
const RESOLVED_EXTENSION_ATTRS: &[&str] = &[
    "Text",
    "Content",
    "Background",
    "Foreground",
    "BorderBrush",
    "CornerRadius",
    "Style",
];

pub fn render_element<V>(el: &XamlElement, ctx: &XamlContext<'_, V>, path: &str) -> AnyElement {
    warn_unresolved_extensions(el);
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

/// Warns on markup extensions in attributes the renderer does not resolve.
/// Structural unknowns — unknown elements, properties, property elements —
/// surface as WX diagnostics from the model's load pass instead, so they
/// are not re-reported here.
fn warn_unresolved_extensions(el: &XamlElement) {
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
    // WinUI FontWeight subset the PoC renders.
    if el
        .attr("FontWeight")
        .is_some_and(|w| w.trim().eq_ignore_ascii_case("bold"))
    {
        d = d.font_weight(gpui::FontWeight::BOLD);
    } else if el
        .attr("FontWeight")
        .is_some_and(|w| w.trim().eq_ignore_ascii_case("semibold"))
    {
        d = d.font_weight(gpui::FontWeight::SEMIBOLD);
    }
    apply_common(d, el).into_any_element()
}

fn border<V>(el: &XamlElement, ctx: &XamlContext<'_, V>, path: &str) -> AnyElement {
    let mut d = div().flex().flex_col();
    if let Some(bg) = resolve_color(el, "Background", path, ctx) {
        d = d.bg(bg);
    }
    if let Some(cr) = resolve_number(el, "CornerRadius", path, ctx) {
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
    attr_f32_of(el.attr(attr)?.trim(), attr)
}

/// WinUI edge format: "12", "leftRight,topBottom" (2 values), or
/// "left,top,right,bottom" (4 values). An unparsable part or a wrong count
/// warns and discards the whole value — no part of a bad value is used.
fn attr_edges(el: &XamlElement, attr: &str) -> Option<(f32, f32, f32, f32)> {
    let v = el.attr(attr)?.trim().to_string();
    let tokens: Vec<&str> = v.split(',').map(str::trim).collect();
    let numbers: Option<Vec<f32>> = tokens.iter().map(|t| t.parse().ok()).collect();
    let Some(numbers) = numbers else {
        eprintln!("[xaml] edge value \"{v}\" on \"{attr}\" has an unparsable part; ignored");
        return None;
    };
    match numbers.as_slice() {
        [a] => Some((*a, *a, *a, *a)),
        [a, b] => Some((*a, *b, *a, *b)),
        [a, b, c, d] => Some((*a, *b, *c, *d)),
        _ => {
            eprintln!(
                "[xaml] edge value \"{v}\" on \"{attr}\" is not 1, 2, or 4 comma-separated numbers; ignored"
            );
            None
        }
    }
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
    resolve_color_value(el.attr(attr)?, attr, path, ctx.resources, ctx.theme)
}

/// The value-level half of [`resolve_color`], split out so tests can drive
/// it without a gpui entity.
fn resolve_color_value(
    raw: &str,
    attr: &str,
    path: &str,
    resources: &ResourceSet,
    theme: Theme,
) -> Option<gpui::Rgba> {
    let v = raw.trim();
    let parsed = resource_key(v, "StaticResource")
        .or_else(|| resource_key(v, "ThemeResource"))
        .map(|key| match resources.resolve(key, path, theme) {
            Some(ResourceValue::Color(c)) => Some(to_gpui(*c)),
            Some(ResourceValue::Style(_)) | Some(ResourceValue::Text(_)) | None => {
                eprintln!(
                    "[xaml] {attr}=\"{v}\" resolved no color resource \"{key}\"; leaving the property unset"
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

/// Resolves a number-valued attribute: a `{StaticResource …}`/
/// `{ThemeResource …}` value looks up a text resource (WinUI's
/// `CornerRadius`/`String` resources), anything else parses as a literal.
/// Returns `None` and warns when the value cannot be used — no part of a
/// bad value is applied.
fn resolve_number_value(
    raw: &str,
    attr: &str,
    path: &str,
    resources: &ResourceSet,
    theme: Theme,
) -> Option<f32> {
    let v = raw.trim();
    if let Some(key) =
        resource_key(v, "StaticResource").or_else(|| resource_key(v, "ThemeResource"))
    {
        return match resources.resolve(key, path, theme) {
            Some(ResourceValue::Text(t)) => match t.trim().parse::<f32>() {
                Ok(n) => Some(n),
                Err(_) => {
                    eprintln!(
                        "[xaml] {attr}=\"{v}\" resource \"{key}\" value \"{t}\" is not a number; leaving the property unset"
                    );
                    None
                }
            },
            Some(_) => {
                eprintln!(
                    "[xaml] {attr}=\"{v}\" resource \"{key}\" is not a number value; leaving the property unset"
                );
                None
            }
            None => {
                eprintln!(
                    "[xaml] {attr}=\"{v}\" resolved no number resource \"{key}\"; leaving the property unset"
                );
                None
            }
        };
    }
    attr_f32_of(v, attr)
}

/// The element-level half of [`resolve_number_value`].
fn resolve_number<V>(
    el: &XamlElement,
    attr: &str,
    path: &str,
    ctx: &XamlContext<'_, V>,
) -> Option<f32> {
    resolve_number_value(el.attr(attr)?, attr, path, ctx.resources, ctx.theme)
}

/// Parses a literal number, warning like [`attr_f32`] but on a value that
/// was already extracted.
fn attr_f32_of(v: &str, attr: &str) -> Option<f32> {
    match v.parse() {
        Ok(n) => Some(n),
        Err(_) => {
            eprintln!("[xaml] attribute {attr}=\"{v}\" is not a number; ignored");
            None
        }
    }
}

/// `Rgba` is `#AARRGGBB`-ordered; gpui's rgba() wants 0xRRGGBBAA.
fn to_gpui(c: Rgba) -> gpui::Rgba {
    gpui::rgba((c.r as u32) << 24 | (c.g as u32) << 16 | (c.b as u32) << 8 | c.a as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use winui_xaml_syntax::parse;

    /// Test resolver: serves nothing — these tests use inline dictionaries.
    struct NoSources;

    impl winui_xaml_model::SourceResolver for NoSources {
        fn resolve(&self, source: &str) -> Result<String, String> {
            Err(format!("no such file `{source}`"))
        }
    }

    fn resources_from(src: &str) -> ResourceSet {
        let tree = parse(src).unwrap();
        let (resources, diags) = winui_xaml_model::collect_resources(&tree, &NoSources);
        assert!(diags.is_empty(), "{diags:?}");
        resources
    }

    #[test]
    fn numbers_resolve_from_text_resources_and_literals() {
        let set = resources_from(
            r##"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
      xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
  <Page.Resources>
    <CornerRadius x:Key="ControlCornerRadius">4</CornerRadius>
    <FontFamily x:Key="IconFont">Segoe Fluent Icons</FontFamily>
  </Page.Resources>
</Page>"##,
        );
        // A theme-resource value resolves the text token; a literal
        // parses directly; a non-number resource and a missing key leave
        // the property unset.
        assert_eq!(
            resolve_number_value(
                "{ThemeResource ControlCornerRadius}",
                "CornerRadius",
                "0",
                &set,
                Theme::Dark
            ),
            Some(4.0)
        );
        assert_eq!(
            resolve_number_value("8", "CornerRadius", "0", &set, Theme::Dark),
            Some(8.0)
        );
        assert_eq!(
            resolve_number_value(
                "{ThemeResource IconFont}",
                "CornerRadius",
                "0",
                &set,
                Theme::Dark
            ),
            None
        );
        assert_eq!(
            resolve_number_value(
                "{ThemeResource Missing}",
                "CornerRadius",
                "0",
                &set,
                Theme::Dark
            ),
            None
        );
    }

    #[test]
    fn to_gpui_reorders_argb_into_rgba() {
        assert_eq!(
            to_gpui(Rgba {
                a: 0x11,
                r: 0x22,
                g: 0x33,
                b: 0x44
            }),
            gpui::rgba(0x22334411)
        );
    }

    #[test]
    fn colors_resolve_from_literals_and_resources() {
        let resources = resources_from(
            r##"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
      xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
  <Page.Resources>
    <SolidColorBrush x:Key="Accent" Color="#FF4EC9B0"/>
  </Page.Resources>
</Page>"##,
        );
        let dark = Theme::Dark;
        let teal = Some(gpui::rgba(0x4EC9B0FF));

        // Literal forms: 8-digit hex, 6-digit hex, named.
        assert_eq!(
            resolve_color_value("#FF4EC9B0", "Background", "0", &resources, dark),
            teal
        );
        assert_eq!(
            resolve_color_value("#4EC9B0", "Background", "0", &resources, dark),
            teal
        );
        assert_eq!(
            resolve_color_value("transparent", "Background", "0", &resources, dark),
            Some(gpui::rgba(0x00000000))
        );

        // Resource lookups at the element's render path.
        assert_eq!(
            resolve_color_value(
                "{StaticResource Accent}",
                "Background",
                "0.1",
                &resources,
                dark
            ),
            teal
        );
        assert_eq!(
            resolve_color_value(
                "{ThemeResource Accent}",
                "Foreground",
                "0",
                &resources,
                dark
            ),
            teal
        );

        // Misses and non-colors leave the property unset.
        assert_eq!(
            resolve_color_value("{StaticResource Nope}", "Background", "0", &resources, dark),
            None
        );
        assert_eq!(
            resolve_color_value("{x:Bind SomePath}", "Background", "0", &resources, dark),
            None
        );
        assert_eq!(
            resolve_color_value("nope", "Background", "0", &resources, dark),
            None
        );
    }

    #[test]
    fn edge_values_must_fully_parse() {
        fn edges(v: &str) -> Option<(f32, f32, f32, f32)> {
            let src = format!(
                r#"<Border xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" Margin="{v}"/>"#
            );
            let tree = parse(&src).unwrap();
            attr_edges(&tree, "Margin")
        }
        assert_eq!(edges("12"), Some((12.0, 12.0, 12.0, 12.0)));
        assert_eq!(edges("1,2"), Some((1.0, 2.0, 1.0, 2.0)));
        assert_eq!(edges("1,2,3,4"), Some((1.0, 2.0, 3.0, 4.0)));
        // A bad part invalidates the whole value instead of being dropped.
        assert_eq!(edges("12,abc"), None);
        assert_eq!(edges("1,2,3"), None);
        assert_eq!(edges(""), None);
    }
}
