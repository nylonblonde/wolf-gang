use gdnative::prelude::*;
use gdnative::api::{
    ItemList,
};

use crate::editor::PaletteSelection;

#[derive(NativeClass)]
#[inherit(ItemList)]
#[user_data(user_data::LocalCellData<Palette>)]
pub struct Palette {}

#[methods]
impl Palette{
    fn new(item_list: &ItemList) -> Self {

        unsafe { item_list.connect("item_selected", item_list.assume_shared(), "item_selected", VariantArray::default(), 0).ok(); }

        Palette {}
    }

    #[export]
    fn item_selected(&self, _: &ItemList, index: i64) {
        
        let resources = crate::WolfGang::get_resources().unwrap();
        let resources = &mut resources.borrow_mut();

        resources.insert(PaletteSelection::new(index as u32));
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