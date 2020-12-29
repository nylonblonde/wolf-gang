use legion::*;
use crate::{
    actors::actor::ActorDefinitions,
    collections::octree::Octree,
    game_state::{NewState, GameState, GameStateTraits},
    systems::{
        camera,
        history::History,
        level_map,
        selection_box,
        networking::{
            ClientID,
            ServerMessageSender,
            DataType,
            MessageType,
        }
    },
    node,
    node::NodeName
};

type AABB = crate::geometry::aabb::AABB<i32>;

pub struct Editor {
    game_state: GameState,
    camera: String,
    map: level_map::Map,
}

impl GameStateTraits for Editor {

    fn initialize(&mut self, world: &mut World, resources: &mut Resources) {
        
        self.camera = camera::initialize_camera(world);
        resources.insert(self.map);
        resources.insert(level_map::document::Document::default());
        resources.insert(PaletteSelection(0));
        resources.insert(SelectedTool(selection_box::ToolBoxType::TerrainToolBox));

        if let Some(actor_definitions) = ActorDefinitions::from_config() {
            resources.insert(actor_definitions);
        }
    }

    fn free(&mut self, world: &mut World, resources: &mut Resources) {
        resources.remove::<level_map::document::Document>();

        node::free(world, &self.camera);
        self.map.free(world);
    }

    fn on_connection(&self, connection_id: u32, world: &mut World, resources: &mut Resources) {

        let client_id = resources.get::<ClientID>().map(|client_id| *client_id);

        //Only pass the camera name if this selection box belongs to the client
        let camera: Option<String> = match client_id {
            Some(r) if r.val() == connection_id => {
                Some(self.camera.clone())
            },
            _ => None
        };
        
        world.push((
           ClientID::new(connection_id),
           History::new() 
        ));

        selection_box::initialize_selection_box(world, connection_id, selection_box::ToolBoxType::TerrainToolBox, camera.clone());
        selection_box::initialize_selection_box(world, connection_id, selection_box::ToolBoxType::ActorToolBox, camera);

        if let Some(client_id) = client_id {
            //Activate tool if this box belongs to the client
            if client_id.val() == connection_id {

                let selected_tool = resources.get::<SelectedTool>().unwrap();

                match selected_tool.0 {
                    selection_box::ToolBoxType::TerrainToolBox =>{
                        world.push((
                            selection_box::ActivateTerrainToolBox{},
                        ));
                    },
                    selection_box::ToolBoxType::ActorToolBox => {
                        world.push((
                            selection_box::ActivateActorToolBox{},
                        ));
                    }
                }
            }
        }

    }

    fn on_disconnection(&self, connection_id: u32, world: &mut World, _: &mut Resources) {

        let mut query = <(Read<ClientID>, Read<NodeName>)>::query();

        let mut name = query.iter(world)
            .filter(|(id, _)| connection_id == id.val())
            .map(|(_, name)| (*name).clone());

        if let Some(name) = name.next() {
            node::free(world, &name.0);
        }

        let mut query = <(Entity, Read<ClientID>)>::query().filter(component::<History>());
        query.iter(world).filter(|(_, id)| id.val() == connection_id)
            .map(|(entity, _)| *entity)
            .collect::<Vec<Entity>>().into_iter()
            .for_each(|entity| {world.remove(entity);});

    }

    fn on_client_connected(&self, connection_id: u32, world: &mut World, _: &mut Resources) {

        //Get all of the selection boxes to send them to the new client
        let mut query = <(Entity, Read<selection_box::SelectionBox>, Read<ClientID>, Read<level_map::CoordPos>)>::query();

        let results = query.iter(world)
            .map(|(entity, selection_box, client_id, coord_pos)| (*entity, selection_box.aabb, *client_id, *coord_pos))
            .collect::<Vec<(Entity, AABB, ClientID, level_map::CoordPos)>>();


        results.into_iter().for_each(|(entity, aabb, client_id, coord_pos)| {

            if let Some(entry) = world.entry(entity) {

                let box_type: Option<selection_box::ToolBoxType> = if let Ok(_) = entry.get_component::<selection_box::ActorToolBox>() {
                    Some(selection_box::ToolBoxType::ActorToolBox)
                } else if let Ok(_) = entry.get_component::<selection_box::TerrainToolBox>() {
                    Some(selection_box::ToolBoxType::TerrainToolBox)
                } else {
                    None
                };

                let active = entry.get_component::<selection_box::Active>().is_ok();

                world.push(
                    (
                        ServerMessageSender {
                            client_id: connection_id,
                            data_type: DataType::CreateSelectionBox {
                                active,
                                box_type: box_type.unwrap(),
                                client_id: client_id.val(),
                                aabb,
                                coord_pos: coord_pos.value
                            },
                            message_type: MessageType::Reliable,
                        },
                    )
                );

            }

        });

        //send all of the current map data as map inputs to the new client
        let mut query = <Read<level_map::MapChunkData>>::query();

        let results = query.iter(world)
            .map(|map_data| map_data.clone().octree)
            .collect::<Vec<Octree<i32, level_map::TileData>>>();
        
        results.into_iter().for_each(|octree| {

            world.push(
                (
                    ServerMessageSender {
                        client_id: connection_id,
                        data_type: DataType::MapInput(octree),
                        message_type: MessageType::Ordered,
                    },
                )
            );

        })
    }

}

impl AsMut<GameState> for Editor {
    fn as_mut(&mut self) -> &mut GameState {
        &mut self.game_state
    }
}

impl AsRef<GameState> for Editor {
    fn as_ref(&self) -> &GameState {
        &self.game_state
    }
}

impl NewState for Editor {
    fn new(name: &'static str, active: bool) -> Self {

        Self {
            camera: String::default(),
            game_state: GameState::new(name, active),
            map: level_map::Map::default()
        }
    }
}

#[derive(Copy, Clone)]
pub struct PaletteSelection(u32);

impl PaletteSelection {

    pub fn new(id: u32) -> PaletteSelection {
        PaletteSelection(id)
    }

    pub fn val(&self) -> u32 {
        self.0
    }
}

#[derive(Copy, Clone)]
pub struct ActorPaletteSelection(u32);

impl ActorPaletteSelection {

    pub fn new(id: u32) -> ActorPaletteSelection {
        ActorPaletteSelection(id)
    }

    pub fn val(&self) -> u32 {
        self.0
    }
}

#[derive(Copy, Clone, PartialEq)]
pub struct SelectedTool(pub selection_box::ToolBoxType);