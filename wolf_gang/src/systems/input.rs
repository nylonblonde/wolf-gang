use gdnative::*;
use legion::prelude::*;
use ron::ser::{PrettyConfig};
use serde::{Deserialize, Serialize};

use std::collections::{ HashMap, HashSet };

pub const CONFIG_PATH: &'static str = "user://input_map.ron";

#[derive(Deserialize, Serialize, PartialEq, Eq, Hash)]
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

#[derive(Deserialize, Serialize)]
pub struct InputData {
    deadzone: f64,
    code: i64
}

#[derive(Deserialize, Serialize)]
pub struct InputConfig {
    actions: HashMap<String, HashMap<InputType, InputData>>
}

impl InputConfig {
    pub fn new() -> Self {

        let mut input_config: Self = InputConfig {
            actions: HashMap::new()
        };

        input_config.actions.insert(String::from("move_forward"), 
            { 
                let mut event = HashMap::new();
                event.insert(InputType::Key, InputData { deadzone: 0.0, code: 87 });
                event
            }
        );

        input_config.actions.insert(String::from("move_back"), 
            { 
                let mut event = HashMap::new();
                event.insert(InputType::Key, InputData { deadzone: 0.0, code: 83 });
                event
            }
        );

        input_config.actions.insert(String::from("move_left"), 
            { 
                let mut event = HashMap::new();
                event.insert(InputType::Key, InputData { deadzone: 0.0, code: 65 });
                event
            }
        );

        input_config.actions.insert(String::from("move_right"), 
            { 
                let mut event = HashMap::new();
                event.insert(InputType::Key, InputData { deadzone: 0.0, code: 68 });
                event
            }
        );

        input_config.actions.insert(String::from("move_down"), 
            { 
                let mut event = HashMap::new();
                event.insert(InputType::Key, InputData { deadzone: 0.0, code: 90 });
                event
            }
        );

        input_config.actions.insert(String::from("move_up"), 
            { 
                let mut event = HashMap::new();
                event.insert(InputType::Key, InputData { deadzone: 0.0, code: 88 });
                event
            }
        );


        input_config.save(CONFIG_PATH);

        input_config
    }

    fn transcode_to_input_map(&self) {

        let mut input_map = InputMap::godot_singleton();

        for action in &self.actions {
            let (name, input_events) = action;

            for input_event in input_events {
                let (input_type, input_data) = input_event;

                if input_map.has_action(GodotString::from_str(name)) {
                    input_map.action_erase_events(GodotString::from_str(name));
                } else {
                    input_map.add_action(GodotString::from_str(name), input_data.deadzone);
                }

                match input_type {
                    InputType::Key => {
                        input_map.action_add_event(GodotString::from_str(name), 
                            {
                                let mut input_event_key = InputEventKey::new();
                                input_event_key.set_scancode(input_data.code);
                                Some(input_event_key.to_input_event_with_modifiers().to_input_event())
                            }
                        );
                    }
                    _ => {}
                }
                
            }
        }
    }

    pub fn from_file(path: &str) -> Self {
        let mut file = File::new();

        if !file.file_exists(GodotString::from_str(path)) {
            return InputConfig::new();
        }

        match file.open(GodotString::from_str(path), File::READ) {
            Ok(r) => {},
            _err => {
                //maybe return an error message
            }
        };

        let string = file.get_as_text().to_string();

        match ron::de::from_str::<InputConfig>(string.as_str()) {
            Ok(r) => {
                r.transcode_to_input_map(); 
                return r;
            }
            _err => {
                //some kind of error message needed
                return InputConfig::new();
            }
        }
    }

    pub fn save(&self, path: &str) {

        self.transcode_to_input_map();

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

///Repeater incremenets each frame so that individual systems can arbitrarily control length of repeating as needed by checking against it.
/// Strength is zero when action has just been released.
pub struct InputComponent {
    pub strength: f64,
    pub repeater: f64
}

#[derive(Clone, Debug, PartialEq)]
pub struct Action(pub String);

impl InputComponent {
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

pub fn create_system() -> Box<dyn Schedulable> {
    SystemBuilder::<()>::new("input_system")
        .write_component::<InputComponent>()
        .with_query(<(Write<InputComponent>, Tagged<Action>)>::query()
            
        )
        .build(move |commands, world, resource, query| {

            let mut input_map = InputMap::godot_singleton();
            let inputs = Input::godot_singleton();

            let mut already_pressed: HashSet<String> = HashSet::new();

            for (entity, (mut input_component, tag)) in query.iter_entities(&mut *world){

                if inputs.is_action_pressed(GodotString::from(&tag.0)) {
                    already_pressed.insert(tag.0.clone());

                    input_component.strength = inputs.get_action_strength(GodotString::from(&tag.0));
                    unsafe { input_component.repeater += crate::DELTA_TIME; }
                } else {
                    if input_component.strength < std::f32::EPSILON.into() { 
                        // godot_print!("{:?} deleted", tag.0);
                        // If strength is already 0.0, then we've already passed on "on release" frame
                        commands.delete(entity);
                    } else {
                        // If action is no longer pressed, set strength to zero. If a component has a strength of 0.0, we can confirm that it has been released.
                        input_component.strength = 0.0;
                    }
                }
            }

            let mut actions = input_map.get_actions();

            for i in 0..actions.len() {
                let action = actions.get_val(i);

                //shit, this adds a new entity every single frame, need a check to make it unique somehow
                if !already_pressed.contains(&action.to_string()) && inputs.is_action_pressed(action.to_godot_string()) {
                    // godot_print!("{:?}", action.to_string());
                    commands.insert(
                        (Action(action.to_string()),),
                        vec![
                            (InputComponent{ 
                                strength: inputs.get_action_strength(action.to_godot_string()), 
                                repeater: 0. 
                            },)
                        ]
                    );
                }

            }



            //check for all actions, create an inputcomponent for each one
        })
}