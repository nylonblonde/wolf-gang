use gdnative::prelude::*;
use gdnative::api::{
    File,
    InputMap
};
use legion::*;
use ron::ser::{PrettyConfig};
use serde::{Deserialize, Serialize};

use std::collections::{ HashMap, HashSet };

const USER_CONFIG_PATH: &str = "user://input_map.ron";
const RESOURCE_CONFIG_PATH: &str = "res://config/input_map.ron";

#[derive(Deserialize, Serialize, PartialEq, Eq, Hash, Copy, Clone, Debug)]
pub enum InputType {
    Key,
    MouseButton,
    MouseMotion,
    JoystickMotion,
    JoystickButton,
    ScreenTouch,
    ScreenDrag,
    ScreenAction
}

#[derive(Deserialize, Serialize, Clone, Copy)]
pub struct InputData {
    deadzone: f64,
    code: i64
}

pub const MODIFIER_SUFFIX: & str = "_modifier";

#[derive(Deserialize, Serialize)]
pub struct InputConfig {
    actions: HashMap<String, HashMap<InputType, [Option<InputData>; 2]>>
}

impl InputConfig {

    fn from_file(path: &'static str) -> Option<InputConfig> {

        let file = File::new();
        match file.open(path, File::READ) {
            Ok(_) => {
                match ron::de::from_str::<InputConfig>(file.get_as_text().to_string().as_str()) {
                    Ok(result) => Some(result),
                    Err(_) => None
                }
            },
            Err(_) => None
        }

    }

    fn transcode_to_input_map(&self) {

        let input_map = InputMap::godot_singleton();

        for action in &self.actions {
            let (name, inputs) = action;

            for input in inputs {
                let (input_type, input_data) = input;

                for (i, input_data) in input_data.iter().enumerate() {

                    match &input_data {
                        Some(r) => {
                            let name = match i {
                                0 => format!("{}{}", name, MODIFIER_SUFFIX),
                                _ => name.clone()
                            };

                            if input_map.has_action(GodotString::from_str(&name)) {
                                input_map.action_erase_events(GodotString::from_str(&name));
                            } else {
                                input_map.add_action(GodotString::from_str(&name), r.deadzone);
                            }

                            match input_type {
                                InputType::Key => {
                                    input_map.action_add_event(GodotString::from_str(&name), 
                                        {
                                            let input_event_key = InputEventKey::new();
                                            input_event_key.set_scancode(r.code);
                                            input_event_key
                                        }
                                    );
                                }
                                _ => {
                                    unimplemented!();
                                }
                            }
                        },
                        None => {}
                    }

                }
                
            }
        }
    }

    pub fn save(&self, path: &str) {

        let file = File::new();
        match file.open(GodotString::from_str(path), File::WRITE) {
            Ok(_) => {},
            _err => {
                //Should probably feed an error to the user
            }
        }
        let pretty = PrettyConfig::default();
        let ron_pretty = match ron::ser::to_string_pretty(&self, pretty) {
            Ok(r) => r,
            _err => panic!("Failed to serialize to pretty ron")
        };

        file.store_string(GodotString::from(ron_pretty));
        file.close();

    }
}

