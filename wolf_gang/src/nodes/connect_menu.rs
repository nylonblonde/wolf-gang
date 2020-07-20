use crate::{
    node,
    nodes::utils,
    networking,
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
        
        let popup_menu = utils::get_popup_menu(menu_button);

        let join_submenu = PopupMenu::new();
        join_submenu.set_name("JoinSubmenu");
        //Label, id, acceleration
        join_submenu.add_item("Local", 0, 0);
        join_submenu.add_item("Online", 1, 0);

        let host_submenu = PopupMenu::new();
        host_submenu.set_name("HostSubmenu");
        //Label, id, acceleration
        host_submenu.add_item("Local", 0, 0);
        host_submenu.add_item("Online", 1, 0);

        unsafe {
            host_submenu.assume_unique().connect("index_pressed", menu_button.assume_unique(), "host_item_handler", VariantArray::new_shared(), 0).unwrap();
            join_submenu.assume_unique().connect("index_pressed", menu_button.assume_unique(), "join_item_handler", VariantArray::new_shared(), 0).unwrap();

            popup_menu.assume_unique().add_child(join_submenu, true);
            popup_menu.assume_unique().add_child(host_submenu, true);
            popup_menu.assume_unique().set_item_submenu(0, "JoinSubmenu");
            popup_menu.assume_unique().set_item_submenu(1, "HostSubmenu");
        }

        ConnectMenu{
            confirmation: None,
            popup_menu
        }
    }

    #[export]
    fn _ready(&mut self, menu_button: &MenuButton) {
        self.confirmation = unsafe { node::get_child_by_type::<ConfirmationDialog>(menu_button.upcast()) };
    }

    #[export]
    fn join_item_handler(&mut self, _: &MenuButton, id: i64) {
        match id {
            0 => { //Local
                crate::STATE_MACHINE.with(|s| {
                    let state_machine = &mut *s.borrow_mut();

                    let network_state = state_machine.get_state_mut("Networking").expect("Failed to get the Networking state");

                    let mut game_lock = crate::GAME_UNIVERSE.lock().unwrap();
                    let game = &mut *game_lock;
                    let world = &mut game.world;
                    let resources = &mut game.resources;

                    network_state.free(world, resources);

                    resources.insert(networking::ClientAddr("255.255.255.255:1234".parse().unwrap()));

                    network_state.initialize(world, resources);
                })
            },
            1 => { //Online
                if let Some(confirmation) = self.confirmation {
                    unsafe { confirmation.assume_safe().popup_centered(Vector2::new(200.,100.)); }
                }
            },
            _ => {}
        }
    }

    #[export]
    fn host_item_handler(&mut self, _: &MenuButton, id: i64) {
        match id {
            0 => { //Local
                crate::STATE_MACHINE.with(|s| {
                    let state_machine = &mut *s.borrow_mut();

                    let network_state = state_machine.get_state_mut("Networking").expect("Failed to get the Networking state");

                    let mut game_lock = crate::GAME_UNIVERSE.lock().unwrap();
                    let game = &mut *game_lock;
                    let world = &mut game.world;
                    let resources = &mut game.resources;

                    network_state.free(world, resources);

                    resources.insert(networking::ClientAddr("255.255.255.255:1234".parse().unwrap()));
                    resources.insert(networking::ServerAddr("255.255.255.255:1234".parse().unwrap()));

                    network_state.initialize(world, resources);
                })
            },
            1 => { //Online

            },
            _ => {}
        }
    }

    #[export]
    fn item_handler(&mut self, _: &MenuButton, id: i64) {
        match id {
            // 0 => { // Join
            //     //replace this with an emit signal to a handler which switches between host and join
            //     if let Some(confirmation) = self.confirmation {
            //         unsafe { confirmation.assume_safe().popup_centered(Vector2::new(200.,100.)); }
            //     }
            // },
            // 1 => { // Host
            //     //replace this with an emit signal to a handler which switches between host and join
            //     if let Some(confirmation) = self.confirmation {
            //         unsafe { confirmation.assume_safe().popup_centered(Vector2::new(200.,100.)); }
            //     }
            // },
            3 => { // Disconnect

            }, 
            _ => {}, //0 is join, 1 is host, 2 is a separator
        }
    }
}


