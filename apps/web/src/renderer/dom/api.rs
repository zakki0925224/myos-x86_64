use crate::renderer::dom::node::*;
use alloc::{
    rc::Rc,
    string::{String, ToString},
    vec::Vec,
};
use core::cell::RefCell;

pub fn get_tagret_element_node(
    node: Option<Rc<RefCell<Node>>>,
    element_kind: ElementKind,
) -> Option<Rc<RefCell<Node>>> {
    match node {
        Some(n) => {
            if n.borrow().kind()
                == NodeKind::Element(Element::new(&element_kind.to_string(), Vec::new()))
            {
                return Some(n.clone());
            }

            let result1 = get_tagret_element_node(n.borrow().first_child(), element_kind);
            let result2 = get_tagret_element_node(n.borrow().next_sibling(), element_kind);

            if result1.is_none() && result2.is_none() {
                return None;
            }

            if result1.is_none() {
                return result2;
            }

            result1
        }
        None => None,
    }
}

pub fn get_style_content(root: Rc<RefCell<Node>>) -> String {
    let style_node = match get_tagret_element_node(Some(root), ElementKind::Style) {
        Some(node) => node,
        None => return "".to_string(),
    };

    let text_node = match style_node.borrow().first_child() {
        Some(node) => node,
        None => return "".to_string(),
    };

    let content = match &text_node.borrow().kind() {
        NodeKind::Text(s) => s.clone(),
        _ => "".to_string(),
    };

    content
}
