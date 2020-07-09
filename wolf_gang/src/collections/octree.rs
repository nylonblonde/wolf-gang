use core::fmt::Debug;
use serde::{Serialize, Deserialize};
use std::ops::{AddAssign, SubAssign, DivAssign};

use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use rayon::prelude::*;

use std::error;
use std::fmt;

use nalgebra::{Scalar, Vector3};
use num::{Num, NumCast, Signed};
use crate::geometry::aabb::AABB;

pub static DEFAULT_MAX: usize = 16;

pub trait PointData<N: Scalar> : Copy {
    fn get_point(&self) -> Vector3<N>;
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Copy, Clone)]
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

impl<'de, N: Sync + Send + Signed + Scalar + Num + NumCast + Ord + AddAssign + SubAssign + DivAssign + Copy + Clone + Serialize + Deserialize<'de>, T: PointData<N> + PartialEq + Debug + Sync + Send> IntoIterator for Octree<N, T> {
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

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[allow(dead_code)]
pub struct Octree <N: Scalar, T: PointData<N>>{
    aabb: AABB<N>,
    max_elements: usize,
    elements: Vec<T>,
    children: Vec<Octree<N, T>>,
    paternity: Paternity,
    // phantom: PhantomData<&'a Octree<'a, N, T>>
}

#[allow(dead_code)]
impl<N: Sync + Send + Signed + Scalar + Num + NumCast + Ord + AddAssign + SubAssign + DivAssign + Copy + Clone, T: PointData<N> + PartialEq + Debug + Sync + Send> Octree<N, T> {

