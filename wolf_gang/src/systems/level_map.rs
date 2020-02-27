use crate::custom_mesh; 
use crate::geometry::aabb;

use gdnative::*;

use nalgebra;

use legion::prelude::*;

use std::collections::HashMap;

type AABB = aabb::AABB<i32>;
type Point = nalgebra::Vector3<i32>;

pub struct TileDimensions {
    pub x: f32,
    pub y: f32,
    pub z: f32
}

pub struct CoordPos {
    pub value: Point
}

impl Default for CoordPos {
    fn default() -> CoordPos {
        CoordPos {
            value: Point::new(0,0,0)
        }
    }
}

pub const TILE_DIMENSIONS: TileDimensions = TileDimensions {x: 1.0, y: 0.2, z: 1.0};

pub fn create_system() -> Box<dyn Schedulable> {
    SystemBuilder::<()>::new("map_system")
            .with_query(<(Read<MapChunkData>, Write<custom_mesh::MeshData>)>::query()
                .filter(changed::<MapChunkData>())
            )
            .build(move |commands, world, resource, queries| {
                for (entity, (map_data, mut mesh_data)) in queries.iter_entities_mut(&mut *world) {
                    godot_print!("{:?}", "there should only be one tick");
                    mesh_data.verts.push(&Vector3::new(2.0,0.0,0.0));
                    mesh_data.verts.push(&Vector3::new(1.0,0.0,0.0));
                    mesh_data.verts.push(&Vector3::new(2.0,0.0,1.0));
                    mesh_data.verts.push(&Vector3::new(1.0,0.0,1.0));

                    mesh_data.normals.push(&Vector3::new(0.0,1.0,0.0));
                    mesh_data.normals.push(&Vector3::new(0.0,1.0,0.0));
                    mesh_data.normals.push(&Vector3::new(0.0,1.0,0.0));
                    mesh_data.normals.push(&Vector3::new(0.0,1.0,0.0));

                    mesh_data.uvs.push(&Vector2::new(0.0,0.0));
                    mesh_data.uvs.push(&Vector2::new(0.0,0.0));
                    mesh_data.uvs.push(&Vector2::new(0.0,0.0));
                    mesh_data.uvs.push(&Vector2::new(0.0,0.0));

                    mesh_data.indices.push(2);
                    mesh_data.indices.push(1);
                    mesh_data.indices.push(0);
                    mesh_data.indices.push(2);
                    mesh_data.indices.push(3);
                    mesh_data.indices.push(1);
                }
            })
} 

/// Applies the const TILE_DIMENSIONS to each map coord to get its conversion in 3D space.
pub fn map_coords_to_world(map_coord: Point) -> nalgebra::Vector3<f32> {
    nalgebra::Vector3::<f32>::new(
        map_coord.x as f32 * TILE_DIMENSIONS.x, 
        map_coord.y as f32 * TILE_DIMENSIONS.y,
        map_coord.z as f32 * TILE_DIMENSIONS.z
    )
}

pub struct Map {
    chunk_dimensions: AABB,
    map_chunk_pool: HashMap<Point, MapChunkData>
}

impl Map {
    pub fn new() -> Self {
        Map { 
            map_chunk_pool: HashMap::new(),
            chunk_dimensions: AABB::new(Point::new(0,0,0), Point::new(10,10,10))
        }
    }

    
}

//TODO: evaluate whether MapChunkData should be stored in a vec or octree
pub struct MapChunkData {
    tiles: Vec<Vector3>,
}

impl MapChunkData {
    pub fn new() -> Self {
        MapChunkData {
            tiles: Vec::<Vector3>::new(),
        }
    }
}

#[derive(Clone)]
pub struct TileData {
    point: Point
}

impl Copy for TileData {}

impl TileData {
    pub fn new(point: Point) -> Self {
        TileData {
            point
        }
    }
}

impl crate::collections::octree::PointData<i32> for TileData {

    fn get_point(&self) -> Point {
        self.point
    }
}

