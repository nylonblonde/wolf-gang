use gdnative::prelude::*;
use legion::*;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct NodeName(pub String);

pub struct NodeCache {
    pub cache: HashMap<String, Ref<Node, Shared>>
}

static mut NODE_CACHE: Option<NodeCache> = None;

/// Add the node to the owner and set the NodeName. Returns an option so that we can
/// avoid putting whole blocks of code in unsafe by just mutably assigning. Creates a NodeCache if one hasn't
/// been created and adds the node to it.
/// 
/// References being passed into this function are assumed to be unique, which is okay, as they have usually just been created.
pub unsafe fn add_node(parent: &Node, node: Ref<Node, Unique>) -> Option<NodeName> {

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
    //We can generally assume this is a unique reference as it is has just been created and is now being added.
    let mut string = shared_node.assume_unique().name().to_string();

    // godot_print!("{}", string.clone());

    if NODE_CACHE.is_none() {
        create_node_cache();
    }

    let node_cache = NODE_CACHE.as_mut().unwrap();
    let mut i = 1;
    //ensure the name is unique in the cache
    while node_cache.cache.contains_key(&string){
        string = format!("{}{}", string, i);
        i += 1;
    }
    shared_node.assume_unique().set_name(string.clone());
    node_cache.cache.insert(string.clone(), shared_node);

    Some(NodeName(string))
}

/// Removes the Godot Node, and removes the associated legion Entity
pub fn free(world: &mut legion::World, name: &str) {

    let node_name = NodeName(name.to_string());
    let mut query = <(Entity, Read<NodeName>)>::query();

    let results = query.iter(world)
        .filter(|(_, name)| node_name == **name)
        .map(|(entity, node_name)| (*entity, (*node_name).clone()))
        .collect::<Vec<(Entity, NodeName)>>();

    unsafe { remove_node(&name); }

    for (entity, _) in results {

        world.remove(entity);
    }
} 

/// Removes the node from the scene as well as from the node cache
pub unsafe fn remove_node(name: &str) {
    
    if let Some(node_cache) = NODE_CACHE.as_mut() {

        if let Some(node) = node_cache.cache.get(name) {

            let unique_node = node.assume_unique();
            unique_node.queue_free();

            node_cache.cache.remove(name);
        }
    }
}

unsafe fn create_node_cache() {
    NODE_CACHE = Some(NodeCache {
        cache: HashMap::new()
    })
}

/// Retrieves the node from cache if possible, otherwise uses the gdnative bindings to find it.
pub unsafe fn get_node(node: &Node, name: &str, child_lookup: bool) -> Option<Ref<Node, Shared>> {

    if NODE_CACHE.is_none() {
        create_node_cache();
    }

    let node_cache = NODE_CACHE.as_mut().unwrap();

    match node_cache.cache.get(name) {
        Some(r) => Some(*r),
        None => {
            if child_lookup {
                let children = node.get_children();

                for i in 0..children.len() {
                    let child = children.get(i).try_to_object::<Node>().unwrap();

                    if child.assume_safe().name() == GodotString::from(name.to_string()){
                        node_cache.cache.insert(name.to_string(), child);
                        return Some(child)
                    } else if let Some(val) = get_node(&child.assume_safe(), name, true) {
                        node_cache.cache.insert(name.to_string(), val);
                        return Some(val);
                    }
                }
            } 

            None
        }
    }
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

    unsafe { add_node(&parent.assume_unique(), scene_instance.assume_unique()).unwrap() };

    scene_instance

}