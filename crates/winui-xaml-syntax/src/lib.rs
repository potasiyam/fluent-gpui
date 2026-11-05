//! XAML syntax layer: parse a XAML document into a plain element tree with
//! source positions (PRD REQ-DX-01).
//!
//! Backend-independent and gpui-free (PLAN §3). Namespaces are accepted but
//! not validated; `x:`-prefixed attributes from the XAML language namespace
//! are normalized to their `x:` spelling so the renderer can key on them.
//! `d:`/`mc:` designer namespaces are parsed and ignored, per WinUI; `xml:`
//! attributes are accepted and stored for the model layer.

/// 1:1-based line and column in the source document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos {
    pub row: u32,
    pub col: u32,
}

/// One attribute with the source position of its name.
#[derive(Debug, Clone)]
pub struct Attribute {
    pub name: String,
    pub value: String,
    pub pos: Pos,
}

/// A property-element child (`<Button.Content>…</Button.Content>`, or an
/// attached property like `<Grid.Row>1</Grid.Row>`). The dotted name stays
/// WinUI-exact; that is the compatibility surface.
#[derive(Debug, Clone)]
pub struct PropertyElement {
    pub name: String,
    pub pos: Pos,
    pub child_elements: Vec<XamlElement>,
    /// Inner text of the property element, if any.
    pub text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct XamlElement {
    /// Local element name, e.g. `"StackPanel"`.
    pub name: String,
    /// Position of the element's opening tag.
    pub pos: Pos,
    pub attributes: Vec<Attribute>,
    /// Property-element children, in document order.
    pub properties: Vec<PropertyElement>,
    /// Default-content children (not targeting a property element).
    pub children: Vec<XamlElement>,
}

impl XamlElement {
    /// Attribute value by name, `x:`-prefixed names included.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.value.as_str())
    }
}

const XAML_LANGUAGE_NS: &str = "http://schemas.microsoft.com/winfx/2006/xaml";
const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";
const BLEND_NS: &str = "http://schemas.microsoft.com/expression/blend/2008";
const MARKUP_COMPAT_NS: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";

pub fn parse(src: &str) -> Result<XamlElement, String> {
    let doc = roxmltree::Document::parse(src).map_err(|e| {
        let p = e.pos();
        format!("XAML parse error at {}:{}: {e}", p.row, p.col)
    })?;
    let root = doc
        .root()
        .first_element_child()
        .ok_or_else(|| "document has no root element".to_string())?;
    convert(&doc, root)
}

fn pos_of(doc: &roxmltree::Document, byte: usize) -> Pos {
    let p = doc.text_pos_at(byte);
    Pos {
        row: p.row,
        col: p.col,
    }
}

fn convert(doc: &roxmltree::Document, node: roxmltree::Node) -> Result<XamlElement, String> {
    let pos = pos_of(doc, node.range().start);
    // Elements from the XAML language namespace keep their `x:` spelling,
    // like the layer's x:-prefixed attributes (x:Boolean and friends).
    let name = match node.tag_name().namespace() {
        Some(XAML_LANGUAGE_NS) => format!("x:{}", node.tag_name().name()),
        _ => node.tag_name().name().to_string(),
    };

    let mut attributes = Vec::new();
    for attr in node.attributes() {
        let name = match attr.namespace() {
            None => attr.name().to_string(),
            Some(XAML_LANGUAGE_NS) => format!("x:{}", attr.name()),
            Some(XML_NS) => format!("xml:{}", attr.name()),
            Some(BLEND_NS) | Some(MARKUP_COMPAT_NS) => continue,
            Some(_) => {
                // Prefixed attributes from other foreign namespaces are
                // outside the PoC subset; dropping them under their bare
                // local name could collide with a same-named unprefixed
                // attribute.
                eprintln!(
                    "[xaml] attribute \"{}\" from a foreign namespace is not in the PoC subset; ignored",
                    attr.name()
                );
                continue;
            }
        };
        let pos = pos_of(doc, attr.range_qname().start);
        attributes.push(Attribute {
            name,
            value: attr.value().to_string(),
            pos,
        });
    }

    let mut properties = Vec::new();
    let mut children = Vec::new();
    for child in node.children() {
        if child.is_element() {
            let child_name = child.tag_name().name();
            if child_name.contains('.') {
                let (child_elements, text) = convert_property(doc, child)?;
                properties.push(PropertyElement {
                    name: child_name.to_string(),
                    pos: pos_of(doc, child.range().start),
                    child_elements,
                    text,
                });
            } else {
                children.push(convert(doc, child)?);
            }
        } else if child.is_text() {
            // Inner text becomes the default Content (Button/TextBlock style).
            let text = child.text().unwrap_or("").trim();
            if !text.is_empty() && !attributes.iter().any(|a| a.name == "Content") {
                attributes.push(Attribute {
                    name: "Content".to_string(),
                    value: text.to_string(),
                    pos: pos_of(doc, child.range().start),
                });
            }
        }
    }

    Ok(XamlElement {
        name,
        pos,
        attributes,
        properties,
        children,
    })
}

