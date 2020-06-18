use serde::{Serialize, Deserialize};
use nalgebra::{Vector3, Scalar};
use num::{
    Num, NumCast,Signed
};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Copy)]
pub struct AABB<F: Scalar> {
    pub center: Vector3<F>,
    pub dimensions: Vector3<F>,
}

impl<F: Signed + Scalar + Num + NumCast + Ord + Copy + Clone> AABB<F> {
    pub fn new(center: Vector3<F>, dimensions: Vector3<F>) -> Self {
        Self {
            center,
            dimensions
        }
    }

    pub fn from_extents(min: Vector3<F>, max: Vector3<F>) -> Self {
        let two: F = NumCast::from(2).unwrap();
        let one: F = NumCast::from(1).unwrap();
        let zero: F = NumCast::from(0).unwrap();

        let mut dimensions = Vector3::new(
            max.x - min.x,
            max.y - min.y,
            max.z - min.z
        );

        //hacky way to check if F is int, since max is inclusive
        if one/two == zero {
            dimensions.x = dimensions.x + one;
            dimensions.y = dimensions.y + one;
            dimensions.z = dimensions.z + one;
        }

        let center = Vector3::new(
            min.x + dimensions.x/two,
            min.y + dimensions.y/two,
            min.z + dimensions.z/two
        );

        Self {
            center,
            dimensions
        }
    }

    pub fn get_min(&self) -> Vector3<F> {

        let dimensions = self.dimensions.abs();
        let two: F = NumCast::from(2).unwrap();
        Vector3::new(
            self.center.x - dimensions.x/two,
            self.center.y - dimensions.y/two,
            self.center.z - dimensions.z/two,
        )
    }

    pub fn get_max(&self) -> Vector3<F> {

        let dimensions = self.dimensions.abs();
        let two: F = NumCast::from(2).unwrap();
        let one: F = NumCast::from(1).unwrap();
        let zero: F = NumCast::from(0).unwrap();
        let min = self.get_min();

        let mut max = Vector3::new(
            min.x + dimensions.x,
            min.y + dimensions.y,
            min.z + dimensions.z
        );

        //hacky way to check if F is int, since max is inclusive
        if one/two == zero {
            max.x = max.x - one;
            max.y = max.y - one;
            max.z = max.z - one;
        }

        max
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