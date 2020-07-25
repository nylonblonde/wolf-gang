use gdnative::prelude::*;

use gdnative::api::{
    ConfirmationDialog,
    MenuButton,
};

use super::utils;

#[derive(NativeClass)]
#[inherit(ConfirmationDialog)]
#[user_data(user_data::LocalCellData<FileConfirmation>)]
pub struct FileConfirmation {
}

// __One__ `impl` block can have the `#[methods]` attribute, which will generate
// code to automatically bind any exported methods to Godot.
#[methods]
impl FileConfirmation {
    
    /// The "constructor" of the class.
    fn new(_: &ConfirmationDialog) -> Self {

        FileConfirmation{
        }
        
    }

    

    /// Confirmation for ConfirmationDialog that will popup when pressing New when there are unsaved changes
    #[export]
    fn new_confirmation_handler(&mut self, confirmation_dialog: &ConfirmationDialog) {

        unsafe { 

            let signal = "confirmed";
            let method = "new_confirmation_ok_handler";

            utils::disconnect_signal(confirmation_dialog, confirmation_dialog, signal);

            confirmation_dialog.connect(signal, confirmation_dialog.assume_shared(), method, VariantArray::new_shared(), 0).unwrap();

            confirmation_dialog.popup_centered(Vector2::new(0., 0.)); 
        
        }
    }

    /// Confirmation for ConfirmationDialog that will popup when opening a file when there are unsaved changes
    #[export]
    fn open_confirmation_handler(&mut self, confirmation_dialog: &ConfirmationDialog) {
        unsafe { 

            let signal = "confirmed";
            let method = "open_confirmation_ok_handler";

            utils::disconnect_signal(confirmation_dialog, confirmation_dialog, signal);

            confirmation_dialog.connect(signal, confirmation_dialog.assume_shared(), method, VariantArray::new_shared(), 0).unwrap();

            confirmation_dialog.popup_centered(Vector2::new(0., 0.)); 
        }
    }

    #[export]
    fn new_confirmation_ok_handler(&mut self, _: &ConfirmationDialog) {

        let mut game = crate::GAME_UNIVERSE.lock().unwrap();
        let game = &mut *game;

        let world = &mut game.world;
        let resources = &mut game.resources;

        crate::STATE_MACHINE.with(|s| {
            let mut state_machine = s.borrow_mut();

            match state_machine.get_state_mut("MapEditor") {
                Some(editor_state) => {
                    editor_state.free(world, resources);
                    editor_state.initialize(world, resources);
                },
                None => panic!("Couldn't get the MapEditor state")
            }
        });
    }

    #[export]
    fn open_confirmation_ok_handler(&mut self, confirmation_dialog: &ConfirmationDialog) {
        unsafe {
            let menu_button: &MenuButton = &confirmation_dialog.get_parent()
                .expect("Couldn't get MenuButton which should be the parent of the ConfirmationDialog")
                .assume_safe().cast::<MenuButton>().unwrap();

            menu_button.emit_signal("save_load_popup", &[Variant::from_i64(0)]);
        }

    }

}