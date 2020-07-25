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

use std::net::{
    SocketAddr, Ipv4Addr, IpAddr
};

use gip::Provider;

#[derive(NativeClass)]
#[inherit(MenuButton)]
#[register_with(Self::register_signals)]
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

    fn register_signals(builder: &ClassBuilder<Self>) {
        builder.add_signal(Signal {
            name: "join_or_host_online",
            args: &[SignalArgument {
                name: "type_flag",
                default: Variant::from_i64(0),
                export_info: ExportInfo::new(VariantType::I64),
                usage: PropertyUsage::DEFAULT
            }]
        });
    }

    #[export]
    fn _ready(&mut self, menu_button: &MenuButton) {
        self.confirmation = unsafe { node::get_child_by_type::<ConfirmationDialog>(menu_button.upcast()) };
        menu_button.connect("join_or_host_online", self.confirmation.unwrap(), "connection_type_handler", VariantArray::new_shared(), 0).unwrap();
    }

    #[export]
    fn join_item_handler(&mut self, menu_button: &MenuButton, id: i64) {
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

                    resources.insert(networking::Connection::new(networking::ConnectionType::Join, networking::Scope::Multicast));

                    network_state.initialize(world, resources);
                })
            },
            1 => { //Online
                menu_button.emit_signal("join_or_host_online", &[Variant::from_i64(0)]);

            },
            _ => {}
        }
    }

    #[export]
    fn host_item_handler(&mut self, menu_button: &MenuButton, id: i64) {
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

                    resources.insert(networking::Connection::new(networking::ConnectionType::Host, networking::Scope::Multicast));

                    network_state.initialize(world, resources);
                })
            },
            1 => { //Online
                menu_button.emit_signal("join_or_host_online", &[Variant::from_i64(1)]);


                // let global_ip = match gip::ProviderDefaultV4::new().get_addr() {
                //     Ok(addr) => match addr.v4addr {
                //         Some(addr) => addr,
                //         None => std::net::Ipv4Addr::new(0,0,0,0)
                //     },
                //     Err(err) => std::net::Ipv4Addr::new(0,0,0,0)
                // };
                // let global_ip = SocketAddr::new(IpAddr::V4(global_ip), 3450);
                // godot_print!("{}", global_ip);

                // if let Some(confirmation) = self.confirmation {
                //     unsafe { 
                //         let confirmation = confirmation.assume_safe();
                //         confirmation.popup_centered(Vector2::new(200.,100.)); 
                //     }
                // }
            },
            _ => {}
        }
    }

    #[export]
    fn item_handler(&mut self, _: &MenuButton, id: i64) {
        match id {
            3 => { // Disconnect

            }, 
            _ => {}, //0 is join submenu, 1 is host submenu, 2 is a separator
        }
    }
}


