use nalgebra::Vector3;

use crate::collections::octree::Octree;
use crate::geometry::aabb;
use crate::level_map::TileData;

type Point = Vector3<i32>;
type AABB = aabb::AABB<i32>;

#[test]
fn even_subdivision() {

    let aabb = AABB::new(
        Point::new(0,0,0), Point::new(4,4,4)
    );

    let mut octree = Octree::<i32, TileData>::new(
        aabb
    );

    let mut count = 0;
    fill_octree(aabb, &mut octree, &mut count);
    
    assert_eq!(octree.count(), count);
}

#[test]
fn odd_subdivision() {

    let test = AABB::new(Vector3::new(-1,1,-1), Vector3::new(2,3,2));

    println!("{:?} {:?}", test.get_min(), test.get_max());

    let aabb = AABB::new(
        Point::new(0,0,0), Point::new(5,5,5)
    );

    let mut octree = Octree::<i32, TileData>::new(
        aabb
    );

    let mut count = 0;
    fill_octree(aabb, &mut octree, &mut count);
    
    assert_eq!(octree.count(), count);
}

fn fill_octree(aabb: AABB, octree: &mut Octree<i32, TileData>, count: &mut usize) {
    let min = aabb.get_min();
    let max = aabb.get_max();

    for z in min.z..max.z {
        for y in min.y..max.y {
            for x in min.x..max.x { 
                *count += 1;
                if octree.insert(TileData::new(Point::new(x,y,z))) == false {
                    panic!("Failed to insert at ({}, {}, {})", x,y,z);
                }
            }
        }
    }

}