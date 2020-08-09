use gdnative::prelude::*;
use gdnative::api::ImmediateGeometry;
use legion::*;
use nalgebra::Rotation3;
use num::Float;

use std::cmp::Ordering;

use crate::{
    geometry::aabb,
    node,
    networking::{ClientID, DataType, MessageSender, MessageType},
    systems::{
        camera,
        custom_mesh,
        transform,
        input,
        level_map,
    }
};

type AABB = aabb::AABB<i32>;
type Point = nalgebra::Vector3<i32>;

type Vector3D = nalgebra::Vector3<f32>;
type Vector2D = nalgebra::Vector2<f32>;

#[derive(Debug, Clone)]
pub struct SelectionBox {
    pub aabb: AABB
}

#[derive(Copy, Clone, PartialEq)]
pub struct CameraAdjustedDirection {
    pub forward: Vector3D,
    pub right: Vector3D
}

impl Default for CameraAdjustedDirection {
    fn default() -> Self {
        CameraAdjustedDirection {
            forward: Vector3D::z(),
            right: Vector3D::x()
        }
    }
}

impl SelectionBox {
    ///Creates a SelectionBox with an aabb at center (0,0,0) with dimensions of (1,1,1).
    pub fn new() -> Self {
        SelectionBox {
            aabb: AABB::new(Point::new(0,0,0), Point::new(1,1,1))
        }
    }

    pub fn from_aabb(aabb: AABB) -> Self {
        SelectionBox {
            aabb
        }
    }
}

#[derive(Default, Copy, Clone)]
pub struct MoveTo(pub Point);

#[derive(Default, Clone)]
pub struct RelativeCamera(pub String);

pub fn initialize_selection_box(world: &mut World, client_id: u32, camera_name: Option<String>) {

    let mesh: Ref<ImmediateGeometry, Unique> = ImmediateGeometry::new();

    let node_name = unsafe { 
        node::add_node(mesh.upcast())
    }.unwrap();

    let entity = world.push(
        (
            node_name,
            ClientID::new(client_id),
            SelectionBox::new(), 
            custom_mesh::MeshData::new(),
            level_map::CoordPos::default(),
            transform::position::Position::default(), 
            CameraAdjustedDirection::default(),
            custom_mesh::Material::from_str("res://materials/select_box.material")
        )
    );

    if let Some(camera_name) = camera_name {
        if let Some(mut entry) = world.entry(entity) {
            entry.add_component(RelativeCamera(camera_name))
        }
    }
}

/// Removes all SelectionBox entities from the world, and frees and removes the related Godot nodes
pub fn free_all(world: &mut World) {
    let mut selection_box_query = <(Entity, Read<node::NodeName>)>::query()
        .filter(component::<SelectionBox>());

    let mut entities: Vec<Entity> = Vec::new();

    selection_box_query.for_each(world, |(entity, node_name)| {
        unsafe {
            node::remove_node(&node_name.0);
        }

        entities.push(*entity);
    });

    for entity in entities {
        world.remove(entity);
    }
}

/// Gets the axis closest to forward from a or b, adjusted by adjust_angle around the up axis. We adjust it so that we can smooth out the comparison at 45
/// degree angles.
fn get_forward_closest_axis(a: &Vector3D, b: &Vector3D, forward: &Vector3D, right: &Vector3D, up: &nalgebra::Unit<Vector3D>, adjust_angle: f32) -> std::cmp::Ordering {
    
    let a_dot = a.dot(right);
    let b_dot = b.dot(right);

    let dot = match PartialOrd::partial_cmp(&a_dot, &b_dot) {
        None => 0., //If NaN just set it to 0
        Some(Ordering::Less) => a_dot,
        Some(_) => b_dot
    };

    let dir = match PartialOrd::partial_cmp(&dot, &0.) {
        None => 0., //If NaN just set it to 0
        Some(Ordering::Less) => -1.,
        Some(_) => 1.
    };

    let forward = nalgebra::UnitQuaternion::<f32>::from_axis_angle(up, adjust_angle*dir) * forward;

    a.dot(&forward).partial_cmp(
        &b.dot(&forward)
    ).unwrap()
}

