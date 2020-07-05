use gdnative::*;

pub fn get_popup_menu(menu_button: MenuButton) -> PopupMenu {
    unsafe { 
        match menu_button.get_popup() {
            Some(mut popup) => {
                match popup.connect(GodotString::from_str("index_pressed"), Some(menu_button.to_object()), GodotString::from_str("item_handler"), VariantArray::new(), 0) {
                    Ok(_) => { 
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
    }
}