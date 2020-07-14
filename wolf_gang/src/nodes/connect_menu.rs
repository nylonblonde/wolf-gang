use crate::{
    node,
    nodes::utils,
};

use gdnative::prelude::*;

use gdnative::api::{
    ConfirmationDialog,
    MenuButton,
    PopupMenu
};

#[derive(NativeClass)]
#[inherit(MenuButton)]
#[user_data(user_data::LocalCellData<ConnectMenu>)]
pub struct ConnectMenu {
    confirmation: Option<Ref<ConfirmationDialog>>,
    popup_menu: Ref<PopupMenu>,
}

#[methods]
impl ConnectMenu {
    
    /// The "constructor" of the class.
    fn new(menu_button: &MenuButton) -> Self {
        
        ConnectMenu{
            confirmation: None,
            popup_menu: utils::get_popup_menu(menu_button)
        }
        
    }

    #[export]
    fn _ready(&mut self, menu_button: &MenuButton) {
        self.confirmation = unsafe { node::get_child_by_type::<ConfirmationDialog>(menu_button.upcast()) };
    }

    #[export]
    fn item_handler(&mut self, _: &MenuButton, id: i64) {
        match id {
            0 => { // Join

                //replace this with an emit signal to a handler which switches between host and join
                if let Some(confirmation) = self.confirmation {
                    unsafe { confirmation.assume_safe().popup_centered(Vector2::new(200.,100.)); }
                }

            },
            1 => { // Host

                //replace this with an emit signal to a handler which switches between host and join
                if let Some(confirmation) = self.confirmation {
                    unsafe { confirmation.assume_safe().popup_centered(Vector2::new(200.,100.)); }
                }
            },
            _ => {}
        }
    }
}


