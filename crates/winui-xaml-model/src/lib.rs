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
//! | WX1001 | unresolved `{StaticResource}`/`{ThemeResource}` key |
//! | WX1002 | malformed resource entry (no key, unparsable color, unsupported type) |
//! | WX1003 | unknown `ThemeDictionaries` key |
//! | WX1004 | style error (TargetType mismatch or unknown, BasedOn cycle or not a Style) |
//!
//! Resources (REQ-RES-01): [`collect_resources`] gathers `.Resources`
//! dictionaries (plain and theme) into a [`ResourceSet`]; [`ResourceSet::resolve`]
//! implements the WinUI nearest-scope lookup with per-theme dictionaries.
//! [`check_resource_references`] verifies every `{StaticResource}`/`{ThemeResource}`
//! use resolves — an unresolvable `{StaticResource}` is an error, matching
//! WinUI's parse exception. `MergedDictionaries` and dictionary `Source`
//! files are later M1 work; WinUI's `"Default"` theme key is rejected
//! because the vendored authoring rules ban it.
//!
//! Styles (REQ-RES-02): `Style` entries (keyed, or implicit keyed by
//! TargetType) parse into [`StyleDef`]; [`style_setters`] flattens the
//! BasedOn chain, and [`resolve_styles`] produces the resolved tree
//! (PLAN §3) with setter values applied wherever no local value exists.

use std::cmp::Reverse;
use std::collections::HashMap;

use winui_xaml_syntax::{Attribute, Pos, XamlElement};

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
    "Style",
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
            // Resource infrastructure (REQ-RES-01); their values resolve
            // through `ResourceSet`, not the renderer.
            ("ResourceDictionary", &[][..]),
            ("SolidColorBrush", &["Color"]),
            // The parser maps a Color element's text to a synthesized
            // Content attribute, so that is the property to check.
            ("Color", &["Content"]),
            // Style infrastructure (REQ-RES-02).
            ("Style", &["BasedOn", "TargetType"]),
            ("Setter", &["Property", "Value"]),
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
        // Resource dictionaries and the resource infrastructure's own
        // property elements are known (REQ-RES-01); the resources unit
        // consumes them, so only their children are checked here.
        if p.name == format!("{}.Resources", el.name)
            || matches!(
                p.name.as_str(),
                "ResourceDictionary.ThemeDictionaries" | "SolidColorBrush.Color" | "Style.Setters"
            )
        {
            for child in &p.child_elements {
                resolve_element(child, catalog, diags);
            }
            continue;
        }
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

/// The themes theme dictionaries can key on (REQ-RES-01). OS theme
/// detection is the host's concern until the token work (REQ-TOK-01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Theme {
    Light,
    Dark,
    HighContrast,
}

/// An 8-bit RGBA color. WinUI color strings read `#AARRGGBB`; the field
/// order keeps that reading explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub a: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Parses a WinUI color literal: 6- or 8-digit `#RRGGBB`/`#AARRGGBB` hex,
/// or the PoC's named subset (black/white/transparent).
pub fn parse_color(value: &str) -> Option<Rgba> {
    let v = value.trim();
    if let Some(hex) = v.strip_prefix('#') {
        return match hex.len() {
            6 => u32::from_str_radix(hex, 16).ok().map(|n| Rgba {
                a: 0xFF,
                r: (n >> 16) as u8,
                g: (n >> 8) as u8,
                b: n as u8,
            }),
            8 => u32::from_str_radix(hex, 16).ok().map(|n| Rgba {
                a: (n >> 24) as u8,
                r: (n >> 16) as u8,
                g: (n >> 8) as u8,
                b: n as u8,
            }),
            _ => None,
        };
    }
    match v.to_ascii_lowercase().as_str() {
        "black" => Some(Rgba {
            a: 0xFF,
            r: 0,
            g: 0,
            b: 0,
        }),
        "white" => Some(Rgba {
            a: 0xFF,
            r: 0xFF,
            g: 0xFF,
            b: 0xFF,
        }),
        "transparent" => Some(Rgba {
            a: 0,
            r: 0,
            g: 0,
            b: 0,
        }),
        _ => None,
    }
}

/// The value a resource key maps to: a color or a style (REQ-RES-01/02).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceValue {
    Color(Rgba),
    Style(StyleDef),
}

/// One `Setter` inside a `Style`: the target property name and its raw
/// value string. Markup extensions in the value resolve where the property
/// is applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Setter {
    pub property: String,
    pub value: String,
}

/// A WinUI `Style` (REQ-RES-02): a TargetType plus Setter list, optionally
/// `BasedOn` another keyed style. Explicit styles carry an `x:Key`;
/// implicit styles are keyed by TargetType alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleDef {
    /// Position of the Style element, for diagnostics.
    pub pos: Pos,
    /// TargetType local name. `None` only on `BasedOn` styles, which
    /// inherit the base style's TargetType (WinUI requires it otherwise).
    pub target_type: Option<String>,
    pub setters: Vec<Setter>,
    /// Raw `BasedOn` value; the key is extracted at resolution time.
    pub based_on: Option<String>,
}