/// Calculates the orthogonal direction that should be considered forward and right when grid-like directional input is used.
pub fn create_orthogonal_dir_system() -> impl systems::Schedulable {

    SystemBuilder::new("orthogonal_dir_system")
        .with_query(<(Write<CameraAdjustedDirection>, Read<RelativeCamera>)>::query())
        .with_query(<(Read<transform::rotation::Direction>, Read<node::NodeName>)>::query()
            .filter(maybe_changed::<transform::rotation::Direction>() & component::<camera::FocalPoint>()))
        .build(|_, world, _, queries| {

            let (selection_box_query, cam_query) = queries;

            let cameras = cam_query.iter(world)
                .map(|(dir, name)| (*dir, (*name).clone()))
                .collect::<Vec<(transform::rotation::Direction, node::NodeName)>>();

            for (mut camera_adjusted_dir, relative_cam) in selection_box_query.iter_mut(world) {

                let node_name = node::NodeName(relative_cam.0.clone());

                match cameras.iter().filter(|(_,name)| *name == node_name).next() {
                    Some((dir, _)) => {

                        // Get whichever cartesian direction in the grid is going to act as "forward" based on its closeness to the camera's forward
                        // view.
                        let mut forward = dir.forward;
                        let mut right = dir.right;

                        forward.y = 0.;
                        
                        let adjustment_angle = std::f32::consts::FRAC_PI_8;

                        forward = std::cmp::min_by(Vector3D::z(), 
                            std::cmp::min_by(-Vector3D::z(), 
                                std::cmp::min_by(Vector3D::x(), -Vector3D::x(),
                                    |lh: &Vector3D, rh: &Vector3D| {
                                        get_forward_closest_axis(lh, rh, &forward, &right, &Vector3D::y_axis(), adjustment_angle)
                                    }
                                ), 
                                |lh: &Vector3D, rh: &Vector3D| {
                                    get_forward_closest_axis(lh, rh, &forward, &right, &Vector3D::y_axis(), adjustment_angle)
                                }
                            ), 
                            |lh: &Vector3D, rh: &Vector3D| {
                                get_forward_closest_axis(lh, rh, &forward, &right, &Vector3D::y_axis(), adjustment_angle)
                            }
                        );

                        //calculate right from up and forward by just rotating forward by -90 degrees
                        right =  nalgebra::UnitQuaternion::<f32>::from_axis_angle(&Vector3D::y_axis(), -std::f32::consts::FRAC_PI_2) * forward;

                        forward = forward.normalize();
                        right = right.normalize();

                        camera_adjusted_dir.forward = forward;
                        camera_adjusted_dir.right = right;
                    },
                    None => {}
                }
            }
    })
} 

/// This system reads input, then moves the coord position of the selection_box
pub fn create_movement_system() -> impl systems::Schedulable {
    
    let move_forward = input::Action("move_forward".to_string());
    let move_back = input::Action("move_back".to_string());
    let move_left = input::Action("move_left".to_string());
    let move_right = input::Action("move_right".to_string());
    let move_up = input::Action("move_up".to_string());
    let move_down = input::Action("move_down".to_string());

    SystemBuilder::new("selection_box_movement_system")
        .read_resource::<crate::Time>()
        .read_resource::<ClientID>()
        .with_query(<(Read<input::InputActionComponent>, Read<input::Action>)>::query())
        .with_query(<(Read<CameraAdjustedDirection>, Read<ClientID>, Read<level_map::CoordPos>)>::query()
            .filter(component::<SelectionBox>()))
        .build(move |commands, world, (time, client_id), queries| {

            let (input_query, selection_box_query) = queries;

            let inputs = input_query.iter(world)
                .map(|(input, action)| (*input, (*action).clone()))
                .collect::<Vec<(input::InputActionComponent, input::Action)>>();

            for(input_component, action) in inputs.iter().filter(|(_, a)|
                a == &move_forward ||
                a == &move_back ||
                a == &move_left ||
                a == &move_right ||
                a == &move_up ||
                a == &move_down
            ) {                    
                
                if input_component.repeated(time.delta, 0.25) {

                    selection_box_query.iter(world)
                        .filter(|(_, id, _)| **id == **client_id)
                        .for_each(|(camera_adjusted_dir, _, coord_pos)| {

                        let mut movement = Point::zeros();

                        if action.0 == move_forward.0 {
                            movement.z += 1;
                        } else if action.0 == move_back.0 {
                            movement.z -= 1;
                        } else if action.0 == move_left.0 {
                            movement.x -= 1;
                        } else if action.0 == move_right.0 {
                            movement.x += 1;
                        } else if action.0 == move_up.0 {
                            movement.y += 1;
                        } else if action.0 == move_down.0 {
                            movement.y -= 1;
                        }
                        
                        let forward = camera_adjusted_dir.forward;
                        let right = camera_adjusted_dir.right;

                        let mut adjusted = Point::new(
                            forward.x.round() as i32,
                            0,
                            forward.z.round() as i32
                        ) * movement.z + Point::new(
                            right.x.round() as i32,
                            0,
                            right.z.round() as i32
                        ) * movement.x;

                        adjusted.y = movement.y;
                        
                        let move_to_pos = coord_pos.value + adjusted;

                        commands.push(
                            (
                                MoveTo(coord_pos.value + adjusted),
                                **client_id
                            )
                        );

                        commands.push((MessageSender{
                            data_type: DataType::MoveSelection{ client_id: client_id.val(), point: move_to_pos },
                            message_type: MessageType::Ordered
                        },));

                    });
                }            
            }       
        })
}

