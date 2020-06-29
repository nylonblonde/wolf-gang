use gdnative::*;

/// The EditMenu "class"
#[derive(NativeClass)]
#[inherit(MenuButton)]
#[user_data(user_data::LocalCellData<EditMenu>)]
pub struct EditMenu {

}

// __One__ `impl` block can have the `#[methods]` attribute, which will generate
// code to automatically bind any exported methods to Godot.
#[methods]
impl EditMenu {
    
    /// The "constructor" of the class.
    fn _init(_: MenuButton) -> Self {
        EditMenu{}
    }

    #[export]
    fn _pressed(&mut self, _: MenuButton) {
        godot_print!("Howdy!");
    }
    
}