/// All resource scopes collected from one page. Element-level `.Resources`
/// dictionaries are keyed by the element's render path; lookup prefers the
/// nearest scope (WinUI element → page → app order — app resources do not
/// exist yet).
#[derive(Default)]
pub struct ResourceSet {
    scopes: Vec<Scope>,
}

struct Scope {
    /// Render path of the owning element (`"0"` for the page root).
    owner: String,
    entries: HashMap<String, ResourceValue>,
    /// Per-theme dictionaries from `<…ResourceDictionary.ThemeDictionaries>`.
    theme_entries: HashMap<Theme, HashMap<String, ResourceValue>>,
    /// Implicit styles keyed by TargetType (REQ-RES-02).
    implicit_styles: HashMap<String, StyleDef>,
}

/// Scopes owning `path`, nearest first.
fn owning_scopes<'a>(set: &'a ResourceSet, path: &str) -> Vec<&'a Scope> {
    let mut owners: Vec<&Scope> = set
        .scopes
        .iter()
        .filter(|s| in_scope(path, &s.owner))
        .collect();
    owners.sort_by_key(|s| Reverse(s.owner.len()));
    owners
}

impl ResourceSet {
    /// Resolves `key` for the element at render `path`: the nearest owning
    /// scope containing the key wins; within a scope the active theme's
    /// dictionary wins over plain entries. `{ThemeResource}` re-resolution
    /// on theme switch re-runs this lookup (REQ-RES-01).
    pub fn resolve(&self, key: &str, path: &str, theme: Theme) -> Option<&ResourceValue> {
        for s in owning_scopes(self, path) {
            if let Some(v) = s.theme_entries.get(&theme).and_then(|m| m.get(key)) {
                return Some(v);
            }
            if let Some(v) = s.entries.get(key) {
                return Some(v);
            }
        }
        None
    }

    /// The implicit style for `target_type` at render `path` — nearest
    /// scope wins. Theme dictionaries are not consulted for implicit
    /// styles in the PoC subset.
    pub fn resolve_implicit_style(&self, target_type: &str, path: &str) -> Option<&StyleDef> {
        for s in owning_scopes(self, path) {
            if let Some(style) = s.implicit_styles.get(target_type) {
                return Some(style);
            }
        }
        None
    }
}

/// `path == owner` or a descendant: `"0.1"` is inside `"0"`, `"01"` is not.
fn in_scope(path: &str, owner: &str) -> bool {
    path == owner
        || (path.len() > owner.len()
            && path.starts_with(owner)
            && path.as_bytes()[owner.len()] == b'.')
}

/// Extracts the lookup key from `{StaticResource Key}` / `{ThemeResource
/// Key}` values — the first token after the extension name, trailing
/// parameters dropped. Returns `None` for other values and other
/// extensions; `{{` escapes a literal brace.
pub fn resource_key<'a>(value: &'a str, extension: &str) -> Option<&'a str> {
    let t = value.trim();
    if !(t.starts_with('{') && t.ends_with('}')) || t.starts_with("{{") {
        return None;
    }
    let body = &t[1..t.len() - 1];
    let mut parts = body.split_whitespace();
    if parts.next()? != extension {
        return None;
    }
    let key = parts.next()?;
    Some(key.split(',').next().unwrap_or(key))
}

/// Collects every `.Resources` dictionary in the tree into a
/// [`ResourceSet`], plus diagnostics for malformed entries (WX1002) and
/// unknown theme-dictionary keys (WX1003).
pub fn collect_resources(root: &XamlElement) -> (ResourceSet, Vec<Diagnostic>) {
    let mut set = ResourceSet::default();
    let mut diags = Vec::new();
    collect_scope(root, "0", &mut set, &mut diags);
    (set, diags)
}

/// Walks rendered elements with the same index paths the renderer computes
/// (`"0"`, `"0.1"`, …), gathering each element's `.Resources` dictionary.
fn collect_scope(el: &XamlElement, path: &str, set: &mut ResourceSet, diags: &mut Vec<Diagnostic>) {
    for p in &el.properties {
        if p.name == format!("{}.Resources", el.name) {
            parse_dictionary(&p.child_elements, path, set, diags);
        }
    }
    for (i, child) in el.children.iter().enumerate() {
        collect_scope(child, &format!("{path}.{i}"), set, diags);
    }
}

/// Parses one dictionary scope. WinUI's `.Resources` is an implicit
/// `ResourceDictionary`, so both the keyed-entries-directly shape and the
/// explicit `<ResourceDictionary>` child arrive here.
fn parse_dictionary(
    elements: &[XamlElement],
    owner: &str,
    set: &mut ResourceSet,
    diags: &mut Vec<Diagnostic>,
) {
    let mut scope = Scope {
        owner: owner.to_string(),
        entries: HashMap::new(),
        theme_entries: HashMap::new(),
        implicit_styles: HashMap::new(),
    };
    parse_dictionary_entries(elements, &mut scope, diags);
    set.scopes.push(scope);
}

