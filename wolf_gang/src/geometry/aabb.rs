use nalgebra::{Vector3, Scalar};
use num::{
    Num, NumCast,Signed
};

#[derive(Clone, Copy)]
pub struct AABB<F: Scalar> {
    pub center: Vector3<F>,
    pub dimensions: Vector3<F>,
}

impl<F: Signed + Scalar + Num + NumCast + Ord> AABB<F> {
    pub fn new(center: Vector3<F>, dimensions: Vector3<F>) -> Self {
        Self {
            center,
            dimensions
        }
    }

    pub fn get_min(&self) -> Vector3<F> {

        let max = self.get_max();

        Vector3::new(
            max.x - self.dimensions.x.abs(),
            max.y - self.dimensions.y.abs(),
            max.z - self.dimensions.z.abs()
        )
    }

    pub fn get_max(&self) -> Vector3<F> {
        //We perform this match in case our format is Int, and 1/2 == 0
        let x = match self.center.x + self.dimensions.x.abs()/ NumCast::from(2).unwrap() {
            x if self.center.x == x => { self.center.x + self.dimensions.x.abs() },
            x => x
        };
        let y = match self.center.y + self.dimensions.y.abs()/ NumCast::from(2).unwrap() {
            y if self.center.y == y => { self.center.y + self.dimensions.y.abs() },
            y => y
        };
        let z = match self.center.z + self.dimensions.z.abs()/ NumCast::from(2).unwrap() {
            z if self.center.z == z => { self.center.z + self.dimensions.z.abs() },
            z => z
        };
        Vector3::new(
            x,y,z
        )
    }

    pub fn intersects_bounds(&self, other: AABB<F>) -> bool{
        let min = self.get_min();
        let max = self.get_max();

        let other_min = other.get_min();
        let other_max = other.get_max();

        min.x <= other_max.x && max.x >= other_min.x
        && min.y <= other_max.y && max.y >= other_min.y
        && min.z <= other_max.z && max.z >= other_min.z
        
    }

    pub fn contains_point(&self, point: Vector3<F>) -> bool {
        let min = self.get_min();
        let max = self.get_max();

        point.x >= min.x && point.x <= max.x
        && point.y >= min.y && point.y <= max.y
        && point.z >= min.z && point.z <= max.z
    }
}