use gdnative::prelude::*;
use gdnative::api::{
    ItemList,
};

use crate::{
    editor::ActorPaletteSelection,
    systems::selection_box,
};

#[derive(NativeClass)]
#[inherit(ItemList)]
#[user_data(user_data::LocalCellData<ActorPalette>)]
pub struct ActorPalette {}

#[methods]
impl ActorPalette{
    fn new(item_list: &ItemList) -> Self {

        unsafe { 
            item_list.connect("item_selected", item_list.assume_shared(), "item_selected", VariantArray::default(), 0).ok(); 
        }

        ActorPalette {}
    }

    #[export]
    fn item_selected(&self, _: &ItemList, index: i64) {
        
        let resources = crate::WolfGang::get_resources().unwrap();
        let resources = &mut resources.borrow_mut();

        let world_lock = crate::WolfGang::get_world().unwrap();
        let world = &mut world_lock.write().unwrap();

        resources.insert(ActorPaletteSelection::new(index as u32));

        world.push((
            selection_box::MakeActorSelectionChosen{},
        ));
    }

    #[export]
    fn _ready(&self, item_list: &ItemList) {

        item_list.emit_signal("item_selected", &[Variant::from_i64(0)]);

    }

    #[export]
    fn _process(&self, item_list: &ItemList, _: f64) {
        if !item_list.is_anything_selected() {
            item_list.select(0, true);
        }
    }
}