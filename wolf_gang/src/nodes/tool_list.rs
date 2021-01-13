use gdnative::prelude::*;
use gdnative::api::{
    AtlasTexture,
    ItemList,
    ResourceLoader,
    ScrollContainer,
    StreamTexture,
};
use crate::{
    editor,
    node,
    systems::{
        selection_box,
    }
};

#[derive(NativeClass)]
#[inherit(ItemList)]
#[user_data(user_data::LocalCellData<ToolList>)]
pub struct ToolList {
}

#[methods]
impl ToolList {
    fn new(item_list: &ItemList) -> Self {

        unsafe { item_list.connect("item_selected", item_list.assume_shared(), "item_selected", VariantArray::default(), 0).ok(); }

        ToolList {}
    }

    #[export]
    fn item_selected(&self, item_list: &ItemList, index: i64) {

        let resources = crate::WolfGang::get_resources().unwrap();
        let resources = &mut resources.borrow_mut();

        let world_lock = &mut crate::WolfGang::get_world().unwrap();
        let world = &mut world_lock.write().unwrap();

        unsafe {

            let palette = get_palette(item_list);
            let palette_window = palette.get_parent().unwrap().assume_unique().cast::<ScrollContainer>().unwrap().into_shared();

            let actor_palette = get_actor_palette(item_list);
            let actor_window = actor_palette.get_parent().unwrap().assume_unique().cast::<ScrollContainer>().unwrap().into_shared();

            match index {
                0 => { 
                    palette_window.assume_safe().set_visible(true);
                    actor_window.assume_safe().set_visible(false);

                    resources.insert(editor::SelectedTool(selection_box::ToolBoxType::TerrainToolBox));

                    world.push((selection_box::ActivateTerrainToolBox{},));

                },
                1 => {
                    palette_window.assume_safe().set_visible(false);
                    actor_window.assume_safe().set_visible(true);

                    let actor_selection = resources.get::<editor::ActorPaletteSelection>().map(|selection| selection.val());
                    if let Some(actor_selection) = actor_selection {
                        resources.insert(editor::SelectedTool(selection_box::ToolBoxType::ActorToolBox(actor_selection)))
                    }

                    world.push((selection_box::ActivateActorToolBox{},));

                }
                _ => palette_window.assume_safe().set_visible(false)

            }
        }

    }

    #[export]
    fn _ready(&self, item_list: &ItemList) {

        unsafe {
            let palette = get_palette(item_list);
            populate_palette(&palette);
        }
        item_list.emit_signal("item_selected", &[Variant::from_i64(0)]);

    }

    #[export]
    fn _process(&self, item_list: &ItemList, _: f64) {
        if !item_list.is_anything_selected() {
            item_list.select(0, true);
        }
    }
}

unsafe fn get_actor_palette(item_list: &ItemList) -> TRef<ItemList> {
    let main_tools_panel = item_list.get_parent().unwrap().assume_safe().get_parent().unwrap().assume_unique();
    let actor_palette = node::get_node(&main_tools_panel, &"ActorPalette".to_string(), true)
        .unwrap().assume_unique().cast::<ItemList>().unwrap().into_shared();

    actor_palette.assume_safe()
}

unsafe fn get_palette(item_list: &ItemList) -> TRef<ItemList> {
    let main_tools_panel = item_list.get_parent().unwrap().assume_safe().get_parent().unwrap().assume_unique();
    let palette = node::get_node(&main_tools_panel, &"Palette".to_string(), true)
        .unwrap().assume_unique().cast::<ItemList>().unwrap().into_shared();

    palette.assume_safe()
}

unsafe fn populate_palette(palette: &ItemList) {

    let texture_resource = ResourceLoader::godot_singleton().load("res://images/ground.png", "StreamTexture", false).unwrap();
    let texture = texture_resource.cast::<StreamTexture>().unwrap();

    for i in 0..64 {

        let x = i % 16;
        let y = i / 16;

        let icon = AtlasTexture::new();

        icon.set_atlas(texture.clone());
        icon.set_region(Rect2::new(Point2::new(x as f32 * 128.,y as f32 * 128.), Size2::new(128.,128.)));

        palette.add_icon_item(icon, true);

    }
}

// unsafe fn populate_actor_palette(item_list: &ItemList, actor_definitions: &ActorDefinitions) {

//     for definition in actor_definitions.get_definitions() {

//         godot_print!("{:?}", definition.get_name());
            
//         let texture = ImageTexture::new();

//         item_list.add_icon_item(texture, true);
//     }
// }

