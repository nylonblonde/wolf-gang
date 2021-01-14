use legion::*;
use gdnative::prelude::*;

use gdnative::api::{
    AnimationTree,
    AnimationNodeStateMachinePlayback
};

use crate::{
    node::NodeRef
};

pub struct AnimationControlCreator {}

pub struct AnimationControl {
    animation_trees: Vec<Ref<AnimationTree>>
}

///passes the name of the state which will be played 
pub struct PlayAnimationState(pub String);

pub fn create_animation_control_creation_system() -> impl legion::systems::Runnable {
    SystemBuilder::new("animation_control_creation_system")
        .with_query(<(Entity, Read<NodeRef>, Read<AnimationControlCreator>)>::query())
        .build(move |command, world, _, query| {

            for (entity, node_ref, _) in query.iter(world) {
                
                let animation_trees = unsafe { get_animation_trees(&node_ref.val().assume_safe()) };

                command.add_component(*entity, AnimationControl{
                    animation_trees
                });

                command.remove_component::<AnimationControlCreator>(*entity);
            }
        })
}

pub fn create_animation_control_system() -> impl legion::systems::Runnable {
    SystemBuilder::new("animation_control_system")
        .with_query(<(Entity, Read<PlayAnimationState>, Read<AnimationControl>)>::query())
        .build(move |command, world, _, query| {
            query.for_each(world, |(entity, play_state, animation_control)| {

                animation_control.animation_trees.iter().for_each(|anim_tree| {
                    let variant = unsafe { anim_tree.assume_safe().get("parameters/playback") };

                    if let Some(anim_node_state_machine) = variant.try_to_object::<AnimationNodeStateMachinePlayback>() {

                        unsafe { 
                            anim_node_state_machine.assume_safe().start(play_state.0.clone());
                        }
                    }
                });

                command.remove_component::<PlayAnimationState>(*entity);
            });
        })
}

unsafe fn get_animation_trees(parent: &Node) -> Vec<Ref<AnimationTree>> {
    let mut results = Vec::new();

    let children = parent.get_children();

    for i in 0..children.len() {
        let child = children.get(i);

        if let Some(animation_node) = child.try_to_object::<AnimationTree>() {
            results.push(animation_node);
        }
        
        let child_node = child.try_to_object::<Node>().unwrap();

        results.extend(get_animation_trees(&child_node.assume_safe()).into_iter());
    }

    results
}