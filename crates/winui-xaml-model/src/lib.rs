//! XAML model layer: the element/property catalog and name-level resolution
//! of a parsed XAML tree against it (PRD REQ-DX-02, REQ-DIAG-01/02).
//!
//! Every construct the dialect does not know produces a `WX`-coded
//! diagnostic with source position — never a silent fallback. Diagnostic
//! code families (REQ-DIAG-01): parse WX0xxx, resource WX1xxx, binding
//! WX2xxx, layout WX3xxx, a11y WX4xxx, perf-lint WX5xxx. Currently used:
//!
//! | Code | Meaning |
//! |---|---|
//! | WX0001 | XML syntax error (emitted by the host on parse failure) |
//! | WX0002 | unknown element |
//! | WX0003 | unknown property on an element |
//! | WX0004 | property element (dotted child) not implemented yet |
//! | WX0005 | unknown markup extension |
//! | WX0006 | attached property (dotted attribute) not implemented yet |
//! | WX0007 | WinUI property recognized but not implemented yet |

use std::collections::HashMap;

use winui_xaml_syntax::{Pos, XamlElement};

/// How serious a diagnostic is. Errors stop the build/run (fail loud);
/// warnings are printed and rendering continues.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// One diagnostic: code, severity, message, and where it points.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    /// Position of the offending token, if known.
    pub pos: Option<Pos>,
    /// Caret width in the emitted snippet (the token's length).
    pub underline: u32,
}

impl Diagnostic {
    fn error(code: &'static str, message: String, pos: Pos, underline: u32) -> Diagnostic {
        Diagnostic {
            code,
            severity: Severity::Error,
            message,
            pos: Some(pos),
            underline,
        }
    }

    fn warning(code: &'static str, message: String, pos: Pos, underline: u32) -> Diagnostic {
        Diagnostic {
            code,
            severity: Severity::Warning,
            message,
            pos: Some(pos),
            underline,
        }
    }
}

/// Properties every WinUI element carries (subset); the PoC renderer
/// implements all of these.
const COMMON_PROPERTIES: &[&str] = &[
    "Height",
    "HorizontalAlignment",
    "Margin",
    "Opacity",
    "Visibility",
    "Width",
];

/// WinUI-exact names the dialect knows but the PoC renderer does not
/// implement; recognized with a warning instead of reported as unknown.
const UNIMPLEMENTED_PROPERTIES: &[&str] = &[
    "IsEnabled",
    "MaxHeight",
    "MaxWidth",
    "MinHeight",
    "MinWidth",
    "VerticalAlignment",
];

/// The element/property registry a XAML file resolves against.
pub struct Catalog {
    /// Element name -> element-specific property names. The common set in
    /// [`COMMON_PROPERTIES`] applies to every element.
    elements: HashMap<&'static str, &'static [&'static str]>,
    /// Markup extension names recognized at parse level. Resolution of
    /// StaticResource/ThemeResource/x:Null comes with the resources unit
    /// later in M1; `{x:Bind}` resolution is the renderer's today.
    extensions: &'static [&'static str],
}

impl Catalog {
    /// The PoC-subset catalog: the five M0 elements plus the markup
    /// extensions the dialect recognizes. Grows across M1.
    pub fn poc_subset() -> Catalog {
        let elements = HashMap::from([
            // `SystemBackdrop` is read by the host off the page root as an
            // interim; the real XAML `Window` element is REQ-WIN-01 (M2).
            ("Page", &["Background", "SystemBackdrop"][..]),
            ("StackPanel", &["Orientation", "Padding", "Spacing"][..]),
            (
                "TextBlock",
                &["FontSize", "FontWeight", "Foreground", "Text"][..],
            ),
            (
                "Border",
                &[
                    "Background",
                    "BorderBrush",
                    "BorderThickness",
                    "CornerRadius",
                    "Padding",
                ][..],
            ),
            ("Button", &["Click", "Content"][..]),
        ]);
        Catalog {
            elements,
            extensions: &["StaticResource", "ThemeResource", "x:Bind", "x:Null"],
        }
    }
}

