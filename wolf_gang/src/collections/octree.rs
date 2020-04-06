use core::fmt::Debug;
use std::ops::{AddAssign, SubAssign, DivAssign};

use std::error;
use std::fmt;

use nalgebra::{Scalar, Vector3};
use num::{Num, NumCast, Signed};
use crate::geometry::aabb::AABB;

pub trait PointData<N: Scalar> : Copy {
    fn get_point(&self) -> Vector3<N>;
}

#[derive(Debug, Copy, Clone)]
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

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Octree <N: Scalar, T: PointData<N>>{
    aabb: AABB<N>,
    num_elements: usize,
    elements: [Option<T>; 32],
    children: Vec<Octree<N, T>>,
    paternity: Paternity
}

#[allow(dead_code)]
impl<N: Signed + Scalar + Num + NumCast + Ord + AddAssign + SubAssign + DivAssign, T: PointData<N> + Debug> Octree<N, T> {

    pub fn new(aabb: AABB<N>) -> Octree<N, T> {
        println!("Creating new Octree with a min of {:?} and a max of {:?}", aabb.get_min(), aabb.get_max());

        Octree {
            aabb,
            num_elements: 0,
            elements: [Option::<T>::None; 32],
            children: Vec::with_capacity(8),
            paternity: Paternity::ChildFree
        }
    }

    pub fn get_aabb(&self) -> AABB<N> {
        self.aabb
    } 

    fn subdivide(&mut self) -> Result<(), SubdivisionError<N>> {

        let zero: N = NumCast::from(0).unwrap();
        let one: N = NumCast::from(1).unwrap();
        let two: N = NumCast::from(2).unwrap();

        // Hacky way of checking if it's an integer and then adjusting min so all values behave like indices
        let adj = if one/two == zero {
            one
        } else {
            zero
        };

        let min = self.aabb.get_min();
        let max = self.aabb.get_max();

        let dimensions = self.aabb.dimensions.abs();

        let smaller_half = dimensions/two;
        let larger_half = dimensions - smaller_half;

        //down back left
        let sub_max = min + larger_half;
        let downbackleft = AABB::<N>::from_extents(
            min,
            sub_max
        );
        self.children.push(Octree::new(downbackleft));

        if dimensions.x > one {
            //down back right
            let sub_min = min + Vector3::new(larger_half.x + adj, zero, zero);
            let sub_max = Vector3::new(
                max.x, sub_min.y + larger_half.y, sub_min.z + larger_half.z
            );

            let downbackright = AABB::<N>::from_extents(
                sub_min,
                sub_max
            );
            self.children.push(Octree::new(downbackright));

            if dimensions.z > one {
                //down forward right
                let sub_min = min + Vector3::new(larger_half.x + adj, zero, larger_half.z + adj);
                let sub_max = Vector3::new(
                    max.x, sub_min.y + larger_half.y, max.z
                );
                let downforwardright = AABB::<N>::from_extents(
                    sub_min,
                    sub_max
                );
                self.children.push(Octree::new(downforwardright));

                if dimensions.y > one {
                    //up forward right
                    let sub_min = min + Vector3::new(larger_half.x + adj, larger_half.y + adj, larger_half.z + adj);
                    let upforwardright = AABB::<N>::from_extents(
                        sub_min,
                        max
                    );
                    self.children.push(Octree::new(upforwardright));
                }
            }
        }

        if dimensions.z > one {
            //down forward left
            let sub_min = min + Vector3::new(zero, zero, larger_half.z + adj);
            let sub_max = Vector3::new(
                sub_min.x + larger_half.x, sub_min.y + larger_half.y, max.z
            );

            let downforwardleft = AABB::<N>::from_extents(
                sub_min, 
                sub_max
            );
            self.children.push(Octree::new(downforwardleft));

            if dimensions.y > one {
                //up forward left
                let sub_min = min + Vector3::new(zero, larger_half.y + adj, larger_half.z + adj);
                let sub_max = Vector3::new(
                    sub_min.x + larger_half.x, max.y, max.z
                );
                let upforwardleft = AABB::<N>::from_extents(
                    sub_min,
                    sub_max
                );
                self.children.push(Octree::new(upforwardleft));
            }
        }

        if dimensions.y > one {
            //up back left
            let sub_min = min + Vector3::new(zero, larger_half.y + adj, zero);
            let sub_max = Vector3::new(
                sub_min.x + larger_half.x, max.y, sub_min.z + larger_half.z
            );
            let upbackleft = AABB::<N>::from_extents(
                sub_min,
                sub_max
            );
            self.children.push(Octree::new(upbackleft));

            if dimensions.x > one {
                //up back right
                let sub_min = min + Vector3::new(larger_half.x + adj, larger_half.y + adj, zero);
                let sub_max = Vector3::new(
                    max.x, max.y, sub_min.z + larger_half.z
                );
                let upbackright = AABB::<N>::from_extents(
                    sub_min,
                    sub_max
                );
                self.children.push(Octree::new(upbackright));
            }
        }
        
        self.paternity = Paternity::ProudParent;

        let mut total_volume = zero;
        for child in &self.children {
            total_volume= total_volume + child.aabb.dimensions.x * child.aabb.dimensions.y * child.aabb.dimensions.z;
        }

        let volume = dimensions.x * dimensions.y * dimensions.z;

        if cfg!(debug_assertions) {
            if total_volume == volume {
                Ok({})
            } else {
                Err(
                    SubdivisionError{
                        error_type: SubdivisionErrorType::IncorrectDimensions(total_volume, volume)
                    }
                )
            }
        } else {
            Ok({})
        }
    }

