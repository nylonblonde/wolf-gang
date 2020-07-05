use gdnative::*;
use super::utils;
use crate::{
    node,
    game_state::GameState,
    systems::{
        level_map,
        selection_box,
    }
};
use legion::prelude::*;

/// The EditMenu "class"
#[derive(NativeClass)]
#[inherit(MenuButton)]
#[register_with(Self::register_signals)]
#[user_data(user_data::LocalCellData<FileMenu>)]
pub struct FileMenu {
    popup_menu: PopupMenu,
    file_dialog: Option<FileDialog>
}

// __One__ `impl` block can have the `#[methods]` attribute, which will generate
// code to automatically bind any exported methods to Godot.
#[methods]
impl FileMenu {
    
    /// The "constructor" of the class.
    fn _init(menu_button: MenuButton) -> Self {

        let popup_menu = utils::get_popup_menu(menu_button);

        FileMenu{
            popup_menu,
            file_dialog: None
        }

    }

    fn register_signals(builder: &init::ClassBuilder<Self>) {
        builder.add_signal(init::Signal {
            name: "save_load_popup",
            args: &[init::SignalArgument {
                name: "type_flag",
                default: Variant::from_i64(0),
                export_info: init::ExportInfo::new(VariantType::I64),
                usage: init::PropertyUsage::DEFAULT
            }]
        })
    }

    #[export]
    fn _ready(&mut self, mut menu_button: MenuButton) {
        unsafe {

            let dialog = node::get_node(&menu_button, "FileDialog".to_string()); 

            match dialog {
                Some(dialog) => {
                    match dialog.cast::<FileDialog>() {
                        Some(file_dialog) => {

                            match menu_button.connect(GodotString::from("save_load_popup"), Some(file_dialog.to_object()), GodotString::from("save_load_handler"), VariantArray::new(), 0) {
                                Ok(_) => {
                                    self.file_dialog = Some(file_dialog)
                                },
                                Err(err) => panic!("{:?}", err)
                            }
                        },
                        None => panic!("Couldn't cast the FileDialog Node")
                    }
                },
                None => panic!("Couldn't find the FileDialog!")
            }

        }
    }

    #[export]
    fn _pressed(&mut self, _: MenuButton) {

    }

    #[export]
    fn item_handler(&mut self, mut menu_button: MenuButton, id: i64) {

        match id {
            0 => { //new

                godot_print!("New");

                let mut game = crate::GAME_UNIVERSE.lock().unwrap();
                let game = &mut *game;
                let world = &mut game.world;
                let resources = &mut game.resources;

                crate::STATE_MACHINE.with(|s| {

                    godot_print!("k");

                    // let mut remove_at_index:Option<usize> = None;

                    // for state in &s.borrow().states {
                        // let state: &mut crate::editor::Editor<'static> = state.as_mut().as_mut().as_any().downcast_mut::<crate::editor::Editor>().unwrap();

                        // let game_state: &GameState = state.as_ref();
                        // if game_state.get_name() == "MapEditor" {
                        //     state.free(world, resources);
                        //     // remove_at_index = Some(i);

                        //     state.initialize(world, resources);
                        // }
                    // }
                    // if let Some(idx) = remove_at_index {
                    //     states.remove(idx);
                    // }
                });

                // let map = match resources.get_mut::<level_map::Map>() {
                //     Some(map) => map,
                //     None => panic!("Couldn't get Map from Resources")
                // };

                // map.free(world, resources);

                // let selection_box_query = <(Write<selection_box::SelectionBox>, Write<level_map::CoordPos>)>::query();

                // for (mut selection_box, mut coord_pos) in  selection_box_query.iter_mut(world) {
                //     *selection_box = selection_box::SelectionBox::new();
                //     *coord_pos = level_map::CoordPos::default();
                // }

            },
            1 => { //open

                unsafe { menu_button.emit_signal(GodotString::from("save_load_popup"), &[Variant::from_i64(0)]); }

            },
            2 => { //save

            },
            3 => { //save-as

                unsafe { menu_button.emit_signal(GodotString::from("save_load_popup"), &[Variant::from_i64(1)]); }
                
            },
            _ => {}
        }
    }

}