/// Resolves `root` against `catalog`, returning all diagnostics found.
pub fn resolve(root: &XamlElement, catalog: &Catalog) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    resolve_element(root, catalog, &mut diags);
    diags
}

fn resolve_element(el: &XamlElement, catalog: &Catalog, diags: &mut Vec<Diagnostic>) {
    let known_element = catalog.elements.contains_key(el.name.as_str());
    if !known_element {
        diags.push(Diagnostic::error(
            "WX0002",
            format!("unknown element `{}`", el.name),
            el.pos,
            el.name.len() as u32 + 1,
        ));
        // Attribute checks against an unknown element would only produce
        // cascading noise; still recurse into children for full coverage.
        for child in &el.children {
            resolve_element(child, catalog, diags);
        }
        return;
    }

    for a in &el.attributes {
        // `x:` directives and `xml:*` language attributes are not element
        // properties; the dialect rules that consume them come later in M1.
        if a.name.starts_with("x:") || a.name.starts_with("xml:") {
            continue;
        }
        if a.name.contains('.') {
            diags.push(Diagnostic::warning(
                "WX0006",
                format!(
                    "attached property `{}` is not implemented in the PoC subset",
                    a.name
                ),
                a.pos,
                a.name.len() as u32,
            ));
            continue;
        }
        let known = catalog
            .elements
            .get(el.name.as_str())
            .is_some_and(|props| props.contains(&a.name.as_str()))
            || COMMON_PROPERTIES.contains(&a.name.as_str());
        if !known {
            if UNIMPLEMENTED_PROPERTIES.contains(&a.name.as_str()) {
                diags.push(Diagnostic::warning(
                    "WX0007",
                    format!(
                        "property `{}` is recognized but not implemented in the PoC subset",
                        a.name
                    ),
                    a.pos,
                    a.name.len() as u32,
                ));
            } else {
                diags.push(Diagnostic::error(
                    "WX0003",
                    format!("unknown property `{}` on `{}`", a.name, el.name),
                    a.pos,
                    a.name.len() as u32,
                ));
            }
        }

        check_markup_extension(el, a.name.as_str(), &a.value, catalog, diags);
    }

    for p in &el.properties {
        diags.push(Diagnostic::warning(
            "WX0004",
            format!(
                "property element `<{}>` is not implemented in the PoC subset",
                p.name
            ),
            p.pos,
            p.name.len() as u32 + 1,
        ));
        for child in &p.child_elements {
            resolve_element(child, catalog, diags);
        }
    }

    for child in &el.children {
        resolve_element(child, catalog, diags);
    }
}

/// `{Ext ...}` values must name a recognized extension; `{{` escapes a
/// literal brace and `{}` is the empty escape, both per XAML.
fn check_markup_extension(
    el: &XamlElement,
    attr: &str,
    value: &str,
    catalog: &Catalog,
    diags: &mut Vec<Diagnostic>,
) {
    let t = value.trim();
    if !(t.starts_with('{') && t.ends_with('}')) || t.starts_with("{{") {
        return;
    }
    let body = &t[1..t.len() - 1];
    let ext = body.split_whitespace().next().unwrap_or("");
    if ext.is_empty() {
        return;
    }
    if !catalog.extensions.contains(&ext) {
        diags.push(Diagnostic::error(
            "WX0005",
            format!(
                "unknown markup extension `{ext}` on `{attr}` of `{}`",
                el.name
            ),
            el.pos,
            el.name.len() as u32 + 1,
        ));
    }
}

