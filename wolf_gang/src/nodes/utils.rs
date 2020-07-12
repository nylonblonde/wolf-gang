use gdnative::prelude::*;

use gdnative::api::{
    MenuButton,
    PopupMenu
};

pub fn get_popup_menu(menu_button: &MenuButton) -> Ref<PopupMenu> {
    match menu_button.get_popup() {
        Some(mut popup) => {
            unsafe { 
                match popup.assume_safe().connect(GodotString::from_str("index_pressed"), menu_button.assume_shared().assume_safe(), GodotString::from_str("item_handler"), VariantArray::new_shared(), 0) {
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
            panic!("edit_menu could not retrieve its PopupMenu");
        }
    }
}