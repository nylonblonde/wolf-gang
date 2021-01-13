use gdnative::prelude::*;
use gdnative::api::{
    File,
    ImageTexture,
    ItemList,
};

use legion::*;

use serde::de::DeserializeSeed;

use crate::{
    editor::ActorPaletteSelection,
    systems::{
        actor::{
            MyEntitySerializer,
            REGISTRY,
        },
        selection_box,
    }
};

use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
};

thread_local! {
    pub static ENTITY_REFS: RefCell<HashMap<i64, Entity>> = RefCell::new(
        HashMap::new()
    )
}

static mut OWNER_NODE: Option<Ref<ItemList>> = None;

#[derive(NativeClass)]
#[inherit(ItemList)]
#[user_data(user_data::LocalCellData<ActorPalette>)]
pub struct ActorPalette {
    world: Rc<RefCell<Option<World>>>,
}

#[methods]
impl ActorPalette{
    fn new(item_list: &ItemList) -> Self {

        unsafe { OWNER_NODE = Some(item_list.assume_shared()); }

        unsafe { 
            item_list.connect("item_selected", item_list.assume_shared(), "item_selected", VariantArray::default(), 0).ok(); 
        }

        ActorPalette {
            world: Rc::new(RefCell::new(None))
        }
    }

    pub fn get_world() -> Option<Rc<RefCell<Option<World>>>> {
        let owner = unsafe { OWNER_NODE.unwrap().assume_safe() };
        let instance = owner.cast_instance::<ActorPalette>();

        match instance {
            Some(instance) =>  
                instance.map(|inst, _| 
                    Some(Rc::clone(&inst.world))
                ).unwrap_or_else(|_| None),
            _ => None
        }
    }

    #[export]
    fn item_selected(&self, _: &ItemList, index: i64) {
        
        let resources = crate::WolfGang::get_resources().unwrap();
        let resources = &mut resources.borrow_mut();

        let world_lock = crate::WolfGang::get_world().unwrap();
        let world = &mut world_lock.write().unwrap();

        resources.insert(ActorPaletteSelection::new(index));

        world.push((
            selection_box::MakeActorSelectionChosen{},
        ));
    }

    #[export]
    fn _ready(&self, item_list: &ItemList) {

        self.deserialize_entities();
        self.populate_actor_palette(item_list);

        item_list.emit_signal("item_selected", &[Variant::from_i64(0)]);
    }

    #[export]
    fn _process(&self, item_list: &ItemList, _: f64) {
        if !item_list.is_anything_selected() && item_list.get_item_count() > 0 {
            item_list.select(0, true);
        }
    }

    fn deserialize_entities(&self) {
        let file = File::new();

        if file.open("res://config/actors.ron", File::READ).is_ok() {
            let file_string = file.get_as_text().to_string();

            if let Ok(mut deserializer) = ron::de::Deserializer::from_str(file_string.as_str()) {
                let ref_mut = &mut self.world.borrow_mut();

                REGISTRY.with(|r| {
                    let registry = r.borrow();
                    let serializer = MyEntitySerializer::default();
                    **ref_mut = registry.as_deserialize(&serializer).deserialize(&mut deserializer).ok();

                });
            }

            file.close();
        }
    }

    fn populate_actor_palette(&self, item_list: &ItemList) {
        let world = &mut self.world.borrow_mut();

        if let Some(world) = world.as_mut() {

            let mut query = <Entity>::query();

            let mut i = 0;
            query.iter(world)
                .copied()
                .collect::<Vec<Entity>>()
                .into_iter()
                .for_each(|entity| {
                    let texture = ImageTexture::new();

                    ENTITY_REFS.with(|e| {
                        let entity_refs = &mut e.borrow_mut();

                        entity_refs.insert(
                            i, entity
                        );

                        i += 1;
                        item_list.add_icon_item(texture, true);
                    })
                    
                });
        }
    }
}