use gdnative::prelude::*;
use legion::*;

// #[derive(Clone, Debug, PartialEq)]
// pub struct NodeName(pub String);

#[derive(Copy, Clone, Debug)]
pub struct NodeRef(Ref<Node>);

impl NodeRef {

    pub fn new(node: Ref<Node>) -> Self {
        Self(node)
    }

    pub fn val(&self) -> Ref<Node> {
        self.0
    }
}

#[derive(Copy, Clone, Debug)]
pub struct NodeParent(Ref<Node>);

impl NodeParent {

    pub fn new(node: Ref<Node>) -> Self {
        Self(node)
    }

    pub fn val(&self) -> Ref<Node> {
        self.0
    }
}

/// Add the node to the parent
/// 
/// References being passed into this function are assumed to be unique, which is okay, as they have usually just been created.
pub unsafe fn add_node(parent: &Node, node: Ref<Node, Unique>) -> Ref<Node> {

    //Disable all processing since we're not using it anyway. Maybe it makes it faster? Who knows
    node.set_physics_process(false);
    node.set_physics_process_internal(false);
    node.set_process(false);
    node.set_process_input(false);
    node.set_process_internal(false);
    node.set_process_unhandled_input(false);
    node.set_process_unhandled_key_input(false);

    let shared_node = node.into_shared();

    parent.add_child(shared_node, true); 

    shared_node
}

/// Removes the Godot Node, and removes the associated legion Entity
pub fn free(world: &mut legion::World, node: Ref<Node>) {

    let mut query = <(Entity, Read<NodeRef>)>::query();

    let results = query.iter(world)
        .filter(|(_, node_ref)| node_ref.val() == node)
        .map(|(entity, node_ref)| (*entity, node_ref.val()))
        .collect::<Vec<(Entity, Ref<Node>)>>();

    for (entity, node) in results {

        let unique_node = unsafe { node.assume_unique() };
        unique_node.queue_free();
        world.remove(entity);
    }
} 

/// Retrieves the node from cache if possible, otherwise uses the gdnative bindings to find it.
pub unsafe fn get_node(node: &Node, name: &str, child_lookup: bool) -> Option<Ref<Node, Shared>> {

    let children = node.get_children();

    for i in 0..children.len() {
        let child = children.get(i).try_to_object::<Node>().unwrap();

        if child.assume_safe().name() == GodotString::from(name.to_string()){
            return Some(child)
        } else if child_lookup {
            if let Some(val) = get_node(&child.assume_safe(), name, true) {
                return Some(val);
            }
        }
    }

    None
}

/// Look for children by type T, recursive true if you want to cycle through all children, false if you want to look just within the single child group.
pub unsafe fn get_child_by_type<T: GodotObject>(node: &Node, recursive: bool) -> Option<Ref<T>> {

    let children = node.get_children();

    let len = children.len();

    let mut ret_val: Option<Ref<T>> = None;

    for i in 0..len {
        
        let child = children.get(i);

        match child.try_to_object::<T>() {
            Some(child) => {
                return Some(child)
            },
            _ if recursive => {
                ret_val = get_child_by_type(&child.try_to_object::<Node>().unwrap().assume_safe(), recursive)
            },
            _ => {}
        }

        if ret_val.is_some() {
            return ret_val
        }
        
    }

    None
}

pub fn init_scene(parent: &Node, path: &str) -> Ref<Node> {
    let scene = ResourceLoader::godot_singleton().load(path, "PackedScene", false).unwrap().cast::<PackedScene>().unwrap();
    let scene_instance = unsafe { scene.assume_safe().instance(0).unwrap() };

    unsafe { add_node(&parent.assume_unique(), scene_instance.assume_unique()) };

    scene_instance

}