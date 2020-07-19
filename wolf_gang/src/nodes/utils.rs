use gdnative::prelude::*;

use gdnative::api::{
    MenuButton,
    PopupMenu
};

/// Sets up the popup menu for menu buttons with a connection to the index_pressed signal using an item_handler method, and returns the popupmenu
pub fn get_popup_menu(menu_button: &MenuButton) -> Ref<PopupMenu> {
    match menu_button.get_popup() {
        Some(popup) => {
            unsafe { 
                match popup.assume_safe().connect("index_pressed", menu_button.assume_shared().assume_safe(), "item_handler", VariantArray::new_shared(), 0) {
                    Ok(_) => { 
                        popup
                    },
                    Err(err) => {
                        panic!("{:?}", err);
                    }
                }
            }
        },
        None => {
            panic!("Menu could not retrieve its PopupMenu");
        }
    }
}