pub fn create_move_to_system() -> impl systems::Schedulable {
    SystemBuilder::new("selection_box_move_to_system")
        .with_query(<(Read<ClientID>, Write<level_map::CoordPos>)>::query()
            .filter(component::<SelectionBox>())
        )
        .with_query(<(Entity, Read<ClientID>, Read<MoveTo>)>::query())
        .build(|commands, world, _, queries| {

            let (selection_box_query, move_to_query) = queries;

            let move_tos = move_to_query.iter(world)
                .map(|(entity, client_id, move_to)| (*entity, *client_id, *move_to))
                .collect::<Vec<(Entity, ClientID, MoveTo)>>();

            selection_box_query.for_each_mut(world, |(client_id, coord_pos)| {

                if let Some((entity, _, move_to)) = move_tos.iter().find(|(_,id,_)| id == client_id) {
                    coord_pos.value = move_to.0;
                    commands.remove(*entity);
                }
            });
        })
}

pub fn create_coord_to_pos_system() -> impl systems::Schedulable {
    SystemBuilder::new("selection_box_coord_system")
        .with_query(<(Read<level_map::CoordPos>, Write<transform::position::Position>,)>::query()
            .filter(maybe_changed::<level_map::CoordPos>() & component::<SelectionBox>())
        )
        .build(move |_, world, _, query| {

            query.for_each_mut(world, |(coord_pos, mut position)| {

                let coord_pos = level_map::map_coords_to_world(coord_pos.value);
                position.value = Vector3::new(coord_pos.x, coord_pos.y, coord_pos.z); 
            })
        })
}

pub fn create_tile_tool_system() -> impl systems::Schedulable {
    let insertion = input::Action(("insertion").to_string());
    let removal = input::Action(("removal").to_string());

    SystemBuilder::new("tile_tool_system")
        .read_resource::<level_map::Map>()
        .with_query(<(Read<SelectionBox>, Read<level_map::CoordPos>)>::query()) //all selection_boxes
        .with_query(<(Read<SelectionBox>, Read<level_map::CoordPos>)>::query() //only moved selection_boxes
            .filter(maybe_changed::<level_map::CoordPos>()))
        .with_query(<(Read<input::InputActionComponent>, Read<input::Action>)>::query())
        .build(move |commands, world, map, queries| {

            let (selection_box_query, selection_box_moved_query, input_query) = queries;

            let mut to_insert: Option<AABB> = None;
            let mut to_remove: Option<AABB> = None;

            for (input_component, action) in input_query.iter(world).filter(|(_,a)|
                *a == &insertion ||
                *a == &removal
            ) {
                selection_box_query.for_each(world, |(selection_box, coord_pos)| {
                    
                    let moved = selection_box_moved_query.iter(world).next().is_some();

                    if input_component.just_pressed() 
                    || (input_component.is_held() && moved) 
                    {
                        if action == &insertion {
                            godot_print!("Pressed insertion at {:?}!", coord_pos.value);

                            to_insert = Some(AABB::new(coord_pos.value, selection_box.aabb.dimensions));
                        } else if action == &removal {
                            godot_print!("Pressed removal at {:?}!", coord_pos.value);

                            to_remove = Some(AABB::new(coord_pos.value, selection_box.aabb.dimensions));
                        }
                        
                    }
                })
            }

            let map = **map;

            commands.exec_mut(move |world|{
                if let Some(r) = to_insert {
                    map.insert(world, level_map::TileData::new(Point::zeros()), r);
                }
        
                if let Some(r) = to_remove {
                    map.remove(world, r);
                }
            });
        })
}

