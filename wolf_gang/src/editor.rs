use legion::*;
use crate::{
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
    }

    fn free(&mut self, world: &mut World, resources: &mut Resources) {
        resources.remove::<level_map::document::Document>();

        node::free(world, &self.camera);
        self.map.free(world);
    }

    fn on_connection(&self, connection_id: u32, world: &mut World, resources: &mut Resources) {

        //Only pass the camera name if this selection box belongs to the client
        let camera: Option<String> = match resources.get::<ClientID>() {
            Some(r) if r.val() == connection_id => {
                Some(self.camera.clone())
            },
            _ => None
        };
        
        world.push((
           ClientID::new(connection_id),
           History::new() 
        ));

        selection_box::initialize_selection_box(world, connection_id, camera);

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
        let mut query = <(Read<selection_box::SelectionBox>, Read<ClientID>, Read<level_map::CoordPos>)>::query();

        let results = query.iter(world)
            .map(|(selection_box, client_id, coord_pos)| (selection_box.aabb, *client_id, *coord_pos))
            .collect::<Vec<(AABB, ClientID, level_map::CoordPos)>>();


        results.into_iter().for_each(|(aabb, client_id, coord_pos)| {

            world.push(
                (
                    ServerMessageSender {
                        client_id: connection_id,
                        data_type: DataType::CreateSelectionBox {
                            client_id: client_id.val(),
                            aabb,
                            coord_pos: coord_pos.value
                        },
                        message_type: MessageType::Reliable,
                        
                    },
                )
            );

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
pub struct PaletteSelection(pub i64);