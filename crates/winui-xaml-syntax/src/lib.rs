//! XAML syntax layer: parse a XAML document into a plain element tree.
//!
//! Backend-independent and gpui-free (PLAN §3). Namespaces are accepted but
//! not validated; `x:`-prefixed attributes from the XAML language namespace
//! are normalized to their `x:` spelling so the renderer can key on them.

use std::collections::HashMap;

const XAML_LANGUAGE_NS: &str = "http://schemas.microsoft.com/winfx/2006/xaml";

#[derive(Debug, Clone, Default)]
pub struct XamlElement {
    /// Local element name, e.g. `"StackPanel"`.
    pub name: String,
    pub attributes: HashMap<String, String>,
    pub children: Vec<XamlElement>,
}

pub fn parse(src: &str) -> Result<XamlElement, String> {
    let doc = roxmltree::Document::parse(src).map_err(|e| format!("XAML parse error: {e}"))?;
    convert(doc.root_element())
}

fn convert(node: roxmltree::Node) -> Result<XamlElement, String> {
    let name = node.tag_name().name().to_string();
    if name.is_empty() {
        return Err("document has no root element".into());
    }

    let mut attributes = HashMap::new();
    for attr in node.attributes() {
        let key = if attr.namespace() == Some(XAML_LANGUAGE_NS) {
            format!("x:{}", attr.name())
        } else if attr.namespace().is_some() {
            // Prefixed attributes from foreign namespaces are outside the
            // PoC subset; dropping them under their bare local name could
            // collide with a same-named unprefixed attribute.
            eprintln!(
                "[xaml] attribute \"{}\" from a foreign namespace is not in the PoC subset; ignored",
                attr.name()
            );
            continue;
        } else {
            attr.name().to_string()
        };
        attributes.insert(key, attr.value().to_string());
    }

    let mut children = Vec::new();
    for child in node.children() {
        if child.is_element() {
            children.push(convert(child)?);
        } else if child.is_text() {
            // Inner text becomes the default Content (Button/TextBlock style).
            let text = child.text().unwrap_or("").trim().to_string();
            if !text.is_empty() {
                attributes.entry("Content".to_string()).or_insert(text);
            }
        }
    }

    Ok(XamlElement {
        name,
        attributes,
        children,
    })
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
        assert_eq!(
            title.attributes.get("x:Name").map(String::as_str),
            Some("title")
        );
        let button = &panel.children[1];
        assert_eq!(
            button.attributes.get("Click").map(String::as_str),
            Some("OnIncrement")
        );
        assert_eq!(
            button.attributes.get("Content").map(String::as_str),
            Some("Increment")
        );
    }

    #[test]
    fn empty_document_is_an_error() {
        assert!(parse("").is_err());
    }
}
