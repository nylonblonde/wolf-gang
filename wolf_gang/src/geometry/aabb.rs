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

    pub fn get_extents_from_matrix(&self, matrix: nalgebra::MatrixMN<F, nalgebra::dimension::U4, nalgebra::dimension::U3>) -> (Vector3<F>, Vector3<F>) {

        unimplemented!()

    }

    fn get_corners<T: nalgebra::SimdRealField + NumCast>(&self) -> [Vector3<T>; 8] {
        let min = self.get_min();
        let max = self.get_max();

        let min_x = NumCast::from(min.x).unwrap();
        let min_y = NumCast::from(min.y).unwrap();
        let min_z = NumCast::from(min.z).unwrap();

        let max_x = NumCast::from(max.x).unwrap();
        let max_y = NumCast::from(max.y).unwrap();
        let max_z = NumCast::from(max.z).unwrap();

        [
            nalgebra::Vector3::new(min_x, min_y, min_z),
            nalgebra::Vector3::new(min_x, max_y, min_z),
            nalgebra::Vector3::new(min_x, min_y, max_z),
            nalgebra::Vector3::new(min_x, max_y, max_z),

            nalgebra::Vector3::new(max_x, min_y, min_z),
            nalgebra::Vector3::new(max_x, max_y, min_z),
            nalgebra::Vector3::new(max_x, min_y, max_z),
            nalgebra::Vector3::new(max_x, max_y, max_z)
        ]
    }

    pub fn rotate<T: nalgebra::SimdRealField + NumCast>(&self, rotation: nalgebra::Rotation3<T>) -> Self {
        let corners = self.get_corners();

        let rotated_corners = corners.iter().map(|corner| rotation * *corner).collect::<Vec<Vector3<T>>>();
        let mut rotated_corners_iter = rotated_corners.iter();

        println!("{:#?}", rotated_corners);

        if let Some(corner) = rotated_corners_iter.next() {
            let mut min_x: F = NumCast::from(corner.x).unwrap();
            let mut min_y: F = NumCast::from(corner.y).unwrap();
            let mut min_z: F = NumCast::from(corner.z).unwrap();

            let mut max_x: F = NumCast::from(corner.x).unwrap();
            let mut max_y: F = NumCast::from(corner.y).unwrap();
            let mut max_z: F = NumCast::from(corner.z).unwrap();

            while let Some(corner) = rotated_corners_iter.next() {
                min_x = std::cmp::min(min_x, NumCast::from(corner.x).unwrap());
                min_y = std::cmp::min(min_y, NumCast::from(corner.y).unwrap());
                min_z = std::cmp::min(min_z, NumCast::from(corner.z).unwrap());
    
                max_x = std::cmp::max(max_x, NumCast::from(corner.x).unwrap());
                max_y = std::cmp::max(max_y, NumCast::from(corner.y).unwrap());
                max_z = std::cmp::max(max_z, NumCast::from(corner.z).unwrap());
            }

            let min = Vector3::<F>::new(
                NumCast::from(min_x).unwrap(), NumCast::from(min_y).unwrap(), NumCast::from(min_z).unwrap()
            );

            let max = Vector3::<F>::new(
                NumCast::from(max_x).unwrap(), NumCast::from(max_y).unwrap(), NumCast::from(max_z).unwrap()
            );

            return AABB::from_extents(min, max)
        }

        AABB::from_extents(Vector3::zeros(), Vector3::zeros())
    }

    pub fn get_intersection(&self, other: AABB<F>) -> AABB<F> {
        let min = self.get_min();
        let max = self.get_max();

        let other_min = other.get_min();
        let other_max = other.get_max();

        let intersect_min = Vector3::<F>::new(
            std::cmp::max(min.x, other_min.x),
            std::cmp::max(min.y, other_min.y),
            std::cmp::max(min.z, other_min.z)
        );

        let intersect_max = Vector3::<F>::new(
            std::cmp::min(max.x, other_max.x),
            std::cmp::min(max.y, other_max.y),
            std::cmp::min(max.z, other_max.z)
        );

        AABB::from_extents(intersect_min, intersect_max)
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