fn parse_dictionary_entries(
    elements: &[XamlElement],
    scope: &mut Scope,
    diags: &mut Vec<Diagnostic>,
) {
    for e in elements {
        if e.name == "ResourceDictionary" {
            for p in &e.properties {
                if p.name == "ResourceDictionary.ThemeDictionaries" {
                    parse_theme_dictionaries(&p.child_elements, scope, diags);
                } else {
                    // MergedDictionaries and friends are later M1 work.
                    diags.push(Diagnostic::warning(
                        "WX0004",
                        format!(
                            "property element `<{}>` is not implemented in the PoC subset",
                            p.name
                        ),
                        p.pos,
                        p.name.len() as u32 + 1,
                    ));
                }
            }
            insert_entries(
                &e.children,
                &mut scope.entries,
                Some(&mut scope.implicit_styles),
                diags,
            );
        } else {
            insert_entries(
                std::slice::from_ref(e),
                &mut scope.entries,
                Some(&mut scope.implicit_styles),
                diags,
            );
        }
    }
}

fn insert_entries(
    elements: &[XamlElement],
    into: &mut HashMap<String, ResourceValue>,
    mut implicit: Option<&mut HashMap<String, StyleDef>>,
    diags: &mut Vec<Diagnostic>,
) {
    for e in elements {
        if e.attr("x:Key").is_none() && e.name == "Style" {
            match implicit.as_mut() {
                Some(map) => insert_implicit_style(e, map, diags),
                None => diags.push(Diagnostic::warning(
                    "WX1002",
                    "implicit Style is not supported inside ThemeDictionaries; give it an x:Key"
                        .to_string(),
                    e.pos,
                    e.name.len() as u32 + 1,
                )),
            }
            continue;
        }
        let Some(key) = e.attr("x:Key") else {
            diags.push(Diagnostic::warning(
                "WX1002",
                format!("resource `<{}>` has no x:Key; entry ignored", e.name),
                e.pos,
                e.name.len() as u32 + 1,
            ));
            continue;
        };
        match parse_entry(e) {
            Ok(value) => {
                into.insert(key.to_string(), value);
            }
            Err(reason) => diags.push(Diagnostic::warning(
                "WX1002",
                format!("resource entry `{key}` ignored: {reason}"),
                e.pos,
                e.name.len() as u32 + 1,
            )),
        }
    }
}

/// An unkeyed `Style` is implicit: keyed by its TargetType (REQ-RES-02).
fn insert_implicit_style(
    e: &XamlElement,
    into: &mut HashMap<String, StyleDef>,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(target) = e.attr("TargetType").map(str::trim) else {
        diags.push(Diagnostic::warning(
            "WX1002",
            "implicit Style needs a TargetType; entry ignored".to_string(),
            e.pos,
            e.name.len() as u32 + 1,
        ));
        return;
    };
    match parse_style(e) {
        Ok(style) => {
            into.insert(target.to_string(), style);
        }
        Err(reason) => diags.push(Diagnostic::warning(
            "WX1002",
            format!("implicit `{target}` style ignored: {reason}"),
            e.pos,
            e.name.len() as u32 + 1,
        )),
    }
}

fn parse_theme_dictionaries(
    elements: &[XamlElement],
    scope: &mut Scope,
    diags: &mut Vec<Diagnostic>,
) {
    for dict in elements {
        let Some(key) = dict.attr("x:Key") else {
            diags.push(Diagnostic::warning(
                "WX1002",
                "theme dictionary has no x:Key; ignored".to_string(),
                dict.pos,
                dict.name.len() as u32 + 1,
            ));
            continue;
        };
        let theme = match key.trim() {
            "Light" => Theme::Light,
            "Dark" => Theme::Dark,
            "HighContrast" => Theme::HighContrast,
            other => {
                diags.push(Diagnostic::error(
                    "WX1003",
                    format!("unknown theme dictionary key `{other}` (Light/Dark/HighContrast)"),
                    dict.pos,
                    key.len() as u32 + 1,
                ));
                continue;
            }
        };
        let entries = scope.theme_entries.entry(theme).or_default();
        insert_entries(&dict.children, entries, None, diags);
    }
}

/// Parses one keyed resource element. The value forms for now:
/// `SolidColorBrush` (Color attribute or `.Color` property element), a
/// `Color` element (whose text the parser synthesizes into Content), and
/// a keyed `Style`.
fn parse_entry(el: &XamlElement) -> Result<ResourceValue, String> {
    let raw = match el.name.as_str() {
        "SolidColorBrush" => el
            .attr("Color")
            .or_else(|| {
                el.properties
                    .iter()
                    .find(|p| p.name == "SolidColorBrush.Color")
                    .and_then(|p| p.text.as_deref())
            })
            .ok_or_else(|| "SolidColorBrush has no Color".to_string())?,
        "Color" => el
            .attr("Content")
            .ok_or_else(|| "Color element has no text value".to_string())?,
        "Style" => return parse_style(el).map(ResourceValue::Style),
        other => {
            return Err(format!(
                "resource type `<{other}>` is not implemented in the PoC subset"
            ));
        }
    };
    parse_color(raw)
        .map(ResourceValue::Color)
        .ok_or_else(|| format!("`{raw}` is not a color the PoC subset parses"))
}

