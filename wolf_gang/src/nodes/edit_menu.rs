use gdnative::prelude::*;
use gdnative::api::{
    MenuButton,
    PopupMenu,
};

use legion::*;

use super::utils;

use crate::{
    systems::{
        history::History,
        networking::ClientID,
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
        
        let world_lock = crate::WolfGang::get_world().unwrap();
        let world = &mut world_lock.write().unwrap();
        let resources = crate::WolfGang::get_resources().unwrap();
        let resources = &mut resources.borrow_mut();

        if let Some(client_id) = resources.get::<ClientID>().map(|client_id| client_id.val()) {
            let mut query = <(Read<History>, Read<ClientID>)>::query();

            if let Some((history, _)) = query.iter(&**world).find(|(_, id)| id.val() == client_id) {
                let popup_menu = unsafe { self.popup_menu.assume_safe() };
                popup_menu.set_item_disabled(0,history.can_undo().is_err());

                popup_menu.set_item_disabled(1,history.can_redo().is_err());
            }
        }
    }

    #[export]
    fn item_handler(&mut self, _: &MenuButton, id: i64) {

        let world_lock = crate::WolfGang::get_world().unwrap();
        let world = &mut world_lock.write().unwrap();
        let resources = crate::WolfGang::get_resources().unwrap();
        let resources = &mut resources.borrow_mut();

        if let Some(client_id) = resources.get::<ClientID>().map(|client_id| client_id.val()) {
            let mut query = <(Write<History>, Read<ClientID>)>::query();

            let mut commands = legion::systems::CommandBuffer::new(world);

            if let Some((history, _)) = query.iter_mut(&mut **world).find(|(_, id)| id.val() == client_id) {
                match id {
                    0 => { //undo
                        history.move_by_step(&mut commands, resources, -1);
                    },
                    1 => { //redo
                        history.move_by_step(&mut commands, resources, 1);
                    },
                    _ => {}
                }
            }

            commands.flush(world, resources);
        }
            
    }   
}
