use core::fmt::Debug;
use std::ops::{AddAssign, SubAssign, DivAssign};
use std::slice::Iter;

use nalgebra::{Scalar, Vector3};
use num::{Num, NumCast, Signed};
use crate::geometry::aabb::AABB;

pub trait PointData<N: Scalar> : Copy {
    fn get_point(&self) -> Vector3<N>;
}

#[derive(Copy, Clone)]
#[allow(dead_code)]
enum Paternity {
    ProudParent,
    ChildFree
}

pub struct OctreeIter <N: Scalar, T: PointData<N>> {
    elements: std::vec::IntoIter<T>,
    phantom: std::marker::PhantomData<N>
}

impl<'a, N: Scalar, T: PointData<N>> Iterator for OctreeIter<N, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        match self.elements.next() {
            Some(r) => Some(r),
            None => None
        }
    }
}

impl<N: Signed + Scalar + Num + NumCast + Ord + AddAssign + SubAssign + DivAssign, T: PointData<N> + Debug> IntoIterator for Octree<N, T> {
    type Item = T;
    type IntoIter = OctreeIter<N, T>;
    fn into_iter(self) -> Self::IntoIter {
        let aabb = self.aabb;
        OctreeIter{
            elements: self.query_range(aabb).into_iter(),
            phantom: std::marker::PhantomData
        }
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct Octree <N: Scalar, T: PointData<N>>{
    aabb: AABB<N>,
    num_elements: usize,
    elements: [Option<T>; 8],
    children: Vec<Option<Octree<N, T>>>,
    paternity: Paternity
}

#[allow(dead_code)]
impl<N: Signed + Scalar + Num + NumCast + Ord + AddAssign + SubAssign + DivAssign, T: PointData<N> + Debug> Octree<N, T> {

    pub fn new(aabb: AABB<N>) -> Octree<N, T> {
        println!("Creating new Octree with a min of {:?} and a max of {:?}", aabb.get_min(), aabb.get_max());

        Octree {
            aabb,
            num_elements: 0,
            elements: [Option::<T>::None; 8],
            children: vec![None, None, None, None, 
                            None, None, None, None], //Going ahead and allocating for the vector
            paternity: Paternity::ChildFree
        }
    }

    pub fn get_aabb(&self) -> AABB<N> {
        self.aabb
    } 

    fn subdivide(&mut self) {

        let one: N = NumCast::from(1).unwrap();
        let two: N = NumCast::from(2).unwrap();

        let min = self.aabb.get_min();
        let max = self.aabb.get_max();

        let dimensions = self.aabb.dimensions.abs();

        let smaller_half = dimensions/two;
        let larger_half = dimensions - smaller_half;

        println!("subdividing at center {:?} : {:?} {:?}", self.aabb.center, min, max);
        //down back left
        let sub_max = min + (self.aabb.center - min);
        let downbackleft = AABB::<N>::from_extents(
            min,
            sub_max
        );

        println!("downbackleft min {:?} max {:?}", downbackleft.get_min(), downbackleft.get_max());

        //down back right
        let sub_min = Vector3::new(min.x + downbackleft.dimensions.x, min.y, min.z);
        let sub_max = Vector3::new(
            max.x, sub_min.y + self.aabb.center.y - min.y, sub_min.z + self.aabb.center.z - min.z
        );

        let downbackright = AABB::<N>::from_extents(
            sub_min,
            sub_max
        );

        println!("downbackright min {:?} max {:?}", downbackright.get_min(), downbackright.get_max());

        //down forward left
        let sub_min = Vector3::new(min.x, min.y, min.z + downbackleft.dimensions.z);
        let sub_max = Vector3::new(
            sub_min.x + self.aabb.center.x - min.x, sub_min.y + self.aabb.center.y - min.y, max.z
        );

        let downforwardleft = AABB::<N>::from_extents(
            sub_min, 
            sub_max
        );

        println!("downforwardleft min {:?} max {:?}", downforwardleft.get_min(), downforwardleft.get_max());

        let sub_min = Vector3::new(min.x + downbackleft.dimensions.x, min.y, min.z + downbackleft.dimensions.z);
        let sub_max = Vector3::new(
            max.x, sub_min.y + self.aabb.center.y - min.y, max.z
        );
        let downforwardright = AABB::<N>::from_extents(
            sub_min,
            sub_max
        );

        println!("downforwardright min {:?} max {:?}", downforwardright.get_min(), downforwardright.get_max());

        let sub_min = Vector3::new(min.x, min.y + downbackleft.dimensions.y, min.z);
        let sub_max = Vector3::new(
            sub_min.x + self.aabb.center.x - min.x, max.y, sub_min.z + self.aabb.center.z - min.z
        );
        let upbackleft = AABB::<N>::from_extents(
            sub_min,
            sub_max
        );

        println!("upbackleft min {:?} max {:?}", upbackleft.get_min(), upbackleft.get_max());

        let sub_min = Vector3::new(min.x + downbackleft.dimensions.x, min.y + downbackleft.dimensions.y, min.z);
        let sub_max = Vector3::new(
            max.z, max.y, sub_min.z + self.aabb.center.z - min.z
        );
        let upbackright = AABB::<N>::from_extents(
            sub_min,
            sub_max
        );

        println!("upbackright min {:?} max {:?}", upbackright.get_min(), upbackright.get_max());
        
        let sub_min = Vector3::new(min.x, min.y + downbackleft.dimensions.y, min.z + downbackleft.dimensions.z);
        let sub_max = Vector3::new(
            sub_min.x + self.aabb.center.x - min.x, max.y, max.z
        );
        let upforwardleft = AABB::<N>::from_extents(
            sub_min,
            sub_max
        );

        println!("upforwardleft min {:?} max {:?}", upforwardleft.get_min(), upforwardleft.get_max());

        let sub_min = min + downbackleft.dimensions;
        let upforwardright = AABB::<N>::from_extents(
            sub_min,
            max
        );

        println!("upforwardright min {:?} max {:?}", upforwardright.get_min(), upforwardright.get_max());

        self.children[0] = Some(Octree::new(downbackleft));
        self.children[1] = Some(Octree::new(downbackright));
        self.children[2] = Some(Octree::new(downforwardleft));
        self.children[3] = Some(Octree::new(downforwardright));
        self.children[4] = Some(Octree::new(upbackleft));
        self.children[5] = Some(Octree::new(upbackright));
        self.children[6] = Some(Octree::new(upforwardleft));
        self.children[7] = Some(Octree::new(upforwardright));
        
        self.paternity = Paternity::ProudParent;

    }

    pub fn insert(&mut self, element: T) -> bool{  

        let pt = element.get_point();

        if !self.aabb.contains_point(pt) {
            // println!("{:?} didn't fit between {:?} and {:?}",element.get_point(), self.aabb.get_min(), self.aabb.get_max());
            return false
        }

        //if element already exists at point, replace it
        for el in &mut self.elements {
            match el {
                Some(r) if r.get_point() == pt => {
                    *el = Some(*r);
                    return true
                },
                _ => {}
            }
        }
        
        match &self.paternity { //do first match because you still need to insert into children after subdividing, not either/or

            Paternity::ChildFree if self.num_elements < self.elements.len() => {
                self.elements[self.num_elements as usize] = Some(element);
                self.num_elements = self.num_elements + 1;
                // println!("Inserted {:?} between {:?} and {:?} at position {}", element.get_point(), self.aabb.get_min(), self.aabb.get_max(), self.num_elements);

                return true;
            }

            Paternity::ChildFree => { 
                self.subdivide();
            }
            
            _ => {}

        };

        return match &self.paternity {

            Paternity::ProudParent => {
                for i in 0..self.children.len() {
                    
                    //only return true if true because we need to check all of them
                    if self.children[i].as_mut().unwrap().insert(element) == true {
                        return true;
                    }
                            
                }

                false
            }

            _ => false
        }
    }

    pub fn count(&self) -> usize{
        let mut count: usize = 0;

        for el in &self.elements {
            match el {
                Some(_) => {count += 1},
                None => {}
            }
        }

        return match &self.paternity {
            Paternity::ChildFree => count,
            Paternity::ProudParent => {
                for child in &self.children {
                    count += child.as_ref().unwrap().count();
                }
                count
            }
        }
    }

    pub fn query_point(&self, point: Vector3<N>) -> Option<T> {

        if !self.aabb.contains_point(point){
            return None;
        }

        for &element in self.elements.iter() {

            match element {
                Some(el) => {
                    if el.get_point() == point  {
                        return element
                    }
                },
                _ => continue
            }
        }

        if let Paternity::ChildFree = self.paternity {
            return None
        }


        for child_option in &self.children {
            if let Some(child) = child_option {
                let child_query = child.query_point(point);
                println!("{:?}", child_query);

                if child_query.is_some() {

                    return child_query; 
                }
            }
        }

        None
    }

    pub fn query_range(&self, range: AABB<N>) -> Vec<T> {

        let mut elements_in_range = vec![];
        
        if !self.aabb.intersects_bounds(range) {
            return elements_in_range
        }

        if self.num_elements == 0 {
            return elements_in_range
        }

        for &element in self.elements.iter() {

            match element {
                None => continue,
                Some(el) => {
                    if self.aabb.contains_point(el.get_point()) {
                        elements_in_range.push(el);
                    }
                } 
            }
        }

        if let Paternity::ChildFree = self.paternity {
            return elements_in_range
        }

        for child_option in &self.children {
            if let Some(child) = child_option {
                elements_in_range.append(&mut child.query_range(range));
            }
        }

        elements_in_range
    }
}