/// Parses a `Style` element: TargetType, BasedOn, and the Setter list.
/// Setters is Style's content property, so setters arrive either as direct
/// children or inside `<Style.Setters>`.
fn parse_style(el: &XamlElement) -> Result<StyleDef, String> {
    let mut setter_elements: Vec<&XamlElement> = el.children.iter().collect();
    for p in &el.properties {
        if p.name == "Style.Setters" {
            setter_elements.extend(p.child_elements.iter());
        } else {
            return Err(format!(
                "property element `<{}>` on Style is not implemented in the PoC subset",
                p.name
            ));
        }
    }
    let mut setters = Vec::new();
    for s in setter_elements {
        if s.name != "Setter" {
            return Err(format!("<{}> inside a Style is not a Setter", s.name));
        }
        let (Some(property), Some(value)) = (s.attr("Property"), s.attr("Value")) else {
            return Err("Setter needs Property and Value attributes".to_string());
        };
        setters.push(Setter {
            property: property.trim().to_string(),
            value: value.to_string(),
        });
    }
    Ok(StyleDef {
        pos: el.pos,
        target_type: el.attr("TargetType").map(|t| t.trim().to_string()),
        setters,
        based_on: el.attr("BasedOn").map(str::to_string),
    })
}

/// Scans rendered attributes for `{StaticResource …}`/`{ThemeResource …}`
/// uses and verifies the keys resolve (REQ-RES-01). A missing
/// `{StaticResource}` key is an error, matching WinUI's parse exception; a
/// missing `{ThemeResource}` key warns, because WinUI leaves the property
/// unset and keeps going.
pub fn check_resource_references(
    root: &XamlElement,
    resources: &ResourceSet,
    theme: Theme,
) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    check_refs(root, "0", resources, theme, &mut diags);
    diags
}

fn check_refs(
    el: &XamlElement,
    path: &str,
    resources: &ResourceSet,
    theme: Theme,
    diags: &mut Vec<Diagnostic>,
) {
    for a in &el.attributes {
        for ext in ["StaticResource", "ThemeResource"] {
            let Some(key) = resource_key(&a.value, ext) else {
                continue;
            };
            if resources.resolve(key, path, theme).is_none() {
                let message = format!("{ext} key `{key}` not found on `{}`", a.name);
                match ext {
                    "StaticResource" => diags.push(Diagnostic::error(
                        "WX1001",
                        message,
                        a.pos,
                        a.value.len() as u32,
                    )),
                    _ => diags.push(Diagnostic::warning(
                        "WX1001",
                        message,
                        a.pos,
                        a.value.len() as u32,
                    )),
                }
            }
        }
    }
    // A `Style` attribute must name a style whose TargetType matches the
    // element it decorates (WinUI throws otherwise).
    if let Some(key) = el.attr("Style").and_then(|v| {
        resource_key(v, "StaticResource").or_else(|| resource_key(v, "ThemeResource"))
    }) {
        match resources.resolve(key, path, theme) {
            Some(ResourceValue::Style(style)) => {
                if let Some(target) = &style.target_type
                    && *target != el.name
                {
                    diags.push(Diagnostic::error(
                        "WX1004",
                        format!(
                            "Style `{key}` targets `{target}` but is applied to `{}`",
                            el.name
                        ),
                        el.pos,
                        el.name.len() as u32 + 1,
                    ));
                }
            }
            Some(_) => diags.push(Diagnostic::error(
                "WX1004",
                format!("resource `{key}` used as Style is not a Style"),
                el.pos,
                el.name.len() as u32 + 1,
            )),
            // The extension loop above already reported the missing key.
            None => {}
        }
    }
    for (i, child) in el.children.iter().enumerate() {
        check_refs(child, &format!("{path}.{i}"), resources, theme, diags);
    }
}

/// The flattened setter list for the style in effect on `el` at `path`:
/// its explicit `Style` attribute, else the implicit style for its type.
/// Based-on setters come first and derived setters last, so later entries
/// win. BasedOn chains are followed at most 16 levels; [`check_style_chains`]
/// reports broken references and cycles at load time.
pub fn style_setters<'s>(
    set: &'s ResourceSet,
    el: &XamlElement,
    path: &str,
    theme: Theme,
) -> Vec<(&'s str, &'s str)> {
    fn as_style(value: &ResourceValue) -> Option<&StyleDef> {
        match value {
            ResourceValue::Style(s) => Some(s),
            _ => None,
        }
    }
    let explicit = el
        .attr("Style")
        .and_then(|v| {
            resource_key(v, "StaticResource").or_else(|| resource_key(v, "ThemeResource"))
        })
        .and_then(|key| set.resolve(key, path, theme))
        .and_then(as_style);
    let mut chain = Vec::new();
    let mut current = explicit.or_else(|| set.resolve_implicit_style(el.name.as_str(), path));
    for _ in 0..16 {
        let Some(style) = current else { break };
        chain.push(style);
        current = style
            .based_on
            .as_deref()
            .and_then(|b| {
                resource_key(b, "StaticResource").or_else(|| resource_key(b, "ThemeResource"))
            })
            .and_then(|key| set.resolve(key, path, theme))
            .and_then(as_style);
    }
    chain.reverse();
    chain
        .iter()
        .flat_map(|s| {
            s.setters
                .iter()
                .map(|st| (st.property.as_str(), st.value.as_str()))
        })
        .collect()
}

