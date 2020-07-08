use gdnative::*;

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
    fn _init(_: ConfirmationDialog) -> Self {

        FileConfirmation{
        }
        
    }

    unsafe fn disconnect_signal(mut emitter: ConfirmationDialog, target: ConfirmationDialog, signal: GodotString) {

        let mut connections = target.get_incoming_connections();

        for i in 0..connections.len() {
            let connection = connections.get_val(i);

            let dict = connection.to_dictionary();

            let incoming_signal = dict.get(&GodotString::from("signal_name").to_variant()).to_godot_string();

            if incoming_signal == signal {
                let incoming_method = dict.get(&GodotString::from("method_name").to_variant()).to_godot_string();

                emitter.disconnect(incoming_signal, Some(target.to_object()), incoming_method);
            }
        }
    }

    /// Confirmation for ConfirmationDialog that will popup when pressing New when there are unsaved changes
    #[export]
    fn new_confirmation_handler(&mut self, mut confirmation_dialog: ConfirmationDialog) {

        unsafe { 

            let signal = GodotString::from("confirmed");
            let method = GodotString::from("new_confirmation_ok_handler");

            let mut emitter = confirmation_dialog;

            Self::disconnect_signal(emitter, confirmation_dialog, signal.clone());

            emitter.connect(signal, Some(confirmation_dialog.to_object()), method, VariantArray::new(), 0).unwrap();

            confirmation_dialog.popup_centered(Vector2::new(0., 0.)); 
        
        }
    }

    /// Confirmation for ConfirmationDialog that will popup when opening a file when there are unsaved changes
    #[export]
    fn open_confirmation_handler(&mut self, mut confirmation_dialog: ConfirmationDialog) {
        unsafe { 

            let signal = GodotString::from("confirmed");
            let method = GodotString::from("open_confirmation_ok_handler");

            let mut emitter = confirmation_dialog;

            Self::disconnect_signal(emitter, confirmation_dialog, signal.clone());

            emitter.connect(signal, Some(confirmation_dialog.to_object()), method, VariantArray::new(), 0).unwrap();

            confirmation_dialog.popup_centered(Vector2::new(0., 0.)); 
        }
    }

    #[export]
    fn new_confirmation_ok_handler(&mut self, _: ConfirmationDialog) {

        let mut game = crate::GAME_UNIVERSE.lock().unwrap();
        let game = &mut *game;

        let world = &mut game.world;
        let resources = &mut game.resources;

        crate::STATE_MACHINE.with(|s| {
            let mut state_machine = s.borrow_mut();

            match state_machine.get_state_mut("MapEditor") {
                Some(editor_state) => {
                    editor_state.free_func()(world, resources);
                    editor_state.initialize_func()(world, resources);
                },
                None => panic!("Couldn't get the MapEditor state")
            }
        });
    }

    #[export]
    fn open_confirmation_ok_handler(&mut self, confirmation_dialog: ConfirmationDialog) {
        unsafe {
            let mut menu_button: MenuButton = confirmation_dialog.get_parent()
                .expect("Couldn't get MenuButton which should be the parent of the ConfirmationDialog")
                .cast::<MenuButton>().unwrap();

            menu_button.emit_signal(GodotString::from("save_load_popup"), &[Variant::from_i64(0)]);
        }

    }

}