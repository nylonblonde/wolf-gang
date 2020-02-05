
use gdnative::{GodotString, Int32Array, MeshInstance, Vector2Array, Vector3Array};
use std::collections::HashMap;

pub struct MeshInstancePool {
    pool: HashMap<GodotString, MeshInstance>,
}

impl MeshInstancePool {
    pub fn new() -> Self {
        MeshInstancePool {
            pool: HashMap::new()
        }
    }
}

pub struct MeshData {
    pub verts: Vector3Array,
    pub uvs: Vector2Array,
    pub normals: Vector3Array,
    pub indices: Int32Array,
}

impl MeshData {
    pub fn new() -> Self {
        MeshData {
            verts: Vector3Array::new(),
            uvs: Vector2Array::new(),
            normals: Vector3Array::new(),
            indices: Int32Array::new()
        }
    }
}