/// Applies the style in effect on every element into a resolved copy of
/// the tree (PLAN §3's resolved visual tree): setter values become
/// attributes wherever the element has no local value — WinUI precedence
/// is local value over style setter. Bindings and event attributes pass
/// through untouched; they stay per-render concerns.
pub fn resolve_styles(root: &XamlElement, resources: &ResourceSet, theme: Theme) -> XamlElement {
    apply_styles(root, "0", resources, theme)
}

fn apply_styles(
    el: &XamlElement,
    path: &str,
    resources: &ResourceSet,
    theme: Theme,
) -> XamlElement {
    let mut attributes = el.attributes.clone();
    // The flattened list has derived setters last; they override base
    // setters, so dedupe keeping the last occurrence. A local attribute
    // still beats every setter.
    let mut setters: HashMap<&str, &str> = HashMap::new();
    for (property, value) in style_setters(resources, el, path, theme) {
        setters.insert(property, value);
    }
    for (property, value) in setters {
        if !attributes.iter().any(|a| a.name == property) {
            attributes.push(Attribute {
                name: property.to_string(),
                value: value.to_string(),
                pos: el.pos,
            });
        }
    }
    XamlElement {
        name: el.name.clone(),
        pos: el.pos,
        attributes,
        // Property-element children are resource entries and content
        // bodies here, not rendered elements; they carry no styles.
        properties: el.properties.clone(),
        children: el
            .children
            .iter()
            .enumerate()
            .map(|(i, c)| apply_styles(c, &format!("{path}.{i}"), resources, theme))
            .collect(),
    }
}

/// Validates every collected style against the catalog (REQ-RES-02):
/// TargetType present and known, setter properties known on the
/// TargetType, and BasedOn chains resolvable, style-typed,
/// TargetType-compatible, and cycle-free (all WX1004-class, plus WX0003
/// for unknown setter properties and WX1001 for missing BasedOn keys).
pub fn check_style_chains(resources: &ResourceSet, catalog: &Catalog) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for scope in &resources.scopes {
        for (key, value) in &scope.entries {
            if let ResourceValue::Style(style) = value {
                check_one_style(&format!("`{key}`"), style, scope, catalog, &mut diags);
            }
        }
        for (target, style) in &scope.implicit_styles {
            check_one_style(
                &format!("(implicit `{target}`)"),
                style,
                scope,
                catalog,
                &mut diags,
            );
        }
    }
    diags
}

