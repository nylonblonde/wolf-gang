use legion::*;
use crate::{
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
        resources.insert(History::new());
        resources.insert(self.map);    
        resources.insert(level_map::document::Document::default());
    }

    fn free(&mut self, world: &mut World, resources: &mut Resources) {
        resources.remove::<History>();
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
    }

    fn on_client_connected(&self, _connection_id: u32, world: &mut World, resources: &mut Resources) {
        if let Some(self_id) = resources.get::<ClientID>() {

        //Get all of the selection boxes to send them to the new client at self_id
        let mut query = <(Read<selection_box::SelectionBox>, Read<ClientID>, Read<level_map::CoordPos>)>::query();

        let results = query.iter(world)
            .map(|(selection_box, client_id, coord_pos)| (selection_box.aabb, *client_id, *coord_pos))
            .collect::<Vec<(AABB, ClientID, level_map::CoordPos)>>();


            results.into_iter().for_each(|(aabb, client_id, coord_pos)| {

                world.push(
                    (
                        ServerMessageSender {
                            client_id: self_id.val(),
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

        }
        
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
    fn new(name: &'static str, schedule: Schedule, active: bool) -> Self {

        Self {
            camera: String::default(),
            game_state: GameState::new(name, schedule, active),
            map: level_map::Map::default()
        }
    }
}