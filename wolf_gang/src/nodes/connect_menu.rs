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
        self.confirmation = unsafe { node::get_child_by_type::<ConfirmationDialog>(menu_button, false) };
        menu_button.connect("join_or_host_online", self.confirmation.unwrap(), "connection_type_handler", VariantArray::new_shared(), 0).unwrap();
    }

    #[export]
    fn join_item_handler(&mut self, menu_button: &MenuButton, id: i64) {
        match id {
            0 => { //Local
                crate::STATE_MACHINE.with(|s| {
                    let state_machine = &mut *s.borrow_mut();

                    let network_state = state_machine.get_state("Networking").expect("Failed to get the Networking state");
                    let mut network_state = network_state.borrow_mut();

                    let world_lock = crate::WolfGang::get_world().unwrap();
                    let world = &mut world_lock.write().unwrap();
                    let resources = crate::WolfGang::get_resources().unwrap();
                    let resources = &mut resources.borrow_mut();

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

                    let network_state = state_machine.get_state("Networking").expect("Failed to get the Networking state");
                    let mut network_state = network_state.borrow_mut();

                    let world_lock = crate::WolfGang::get_world().unwrap();
                    let world = &mut world_lock.write().unwrap();
                    let resources = crate::WolfGang::get_resources().unwrap();
                    let resources = &mut resources.borrow_mut();

                    network_state.free(world, resources);

                    resources.insert(networking::Connection::new(networking::ConnectionType::Host, networking::Scope::Multicast));

                    network_state.initialize(world, resources);
                })
            },
            1 => { //Online
                menu_button.emit_signal("join_or_host_online", &[Variant::from_i64(1)]);
            },
            _ => {}
        }
    }

    #[export]
    fn item_handler(&mut self, _: &MenuButton, id: i64) {
        match id {
            3 => { // Disconnect
                unimplemented!()
            }, 
            _ => {
                unimplemented!()
            }, //0 is join submenu, 1 is host submenu, 2 is a separator
        }
    }
}