fn check_one_style(
    label: &str,
    style: &StyleDef,
    scope: &Scope,
    catalog: &Catalog,
    diags: &mut Vec<Diagnostic>,
) {
    let target = match &style.target_type {
        Some(t) => Some(t.as_str()),
        None => {
            if style.based_on.is_none() {
                diags.push(Diagnostic::error(
                    "WX1004",
                    format!("Style {label} needs a TargetType or BasedOn"),
                    style.pos,
                    5,
                ));
            }
            None
        }
    };
    if let Some(t) = target {
        match catalog.elements.get(t) {
            None => diags.push(Diagnostic::error(
                "WX1004",
                format!("Style {label} targets unknown element `{t}`"),
                style.pos,
                t.len() as u32 + 1,
            )),
            Some(props) => {
                for setter in &style.setters {
                    if props.contains(&setter.property.as_str())
                        || COMMON_PROPERTIES.contains(&setter.property.as_str())
                    {
                        continue;
                    }
                    if UNIMPLEMENTED_PROPERTIES.contains(&setter.property.as_str()) {
                        diags.push(Diagnostic::warning(
                            "WX0007",
                            format!(
                                "Setter property `{}` on `{t}` is recognized but not implemented in the PoC subset",
                                setter.property
                            ),
                            style.pos,
                            setter.property.len() as u32,
                        ));
                    } else {
                        diags.push(Diagnostic::error(
                            "WX0003",
                            format!("unknown property `{}` on Setter for `{t}`", setter.property),
                            style.pos,
                            setter.property.len() as u32 + 1,
                        ));
                    }
                }
            }
        }
    }

    let mut seen: Vec<&str> = Vec::new();
    let mut current = style;
    while let Some(based_on) = &current.based_on {
        let Some(key) = resource_key(based_on, "StaticResource")
            .or_else(|| resource_key(based_on, "ThemeResource"))
        else {
            diags.push(Diagnostic::error(
                "WX1002",
                format!(
                    "Style {label} BasedOn must be `{{StaticResource …}}` or `{{ThemeResource …}}`"
                ),
                style.pos,
                based_on.len() as u32,
            ));
            break;
        };
        if seen.contains(&key) {
            diags.push(Diagnostic::error(
                "WX1004",
                format!("Style {label} BasedOn cycle involving `{key}`"),
                style.pos,
                key.len() as u32 + 1,
            ));
            break;
        }
        match scope.entries.get(key) {
            Some(ResourceValue::Style(base)) => {
                if let (Some(a), Some(b)) = (&current.target_type, &base.target_type)
                    && a != b
                {
                    diags.push(Diagnostic::error(
                        "WX1004",
                        format!("Style {label} targets `{a}` but BasedOn `{key}` targets `{b}`"),
                        style.pos,
                        key.len() as u32 + 1,
                    ));
                    break;
                }
                seen.push(key);
                current = base;
            }
            Some(_) => {
                diags.push(Diagnostic::error(
                    "WX1004",
                    format!("Style {label} BasedOn `{key}` is not a Style"),
                    style.pos,
                    key.len() as u32 + 1,
                ));
                break;
            }
            None => {
                diags.push(Diagnostic::error(
                    "WX1001",
                    format!("Style {label} BasedOn key `{key}` not found"),
                    style.pos,
                    key.len() as u32 + 1,
                ));
                break;
            }
        }
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
        // Page.Resources is known resource infrastructure now, and Style
        // is a known catalog element since the styles unit landed.
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "WX0006");
        assert_eq!(diags[0].pos.unwrap().row, 2);
    }

    const RESOURCES_DOC: &str = r##"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
      xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
  <Page.Resources>
    <ResourceDictionary>
      <ResourceDictionary.ThemeDictionaries>
        <ResourceDictionary x:Key="Light">
          <SolidColorBrush x:Key="TextBrush" Color="#FF111111"/>
        </ResourceDictionary>
        <ResourceDictionary x:Key="Dark">
          <SolidColorBrush x:Key="TextBrush" Color="#FFF3F3F3"/>
        </ResourceDictionary>
      </ResourceDictionary.ThemeDictionaries>
      <SolidColorBrush x:Key="AccentBrush" Color="#FF4EC9B0"/>
    </ResourceDictionary>
  </Page.Resources>
  <StackPanel Padding="8">
    <TextBlock Text="a" Foreground="{ThemeResource TextBrush}"/>
    <Border Background="{StaticResource AccentBrush}"/>
  </StackPanel>
