use crate::{
    nodes::utils,
    networking::{
        Connection, ConnectionType, Scope
    }
};

use gdnative::prelude::*;
use gdnative::api::{
    ConfirmationDialog,
    LineEdit,
};

use std::net::{
    SocketAddr, IpAddr
};

use gip::Provider;

#[derive(NativeClass)]
#[inherit(ConfirmationDialog)]
#[user_data(user_data::LocalCellData<ConnectDialog>)]
pub struct ConnectDialog {
    line_edit: Option<Ref<LineEdit>>,
}

#[methods]
impl ConnectDialog {
 
    fn new(_: &ConfirmationDialog) -> Self {

        ConnectDialog{
            line_edit: None,
        }
    }

    #[export]
    fn _ready(&mut self, confirmation_dialog: &ConfirmationDialog) {
        unsafe {
            self.line_edit = Some(
                confirmation_dialog.assume_unique()
                    .find_node("LineEdit", true, true).unwrap()
                    .assume_unique().cast::<LineEdit>().unwrap()
                    .into_shared()
            );
        }
    }

    #[export]
    fn on_host_confirmation(&mut self, _: &ConfirmationDialog) {
        //TODO: Proper error handling
        let addr = unsafe { self.line_edit.unwrap().assume_safe().text().to_string().parse::<SocketAddr>().unwrap() };

        crate::STATE_MACHINE.with(move |s| {
            let state_machine = &mut *s.borrow_mut();
            state_machine.set_state_active("MapEditor", true);

            let network_state = state_machine.get_state("Networking").expect("Failed to get the Networking state");
            let mut network_state = network_state.borrow_mut();
            
            let world_lock = crate::WolfGang::get_world().unwrap();
            let world = &mut world_lock.write().unwrap();
            let resources = crate::WolfGang::get_resources().unwrap();
            let resources = &mut resources.borrow_mut();

            network_state.free(world, resources);

            resources.insert(Connection::new(ConnectionType::Host, Scope::Online(addr)));

            network_state.initialize(world, resources);
            
        });
    }

    #[export]
    fn on_join_confirmation(&mut self, _: &ConfirmationDialog) {

        //TODO: Proper error handling
        let addr = unsafe { self.line_edit.unwrap().assume_safe().text().to_string().parse::<SocketAddr>().unwrap() };

        crate::STATE_MACHINE.with(move |s| {
            let state_machine = &mut *s.borrow_mut();
            state_machine.set_state_active("MapEditor", true);

            let network_state = state_machine.get_state("Networking").expect("Failed to get the Networking state");
            let mut network_state = network_state.borrow_mut();
            
            let world_lock = crate::WolfGang::get_world().unwrap();
            let world = &mut world_lock.write().unwrap();
            let resources = crate::WolfGang::get_resources().unwrap();
            let resources = &mut resources.borrow_mut();

            network_state.free(world, resources);

            resources.insert(Connection::new(ConnectionType::Join, Scope::Online(addr)));

            network_state.initialize(world, resources);
            
        });
    }

    #[export]
    fn connection_type_handler(&mut self, confirmation_dialog: &ConfirmationDialog, id: i64) {
        match id {
            0 => { //Join
                confirmation_dialog.set_title("Join a host");
                
                crate::STATE_MACHINE.with(|s| {
                    let state_machine = &mut *s.borrow_mut();
                    state_machine.set_state_active("MapEditor", false);
                });

                unsafe { 
                    utils::disconnect_signal(confirmation_dialog, confirmation_dialog, "confirmed");
                    confirmation_dialog.connect("confirmed", confirmation_dialog.assume_shared(), "on_host_confirmation", VariantArray::new_shared(), 0).unwrap();
                }
                confirmation_dialog.popup_centered(Vector2::new(200.,100.));
            },
            1 => { //Host
                confirmation_dialog.set_title("Host a session");

                if let Some(line_edit) = self.line_edit {
                    let global_ip = match gip::ProviderDefaultV4::new().get_addr() {
                        Ok(addr) => match addr.v4addr {
                            Some(addr) => addr,
                            None => std::net::Ipv4Addr::new(0,0,0,0)
                        },
                        Err(_) => std::net::Ipv4Addr::new(0,0,0,0)
                    };
                    let global_ip = SocketAddr::new(IpAddr::V4(global_ip), 3450);

                    unsafe { line_edit.assume_safe().set_text(format!("{}", global_ip)); }
                }

                crate::STATE_MACHINE.with(|s| {
                    let state_machine = &mut *s.borrow_mut();
                    state_machine.set_state_active("MapEditor", false);
                });

                unsafe { 
                    utils::disconnect_signal(confirmation_dialog, confirmation_dialog, "confirmed");
                    confirmation_dialog.connect("confirmed", confirmation_dialog.assume_shared(), "on_host_confirmation", VariantArray::new_shared(), 0).unwrap();
                }
                confirmation_dialog.popup_centered(Vector2::new(200.,100.));
            },
            _ => {}
        }
    }
}