/// Expands the dimensions of the selection box
pub fn create_expansion_system() -> impl systems::Schedulable {    

    let expand_selection_forward = input::Action("expand_selection_forward".to_string());
    let expand_selection_back = input::Action("expand_selection_back".to_string());
    let expand_selection_left = input::Action("expand_selection_left".to_string());
    let expand_selection_right = input::Action("expand_selection_right".to_string());
    let expand_selection_up = input::Action("expand_selection_up".to_string());
    let expand_selection_down = input::Action("expand_selection_down".to_string());

    SystemBuilder::new("selection_expansion_system")
        .read_resource::<crate::Time>()
        .with_query(<(Read<input::InputActionComponent>, Read<input::Action>)>::query())
        .with_query(<(Write<SelectionBox>, Write<level_map::CoordPos>, Read<CameraAdjustedDirection>)>::query())
        .build(move |_, world, time, queries| {
            let (input_query, selection_box_query) = queries;

            let inputs = input_query.iter(world)
                .map(|(input, action)| (*input, (*action).clone()))
                .collect::<Vec<(input::InputActionComponent, input::Action)>>();

            for(input_component, action) in inputs.iter().filter(|(_, a)|
                a == &expand_selection_forward ||
                a == &expand_selection_back ||
                a == &expand_selection_left ||
                a == &expand_selection_right ||
                a == &expand_selection_up ||
                a == &expand_selection_down
            ) {                    
                
                if input_component.repeated(time.delta, 0.25) {

                    selection_box_query.for_each_mut(world, |(mut selection_box, coord_pos, camera_adjusted_dir)| {

                        let mut expansion = Point::zeros();

                        if action == &expand_selection_forward {
                            expansion.z += 1;
                        } else if action == &expand_selection_back {
                            expansion.z -= 1;
                        } else if action == &expand_selection_left {
                            expansion.x -= 1;
                        } else if action == &expand_selection_right {
                            expansion.x += 1;
                        } else if action == &expand_selection_down {
                            expansion.y -= 1;
                        } else if action == &expand_selection_up {
                            expansion.y += 1;
                        }

                        let forward = camera_adjusted_dir.forward;
                        let right = camera_adjusted_dir.right;

                        let mut adjusted = Point::new(
                            forward.x.round().abs() as i32,
                            0,
                            forward.z.round().abs() as i32
                        ) * expansion.z as i32 + Point::new(
                            right.x.round().abs() as i32,
                            0,
                            right.z.round().abs() as i32
                        ) * expansion.x as i32;

                        adjusted.y = expansion.y as i32;

                        let mut new_aabb = crate::geometry::aabb::AABB::new(Point::zeros(), selection_box.aabb.dimensions + adjusted);

                        if new_aabb.dimensions.x == 0 {
                            new_aabb.dimensions.x += adjusted.x * 2;
                        }

                        if new_aabb.dimensions.y == 0 {
                            new_aabb.dimensions.y += adjusted.y * 2;
                        }

                        if new_aabb.dimensions.z == 0 {
                            new_aabb.dimensions.z += adjusted.z * 2;
                        }

                        let mut min = selection_box.aabb.get_min();
                        let mut max = selection_box.aabb.get_max();

                        let mut new_min = new_aabb.get_min();
                        let mut new_max = new_aabb.get_max();

                        //Adjust the offset based off of camera direction
                        if camera_adjusted_dir.right.x < 0. { 
                            let tmp_min = min.x;
                            let tmp_new_min = new_min.x;
                            min.x = max.x; 
                            new_min.x = new_max.x; 
                            max.x = tmp_min;
                            new_max.x = tmp_new_min;
                        } 
                        if camera_adjusted_dir.right.z < 0. { 
                            let tmp_min = min.z;
                            let tmp_new_min = new_min.z;
                            min.z = max.z; 
                            new_min.z = new_max.z; 
                            max.z = tmp_min;
                            new_max.z = tmp_new_min;
                        }

                        let diff = Point::new(
                            if new_aabb.dimensions.x < 0 { new_max.x - max.x } else { new_min.x - min.x },
                            if new_aabb.dimensions.y < 0 { new_max.y - max.y } else { new_min.y - min.y },
                            if new_aabb.dimensions.z < 0 { new_max.z - max.z } else { new_min.z - min.z },
                        );

                        coord_pos.value -= diff;

                        selection_box.aabb.dimensions = new_aabb.dimensions;

                    }); 
                }
            }
        })

}

