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
    game_state::GameStateTraits,
    systems::level_map::{
        document::Document
    }
};

use std::borrow::BorrowMut;

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
            let file_dialog = node::get_child_by_type::<FileDialog>(menu_button); 

            match file_dialog {
                Some(file_dialog) => {

                    match menu_button.connect(GodotString::from("save_load_popup"), file_dialog, GodotString::from("save_load_handler"), VariantArray::new_shared(), 0) {
                        Ok(_) => {
                            self.file_dialog = Some(file_dialog)
                        },
                        Err(err) => panic!("{:?}", err)
                    }
                    
                },
                None => panic!("Couldn't find the FileDialog!")
            }

            //Get the confirmation dialog
            let dialog = node::get_child_by_type::<ConfirmationDialog>(menu_button);

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

    }

    #[export]
    fn item_handler(&mut self, menu_button: &MenuButton, id: i64) {

        let mut game = crate::GAME_UNIVERSE.lock().unwrap();
        let game = &mut *game;
        let world = &mut game.world;
        let resources = &mut game.resources;

        let mut doc = match resources.get_mut::<Document>() {
            Some(document) => document.clone(),
            None => panic!("Couldn't retrieve document Resource")
        };

        match id {
            0 => { //new

                godot_print!("New");
                
                match &doc.file_path {
                    Some(file_path) => {
                        let saved = Document::raw_from_file(file_path);

                        doc.update_data(world);
                        let current = doc.to_raw();

                        if saved != current {
                            menu_button.emit_signal(GodotString::from("confirmation_popup"), &[]);
                            return
                        }
                    },
                    None => {

                        doc.update_data(world);

                        if doc != Document::default() {
                            //Emit signal to confirm if you want new document despite unsaved changes
                            menu_button.emit_signal(GodotString::from("confirmation_popup"), &[]);
                        }

                        //get outta here, we're done
                        return
                    }
                    
                }

                crate::STATE_MACHINE.with(|s| {

                    for state in &mut s.borrow_mut().states {

                        let state: &mut (dyn GameStateTraits) = state.borrow_mut();

                        //clear the world of related entities and free related nodes before re-initializing
                        state.free_func()(world, resources);
                        state.initialize_func()(world, resources);
                    }
                    
                });

            },
            1 => { //open

                let file_dialog = self.file_dialog.unwrap();

                //If working from a saved document, check to see if the current document is up to date with the saved one
                match &doc.file_path {
                    Some(file_path) => {
                        let saved = Document::raw_from_file(file_path);

                        doc.update_data(world);
                        let current = doc.to_raw();

                        if saved != current {
                            unsafe { file_dialog.assume_safe().emit_signal("confirmation_popup", &[]); }
                            return
                        }
                    },
                    None => {
                        doc.update_data(world);

                        if doc != Document::default() {
                            unsafe { file_dialog.assume_safe().emit_signal("confirmation_popup", &[]); }
                            return
                        }
                    }
                }

                menu_button.emit_signal("save_load_popup", &[Variant::from_i64(0)]); 

            },
            2 => { //save

            },
            3 => { //save-as

                menu_button.emit_signal("save_load_popup", &[Variant::from_i64(1)]); 
                
            },
            _ => {}
        }
    }

}