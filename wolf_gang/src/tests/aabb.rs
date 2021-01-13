use crate::geometry::aabb;

type AABB = aabb::AABB<i32>;
type Point = nalgebra::Vector3<i32>;

#[test]
fn test_intersection() {
    let aabb1 = AABB::from_extents(Point::new(0,0,0), Point::new(3,3,3));
    let aabb2 = AABB::from_extents(Point::new(-1,-1,-1), Point::new(2,2,2));

    assert_eq!(AABB::from_extents(Point::new(0,0,0), Point::new(2,2,2)), aabb1.get_intersection(aabb2));
}