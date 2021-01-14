use gdnative::prelude::*;

use gdnative::api::{
    Camera,
};

use legion::*;

use crate::systems::{
    selection_box,
    smoothing::Smoothing,
    transform::{
        position::Position,
        rotation::{Rotation, Direction}
    },
    input::{ Action, InputActionComponent },
    level_map
};

use crate::node;

type Vector3D = nalgebra::Vector3<f32>;
type Rotation3D = nalgebra::Rotation3<f32>;

#[derive(Copy, Clone)]
pub struct FocalPoint(pub Vector3D);

impl Default for FocalPoint {
    fn default() -> Self {
        FocalPoint(Vector3D::zeros())
    }
}

pub struct FocalAngle(pub f32, pub f32, pub f32);

pub struct Zoom(pub f32);

impl Default for Zoom {
    fn default() -> Self {
        Zoom(10.)
    }
}

const SPEED : f32 = 4.;

pub fn initialize_camera(world: &mut legion::world::World) -> Ref<Node> {
    
    let camera = Camera::new();

    let owner = unsafe { crate::OWNER_NODE.unwrap().assume_safe() };

    let node = unsafe { node::add_node(&owner, camera.upcast()) };

    unsafe { node.assume_safe().cast::<Camera>().unwrap().make_current(); }

    world.push(
        (
            node::NodeRef::new(node),
            Position::default(),
            FocalAngle(-45.0f32.to_radians(),225.0f32.to_radians(), 0.0),
            Rotation::default(),
            Direction::default(),
            FocalPoint::default(),
            Zoom::default(),
        )
    );

    node
}

pub fn create_movement_system() -> impl systems::Runnable {
    SystemBuilder::new("camera_movement_system")
    .with_query(<(Read<FocalPoint>, Read<FocalAngle>, Read<Zoom>, Write<Position>)>::query()
        .filter(maybe_changed::<FocalPoint>() | maybe_changed::<Zoom>() | maybe_changed::<FocalAngle>())
    )
    .build(move |_, world, _, query|{
        query.for_each_mut(world, |(focal_point, focal_angle, zoom, mut position)| {

            position.value = focal_point.0 + (Rotation3D::from_euler_angles(
                focal_angle.0, 
                focal_angle.1, 
                focal_angle.2
            ) * (Vector3D::z() * zoom.0));
        })
    })
}

pub fn create_rotation_system() -> impl systems::Runnable {
    SystemBuilder::new("camera_rotation_system")
    .with_query(<(Read<FocalPoint>, Read<Position>, Write<Rotation>)>::query()
        .filter(maybe_changed::<Position>())
    )
    .build(move |_, world, _, query|{
        
        query.for_each_mut(world, |(focal_point, position, mut rotation)| {

            let dir = Vector3D::new(position.value.x, position.value.y, position.value.z) - focal_point.0;

            let up = Vector3D::y();

            let rot = Rotation3D::face_towards(&dir, &up);
            
            rotation.value = rot;

        })
    })
}

/// Handles the input for rotating the camera around the focal point
pub fn create_camera_angle_system() -> impl systems::Runnable {
    let camera_rotate_left = Action("camera_rotate_left".to_string());
    let camera_rotate_right = Action("camera_rotate_right".to_string());
    let camera_rotate_up = Action("camera_rotate_up".to_string());
    let camera_rotate_down = Action("camera_rotate_down".to_string());

    SystemBuilder::new("camera_angle_system")
        .with_query(<(Read<InputActionComponent>, Read<Action>)>::query())
        .with_query(<Write<FocalAngle>>::query())
        .read_resource::<crate::Time>()
        .build(move |_, world, time, queries| {

            let (input_query, cam_query) = queries;
            
            let inputs = input_query.iter(world)
                .map(|(input, action)| (*input, (*action).clone()))
                .collect::<Vec<(InputActionComponent, Action)>>();
            
            for(input_component, action) in inputs.iter().filter(|(_, a)| {
                a == &camera_rotate_left ||
                a == &camera_rotate_right ||
                a == &camera_rotate_up ||
                a == &camera_rotate_down
            }) {                    
                
                cam_query.for_each_mut(world, |mut focal_angle| {
                    if action.0 == camera_rotate_left.0 {
                        focal_angle.1 -= input_component.strength as f32 * time.delta * SPEED;
                    } else if action.0 == camera_rotate_right.0 {
                        focal_angle.1 += input_component.strength as f32 * time.delta * SPEED;
                    } else if action.0 == camera_rotate_up.0 {
                        focal_angle.0 -= input_component.strength as f32 * time.delta * SPEED;
                    } else if action.0 == camera_rotate_down.0 {
                        focal_angle.0 += input_component.strength as f32 * time.delta * SPEED;
                    }

                    let min = -(std::f32::consts::FRAC_PI_2 - 0.001);
                    if focal_angle.0 < min {
                        focal_angle.0 = min
                    } else if focal_angle.0 > 0. {
                        focal_angle.0 = 0.
                    }
                })
            }
        })
}

///Updates the focal point of the camera when a smoothing entity is present
pub fn create_focal_point_system() -> impl systems::Runnable {

    SystemBuilder::new("camera_focal_point_system")
        .with_query(<Read<selection_box::RelativeCamera>>::query()
            .filter(component::<selection_box::Active>()))
        .with_query(<(Read<Smoothing>, Read<node::NodeRef>, Write<FocalPoint>)>::query())
        .build(|_, world, _, queries| {

            let (selection_box_query, cam_query) = queries;

            let selection_boxes = selection_box_query.iter(world)
                .map(|relative_cam| relative_cam.val())
                .collect::<Vec<Ref<Node>>>();

            for relative_cam in selection_boxes.iter() {

                if let Some((smoothing, _, mut focal_point)) = cam_query.iter_mut(world).find(|(_,node_ref,_)| node_ref.val() == *relative_cam) {
                    focal_point.0 = smoothing.current;
                }
            }
        })
}

/// Adds a smoothing component that will handle smoothing between the selection box's position and the current focal point
pub fn create_follow_selection_box_system() -> impl systems::Runnable {

    SystemBuilder::new("follow_selection_box_system")
        .with_query(<(Read<selection_box::RelativeCamera>, Read<level_map::CoordPos>)>::query()
            .filter(maybe_changed::<level_map::CoordPos>())
        )
        .with_query(<(Entity, Read<FocalPoint>, Read<node::NodeRef>)>::query())
        .build(|commands, world, _, queries| {

            let (selection_box_query, cam_query) = queries;

            selection_box_query.for_each(world, |(relative_cam, coord_pos)| {

                for (entity, focal_point, _) in cam_query.iter(world).filter(|(_, _, node_ref)| node_ref.val() == relative_cam.val()) {
                    let center = level_map::map_coords_to_world(coord_pos.value);

                    let min = Vector3D::zeros();
                    let max = Vector3D::new(1.,1.,1.);

                    let mid = (max + min)/2.;

                    let heading = center + mid;

                    let entity = *entity;
                    let focal_point = *focal_point;

                    commands.exec_mut(move |world, _| {
                        if let Some(mut entry) = world.entry(entity) {
                            let smoothing = entry.get_component_mut::<Smoothing>();
                            match smoothing {
                                Ok(mut smoothing) => {
                                    smoothing.heading = heading;
                                    return {}
                                },
                                _ => {
                                    entry.add_component(
                                        Smoothing{
                                            current: focal_point.0,
                                            heading,
                                            speed: SPEED
                                        }
                                    )
                                }
                            }
                        }

                    });

                }
            });
        })
}