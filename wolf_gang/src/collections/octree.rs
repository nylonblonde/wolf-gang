use std::ops::{AddAssign, SubAssign, DivAssign};

use std::collections::HashMap;

use nalgebra::{Scalar, Vector3};
use num::{Num, NumCast, Zero};
use crate::geometry::aabb::AABB;

pub trait PointData<N: Scalar> : Copy {
    fn get_point(&self) -> Vector3<N>;
}

#[allow(dead_code)]
enum Paternity {
    ProudParent,
    ChildFree
}

#[allow(dead_code)]
pub struct Octree <N: Scalar, T: PointData<N>>{
    aabb: AABB<N>,
    num_elements: u32,
    elements: [Option<T>; 8],
    children: Vec<Option<Octree<N, T>>>,
    paternity: Paternity
}

#[allow(dead_code)]
impl<N: Scalar + Num + NumCast + Ord + AddAssign + SubAssign + DivAssign, T: PointData<N>> Octree<N, T> {
    const MAX_SIZE: u32 = 8;

    pub fn new(aabb: AABB<N>) -> Octree<N, T> {
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
        
        println!("Subdividing {:?} ", self.aabb.center);

        let min = self.aabb.get_min();
        let max = self.aabb.get_max();

        let half_dimensions = self.aabb.dimensions/NumCast::from(2).unwrap();

        let downbackleft = AABB::<N>::new(
            self.aabb.center + (min - self.aabb.center)/NumCast::from(2).unwrap()
            , half_dimensions);

        let downbackright = AABB::<N>::new(
            self.aabb.center + (Vector3::new(max.x, min.y, min.z) - self.aabb.center)/NumCast::from(2).unwrap()
            , half_dimensions);

        let downforwardleft = AABB::<N>::new(
            self.aabb.center + (Vector3::new(min.x, min.y, max.z) - self.aabb.center)/NumCast::from(2).unwrap()
            , half_dimensions);

        let downforwardright = AABB::<N>::new(
            self.aabb.center + (Vector3::new(max.x, min.y, max.z) - self.aabb.center)/NumCast::from(2).unwrap()
            , half_dimensions);

        let upbackleft = AABB::<N>::new(
            self.aabb.center + (Vector3::new(min.x, max.y, min.z) - self.aabb.center)/NumCast::from(2).unwrap()
            , half_dimensions);

        let upbackright = AABB::<N>::new(
            self.aabb.center + (Vector3::new(max.x, max.y, min.z) - self.aabb.center)/NumCast::from(2).unwrap()
            , half_dimensions);

        let upforwardleft = AABB::<N>::new(
            self.aabb.center + (Vector3::new(min.x, max.y, max.z) - self.aabb.center)/NumCast::from(2).unwrap()
            , half_dimensions);

        let upforwardright = AABB::<N>::new(
            self.aabb.center + (max - self.aabb.center)/NumCast::from(2).unwrap()
            , half_dimensions);

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
            return false
        }
        
        match &self.paternity { //do first match because you still need to insert into children after subdividing, not either/or

            Paternity::ChildFree if self.num_elements < Octree::<N, T>::MAX_SIZE => {
                self.elements[self.num_elements as usize] = Some(element);
                println!("Inserted {:?} as element number {} in center: {:?}, dimensions: {:?}", self.elements[self.num_elements as usize].unwrap().get_point(), self.num_elements, self.aabb.center, self.aabb.dimensions);

                self.num_elements = self.num_elements + 1;

                return true;
            }

            Paternity::ChildFree => { 
                self.subdivide();
                return false;
            }
            
            _ => {}

        };

        return match &self.paternity {

            Paternity::ProudParent => {
                for i in 0..self.children.len() {
                    
                    if self.children[i].as_mut().unwrap().insert(element) == true {
                        return true
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

    pub fn query_range(&mut self, range: AABB<N>) -> Vec<T> {

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

        for child_option in &mut self.children {
            if let Some(_) = child_option {
                elements_in_range.append(&mut child_option.as_mut().unwrap().query_range(range));
            }
        }

        elements_in_range
    }
}
