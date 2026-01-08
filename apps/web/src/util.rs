use crate::renderer::dom::node::Node;
use alloc::{rc::Rc, string::String};
use core::cell::RefCell;

pub fn convert_dom_to_string(root: &Option<Rc<RefCell<Node>>>) -> String {
    let mut result = String::from("\n");
    convert_dom_to_string_internal(root, 0, &mut result);
    result
}

fn convert_dom_to_string_internal(
    node: &Option<Rc<RefCell<Node>>>,
    depth: usize,
    result: &mut String,
) {
    match node {
        Some(n) => {
            result.push_str(&" ".repeat(depth));
            result.push_str(&format!("{:?}", n.borrow().kind()));
            result.push('\n');
            convert_dom_to_string_internal(&n.borrow().first_child(), depth + 1, result);
            convert_dom_to_string_internal(&n.borrow().next_sibling(), depth, result);
        }
        None => (),
    }
}