fn convert_property(
    doc: &roxmltree::Document,
    node: roxmltree::Node,
) -> Result<(Vec<XamlElement>, Option<String>), String> {
    let mut child_elements = Vec::new();
    let mut text = String::new();
    for child in node.children() {
        if child.is_element() {
            child_elements.push(convert(doc, child)?);
        } else if child.is_text() {
            let piece = child.text().unwrap_or("").trim();
            if !piece.is_empty() {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(piece);
            }
        }
    }
    let text = if text.is_empty() { None } else { Some(text) };
    Ok((child_elements, text))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
      xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
  <StackPanel Spacing="14">
    <TextBlock Text="Hi" x:Name="title"/>
    <Button Click="OnIncrement">Increment</Button>
  </StackPanel>
</Page>"#;

    #[test]
    fn parses_tree_shape_and_x_namespace() {
        let tree = parse(SAMPLE).unwrap();
        assert_eq!(tree.name, "Page");
        let panel = &tree.children[0];
        assert_eq!(panel.name, "StackPanel");
        let kinds: Vec<&str> = panel.children.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(kinds, ["TextBlock", "Button"]);

        let title = &panel.children[0];
        assert_eq!(title.attr("x:Name"), Some("title"));
        let button = &panel.children[1];
        assert_eq!(button.attr("Click"), Some("OnIncrement"));
        assert_eq!(button.attr("Content"), Some("Increment"));
    }

    #[test]
    fn positions_are_one_based() {
        let tree = parse(SAMPLE).unwrap();
        assert_eq!(tree.pos, Pos { row: 1, col: 1 });
        let panel = &tree.children[0];
        assert_eq!(panel.pos, Pos { row: 3, col: 3 });
        let title = &panel.children[0];
        assert_eq!(title.pos.row, 4);
        let name_attr = &title
            .attributes
            .iter()
            .find(|a| a.name == "x:Name")
            .unwrap();
        assert_eq!(name_attr.pos.row, 4);
    }

    #[test]
    fn property_elements_attached_properties_designer_ns() {
        const DOC: &str = r#"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
      xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
      xmlns:d="http://schemas.microsoft.com/expression/blend/2008"
      xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"
      mc:Ignorable="d" Grid.Row="1">
  <Page.Resources>
    <Style x:Key="Big"/>
  </Page.Resources>
  <Button.Content><![CDATA[raw <text>]]></Button.Content>
  <!-- a comment, ignored -->
</Page>"#;

        let tree = parse(DOC).unwrap();
        // Attached-property attribute stays under its dotted WinUI name.
        assert_eq!(tree.attr("Grid.Row"), Some("1"));
        // d:/mc: attributes are parsed and ignored, never surfaced.
        assert!(tree.attr("mc:Ignorable").is_none());

        let resources = &tree.properties[0];
        assert_eq!(resources.name, "Page.Resources");
        assert_eq!(resources.child_elements[0].name, "Style");
        assert_eq!(resources.child_elements[0].attr("x:Key"), Some("Big"));
        // CDATA inside a property element arrives as text.
        let content = &tree.properties[1];
        assert_eq!(content.name, "Button.Content");
        assert_eq!(content.text.as_deref(), Some("raw <text>"));

        // No default-content children; no synthesized Content attribute.
        assert!(tree.children.is_empty());
        assert!(tree.attr("Content").is_none());
    }

    #[test]
    fn empty_document_is_an_error() {
        assert!(parse("").is_err());
    }

    #[test]
    fn xaml_language_ns_elements_keep_x_prefix() {
        const DOC: &str = r#"<Page xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
      xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml">
  <Page.Resources>
    <x:String x:Key="S">hi</x:String>
  </Page.Resources>
</Page>"#;
        let tree = parse(DOC).unwrap();
        assert_eq!(tree.properties[0].name, "Page.Resources");
        let entry = &tree.properties[0].child_elements[0];
        assert_eq!(entry.name, "x:String");
        assert_eq!(entry.attr("x:Key"), Some("S"));
    }
}
