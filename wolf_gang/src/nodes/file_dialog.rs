use crate::game_state::StateMachine;
use gdnative::*;
use crate::node;

use crate::systems::level_map;

/// The EditMenu "class"
#[derive(NativeClass)]
#[inherit(FileDialog)]
#[register_with(Self::register_signals)]
#[user_data(user_data::LocalCellData<SaveLoadDialog>)]
pub struct SaveLoadDialog {
    confirm_dialog: Option<ConfirmationDialog>,
}

// __One__ `impl` block can have the `#[methods]` attribute, which will generate
// code to automatically bind any exported methods to Godot.
#[methods]
impl SaveLoadDialog {
    
    /// The "constructor" of the class.
    fn _init(mut file_dialog: FileDialog) -> Self {

        let self_dialog = file_dialog;

        unsafe {

            //I am truly sorry for this lol
            match file_dialog.connect(GodotString::from("popup_hide"), Some(self_dialog.to_object()), GodotString::from("hide_handler"), VariantArray::new(), 0)
                .map(|_| { file_dialog.connect(GodotString::from("file_selected"), Some(self_dialog.to_object()), GodotString::from("file_selection_handler"), VariantArray::new(), 0) })
                .map(|_| { 
                    match file_dialog.get_line_edit() {
                        Some(mut line_edit) => 
                            line_edit.connect(GodotString::from("text_changed"), Some(self_dialog.to_object()), GodotString::from("line_edit_changed_handler"), VariantArray::new(), 0),
                        None => panic!("{:?}", "Couldn't retrieve LineEdit from FileDialog")
                    }
                }) {

                Ok(_) => {

                    let maps_dir = GodotString::from("user://maps");
                    //make maps directory if it doesn't exist
                    let mut directory = Directory::new();
                    if !directory.dir_exists(maps_dir.clone()) {
                        match directory.make_dir(maps_dir.clone()) {
                            Ok(_) => {},
                            Err(err) => panic!("{:?}", err)
                        }
                    }

                    file_dialog.set_current_dir(maps_dir);

                    // let confirm_dialog = match node::get_child_by_type::<ConfirmationDialog>(&file_dialog) { 
                    //     Some(r) => r,
                    //     None => panic!("Couldn't get the ConfirmationDialog child of FileDialog")   
                    // };

                    return SaveLoadDialog{
                        confirm_dialog: None
                    }
                },

                Err(err) => panic!("{:?}", err)
            }
        }
    }

    fn register_signals(builder: &init::ClassBuilder<Self>) {
        builder.add_signal(init::Signal {
            name: "confirmation_popup",
            args: &[]
        });
    }

    #[export]
    fn _ready(&mut self, mut file_dialog: FileDialog) {
        unsafe {
            match file_dialog.get_parent() {
                Some(parent) => {
                    match node::get_child_by_type::<ConfirmationDialog>(&parent) {
                        Some(confirm_dialog) => {

                            match file_dialog.connect(GodotString::from("confirmation_popup"), Some(confirm_dialog.to_object()), GodotString::from("open_confirmation_handler"), VariantArray::new(), 0) {
                                Ok(_) => {
                                    self.confirm_dialog = Some(confirm_dialog)
                                },
                                Err(err) => panic!("{:?}", err)
                            }
                        },
                        None => panic!("Couldn't get ConfirmationDialog")

                    }
                },
                None => panic!("Couldn't get FileDialog's parent")
            }
        }
    }

    #[export]
    /// Tells the FileDialog whether to open as Open or Save dialogs
    fn save_load_handler(&mut self, mut file_dialog: FileDialog, type_flag: i64) {

        unsafe { 

            match type_flag {
                0 => { //open
                    file_dialog.set_mode(FileDialog::MODE_OPEN_FILE);
                },
                1 => { //save
                    file_dialog.set_mode(FileDialog::MODE_SAVE_FILE);   
                },
                _ => {}
            }

            file_dialog.popup_centered_clamped(Vector2::new(800.0, 600.0), 0.75); 
            file_dialog.invalidate();
            
            //Update the Ok button in case Line Edit is blank
            match file_dialog.get_line_edit() {
                Some(line_edit) => {
                    self.line_edit_changed_handler(file_dialog, line_edit.get_text());
                },
                None => panic!("Couldn't get LineEdit from FileDialog")
            }

            crate::STATE_MACHINE.with(|s| {
                let state_machine: &mut StateMachine = &mut s.borrow_mut();
                state_machine.set_state_active("MapEditor", false);
            })
        }
    } 

    #[export]
    fn hide_handler(&mut self, _: FileDialog) {
        crate::STATE_MACHINE.with(|s| {
            let state_machine: &mut StateMachine = &mut s.borrow_mut();
            state_machine.set_state_active("MapEditor", true);
        })
    }

    ///Checks to see whether or not the text field is blank, then disables the confirmation button if it is
    #[export]
    fn line_edit_changed_handler(&mut self, mut file_dialog: FileDialog, new_text: GodotString) {

        unsafe {    
            match file_dialog.get_ok() {
                Some(mut ok_button) => {
                    if new_text.is_empty() {
                        ok_button.set_disabled(true);
                    } else {
                        ok_button.set_disabled(false);
                    }
                },
                None => panic!("Couldn't get Ok button for FileDialog")
            }
        }
    }

    #[export]
    fn file_selection_handler(&mut self, mut file_dialog: FileDialog, mut path: GodotString) {

        let mut game = crate::GAME_UNIVERSE.lock().unwrap();
        let game = &mut *game;
        let world = &mut game.world;
        let resources = &mut game.resources;

        unsafe {
            match file_dialog.get_mode() {
                FileDialogMode::ModeOpenFile => {

                    godot_print!("Opening...");

                    file_dialog.emit_signal(GodotString::from("confirmation_popup"), &[]);

                },
                FileDialogMode::ModeSaveFile => {

                    godot_print!("Saving...");

                    let mut document = match resources.get_mut::<level_map::document::Document>() {
                        Some(document) => document.clone(),
                        None => level_map::document::Document::default()
                    };

                    godot_print!("{:?}", document);

                    let suffix = ".wgm";
                    if !path.ends_with(&GodotString::from(suffix)) {
                        path = GodotString::from(path.to_string() + suffix);
                    }

                    document.file_path = Some(path.to_string());
                    document.update_data(world);

                    document.save();

                },
                _ => {}
            }
        }
    }

}