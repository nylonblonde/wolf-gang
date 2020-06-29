use gdnative::*;

use legion::prelude::*;

use crate::{
    GAME_UNIVERSE,
    history,
    systems::level_map,
};

/// The EditMenu "class"
#[derive(NativeClass)]
#[inherit(MenuButton)]
#[user_data(user_data::LocalCellData<EditMenu>)]
pub struct EditMenu {
    popup_menu: PopupMenu
}

// __One__ `impl` block can have the `#[methods]` attribute, which will generate
// code to automatically bind any exported methods to Godot.
#[methods]
impl EditMenu {
    
    /// The "constructor" of the class.
    fn _init(menu_button: MenuButton) -> Self {

        let popup_menu = unsafe { 
            match menu_button.get_popup() {
                Some(mut popup) => {
                    match popup.connect(GodotString::from_str("index_pressed"), Some(menu_button.to_object()), GodotString::from_str("item_handler"), VariantArray::new(), 0) {
                        Ok(r) => { 
                            popup
                        },
                        Err(err) => {
                            panic!("{:?}", err);
                        }
                    }
                },
                None => {
                    panic!("edit_menu could not retrieve its PopupMenu");
                }
            }
        };

        EditMenu{
            popup_menu
        }
    }
    
    #[export]
    fn _pressed(&mut self, _: MenuButton) {

        unsafe {
            self.popup_menu.set_item_disabled(0, true);
            self.popup_menu.set_item_disabled(1, true);
        }
        
        let mut game = GAME_UNIVERSE.lock().unwrap();

        let game = &mut *game;

        let world = &mut game.world;
        let resources = &mut game.resources;

        let current_step = match resources.get::<history::CurrentHistoricalStep>() {
            Some(r) => r,
            None => panic!("Couldn't retrieve the CurrentHistoricalStep from Resources")
        };

        let history_query = <Read<level_map::history::MapChunkHistory>>::query();

        //if the current step is greater than zero, we can undo
        if current_step.0 > 1 {
            unsafe {
                self.popup_menu.set_item_disabled(0, false);
            }
        }

        //check if there is any history that is more recent than current step, to see if we can redo
        for map_history in history_query.iter(world) {
            
            for change in map_history.steps.iter().rev() {

                if change.step_changed_at.0 >= current_step.0 {
                    unsafe {
                        self.popup_menu.set_item_disabled(1, false);
                    }
                    break;
                }
            }
        }
    }

    #[export]
    fn item_handler(&mut self, _: MenuButton, id: i64) {

        let mut game = GAME_UNIVERSE.lock().unwrap();
        let game = &mut *game;
        let resources = &mut game.resources;
        let world = &mut game.world;

        let current_step = resources.get_mut::<history::CurrentHistoricalStep>();

        match current_step {
            Some(mut current_step) => {
                match id {
                    0 => { //undo
                        level_map::history::move_to_step(&mut *world, &mut current_step, -1);
                    },
                    1 => { //redo
                        level_map::history::move_to_step(&mut *world, &mut current_step, 1);
                    },
                    _ => {}
                }
            },
            None => {
                panic!("Couldn't retrieve CurrentHistoricalStep from Resources");
            }
        }   
    }   
}