/// Renders diagnostics codespan-style: header, file/line/col, source line,
/// caret. Plain text, no color.
pub fn render_diagnostics(file: &str, source: &str, diags: &[Diagnostic]) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let mut out = String::new();
    for d in diags {
        let kind = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        out.push_str(&format!("{kind}[{}]: {}\n", d.code, d.message));
        // `pos` is optional, so the snippet guard must not assume `Some`.
        if let Some(p) = &d.pos
            && p.row >= 1
            && (p.row as usize) <= lines.len()
        {
            let line = lines[p.row as usize - 1];
            let num = p.row.to_string();
            let pad = " ".repeat(num.len());
            out.push_str(&format!("{pad} --> {file}:{}:{}\n", p.row, p.col));
            out.push_str(&format!("{pad} |\n"));
            out.push_str(&format!("{num} | {line}\n"));
            let carets = "^".repeat(d.underline.max(1) as usize);
            let indent = " ".repeat(p.col.saturating_sub(1) as usize);
            out.push_str(&format!("{pad} | {indent}{carets}\n"));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use winui_xaml_syntax::parse;

    fn resolve_str(src: &str) -> Vec<Diagnostic> {
        let tree = parse(src).unwrap();
        resolve(&tree, &Catalog::poc_subset())
    }

    #[test]
    fn unknown_element_is_an_error() {
        let diags = resolve_str(
            r#"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation">
  <Foo/>
</Page>"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "WX0002");
        assert_eq!(diags[0].severity, Severity::Error);
        assert_eq!(diags[0].pos.unwrap().row, 2);
    }

    #[test]
    fn unknown_property_is_an_error() {
        let diags = resolve_str(
            r#"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
      Bg="1"/>"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "WX0003");
        assert_eq!(diags[0].severity, Severity::Error);
        assert_eq!(diags[0].pos.unwrap(), Pos { row: 2, col: 7 });
    }

    #[test]
    fn recognized_unimplemented_property_is_a_warning() {
        let diags = resolve_str(
            r#"<StackPanel xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" MinWidth="1"/>"#,
        );
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "WX0007");
        assert_eq!(diags[0].severity, Severity::Warning);
    }

    #[test]
    fn markup_extensions() {
        let ns = r#"xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation""#;
        // Recognized extensions and the `{{` escape produce nothing.
        for value in [
            "{StaticResource B}",
            "{ThemeResource B}",
            "{x:Bind Path}",
            "{x:Null}",
            "{{literal}}",
            "{}",
        ] {
            let src = format!(r#"<TextBlock {ns} Text="{value}"/>"#);
            assert!(
                resolve_str(&src).is_empty(),
                "expected no diagnostics for {value}"
            );
        }
        // Unknown extension is an error.
        let diags = resolve_str(&format!(r#"<TextBlock {ns} Text="{{MyExt P}}"/>"#));
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "WX0005");
        assert_eq!(diags[0].severity, Severity::Error);
    }

    #[test]
    fn attached_properties_and_property_elements_warn() {
        let src = r#"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
      xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml" Grid.Row="1">
  <Page.Resources>
    <Style x:Key="Big"/>
  </Page.Resources>
</Page>"#;
        let diags = resolve_str(src);
        assert_eq!(diags.len(), 3);
        assert_eq!(diags[0].code, "WX0006");
        assert_eq!(diags[1].code, "WX0004");
        assert_eq!(diags[1].pos.unwrap().row, 3);
        // The property element's children still resolve: Style is unknown
        // to the PoC catalog (it lands there with the M1 resources unit).
        assert_eq!(diags[2].code, "WX0002");
        assert_eq!(diags[2].pos.unwrap().row, 4);
    }

    #[test]
    fn unknown_element_children_still_resolve() {
        let diags = resolve_str(
            r#"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation">
  <Foo>
    <Bar/>
  </Foo>
</Page>"#,
        );
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].code, "WX0002");
        assert_eq!(diags[1].code, "WX0002");
    }

    #[test]
    fn emitter_shows_position_and_caret() {
        let src = "<Page>\n  <Foo/>\n</Page>\n";
        let tree = parse(src).unwrap();
        let diags = resolve(&tree, &Catalog::poc_subset());
        let text = render_diagnostics("demo.xaml", src, &diags);
        assert!(
            text.contains("error[WX0002]: unknown element `Foo`"),
            "{text}"
        );
        assert!(text.contains("--> demo.xaml:2:3"), "{text}");
        assert!(text.contains("2 |   <Foo/>"), "{text}");
        assert!(text.contains("  |   ^^^^"), "{text}");
    }
}
