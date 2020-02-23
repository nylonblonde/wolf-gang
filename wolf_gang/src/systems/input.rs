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
                                0 => format!("{}_modifier", name),
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
                Ok(r) => {},
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

/// Used to store data about configured input events
pub struct InputEventComponent {
    modifier: Option<InputData>,
    main: Option<InputData>
}

#[derive(Clone, Debug, PartialEq)]
pub struct Action(pub String);

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TypeTag(InputType);

///Repeater incremenets by delta time each frame so that individual systems can arbitrarily control length of repeating as needed by checking against it.
/// Also a good way of checking how long a button has been pressed.
/// Strength is zero when action has just been released.
pub struct InputActionComponent {
    pub strength: f64,
    pub repeater: f64
}

impl InputActionComponent {
    pub fn just_pressed(&self) -> bool {
        self.repeater == 0.0
    }
    pub fn just_released(&self) -> bool {
        self.strength == 0.0
    }
    pub fn repeated(&self, increment: f64) -> bool {
        unsafe {
            self.repeater % increment < crate::DELTA_TIME 
        }
    }
}

pub fn create_thread_local_fn() -> Box<dyn FnMut(&mut legion::world::World)> {
    Box::new(|world: &mut legion::world::World|{
    // SystemBuilder::<()>::new("input_system")
    //     .write_component::<InputActionComponent>()
    //     .with_query(<(Write<InputActionComponent>, Tagged<Action>)>::query()
            
    //     )
    //     .build(move |commands, world, resource, query| {

        let input_component_query = <(Write<InputActionComponent>, Tagged<Action>)>::query();

        let mut input_map = InputMap::godot_singleton();
        let inputs = Input::godot_singleton();

        let mut already_pressed: HashSet<String> = HashSet::new();

        unsafe {
            for (entity, (mut input_component, tag)) in input_component_query.iter_entities_unchecked(&mut *world){

                if inputs.is_action_pressed(GodotString::from(&tag.0)) {
                    already_pressed.insert(tag.0.clone());

                    input_component.strength = inputs.get_action_strength(GodotString::from(&tag.0));
                    unsafe { input_component.repeater += crate::DELTA_TIME; }
                } else {
                    if input_component.strength < std::f32::EPSILON.into() { 
                        // godot_print!("{:?} deleted", tag.0);
                        // If strength is already 0.0, then we've already passed on "on release" frame
                        world.delete(entity);
                    } else {
                        // If action is no longer pressed, set strength to zero. If a component has a strength of 0.0, we can confirm that it has been released.
                        input_component.strength = 0.0;
                    }
                }
            }
        }

        let mut actions = input_map.get_actions();

        //check for all actions, create an inputcomponent for each one
        for i in 0..actions.len() {
            let action = actions.get_val(i);

            if !already_pressed.contains(&action.to_string()) && inputs.is_action_pressed(action.to_godot_string()) {
                // godot_print!("{:?}", action.to_string());
                world.insert(
                    (Action(action.to_string()),),
                    vec![
                        (InputActionComponent{ 
                            strength: inputs.get_action_strength(action.to_godot_string()), 
                            repeater: 0. 
                        },)
                    ]
                );
            }

        }

    })
}