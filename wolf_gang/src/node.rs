use gdnative::*;

pub struct NodeName {
    pub name: Option<String>,
}

impl NodeName {
    pub fn new() -> Self {
        NodeName {
            name: None,
        }
    }
}

/// Add the node to the owner and set the NodeName
pub unsafe fn add_node(node: &mut Node, node_name: Option<&mut NodeName>) {
    
    let mut owner = crate::OWNER_NODE.as_mut().unwrap().lock().unwrap();

    //Disable all processing since we're not using it anyway. Maybe it makes it faster? Who knows
    node.set_physics_process(false);
    node.set_physics_process_internal(false);
    node.set_process(false);
    node.set_process_input(false);
    node.set_process_internal(false);
    node.set_process_unhandled_input(false);
    node.set_process_unhandled_key_input(false);

    owner.add_child(Some(*node), true); 

    match node_name {
        Some(r) => { 
            r.name = Some(node.get_name().to_string());
        },
        None => {}
    }

}

pub unsafe fn find_node(name: GodotString) -> Option<Node> {
    let owner = crate::OWNER_NODE.as_ref().unwrap().lock().unwrap();

    owner.find_node(name, true, false)
}