</Page>"##;

    #[test]
    fn resources_resolve_by_scope_and_theme() {
        let tree = parse(RESOURCES_DOC).unwrap();
        let (set, diags) = collect_resources(&tree);
        assert!(diags.is_empty(), "{diags:?}");

        // Theme dictionaries answer for the active theme, plain entries
        // for any theme.
        assert_eq!(
            set.resolve("TextBrush", "0.0.0", Theme::Dark),
            Some(&ResourceValue::Color(Rgba {
                a: 0xFF,
                r: 0xF3,
                g: 0xF3,
                b: 0xF3
            }))
        );
        assert_eq!(
            set.resolve("TextBrush", "0.0.0", Theme::Light),
            Some(&ResourceValue::Color(Rgba {
                a: 0xFF,
                r: 0x11,
                g: 0x11,
                b: 0x11
            }))
        );
        assert_eq!(
            set.resolve("AccentBrush", "0", Theme::Light),
            Some(&ResourceValue::Color(Rgba {
                a: 0xFF,
                r: 0x4E,
                g: 0xC9,
                b: 0xB0
            }))
        );
        assert_eq!(set.resolve("Nope", "0.0.0", Theme::Dark), None);
    }

    #[test]
    fn element_scope_shadows_page_scope() {
        const DOC: &str = r##"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
      xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
  <Page.Resources>
    <SolidColorBrush x:Key="B" Color="#FF111111"/>
  </Page.Resources>
  <StackPanel Padding="8">
    <Border>
      <Border.Resources>
        <SolidColorBrush x:Key="B" Color="#FF222222"/>
      </Border.Resources>
    </Border>
  </StackPanel>
</Page>"##;
        let tree = parse(DOC).unwrap();
        let (set, diags) = collect_resources(&tree);
        assert!(diags.is_empty(), "{diags:?}");
        // The Border (render path "0.0.0") is shadowed by its own scope.
        assert_eq!(
            set.resolve("B", "0.0.0", Theme::Dark),
            Some(&ResourceValue::Color(Rgba {
                a: 0xFF,
                r: 0x22,
                g: 0x22,
                b: 0x22
            }))
        );
        // The StackPanel ("0.0") still sees the page-level entry.
        assert_eq!(
            set.resolve("B", "0.0", Theme::Dark),
            Some(&ResourceValue::Color(Rgba {
                a: 0xFF,
                r: 0x11,
                g: 0x11,
                b: 0x11
            }))
        );
    }

    #[test]
    fn resource_references_are_checked() {
        let tree = parse(RESOURCES_DOC).unwrap();
        let (set, _) = collect_resources(&tree);
        let diags = check_resource_references(&tree, &set, Theme::Dark);
        assert!(diags.is_empty(), "{diags:?}");

        // A missing StaticResource is an error (WinUI parse exception); a
        // missing ThemeResource only warns.
        const DOC: &str = r#"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
      xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
  <TextBlock Text="{StaticResource Missing}" Foreground="{ThemeResource AlsoMissing}"/>
</Page>"#;
        let tree = parse(DOC).unwrap();
        let set = ResourceSet::default();
        let diags = check_resource_references(&tree, &set, Theme::Dark);
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].code, "WX1001");
        assert_eq!(diags[0].severity, Severity::Error);
        assert_eq!(diags[0].pos.unwrap().row, 3);
        assert_eq!(diags[1].code, "WX1001");
        assert_eq!(diags[1].severity, Severity::Warning);
    }

    #[test]
    fn resource_entry_diagnostics() {
        const DOC: &str = r##"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
      xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
  <Page.Resources>
    <ResourceDictionary>
      <ResourceDictionary.ThemeDictionaries>
        <ResourceDictionary x:Key="Default">
          <SolidColorBrush x:Key="B" Color="#FF000000"/>
        </ResourceDictionary>
      </ResourceDictionary.ThemeDictionaries>
      <SolidColorBrush x:Key="Bad" Color="#12345"/>
      <Color x:Key="Good">#FF4EC9B0</Color>
      <Style x:Key="S"/>
    </ResourceDictionary>
  </Page.Resources>
</Page>"##;
        let tree = parse(DOC).unwrap();
        let (set, diags) = collect_resources(&tree);
        // WX1003 for the banned "Default" theme key and WX1002 for the
        // bad color. `Good` still parses (the parser synthesizes its text
        // into Content); the empty keyed Style `S` parses as a style —
        // check_style_chains is what flags its missing TargetType.
        assert_eq!(diags.len(), 2);
        assert_eq!(diags[0].code, "WX1003");
        assert_eq!(diags[1].code, "WX1002");
        assert_eq!(
            set.resolve("Good", "0", Theme::Dark),
            Some(&ResourceValue::Color(Rgba {
                a: 0xFF,
                r: 0x4E,
                g: 0xC9,
                b: 0xB0
            }))
        );
        assert!(matches!(
            set.resolve("S", "0", Theme::Dark),
            Some(ResourceValue::Style(_))
        ));
        assert_eq!(set.resolve("Bad", "0", Theme::Dark), None);
    }

    #[test]
    fn color_and_key_parsing() {
        assert_eq!(
            parse_color("#FFF3F3F3"),
            Some(Rgba {
                a: 0xFF,
                r: 0xF3,
                g: 0xF3,
                b: 0xF3
            })
        );
        assert_eq!(
            parse_color("#4EC9B0"),
            Some(Rgba {
                a: 0xFF,
                r: 0x4E,
                g: 0xC9,
                b: 0xB0
            })
        );
        assert_eq!(
            parse_color("transparent"),
            Some(Rgba {
                a: 0,
                r: 0,
                g: 0,
                b: 0
            })
        );
        assert_eq!(parse_color("#12345"), None);
        assert_eq!(parse_color("nope"), None);

        assert_eq!(
            resource_key("{StaticResource B}", "StaticResource"),
            Some("B")
        );
        assert_eq!(
            resource_key(" {ThemeResource B} ", "ThemeResource"),
            Some("B")
        );
        assert_eq!(resource_key("{ThemeResource B}", "StaticResource"), None);
        assert_eq!(resource_key("{{literal}}", "StaticResource"), None);
        assert_eq!(resource_key("{x:Bind Path}", "StaticResource"), None);
        assert_eq!(resource_key("{StaticResource}", "StaticResource"), None);
    }

    const STYLES_DOC: &str = r##"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
      xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
  <Page.Resources>
    <ResourceDictionary>
      <Style x:Key="BaseText" TargetType="TextBlock">
        <Setter Property="FontSize" Value="12"/>
        <Setter Property="Foreground" Value="{ThemeResource TextBrush}"/>
      </Style>
      <Style x:Key="AccentText" TargetType="TextBlock" BasedOn="{StaticResource BaseText}">
        <Setter Property="FontSize" Value="16"/>
      </Style>
      <Style TargetType="Border">
        <Style.Setters>
          <Setter Property="CornerRadius" Value="8"/>
        </Style.Setters>
      </Style>
    </ResourceDictionary>
  </Page.Resources>
  <StackPanel Padding="8">
    <TextBlock Text="a" Style="{StaticResource AccentText}"/>
    <Border/>
  </StackPanel>
