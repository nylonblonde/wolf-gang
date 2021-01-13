use gdnative::prelude::*;
use gdnative::api::{
    ConfirmationDialog,
    Directory,
    FileDialog,
    file_dialog::Mode
};

use crate::{
    node,
    game_state::{StateMachine},
    systems::{
        level_map,
        level_map::document::Document,
    }
};

#[derive(NativeClass)]
#[inherit(FileDialog)]
#[register_with(Self::register_signals)]
#[user_data(user_data::LocalCellData<SaveLoadDialog>)]
pub struct SaveLoadDialog {
    confirm_dialog: Option<Ref<ConfirmationDialog>>,
}

// __One__ `impl` block can have the `#[methods]` attribute, which will generate
// code to automatically bind any exported methods to Godot.
#[methods]
impl SaveLoadDialog {
    
    /// The "constructor" of the class.
    fn new(file_dialog: &FileDialog) -> Self {

        let self_dialog = file_dialog;

        unsafe {

            //I am truly sorry for this lol
            match file_dialog.connect("popup_hide", self_dialog.assume_shared(), "hide_handler", VariantArray::new_shared(), 0)
                .map(|_| { file_dialog.connect("file_selected", self_dialog.assume_shared(), "file_selection_handler", VariantArray::new_shared(), 0) })
                .map(|_| { 
                    match file_dialog.get_line_edit() {
                        Some(line_edit) => 
                            line_edit.assume_safe().connect("text_changed", self_dialog.assume_shared(), "line_edit_changed_handler", VariantArray::new_shared(), 0),
                        None => panic!("{:?}", "Couldn't retrieve LineEdit from FileDialog")
                    }
                }) {

                Ok(_) => {

                    let maps_dir = GodotString::from("user://maps");
                    //make maps directory if it doesn't exist
                    let directory = Directory::new();
                    if !directory.dir_exists(maps_dir.clone()) {
                        match directory.make_dir(maps_dir.clone()) {
                            Ok(_) => {},
                            Err(err) => panic!("{:?}", err)
                        }
                    }

                    file_dialog.set_current_dir(maps_dir);

                    SaveLoadDialog{
                        confirm_dialog: None
                    }
                },

                Err(err) => panic!("{:?}", err)
            }
        }
    }

    fn register_signals(builder: &ClassBuilder<Self>) {
        builder.add_signal(Signal {
            name: "confirmation_popup",
            args: &[]
        });
    }

    #[export]
    fn _ready(&mut self, file_dialog: &FileDialog) {
        unsafe {
            match file_dialog.get_parent() {
                Some(parent) => {
                    match node::get_child_by_type::<ConfirmationDialog>(&parent.assume_safe(), false) {
                        Some(confirm_dialog) => {

                            match file_dialog.connect("confirmation_popup", confirm_dialog, "open_confirmation_handler", VariantArray::new_shared(), 0) {
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
    fn save_load_handler(&mut self, file_dialog: &FileDialog, type_flag: i64) {

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
                    self.line_edit_changed_handler(file_dialog, line_edit.assume_safe().as_ref().text());
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
    fn hide_handler(&mut self, _: &FileDialog) {
        crate::STATE_MACHINE.with(|s| {
            let state_machine: &mut StateMachine = &mut s.borrow_mut();
            state_machine.set_state_active("MapEditor", true);
        })
    }

    ///Checks to see whether or not the text field is blank, then disables the confirmation button if it is
    #[export]
    fn line_edit_changed_handler(&mut self, file_dialog: &FileDialog, new_text: GodotString) {

        unsafe {    
            match file_dialog.get_ok() {
                Some(ok_button) => {
                    if new_text.is_empty() {
                        ok_button.assume_safe().as_ref().set_disabled(true);
                    } else {
                        ok_button.assume_safe().as_ref().set_disabled(false);
                    }
                },
                None => panic!("Couldn't get Ok button for FileDialog")
            }
        }
    }

    #[export]
    fn file_selection_handler(&mut self, file_dialog: &FileDialog, mut path: GodotString) {

        let world_lock = crate::WolfGang::get_world().unwrap();
        let world = &mut world_lock.write().unwrap();
        let resources = crate::WolfGang::get_resources().unwrap();
        let resources = &mut resources.borrow_mut();

        unsafe {
            match file_dialog.mode() {
                Mode::OPEN_FILE => {
                    
                    if let Ok(doc) = Document::from_file(path) {
                        level_map::send_reset_message(world);
                        doc.populate_world(world, resources);

                        //Overwrite Document resource with loaded one
                        resources.insert(doc);
                    }
                },
                Mode::SAVE_FILE => {

                    match resources.get_mut::<level_map::document::Document>() {
                        Some(mut doc) => { 
                        
                            godot_print!("Saving...");

                            let suffix = ".wgm";
                            if !path.ends_with(&GodotString::from(suffix)) {
                                path = GodotString::from(path.to_string() + suffix);
                            }

                            doc.file_path = Some(path.to_string());
                            doc.update_data(world);

                            doc.save();
                        },
                        None => panic!("Couldn't retrieve document Resource") //TODO: error handling
                    };

                },
                _ => {}
            }
        }
    }

}