    pub fn insert(&mut self, element: T) -> Result<(), InsertionError<N>>{  

        let pt = element.get_point();

        if !self.aabb.contains_point(pt) {
            return Err(
                InsertionError{
                    error_type: InsertionErrorType::OutOfBounds(self.aabb)
                }
            )
        }

        //if element already exists at point, replace it
        for el in &mut self.elements {
            match el {
                Some(r) if r.get_point() == pt => {
                    *el = Some(*r);
                    return Ok({})
                },
                _ => {}
            }
        }
        
        match &self.paternity { //do first match because you still need to insert into children after subdividing, not either/or

            Paternity::ChildFree if self.num_elements < self.elements.len() => {
                self.elements[self.num_elements as usize] = Some(element);
                self.num_elements = self.num_elements + 1;
                // println!("Inserted {:?} between {:?} and {:?} at position {}", element.get_point(), self.aabb.get_min(), self.aabb.get_max(), self.num_elements);

                return Ok({});
            }

            Paternity::ChildFree => { 
                match self.subdivide() {
                    Ok(_) => {},
                    Err(err) => {
                        panic!("{:?}", err);
                    }
                }
            }
            
            _ => {}

        };

        return match &self.paternity {

            Paternity::ProudParent => {

                let mut result = Err(
                    InsertionError {
                        error_type: InsertionErrorType::BlockFull(self.aabb)
                    }
                );

                for child in &mut self.children {
                    
                    //only return true if true because we need to check all of them
                    
                    match child.insert(element) {
                        Ok(_) => return Ok({}),
                        Err(err) => { result = Err(err) }
                    }
                        
                            
                }

                result
            }

            _ => Err(
                InsertionError {
                    error_type: InsertionErrorType::Empty
                }
            )
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
                    count += child.count();
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


        for child in &self.children {

            let child_query = child.query_point(point);

            if child_query.is_some() {

                return child_query; 
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

        for child in &self.children {
            elements_in_range.append(&mut child.query_range(range));
        }

        elements_in_range
    }
}

#[derive(Clone, Debug)]
pub enum SubdivisionErrorType<N: Scalar> {
    IncorrectDimensions(N, N)
}

#[derive(Debug, Clone)]
pub struct SubdivisionError<N: Scalar> {
    error_type: SubdivisionErrorType<N>
}

impl<N: Scalar> fmt::Display for SubdivisionError<N> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.error_type)
    }
}

impl<N: Scalar> error::Error for SubdivisionError<N> {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}

#[derive(Clone, Debug)]
pub enum InsertionErrorType<N: Scalar> {
    Empty,
    BlockFull(AABB<N>),
    OutOfBounds(AABB<N>),
}

#[derive(Debug, Clone)]
pub struct InsertionError<N: Scalar> {
    error_type: InsertionErrorType<N>
}

impl<N: Scalar> fmt::Display for InsertionError<N> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.error_type)
    }
}

impl<N: Scalar> error::Error for InsertionError<N> {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}