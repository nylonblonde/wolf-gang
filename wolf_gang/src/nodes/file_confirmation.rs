use gdnative::*;

#[derive(NativeClass)]
#[inherit(ConfirmationDialog)]
#[user_data(user_data::LocalCellData<FileConfirmation>)]
pub struct FileConfirmation {
    // confirm_dialog: ConfirmationDialog,
}

// __One__ `impl` block can have the `#[methods]` attribute, which will generate
// code to automatically bind any exported methods to Godot.
#[methods]
impl FileConfirmation {
    
    /// The "constructor" of the class.
    fn _init(_: ConfirmationDialog) -> Self {

        FileConfirmation{}
    }

    /// Confirmation for ConfirmationDialog that will popup when opening or pressing New when there are unsaved changes
    #[export]
    fn new_confirmation_handler(&mut self, mut confirmation_dialog: ConfirmationDialog) {

        unsafe { confirmation_dialog.popup_centered(Vector2::new(0., 0.)); }
    }

    #[export]
    fn open_confirmation_handler(&mut self, mut confirmation_dialog: ConfirmationDialog) {

        unsafe { confirmation_dialog.popup_centered(Vector2::new(0., 0.)); }
    }

}