    pub fn new(aabb: AABB<N>, max_elements: usize) -> Octree<N, T> {
        println!("Creating new Octree with a min of {:?} and a max of {:?}", aabb.get_min(), aabb.get_max());

        Octree {
            aabb,
            max_elements,
            elements: Vec::with_capacity(max_elements),
            children: Vec::with_capacity(8),
            paternity: Paternity::ChildFree,
            // phantom: PhantomData
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
        let larger_half = dimensions - smaller_half - Vector3::new(adj, adj, adj);

        let (tx, rx) = mpsc::channel::<Octree<N,T>>();

        let max_elements = Arc::new(self.max_elements);

        rayon::scope(move |s| {
            
            //down back left
            let sub_max = min + larger_half;

            let downbackleft = AABB::<N>::from_extents(
                min,
                sub_max
            );
            tx.send(Octree::new(downbackleft, *max_elements)).unwrap();

            if dimensions.x > one {

                let tx1 = tx.clone();
                let max_elements = Arc::clone(&max_elements);

                s.spawn(move |s| {

                    let sub_min = min + Vector3::new(larger_half.x + adj, zero, zero);
                    let sub_max = Vector3::new(
                        max.x, sub_min.y + larger_half.y, sub_min.z + larger_half.z
                    );
        
                    let downbackright = AABB::<N>::from_extents(
                        sub_min,
                        sub_max
                    );
                    tx1.send(Octree::new(downbackright, *max_elements)).unwrap();
        
                    if dimensions.z > one {
                        
                        s.spawn(move |s| {

                            //down forward right
                            let sub_min = min + Vector3::new(larger_half.x + adj, zero, larger_half.z + adj);
                            let sub_max = Vector3::new(
                                max.x, sub_min.y + larger_half.y, max.z
                            );
                            let downforwardright = AABB::<N>::from_extents(
                                sub_min,
                                sub_max
                            );
                            tx1.send(Octree::new(downforwardright, *max_elements)).unwrap();
            
                            if dimensions.y > one {
                                s.spawn(move |_| {
                                    
                                    //up forward right
                                    let sub_min = min + Vector3::new(larger_half.x + adj, larger_half.y + adj, larger_half.z + adj);
                                    let upforwardright = AABB::<N>::from_extents(
                                        sub_min,
                                        max
                                    );
                                    tx1.send(Octree::new(upforwardright, *max_elements)).unwrap();
                                });
                            }
                        });
                    }
                });
            }

            if dimensions.z > one {

                let tx2 = tx.clone();
                let max_elements = Arc::clone(&max_elements);

                s.spawn(move |s| {

                    //down forward left
                    let sub_min = min + Vector3::new(zero, zero, larger_half.z + adj);
                    let sub_max = Vector3::new(
                        sub_min.x + larger_half.x, sub_min.y + larger_half.y, max.z
                    );
        
                    let downforwardleft = AABB::<N>::from_extents(
                        sub_min, 
                        sub_max
                    );
                    tx2.send(Octree::new(downforwardleft, *max_elements)).unwrap();

                    if dimensions.y > one {
                        s.spawn(move |_| {

                            //up forward left
                            let sub_min = min + Vector3::new(zero, larger_half.y + adj, larger_half.z + adj);
                            let sub_max = Vector3::new(
                                sub_min.x + larger_half.x, max.y, max.z
                            );
                            let upforwardleft = AABB::<N>::from_extents(
                                sub_min,
                                sub_max
                            );
                            tx2.send(Octree::new(upforwardleft, *max_elements)).unwrap();
                        });
                    }
                });
            }

            if dimensions.y > one {

                let tx3 = tx.clone();
                let max_elements = Arc::clone(&max_elements);

                s.spawn(move |s| {

                    //up back left
                    let sub_min = min + Vector3::new(zero, larger_half.y + adj, zero);
                    let sub_max = Vector3::new(
                        sub_min.x + larger_half.x, max.y, sub_min.z + larger_half.z
                    );
                    let upbackleft = AABB::<N>::from_extents(
                        sub_min,
                        sub_max
                    );
                    tx3.send(Octree::new(upbackleft, *max_elements)).unwrap();
        
                    if dimensions.x > one {

                        s.spawn(move |_| {

                            //up back right
                            let sub_min = min + Vector3::new(larger_half.x + adj, larger_half.y + adj, zero);
                            let sub_max = Vector3::new(
                                max.x, max.y, sub_min.z + larger_half.z
                            );
                            let upbackright = AABB::<N>::from_extents(
                                sub_min,
                                sub_max
                            );
                            tx3.send(Octree::new(upbackright, *max_elements)).unwrap();
                        });
                    }
                });
            }
        });

        for received in rx {
            self.children.push(received);
        }

        self.paternity = Paternity::ProudParent;

        let mut total_volume = zero;
        for child in &self.children {
            total_volume = total_volume + child.aabb.dimensions.x * child.aabb.dimensions.y * child.aabb.dimensions.z;
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

    /// Removes all elements which fit inside range, silently avoiding positions that do not fit inside the octree
    pub fn remove_range(&mut self, range: AABB<N>) {

        let (tx, rx) = mpsc::channel::<T>();

        self.elements.par_iter().for_each_with(tx, |tx, element| {

            let pt = element.get_point();

            if range.contains_point(pt) {
                tx.send(*element).unwrap();
            }
            
        });

        let to_remove: Vec<T> = rx.into_iter().collect();

        self.elements = self.elements.clone().into_iter().filter(|&e| !to_remove.contains(&e)).collect();

        if let Paternity::ProudParent = self.paternity {

            self.children.par_iter_mut().for_each(|child|{
                child.remove_range(range);
            });

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
        self.elements.par_iter_mut().for_each_with(element, |el, element| {
            if element.get_point() == pt {
                *element = *el;
            }
        });
       
        match &self.paternity { //do first match because you still need to insert into children after subdividing, not either/or

            Paternity::ChildFree | Paternity::ProudParent if self.max_elements > self.elements.len() => {
                self.elements.push(element);
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

                let (tx, rx) = mpsc::channel::<Result<(), InsertionError<N>>>();

                self.children.par_iter_mut().for_each_with(tx, |tx, child| {
                    // println!("Inserting into child");                    
                    match child.insert(element) {
                        Ok(_) => tx.send(Ok({})),
                        Err(err) => tx.send(Err(err))
                    }.unwrap();               
                });

                let mut received = rx.into_iter();
                if let Some(r) = received.find(|x| x.is_ok()) {
                    return r;
                } else if let Some(r) = received.find(|x| x.is_err()) {
                    return r;
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
        let mut count: usize = self.elements.len();

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

        let (tx, rx) = mpsc::channel::<T>();

        self.elements.par_iter().for_each_with(tx, |tx, element| {
            if element.get_point() == point  {
                tx.send(*element).unwrap();
            }
        });

        if let Some(result) = rx.into_iter().next() {
            return Some(result);
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

        let mut elements_in_range: Vec<T> = Vec::with_capacity(self.max_elements);
        
        if !self.aabb.intersects_bounds(range) {
            return elements_in_range
        }

        if self.elements.len() == 0 {
            return elements_in_range
        }

        let (tx, rx) = mpsc::channel::<T>();

        self.elements.par_iter().for_each_with(tx, |tx, element| {

            if self.aabb.contains_point(element.get_point()) {
                tx.send(*element).unwrap();
            }
        });

        elements_in_range.extend(rx.into_iter());

        if let Paternity::ChildFree = self.paternity {
            return elements_in_range
        }

        let (tx, rx) = mpsc::channel::<Vec<T>>();

        self.children.par_iter().for_each_with(tx, |tx, child| {
            tx.send(child.query_range(range)).unwrap();
        });

        for mut received in rx {
            elements_in_range.append(&mut received)
        }

        return elements_in_range
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