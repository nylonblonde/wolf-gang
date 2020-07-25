use gdnative::prelude::*;

use gdnative::api::{
    MenuButton,
    PopupMenu,
    Node
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

pub unsafe fn disconnect_signal(emitter: &Node, target: &Node, signal: &'static str) {

    let connections = target.get_incoming_connections();

    for i in 0..connections.len() {
        let connection = connections.get(i);

        let dict = connection.to_dictionary();

        let incoming_signal = dict.get("signal_name".to_variant()).to_godot_string();

        if incoming_signal == GodotString::from(signal) {
            let incoming_method = dict.get("method_name".to_variant()).to_godot_string();

            emitter.disconnect(incoming_signal, target.assume_shared(), incoming_method);
        }
    }
}