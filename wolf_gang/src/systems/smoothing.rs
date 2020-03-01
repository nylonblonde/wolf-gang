use legion::prelude::*;

type Vector3D = nalgebra::Vector3<f32>;

//TODO: Make this generic so we can smooth out floats, other types, etc
pub struct Smoothing {
    pub current: Vector3D,
    pub heading: Vector3D,
    pub speed: f32
}

pub fn create_system() -> Box<dyn Schedulable> {
    SystemBuilder::new("smoothing_system")
        .read_resource::<crate::Time>()
        .with_query(<Write<Smoothing>>::query())
        .build(move |commands, world, time, query| {
            for (entity, mut smoothing) in query.iter_entities_mut(&mut *world) {
                smoothing.current = smoothing.current + (smoothing.heading - smoothing.current) * time.delta * smoothing.speed;

                if (smoothing.current - smoothing.heading).norm() < 1.0e-5 {
                    commands.remove_component::<Smoothing>(entity);
                }
            }
        })
}