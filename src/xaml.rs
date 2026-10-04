//! Minimal XAML syntax layer: parse a XAML document into a plain element tree.
//!
//! Namespaces are accepted but not validated; `x:`-prefixed attributes from
//! the XAML language namespace are normalized to their `x:` spelling so the
//! renderer can key on them.

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
