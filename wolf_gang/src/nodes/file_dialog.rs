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

        let self_dialog = file_dialog;

        unsafe {
            match file_dialog.connect(GodotString::from("popup_hide"), Some(self_dialog.to_object()), GodotString::from("hide_handler"), VariantArray::new(), 0) {

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

                    return SaveLoadDialog{}
                },

                Err(err) => panic!("{:?}", err)
            }
        }
    }

    #[export]
    fn _ready(&mut self, file_dialog: FileDialog) {
        
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
            file_dialog.deselect_items();

            crate::STATE_MACHINE.with(|s| {
                s.borrow_mut().set_state_active("MapEditor", false);
            })
        }
    } 

    #[export]
    fn hide_handler(&mut self, _: FileDialog) {
        crate::STATE_MACHINE.with(|s| {
            s.borrow_mut().set_state_active("MapEditor", true);
        })
    }

}