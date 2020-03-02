use gdnative::{godot_print, Basis, Vector3, Spatial};

use legion::prelude::*;

type Vector3D = nalgebra::Vector3<f32>;
type Rotation3D = nalgebra::Rotation3<f32>;

use crate::node;

pub struct Rotation {
    pub value: Rotation3D
}

impl Default for Rotation {
    fn default() -> Self {
        Rotation {
            value: Rotation3D::new(Vector3D::new(0.,0.,0.))
        }
    }
}

pub struct Direction {
    pub right: Vector3D,
    pub up: Vector3D,
    pub forward: Vector3D
}

impl Default for Direction {
    fn default() -> Self {
        Direction {
            right: Vector3D::x(),
            up: Vector3D::y(),
            forward: Vector3D::z()
        }
    }
}

pub fn create_system_local() -> Box<dyn Runnable> {
    SystemBuilder::new("rotation_system")
    .with_query(<(Read<Rotation>, Write<Direction>, Tagged<node::NodeName>)>::query()
        .filter(changed::<Rotation>())
    )
    .build_thread_local(move |_, world, _, query| {

        for (rotation, mut direction, node_name) in query.iter_mut(&mut *world) {
            let spatial_node : Option<Spatial> = {
                unsafe {
                    match node::find_node(node_name.0.clone()) {
                        Some(r) => {
                            r.cast()
                        },
                        None => {
                            godot_print!("Can't find {:?}", node_name.0);                            

                            None
                        }
                    }
                }
                
            };

            match spatial_node {
                Some(mut r) => { 

                    direction.right = rotation.value * Vector3D::x();
                    direction.up = rotation.value * Vector3D::y();
                    direction.forward = rotation.value * Vector3D::z();

                    let unit_quat: nalgebra::UnitQuaternion<f32> = rotation.value.into();
                    let quat = unit_quat.quaternion();

                    //Create a basis from Quaternion, jacked from the Godot source since there are
                    //no bindings
                    let d = quat.norm_squared();
                    let s = 2.0 / d;
                    let xs = quat.coords.x * s;
                    let ys = quat.coords.y * s;
                    let zs = quat.coords.z * s;
                    
                    let wx = quat.coords.w * xs;
                    let wy = quat.coords.w * ys;
                    let wz = quat.coords.w * zs;

                    let xx = quat.coords.x * xs;
                    let xy = quat.coords.x * ys;
                    let xz = quat.coords.x * zs;

                    let yy = quat.coords.y * ys;
                    let yz = quat.coords.y * zs;
                    let zz = quat.coords.z * zs;

                    let mut x_axis = Vector3::new(1.0 - (yy + zz), xy - wz, xz + wy);
                    let mut y_axis = Vector3::new(xy + wz, 1.0 - (xx + zz), yz - wx);
                    let mut z_axis = Vector3::new(xz - wy, yz + wx, 1.0 - (xx + yy));

                    //orthonormalize the axes, again ripped from the Godot source
                    x_axis = x_axis.normalize();
                    y_axis = (y_axis - x_axis * (x_axis.dot(y_axis))).normalize();
                    z_axis = (z_axis - x_axis * (x_axis.dot(z_axis)) - y_axis * y_axis.dot(z_axis)).normalize();

                    unsafe {
                        let mut transform = r.get_transform();
                        transform.basis = Basis{
                            elements: [
                                x_axis,
                                y_axis,
                                z_axis
                            ]
                        };
                        
                        r.set_transform(transform);
                    }

                },

                   
                None => {}
            }
        }
    })
}