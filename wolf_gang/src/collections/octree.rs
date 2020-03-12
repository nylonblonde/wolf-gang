use std::ops::{AddAssign, SubAssign, DivAssign};
use std::slice::Iter;

use nalgebra::{Scalar, Vector3};
use num::{Num, NumCast};
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

impl<N: Scalar + Num + NumCast + Ord + AddAssign + SubAssign + DivAssign, T: PointData<N>> IntoIterator for Octree<N, T> {
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
impl<N: Scalar + Num + NumCast + Ord + AddAssign + SubAssign + DivAssign, T: PointData<N>> Octree<N, T> {

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

    fn subdivide(&mut self) {

        let min = self.aabb.get_min();
        let max = self.aabb.get_max();

        let smaller_half = self.aabb.dimensions/NumCast::from(2).unwrap();
        let larger_half = self.aabb.dimensions - smaller_half;

        //This gets very complicated as we need to account for if the format is integer, and can't
        //be halved properly. Any max values have to be the full dimensions minus the small side's max

        let downbackleft = AABB::<N>::new(
            self.aabb.center + (min - self.aabb.center)/NumCast::from(2).unwrap(), 
            larger_half
        );

        println!("subdividing at center {:?} : {:?} {:?}", self.aabb.center, min, max);
        println!("larger_half.x: {:?}, smaller_half.x: {:?}", larger_half.x, smaller_half.x);
                
        let downbackright = AABB::<N>::new(
            self.aabb.center + (Vector3::new(max.x, min.y, min.z) - self.aabb.center)/NumCast::from(2).unwrap(), 
            Vector3::new(
                smaller_half.x, larger_half.y, larger_half.z
            )
        );

        let downforwardleft = AABB::<N>::new(
            self.aabb.center + (Vector3::new(min.x, min.y, max.z) - self.aabb.center)/NumCast::from(2).unwrap(), 
            Vector3::new(
                larger_half.x, larger_half.y, smaller_half.z
            )
        );

        let downforwardright = AABB::<N>::new(
            self.aabb.center + (Vector3::new(max.x, min.y, max.z) - self.aabb.center)/NumCast::from(2).unwrap(), 
            Vector3::new(
                smaller_half.x, larger_half.y, smaller_half.z
            )
        );

        let upbackleft = AABB::<N>::new(
            self.aabb.center + (Vector3::new(min.x, max.y, min.z) - self.aabb.center)/NumCast::from(2).unwrap(), 
            Vector3::new(
                larger_half.x, smaller_half.y, larger_half.z
            )
        );

        let upbackright = AABB::<N>::new(
            self.aabb.center + (Vector3::new(max.x, max.y, min.z) - self.aabb.center)/NumCast::from(2).unwrap(), 
            Vector3::new(
                smaller_half.x, smaller_half.y, larger_half.z
            )
        );

        let upforwardleft = AABB::<N>::new(
            self.aabb.center + (Vector3::new(min.x, max.y, max.z) - self.aabb.center)/NumCast::from(2).unwrap(), 
            Vector3::new(
                larger_half.x, smaller_half.y, smaller_half.z
            )
        );

        let upforwardright = AABB::<N>::new(
            self.aabb.center + (max - self.aabb.center)/NumCast::from(2).unwrap(), 
            smaller_half
        );

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

        if !self.aabb.contains_point(element.get_point()) {
            // println!("{:?} didn't fit between {:?} and {:?}",element.get_point(), self.aabb.get_min(), self.aabb.get_max());
            return false
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

    pub fn query_range(self, range: AABB<N>) -> Vec<T> {

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
                Some(_) => {
                    let el = element.unwrap();
                    if self.aabb.contains_point(el.get_point()) {
                        elements_in_range.push(el);
                    }
                } 
            }
        }

        if let Paternity::ChildFree = self.paternity {
            return elements_in_range
        }

        for child_option in self.children {
            if let Some(_) = child_option {
                elements_in_range.append(&mut child_option.unwrap().query_range(range));
            }
        }

        elements_in_range
    }
}
