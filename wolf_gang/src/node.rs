use gdnative::*;
use std::collections::HashMap;

#[derive(Clone, PartialEq)]
pub struct NodeName(pub String);

pub struct NodeCache {
    pub cache: HashMap<String, Node>
}

static mut NODE_CACHE: Option<NodeCache> = None;

/// Add the node to the owner and set the NodeName. Returns an option so that we can
/// avoid putting whole blocks of code in unsafe by just mutably assigning. Creates a NodeCache if one hasn't
/// been created and adds the node to it.
pub unsafe fn add_node(node: &mut Node) -> Option<NodeName> {

    let owner = crate::OWNER_NODE.as_mut().unwrap();

    //Disable all processing since we're not using it anyway. Maybe it makes it faster? Who knows
    node.set_physics_process(false);
    node.set_physics_process_internal(false);
    node.set_process(false);
    node.set_process_input(false);
    node.set_process_internal(false);
    node.set_process_unhandled_input(false);
    node.set_process_unhandled_key_input(false);

    owner.add_child(Some(*node), true); 

    let string = node.get_name().to_string();

    // godot_print!("{}", string.clone());

    if NODE_CACHE.is_none() {
        NODE_CACHE = Some(NodeCache {
            cache: HashMap::new()
        })
    }

    let node_cache = NODE_CACHE.as_mut().unwrap();
    node_cache.cache.insert(string.clone(), *node);

    Some(NodeName(string))
}

/// Retrieves the node from cache if possible, otherwise uses the gdnative bindings to find it.
pub unsafe fn find_node(name: String) -> Option<Node> {

    if NODE_CACHE.is_some() {
        let node_cache = NODE_CACHE.as_ref().unwrap();

        match node_cache.cache.get_key_value(&name) {
            Some(r) => {
                return Some(*r.1)
            },
            None => {}
        }
    }

    let owner = crate::OWNER_NODE.as_ref().unwrap();

    owner.find_node(GodotString::from(name), true, false)
}