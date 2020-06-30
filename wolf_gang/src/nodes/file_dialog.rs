use gdnative::*;
use crate::node;

/// The EditMenu "class"
#[derive(NativeClass)]
#[inherit(FileDialog)]
#[user_data(user_data::LocalCellData<SaveLoadDialog>)]
pub struct SaveLoadDialog {

}

// __One__ `impl` block can have the `#[methods]` attribute, which will generate
// code to automatically bind any exported methods to Godot.
#[methods]
impl SaveLoadDialog {
    
    /// The "constructor" of the class.
    fn _init(mut file_dialog: FileDialog) -> Self {

        unsafe {

            file_dialog.set_current_dir(GodotString::from("user://maps"));

            let mut vbox = file_dialog.get_vbox().unwrap();

            //Change the text label from "Files and Directories" to just "Files"
            if let Some(mut label) = node::get_child_by_type::<Label>(&vbox) {
                label.set_text(GodotString::from("Files:"));
            }
            
            //Remove the HBoxContainer which allows us to navigate to different directories
            if let Some(hbox) = node::get_child_by_type::<HBoxContainer>(&vbox) {
                vbox.remove_child(Some(hbox.to_node()));
            }

            if let Some(vbox) = node::get_child_by_type::<VBoxContainer>(&file_dialog) {

                if let Some(mut hbox) = node::get_child_by_type::<HBoxContainer>(&vbox) {

                    // Remove the File extension options
                    if let Some(option) = node::get_child_by_type::<OptionButton>(&hbox) {
                        hbox.remove_child(Some(option.to_node()));
                    }

                }
            }

        }

        SaveLoadDialog{}
    }

    #[export]
    fn _ready(&mut self, file_dialog: FileDialog) {
        
    }

    #[export]
    /// Tells the FileDialog whether to open as Open or Save dialogs
    fn save_load_handler(&mut self, mut file_dialog: FileDialog, type_flag: i64) {

        match type_flag {
            0 => { //open
                unsafe { 
                    file_dialog.set_mode(FileDialog::MODE_OPEN_FILE);
                    file_dialog.popup_centered_clamped(Vector2::new(800.0, 600.0), 0.75);
                    file_dialog.deselect_items();
                };
            },
            1 => { //save

                unsafe { 
                    file_dialog.set_mode(FileDialog::MODE_SAVE_FILE);
                    file_dialog.popup_centered_clamped(Vector2::new(800.0, 600.0), 0.75); 
                    file_dialog.deselect_items();
                };
            },
            _ => {}
        }
    } 

}