/// Loads a config file if it can find it, otherwise, creates a new InputConfig and inserts the config entities from the relevant data
pub fn initialize_input_config(world: &mut legion::world::World) {

    let input_config = match InputConfig::from_file(USER_CONFIG_PATH) {
        Some(input_config) => input_config,
        None => {
            let input_config = InputConfig::from_file(RESOURCE_CONFIG_PATH).unwrap_or_else(|| panic!("Failed to load {}", RESOURCE_CONFIG_PATH));

            input_config.save(USER_CONFIG_PATH);

            input_config    
        }
    };

    input_config.transcode_to_input_map();

    let mut modifiers: Vec<(Action, TypeTag, Modifier, InputData)> = Vec::new();
    let mut non_modifiers: Vec<(Action, TypeTag, InputData)> = Vec::new();

    for action in input_config.actions {
        let (name, events) = action;
        for event in events {
            let (input_type, input_data) = event;

            for i in 0.. input_data.len() {
                let input = input_data[i];
                match input {
                    Some(input_data) if i == 0 => modifiers.push((Action(name.clone()), TypeTag(input_type), Modifier{}, input_data)),
                    Some(input_data) => non_modifiers.push((Action(name.clone()), TypeTag(input_type), input_data)),
                    None => {}
                }
            }
        }
    }

    world.extend(modifiers);
    world.extend(non_modifiers);
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Modifier{}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Action(pub String);

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TypeTag(InputType);

///Repeater incremenets by delta time each frame so that individual systems can arbitrarily control length of repeating as needed by checking against it.
/// Also a good way of checking how long a button has been pressed.
/// Strength is zero when action has just been released.
#[derive(Copy, Clone)]
pub struct InputActionComponent {
    pub strength: f64,
    pub repeater: f32
}

impl InputActionComponent {
    pub fn is_held(&self) -> bool {
        self.strength > 0.0 && self.repeater > 0.0
    }

    pub fn just_pressed(&self) -> bool {
        self.strength > 0.0 && self.repeater == 0.0
    }
    pub fn just_released(&self) -> bool {
        self.strength == 0.0
    }
    pub fn repeated(&self, delta: f32, increment: f32) -> bool {
        self.repeater % increment < delta && self.strength > 0.0
    }
}

pub fn create_input_system() -> impl systems::Runnable {

    SystemBuilder::new("input_system")
        .read_resource::<crate::Time>()
        .with_query(<(Entity, Read<InputData>, Read<Action>)>::query() //input data that is a modifier
            .filter(component::<Modifier>())
        )
        .with_query(<(Entity, Read<InputData>, Read<Action>)>::query() //input data that is not a modifier
            .filter(!component::<Modifier>())
        )
        .with_query(<(Entity, Write<InputActionComponent>, Read<Action>)>::query()) 
        .build(|commands, world, time, queries| {

            let inputs = Input::godot_singleton();

            let mut already_pressed: HashSet<String> = HashSet::new();

            let mut delete_entities: Vec<Entity> = Vec::new();

            let (modifier_config_query, not_modifier_config_query, input_component_query) = queries;

            let modifiers = modifier_config_query.iter(world)
                .map(|(entity, input, action)| (*entity, *input, (*action).clone()))
                .collect::<Vec<(Entity, InputData, Action)>>();
            let non_modifiers = not_modifier_config_query.iter(world)
                .map(|(entity, input, action)| (*entity, *input, (*action).clone()))
                .collect::<Vec<(Entity, InputData, Action)>>();

            for (_, input_data, action) in non_modifiers.iter() {

                //Check if this input config requires a modifier
                //Grab a modifier associated with this action, if there is one
                let modifier = modifiers.iter().find(|(_, _, a)| a == action);                

                if let Some((entity, mut input_component, _)) = input_component_query.iter_mut(world).find(|(_, _, a)| *a == action) {

                    let mut pressed = inputs.is_action_pressed(GodotString::from(action.0.clone()));

                    pressed = match modifier {
                        Some(_) if pressed => inputs.is_action_pressed(GodotString::from(format!("{}{}", action.0.clone(), MODIFIER_SUFFIX))),
                        _ => pressed
                    };

                    pressed = {
    
                        //check to see if another modifier conflicts
                        for (_, _, other_action) in modifiers.iter().filter(|(_,_,a)| a != action) {
    
                            for (_, other_input, _) in non_modifiers.iter().filter(|(_,_,a)| a == other_action) {
                                if other_input.code == input_data.code {
                                    pressed = {
                                        if inputs.is_action_pressed(GodotString::from(format!("{}{}", other_action.0.clone(), MODIFIER_SUFFIX))) {
                                            false
                                        } else {
                                            pressed
                                        }
                                    };
                                    break
                                } 
                            }
    
                            if !pressed {
                                break
                            }
                        }
    
                        pressed  
                    };

                    if pressed && !already_pressed.contains(&action.0) {
                        already_pressed.insert(action.0.clone());

                        input_component.strength = inputs.get_action_strength(GodotString::from(&action.0));
                        input_component.repeater += time.delta;
                    } else if input_component.strength < std::f32::EPSILON.into() { 
                        // If strength is already 0.0, then we've already passed on "on release" frame
                        delete_entities.push(*entity);
                    } else {
                        // If action is no longer pressed, set strength to zero. If a component has a strength of 0.0, we can confirm that it has been released.
                        input_component.strength = 0.0;
                        
                    }
                }

            }

            for entity in delete_entities {
                commands.remove(entity);
            }

            let mut insert_data: Vec<(Action, InputActionComponent)> = Vec::new();

            //Go through each input configuration and check to see if it is pressed
            for (_, input_data, action) in not_modifier_config_query.iter(world) {

                //check to see if this action has a modifier
                let modifier_input = modifiers.iter().find(|(_,_,a)| a == action);

                let mut pressed = inputs.is_action_pressed(GodotString::from(action.0.clone()));

                //If there is a modifier configured, check that it is pressed, otherwise just return pressed
                pressed = match modifier_input {
                    Some(_) if pressed => inputs.is_action_pressed(GodotString::from(format!("{}{}", action.0.clone(), MODIFIER_SUFFIX))),
                    _ => pressed
                };

                pressed = {
                    // check to see if another modifier conflicts
                    for (_, _, other_action) in modifiers.iter().filter(|(_,_,a)| a != action) {
                        
                        if let Some((_, _, _)) = non_modifiers.iter().find(|(_,input,a)| a == other_action && input.code == input_data.code) {
                            pressed = {
                                if inputs.is_action_pressed(GodotString::from(format!("{}{}", other_action.0.clone(), MODIFIER_SUFFIX))) {
                                    false
                                } else {
                                    pressed
                                }
                            };
                        }

                        if !pressed {
                            break
                        }
                    }
                    pressed  
                };

                if !already_pressed.contains(&action.0) && pressed {

                    insert_data.push((action.clone(), InputActionComponent{ 
                        strength: inputs.get_action_strength(&action.0), 
                        repeater: 0. 
                    }));
                }
            }

            #[cfg(debug_assertions)]
            for input in &insert_data {
                godot_print!("{:?}", input.0);
            }
            
            commands.extend(insert_data);
    })
}