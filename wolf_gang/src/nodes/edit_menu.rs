use gdnative::prelude::*;
use gdnative::api::{
    MenuButton,
    PopupMenu,
};

use legion::*;

use super::utils;

use crate::{
    systems::{
        history::History
    }
};

/// The EditMenu "class"
#[derive(NativeClass)]
#[inherit(MenuButton)]
#[user_data(user_data::LocalCellData<EditMenu>)]
pub struct EditMenu {
    popup_menu: Ref<PopupMenu>
}

// __One__ `impl` block can have the `#[methods]` attribute, which will generate
// code to automatically bind any exported methods to Godot.
#[methods]
impl EditMenu {
    
    /// The "constructor" of the class.
    fn new(menu_button: &MenuButton) -> Self {

        let popup_menu = utils::get_popup_menu(menu_button);

        EditMenu{
            popup_menu
        }
    }
    
    #[export]
    fn _pressed(&mut self, _: &MenuButton) {

        unsafe {
            self.popup_menu.assume_safe().set_item_disabled(0, true);
            self.popup_menu.assume_safe().set_item_disabled(1, true);
        }
        
        let resources = crate::WolfGang::get_resources().unwrap();
        let resources = &mut resources.borrow_mut();

        let history = resources.get::<History>().expect("Couldn't retrieve the History resource");

        let popup_menu = unsafe { self.popup_menu.assume_safe() };

        popup_menu.set_item_disabled(0,!history.can_undo());

        popup_menu.set_item_disabled(1,!history.can_redo());

    }

    #[export]
    fn item_handler(&mut self, _: &MenuButton, id: i64) {

        let world_lock = crate::WolfGang::get_world().unwrap();
        let world = &mut world_lock.write().unwrap();
        let resources = crate::WolfGang::get_resources().unwrap();
        let resources = &mut resources.borrow_mut();

        let mut history = resources.get_mut::<History>().expect("Couldn't retrieve History resource!");

        let mut command_buffer = systems::CommandBuffer::new(world);

        match id {
            0 => { //undo
                history.move_by_step(&mut command_buffer, -1);
            },
            1 => { //redo
                history.move_by_step(&mut command_buffer, 1);
            },
            _ => {}
        }

        command_buffer.flush(world);
            
    }   
}