</Page>"##;

    #[test]
    fn styles_resolve_and_apply() {
        let tree = parse(STYLES_DOC).unwrap();
        let (set, diags) = collect_resources(&tree);
        assert!(diags.is_empty(), "{diags:?}");
        assert!(check_style_chains(&set, &Catalog::poc_subset()).is_empty());

        // Explicit style: base setters first, derived override last.
        let bound = &tree.children[0].children[0];
        let setters = style_setters(&set, bound, "0.0.0", Theme::Dark);
        assert_eq!(
            setters,
            [
                ("FontSize", "12"),
                ("Foreground", "{ThemeResource TextBrush}"),
                ("FontSize", "16")
            ]
        );
        // The Border picks up the implicit style written via
        // <Style.Setters>; the TextBlock-shaped implicit style does not
        // leak onto other types.
        let border = &tree.children[0].children[1];
        assert_eq!(
            style_setters(&set, border, "0.0.1", Theme::Dark),
            [("CornerRadius", "8")]
        );
        let page_only: &[(&str, &str)] = &[];
        assert_eq!(style_setters(&set, &tree, "0", Theme::Dark), page_only);
    }

    #[test]
    fn resolve_styles_applies_setters_with_local_precedence() {
        const DOC: &str = r##"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
      xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
  <Page.Resources>
    <Style x:Key="BaseText" TargetType="TextBlock">
      <Setter Property="FontSize" Value="12"/>
      <Setter Property="Foreground" Value="#FF111111"/>
    </Style>
    <Style x:Key="AccentText" TargetType="TextBlock" BasedOn="{StaticResource BaseText}">
      <Setter Property="FontSize" Value="16"/>
    </Style>
  </Page.Resources>
  <StackPanel Padding="8">
    <TextBlock Text="a" Style="{StaticResource AccentText}" Foreground="#FF222222"/>
    <TextBlock Text="b" Style="{StaticResource AccentText}"/>
  </StackPanel>
</Page>"##;
        let tree = parse(DOC).unwrap();
        let (set, _) = collect_resources(&tree);
        let resolved = resolve_styles(&tree, &set, Theme::Dark);
        // First TextBlock: the local Foreground wins over the setter, and
        // FontSize comes from the derived style (16, not the base's 12).
        let first = &resolved.children[0].children[0];
        assert_eq!(first.attr("FontSize"), Some("16"));
        assert_eq!(first.attr("Foreground"), Some("#FF222222"));
        // Second TextBlock: both setters land, including the base's.
        let second = &resolved.children[0].children[1];
        assert_eq!(second.attr("FontSize"), Some("16"));
        assert_eq!(second.attr("Foreground"), Some("#FF111111"));
    }

    #[test]
    fn style_chain_diagnostics() {
        const DOC: &str = r##"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
      xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
  <Page.Resources>
    <Style x:Key="A" TargetType="TextBlock" BasedOn="{StaticResource B}"/>
    <Style x:Key="B" TargetType="TextBlock" BasedOn="{StaticResource A}"/>
    <Style x:Key="C" TargetType="NoSuchElement"/>
    <Style x:Key="D" TargetType="TextBlock">
      <Setter Property="Nope" Value="1"/>
    </Style>
    <Style x:Key="E" TargetType="TextBlock" BasedOn="{StaticResource Missing}"/>
  </Page.Resources>
</Page>"##;
        let tree = parse(DOC).unwrap();
        let (set, collect_diags) = collect_resources(&tree);
        assert!(collect_diags.is_empty(), "{collect_diags:?}");
        let diags = check_style_chains(&set, &Catalog::poc_subset());
        // A and B both detect the cycle, C targets an unknown element, D
        // sets an unknown property, E bases on a missing key. HashMap
        // order is not deterministic, so compare the code set.
        let mut codes: Vec<&str> = diags.iter().map(|d| d.code).collect();
        codes.sort_unstable();
        assert_eq!(codes, ["WX0003", "WX1001", "WX1004", "WX1004", "WX1004"]);
    }

    #[test]
    fn style_use_target_type_is_checked() {
        const DOC: &str = r##"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
      xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
  <Page.Resources>
    <Style x:Key="T" TargetType="TextBlock"/>
  </Page.Resources>
  <Button Style="{StaticResource T}" Content="b"/>
</Page>"##;
        let tree = parse(DOC).unwrap();
        let (set, _) = collect_resources(&tree);
        let diags = check_resource_references(&tree, &set, Theme::Dark);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "WX1004");
        assert_eq!(diags[0].severity, Severity::Error);
    }

    #[test]
    fn implicit_style_needs_page_level_scope() {
        const DOC: &str = r##"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
      xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
  <Page.Resources>
    <ResourceDictionary>
      <ResourceDictionary.ThemeDictionaries>
        <ResourceDictionary x:Key="Dark">
          <Style TargetType="TextBlock">
            <Setter Property="FontSize" Value="12"/>
          </Style>
        </ResourceDictionary>
      </ResourceDictionary.ThemeDictionaries>
    </ResourceDictionary>
  </Page.Resources>
</Page>"##;
        let tree = parse(DOC).unwrap();
        let (set, diags) = collect_resources(&tree);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, "WX1002");
        assert!(set.resolve_implicit_style("TextBlock", "0").is_none());
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
