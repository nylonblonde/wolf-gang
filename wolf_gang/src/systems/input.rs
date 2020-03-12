use gdnative::*;
use legion::prelude::*;
use ron::ser::{PrettyConfig};
use serde::{Deserialize, Serialize};

use std::collections::{ HashMap, HashSet };

pub const CONFIG_PATH: &'static str = "user://input_map.ron";

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

pub const MODIFIER_SUFFIX: &'static str = "_modifier";

#[derive(Deserialize, Serialize)]
pub struct InputConfig {
    actions: HashMap<String, HashMap<InputType, [Option<InputData>; 2]>>
}

impl InputConfig {
    pub fn new() -> Self {

        let mut input_config: Self = InputConfig {
            actions: HashMap::new()
        };

        // First key inserted is a modifier (like Shift), second key is the main input
        input_config.actions.insert(String::from("move_forward"), 
            { 
                let mut event = HashMap::new();
                event.insert(InputType::Key, [None, Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_W })]);
                event
            }
        );

        input_config.actions.insert(String::from("move_back"), 
            { 
                let mut event = HashMap::new();
                event.insert(InputType::Key, [None, Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_S })]);
                event
            }
        );

        input_config.actions.insert(String::from("move_left"), 
            { 
                let mut event = HashMap::new();
                event.insert(InputType::Key, [None, Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_A })]);
                event
            }
        );

        input_config.actions.insert(String::from("move_right"), 
            { 
                let mut event = HashMap::new();
                event.insert(InputType::Key, [None, Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_D })]);
                event
            }
        );

        input_config.actions.insert(String::from("move_down"), 
            { 
                let mut event = HashMap::new();
                event.insert(InputType::Key, [None, Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_Z })]);
                event
            }
        );

        input_config.actions.insert(String::from("move_up"), 
            { 
                let mut event = HashMap::new();
                event.insert(InputType::Key, [None, Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_X })]);
                event
            }
        );

        input_config.actions.insert(String::from("camera_rotate_left"), 
            { 
                let mut event = HashMap::new();
                event.insert(InputType::Key, [None, Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_J })]);
                event
            }
        );

        input_config.actions.insert(String::from("camera_rotate_right"), 
            { 
                let mut event = HashMap::new();
                event.insert(InputType::Key, [None, Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_L })]);
                event
            }
        );

        input_config.actions.insert(String::from("camera_rotate_up"), 
            { 
                let mut event = HashMap::new();
                event.insert(InputType::Key, [None, Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_I })]);
                event
            }
        );

        input_config.actions.insert(String::from("camera_rotate_down"), 
            { 
                let mut event = HashMap::new();
                event.insert(InputType::Key, [None, Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_K })]);
                event
            }
        );

        input_config.actions.insert(String::from("expand_selection_forward"), 
            { 
                let mut event = HashMap::new();
                event.insert(InputType::Key, [
                        Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_SHIFT}), 
                        Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_W })
                    ]
                );
                event
            }
        );

        input_config.actions.insert(String::from("expand_selection_back"), 
            { 
                let mut event = HashMap::new();
                event.insert(InputType::Key, [
                        Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_SHIFT}), 
                        Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_S })
                    ]
                );
                event
            }
        );

        input_config.actions.insert(String::from("expand_selection_left"), 
            { 
                let mut event = HashMap::new();
                event.insert(InputType::Key, [
                        Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_SHIFT}), 
                        Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_A })
                    ]
                );
                event
            }
        );

        input_config.actions.insert(String::from("expand_selection_right"), 
            { 
                let mut event = HashMap::new();
                event.insert(InputType::Key, [
                        Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_SHIFT}), 
                        Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_D })
                    ]
                );
                event
            }
        );

        input_config.actions.insert(String::from("expand_selection_up"), 
            { 
                let mut event = HashMap::new();
                event.insert(InputType::Key, [
                        Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_SHIFT}), 
                        Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_X })
                    ]
                );
                event
            }
        );

        input_config.actions.insert(String::from("expand_selection_down"), 
            { 
                let mut event = HashMap::new();
                event.insert(InputType::Key, [
                        Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_SHIFT}), 
                        Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_Z })
                    ]
                );
                event
            }
        );

        input_config.actions.insert(String::from("confirm"), 
            {
                let mut event = HashMap::new();
                event.insert(InputType::Key, [
                        None,
                        Some(InputData { deadzone: 0.0, code: GlobalConstants::KEY_R})
                    ]
                );
                event
            }
        );

        input_config.save(CONFIG_PATH);

        input_config
    }

    fn transcode_to_input_map(&self) {

        let mut input_map = InputMap::godot_singleton();

        for action in &self.actions {
            let (name, inputs) = action;

            for input in inputs {
                let (input_type, input_data) = input;

                for i in 0..input_data.len() {

                    match &input_data[i] {
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
                                            let mut input_event_key = InputEventKey::new();
                                            input_event_key.set_scancode(r.code);
                                            Some(input_event_key.to_input_event_with_modifiers().to_input_event())
                                        }
                                    );
                                }
                                _ => {}
                            }
                        },
                        None => {}
                    }

                }
                
            }
        }
    }

    pub fn save(&self, path: &str) {

        let mut file = File::new();
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
pub fn initialize_input_config(world: &mut legion::world::World, path: &str) {
    let mut file = File::new();

    let input_config = match file.file_exists(GodotString::from_str(path)) {
        true => InputConfig::new(),
        false => {
            match file.open(GodotString::from_str(path), File::READ) {
                Ok(_) => {},
                _err => {
                    //maybe return an error message
                }
            };
    
            let string = file.get_as_text().to_string();
    
            match ron::de::from_str::<InputConfig>(string.as_str()) {
                Ok(r) => {
                    // r.transcode_to_input_map(); 
                    r
                }
                _err => {
                    //some kind of error message needed
                    InputConfig::new()
                }
            }
        }
    };

    input_config.transcode_to_input_map();

    for action in input_config.actions {
        let (name, events) = action;
        for event in events {
            let (input_type, input_data) = event;

            for i in 0.. input_data.len() {
                let input = input_data[i];
                match input {
                    Some(r) if i == 0 => {
                        world.insert(
                            (Action(name.clone()), TypeTag(input_type), Modifier{}),
                            vec![
                                (
                                    r,
                                )
                            ]
                        );
                    },
                    Some(r) => { 
                        world.insert(
                            (Action(name.clone()), TypeTag(input_type)),
                            vec![
                                (
                                    r,
                                )
                            ]
                        );
                    },
                    None => {}
                }
            }
        }
    }
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
pub struct InputActionComponent {
    pub strength: f64,
    pub repeater: f32
}

impl InputActionComponent {
    pub fn just_pressed(&self) -> bool {
        self.repeater == 0.0
    }
    pub fn just_released(&self) -> bool {
        self.strength == 0.0
    }
    pub fn repeated(&self, delta: f32, increment: f32) -> bool {
        self.repeater % increment < delta && self.strength > 0.0
    }
}

pub fn create_thread_local_fn() -> Box<dyn FnMut(&mut legion::world::World, &mut Resources)> {
    Box::new(|world: &mut legion::world::World, resources: &mut Resources|{
        let time = resources.get::<crate::Time>().unwrap();

        let inputs = Input::godot_singleton();

        let mut already_pressed: HashSet<String> = HashSet::new();

        let mut delete_entities: Vec<Entity> = Vec::new();

        let input_config_query = <(Read<InputData>, Tagged<Action>)>::query()
            .filter(!tag::<Modifier>());

        unsafe {
            for (input_data, action) in input_config_query.iter_unchecked(world) {

                //Check if this input config requires a modifier
                let modifier_config_query = <Read<InputData>>::query()
                    .filter(tag::<Modifier>())
                    .filter(tag_value(action));

                let modifier = modifier_config_query.iter_unchecked(world).next();                
                
                let input_component_query = <Write<InputActionComponent>>::query()
                    .filter(tag_value(action));

                match input_component_query.iter_entities_unchecked(world).next() {
                    Some((entity, mut input_component)) => {

                        let mut pressed = inputs.is_action_pressed(GodotString::from(action.0.clone()));

                        pressed = match modifier {
                            Some(_) if pressed => inputs.is_action_pressed(GodotString::from(format!("{}{}", action.0.clone(), MODIFIER_SUFFIX))),
                            _ => pressed
                        };

                        pressed = {
                            //check to see if another modifier conflicts
                            let other_modifier_query = <(Read<InputData>, Tagged<Action>)>::query()
                            .filter(tag::<Modifier>());
        
                            for (_other_input_modifier, other_action) in other_modifier_query.iter_unchecked(world) {
                                if other_action == action {
                                    continue;
                                }

                                let other_config_query = <Read<InputData>>::query()
                                    .filter(!tag::<Modifier>())
                                    .filter(tag_value(other_action));
        
                                for other_input in other_config_query.iter_unchecked(world) {
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
        
                                if pressed == false {
                                    break
                                }
                            }
        
                            pressed  
                        };

                        if pressed && !already_pressed.contains(&action.0) {
                            already_pressed.insert(action.0.clone());

                            input_component.strength = inputs.get_action_strength(GodotString::from(&action.0));
                            input_component.repeater += time.delta;
                        } else {
                            if input_component.strength < std::f32::EPSILON.into() { 
                                // If strength is already 0.0, then we've already passed on "on release" frame
                                delete_entities.push(entity);
                            } else {
                                // If action is no longer pressed, set strength to zero. If a component has a strength of 0.0, we can confirm that it has been released.
                                input_component.strength = 0.0;
                                
                            }
                        }
                    },
                    _ => {}
                }

            }
        }
        

        for entity in delete_entities {
            world.delete(entity);
        }

        let input_config_query = <(Read<InputData>, Tagged<Action>)>::query()
            .filter(!tag::<Modifier>());

        let mut insert_data: HashMap<Action, InputActionComponent> = HashMap::new();

        //Go through each input configuration and check to see if it is pressed
        unsafe {
            for (input_data, action) in input_config_query.iter_unchecked(world) {

                //check to see if this action has a modifier
                let modifier_query = <Read<InputData>>::query()
                    .filter(tag_value(action))
                    .filter(tag::<Action>())
                    .filter(tag::<Modifier>());

                let modifier_input = modifier_query.iter_unchecked(world).next();

                let mut pressed = inputs.is_action_pressed(GodotString::from(action.0.clone()));
                // if pressed { godot_print!("{:?} {}", action.0, pressed); }

                //If there is a modifier configured, check that it is pressed, otherwise just return pressed
                pressed = match modifier_input {
                    Some(_) if pressed => inputs.is_action_pressed(GodotString::from(format!("{}{}", action.0.clone(), MODIFIER_SUFFIX))),
                    _ => pressed
                };

                pressed = {
                    //check to see if another modifier conflicts
                    let other_modifier_query = <(Read<InputData>, Tagged<Action>)>::query()
                    .filter(tag::<Modifier>());

                    for (_other_input_modifier, other_action) in other_modifier_query.iter_unchecked(world) {
                        if other_action == action {
                            continue;
                        }

                        let other_config_query = <Read<InputData>>::query()
                            .filter(!tag::<Modifier>())
                            .filter(tag_value(other_action));

                        for other_input in other_config_query.iter_unchecked(world) {
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

                        if pressed == false {
                            break
                        }
                    }

                    pressed  
                };

                if !already_pressed.contains(&action.0) && pressed {
                    // if action == &Action("move_forward".to_string()) {
                    //     godot_print!("{}", action.0);
                    // }
                    insert_data.insert(action.clone(), InputActionComponent{ 
                        strength: inputs.get_action_strength(GodotString::from(action.0.clone())), 
                        repeater: 0. 
                    });
                }
            }
        }

        for (action, input) in insert_data {
            world.insert(
                (action.clone(),),
                vec![
                    (input,)
                ]
            );

        }

    })
}