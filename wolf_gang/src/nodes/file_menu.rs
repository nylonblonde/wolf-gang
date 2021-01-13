use gdnative::prelude::*;
use gdnative::api::{
    ConfirmationDialog,
    FileDialog,
    MenuButton,
    PopupMenu,
};

use super::utils;
use crate::{
    node,
    systems::{
        level_map,
        level_map::{
            document::Document,
        },
    },
    networking::{Connection, ConnectionType},
};

#[derive(NativeClass)]
#[inherit(MenuButton)]
#[register_with(Self::register_signals)]
#[user_data(user_data::LocalCellData<FileMenu>)]
pub struct FileMenu {
    popup_menu: Ref<PopupMenu>,
    file_dialog: Option<Ref<FileDialog>>,
    confirmation_dialog: Option<Ref<ConfirmationDialog>>    
}

// __One__ `impl` block can have the `#[methods]` attribute, which will generate
// code to automatically bind any exported methods to Godot.
#[methods]
impl FileMenu {
    
    /// The "constructor" of the class.
    fn new(menu_button: &MenuButton) -> Self {

        let popup_menu = utils::get_popup_menu(menu_button);

        FileMenu{
            popup_menu,
            file_dialog: None,
            confirmation_dialog: None
        }

    }

    fn register_signals(builder: &ClassBuilder<Self>) {
        builder.add_signal(Signal {
            name: "save_load_popup",
            args: &[SignalArgument {
                name: "type_flag",
                default: Variant::from_i64(0),
                export_info: ExportInfo::new(VariantType::I64),
                usage: PropertyUsage::DEFAULT
            }]
        });
        builder.add_signal(Signal {
            name: "confirmation_popup",
            args: &[]
        });
    }

    #[export]
    fn _ready(&mut self, menu_button: &MenuButton) {
        unsafe {

            //Get the FileDilaog for saving and loading
            let file_dialog = node::get_child_by_type::<FileDialog>(menu_button, false); 

            match file_dialog {
                Some(file_dialog) => {

                    match menu_button.connect("save_load_popup", file_dialog, "save_load_handler", VariantArray::new_shared(), 0) {
                        Ok(_) => {
                            self.file_dialog = Some(file_dialog)
                        },
                        Err(err) => panic!("{:?}", err)
                    }
                    
                },
                None => panic!("Couldn't find the FileDialog!")
            }

            //Get the confirmation dialog
            let dialog = node::get_child_by_type::<ConfirmationDialog>(menu_button, false);

            match dialog {
                Some(dialog) => {
                    match menu_button.connect("confirmation_popup", dialog, "new_confirmation_handler", VariantArray::new_shared(), 0) {
                        Ok(_) => {
                            self.confirmation_dialog = Some(dialog);
                        },
                        Err(err) => panic!("{:?}", err)
                    }
                },
                None => panic!("Couldn't find the ConfirmationDialog")
            }

        }
    }

    #[export]
    fn _pressed(&mut self, _: &MenuButton) {

        let popup_menu = unsafe { self.popup_menu.assume_safe() };
    
        popup_menu.set_item_disabled(0, true);
        popup_menu.set_item_disabled(1, true);
        popup_menu.set_item_disabled(2, true);
        popup_menu.set_item_disabled(3, true);
        
        let world_lock = crate::WolfGang::get_world().unwrap();
        let world = &mut world_lock.write().unwrap();
        let resources = crate::WolfGang::get_resources().unwrap();
        let resources = &mut resources.borrow_mut();

        let is_host = resources.get::<Connection>().map_or(false, |conn| {
            ConnectionType::Host == conn.get_type()
        });

        let can_quick_save = resources.get_mut::<Document>().map_or(false, |mut doc| {
            doc.update_data(world);
            doc.file_path != None && doc.has_unsaved_changes()
        });

        popup_menu.set_item_disabled(0, !is_host);
        popup_menu.set_item_disabled(1, !is_host);
        popup_menu.set_item_disabled(2, !is_host || !can_quick_save);
        popup_menu.set_item_disabled(3, !is_host);
    }

    #[export]
    fn item_handler(&mut self, menu_button: &MenuButton, id: i64) {

        let world_lock = crate::WolfGang::get_world().unwrap();
        let world = &mut world_lock.write().unwrap();
        let resources = crate::WolfGang::get_resources().unwrap();
        let resources = &mut resources.borrow_mut();

        match id {
            0 => { //new

                godot_print!("New");
                match resources.get_mut::<Document>() {
                    Some(mut doc) => {
                        doc.update_data(world);
                        if doc.has_unsaved_changes() {
                            menu_button.emit_signal("confirmation_popup", &[]);
                            return
                        }
                    },
                    _ => { todo!() }
                }

                level_map::send_reset_message(world);

            },
            1 => { //open

                match resources.get_mut::<Document>() {
                    Some(mut doc) => {
                        if let Some(file_dialog) = self.file_dialog {
                            doc.update_data(world);
                            if doc.has_unsaved_changes() {
                                unsafe { file_dialog.assume_safe().emit_signal("confirmation_popup", &[]); }
                                return
                            }
                        }
                    },
                    _ => { todo!() }
                }

                menu_button.emit_signal("save_load_popup", &[Variant::from_i64(0)]); 

            },
            2 => { //save

                match resources.get_mut::<Document>() {
                    Some(mut doc) => {
                        doc.update_data(world);
                        doc.save();
                    },
                    _ => { todo!() }
                }

            },
            3 => { //save-as

                menu_button.emit_signal("save_load_popup", &[Variant::from_i64(1)]); 
                
            },
            _ => {}
        }
    }

}