pub fn create_system() -> impl systems::Schedulable {
    
    SystemBuilder::new("selection_box_system")
        .with_query(<(Read<SelectionBox>, Write<custom_mesh::MeshData>,)>::query()
            .filter(maybe_changed::<SelectionBox>(),)
        )
        .build(move |_, world, _, query| {

            query.for_each_mut(world, |(selection_box, mesh_data)| {

                mesh_data.verts.clear();
                mesh_data.normals.clear();
                mesh_data.uvs.clear();
                mesh_data.indices.clear();

                //offset that the next face will begin on, increments by the number of verts for each face
                //at the end of each loop
                let mut offset = 0;

                let min = level_map::map_coords_to_world(selection_box.aabb.get_min()) - level_map::map_coords_to_world(selection_box.aabb.center);
                let max = level_map::map_coords_to_world(selection_box.aabb.get_max() + Point::new(1,1,1)) - level_map::map_coords_to_world(selection_box.aabb.center);

                let true_center = (max + min) / 2.0;
                let true_dimensions = level_map::map_coords_to_world(selection_box.aabb.dimensions);

                let abs_dimensions = Vector3D::new(
                    true_dimensions.x.abs(),
                    true_dimensions.y.abs(),
                    true_dimensions.z.abs()
                );

                for i in 0..3 { 

                    let mut verts: Vec<Vector3> = Vec::new();  
                    let mut normals: Vec<Vector3> = Vec::new();
                    let mut uvs: Vec<Vector2> = Vec::new();

                    let max_margin = 0.9;

                    let smaller_x = Float::min(max_margin, abs_dimensions.x /2.0);
                    let smaller_y = Float::min(max_margin, abs_dimensions.y /2.0);
                    let smaller_z = Float::min(max_margin, abs_dimensions.z /2.0);

                    let margin = Float::min(smaller_x, Float::min(smaller_y, smaller_z));

                    match i {
                        0 => { // top and bottom

                            //store vectors as nalgebra's Vector3 to do transformations
                            let mut pts: Vec<Vector3D> = Vec::new();

                            let top_right = Vector3D::new(max.x , max.y , max.z );
                            let inner_top_right = Vector3D::new( //inner top right
                                max.x  - margin,
                                max.y ,
                                max.z  - margin
                            );

                            pts.push(Vector3D::new(min.x , max.y , max.z )); //0 top left
                            pts.push(top_right); //1
                            pts.push(Vector3D::new( //2 inner top left
                                min.x  + margin,
                                max.y ,
                                max.z  - margin
                            ));
                            pts.push(inner_top_right); //3
                            pts.push(top_right); //4
                            pts.push(Vector3D::new(max.x , max.y , min.z )); //5 bottom right
                            pts.push(inner_top_right); //6
                            pts.push(Vector3D::new( //7 inner bottom right
                                    max.x  - margin,
                                    max.y ,
                                    min.z  + margin
                            ));

                            let mut uv: Vec<Vector2D> = Vec::new();

                            uv.push(Vector2D::new(0.0, 0.0));
                            uv.push(Vector2D::new(1.0 * abs_dimensions.x , 0.0));
                            uv.push(Vector2D::new(margin, margin));
                            uv.push(Vector2D::new(1.0 * abs_dimensions.x - margin, margin));

                            uv.push(Vector2D::new(0.0, 0.0));
                            uv.push(Vector2D::new(1.0 * abs_dimensions.z , 0.0));
                            uv.push(Vector2D::new(margin, margin));
                            uv.push(Vector2D::new(1.0 * abs_dimensions.z - margin, margin));

                            for (pt, u) in pts.iter().zip(uv.iter()) {

                                uvs.push(Vector2::new(u.x, u.y));
                                verts.push(Vector3::new(pt.x, pt.y, pt.z));
                            }

                            let pts_len = pts.len();
                            for i in 0..pts_len {

                                let new_pt = pts[i] - true_center;
                                let u = uv[i];

                                let rot = Rotation3::new(Vector3D::y() * std::f32::consts::PI);
                                let rotated_pt = rot.transform_vector(&new_pt) + true_center;

                                pts.push(rotated_pt);
                                uv.push(u);

                                uvs.push(Vector2::new(u.x, u.y));
                                verts.push(Vector3::new(rotated_pt.x, rotated_pt.y, rotated_pt.z));
                            }

                            for _ in 0..pts.len() {
                                normals.push(Vector3::new(0.0, 1.0, 0.0));
                            }

                            for (pt, u) in pts.iter().zip(uv.iter()) {
                                let new_pt = pt - true_center;

                                let rot = Rotation3::new(Vector3D::x() * std::f32::consts::PI);
                                let rotated_pt = rot.transform_vector(&new_pt) + true_center;

                                uvs.push(Vector2::new(u.x, u.y));
                                verts.push(Vector3::new(rotated_pt.x, rotated_pt.y, rotated_pt.z));
                                normals.push(Vector3::new(0.0,-1.0,0.0));
                            }

                        },
                        1 => { //left and right

                            let mut pts: Vec<Vector3D> = Vec::new();

                            let top_right = Vector3D::new(max.x , max.y , max.z );
                            let inner_top_right = Vector3D::new( //inner top right
                                max.x ,
                                max.y  - margin,
                                max.z  - margin
                            );

                            pts.push(Vector3D::new(max.x , max.y , min.z )); //0 top left
                            pts.push(top_right); //1
                            pts.push(Vector3D::new( //2 inner top left
                                max.x ,
                                max.y  - margin,
                                min.z  + margin
                            ));
                            pts.push(inner_top_right); //3
                            pts.push(top_right); //4
                            pts.push(Vector3D::new(max.x , min.y , max.z )); //5 bottom right
                            pts.push(inner_top_right); //6
                            pts.push(Vector3D::new( //7 inner bottom right
                                max.x ,
                                min.y  + margin,
                                max.z  - margin
                            ));

                            let mut uv: Vec<Vector2D> = Vec::new();

                            uv.push(Vector2D::new(1.0 * abs_dimensions.z , 0.0));
                            uv.push(Vector2D::new(0.0, 0.0));
                            uv.push(Vector2D::new(1.0 * abs_dimensions.z  - margin, margin));
                            uv.push(Vector2D::new(margin, margin));

                            uv.push(Vector2D::new(1.0 * abs_dimensions.y , 0.0));
                            uv.push(Vector2D::new(0.0, 0.0));
                            uv.push(Vector2D::new(1.0 * abs_dimensions.y  - margin, margin));
                            uv.push(Vector2D::new(margin, margin));

                            for (pt, u) in pts.iter().zip(uv.iter()) {

                                uvs.push(Vector2::new(u.x, u.y));
                                verts.push(Vector3::new(pt.x, pt.y, pt.z));
                            }

                            let pts_len = pts.len();
                            for i in 0..pts_len {

                                let new_pt = pts[i] - true_center;
                                let u = uv[i];

                                let rot = Rotation3::new(Vector3D::x() * std::f32::consts::PI);
                                let rotated_pt = rot.transform_vector(&new_pt) + true_center;

                                pts.push(rotated_pt);
                                uv.push(u);

                                uvs.push(Vector2::new(u.x, u.y));
                                verts.push(Vector3::new(rotated_pt.x, rotated_pt.y, rotated_pt.z));
                            }

                            for _ in 0..pts.len() {
                                normals.push(Vector3::new(1.0, 0.0, 0.0));
                            }

                            for (pt, u) in pts.iter().zip(uv.iter()) {
                                let new_pt = pt - true_center;

                                let rot = Rotation3::new(Vector3D::y() * std::f32::consts::PI);
                                let rotated_pt = rot.transform_vector(&new_pt) + true_center;

                                uvs.push(Vector2::new(u.x, u.y));
                                verts.push(Vector3::new(rotated_pt.x, rotated_pt.y, rotated_pt.z));
                                normals.push(Vector3::new(-1.0,0.0,0.0));
                            }

                        }
                        2 => { //front and back
                            let mut pts: Vec<Vector3D> = Vec::new();

                            let top_right = Vector3D::new(max.x , max.y , min.z );
                            let inner_top_right = Vector3D::new( //inner top right
                                max.x  - margin,
                                max.y  - margin,
                                min.z 
                            );

                            pts.push(Vector3D::new(min.x , max.y , min.z )); //0 top left
                            pts.push(top_right); //1
                            pts.push(Vector3D::new( //2 inner top left
                                min.x  + margin,
                                max.y  - margin,
                                min.z 
                            ));
                            pts.push(inner_top_right); //3
                            pts.push(top_right); //4
                            pts.push(Vector3D::new(max.x , min.y , min.z )); //5 bottom right
                            pts.push(inner_top_right); //6
                            pts.push(Vector3D::new( //7 inner bottom right
                                max.x  - margin,
                                min.y  + margin,
                                min.z 
                            ));

                            let mut uv: Vec<Vector2D> = Vec::new();

                            uv.push(Vector2D::new(1.0 * abs_dimensions.x , 0.0));
                            uv.push(Vector2D::new(0.0, 0.0));
                            uv.push(Vector2D::new(1.0 * abs_dimensions.x  - margin, margin));
                            uv.push(Vector2D::new(margin, margin));

                            uv.push(Vector2D::new(1.0 * abs_dimensions.y , 0.0));
                            uv.push(Vector2D::new(0.0, 0.0));
                            uv.push(Vector2D::new(1.0 * abs_dimensions.y  - margin, margin));
                            uv.push(Vector2D::new(margin, margin));

                            for (pt, u) in pts.iter().zip(uv.iter()) {

                                uvs.push(Vector2::new(u.x, u.y));
                                verts.push(Vector3::new(pt.x, pt.y, pt.z));
                            }

                            let pts_len = pts.len();
                            for i in 0..pts_len {

                                let new_pt = pts[i] - true_center;
                                let u = uv[i];

                                let rot = Rotation3::new(Vector3D::z() * std::f32::consts::PI);
                                let rotated_pt = rot.transform_vector(&new_pt) + true_center;

                                pts.push(rotated_pt);
                                uv.push(u);

                                uvs.push(Vector2::new(u.x, u.y));
                                verts.push(Vector3::new(rotated_pt.x, rotated_pt.y, rotated_pt.z));
                            }

                            for _ in 0..pts.len() {
                                normals.push(Vector3::new(0.0, 0.0, 1.0));
                            }

                            for (pt, u) in pts.iter().zip(uv.iter()) {
                                let new_pt = pt - true_center;

                                let rot = Rotation3::new(Vector3D::y() * std::f32::consts::PI);
                                let rotated_pt = rot.transform_vector(&new_pt) + true_center;

                                uvs.push(Vector2::new(u.x, u.y));
                                verts.push(Vector3::new(rotated_pt.x, rotated_pt.y, rotated_pt.z));
                                normals.push(Vector3::new(0.0,0.0,-1.0));
                            }
                        },
                        _ => {}
                    } 

                    let mut indices: Vec<i32> = Vec::with_capacity(48);

                    //add indices for all "quads" in the face;
                    for j in 0..8 {
                        let k = offset + j*4;

                        indices.push(k+2);
                        indices.push(k+1);
                        indices.push(k);

                        indices.push(k+2);
                        indices.push(k+3);
                        indices.push(k+1);

                    }

                    //increase the offset for the next loop by the number of verts in the face before consuming verts
                    offset += verts.len() as i32;

                    mesh_data.verts.extend(verts);
                    mesh_data.normals.extend(normals);
                    mesh_data.uvs.extend(uvs);
                    mesh_data.indices.extend(indices);
 
                }

                // godot_print!("Updated selection box mesh");
                
            })

        })
    
}
    