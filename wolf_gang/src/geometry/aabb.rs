use crate::Point;

pub struct Int32AABB {
    center: Point,
    dimensions: Point,
}

impl Int32AABB {
    pub fn new(center: Point, dimensions: Point) -> Self {
        Int32AABB {
            center,
            dimensions
        }
    }
}