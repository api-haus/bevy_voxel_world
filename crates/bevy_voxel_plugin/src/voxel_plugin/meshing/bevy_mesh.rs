use bevy::asset::RenderAssetUsages;
use bevy::render::mesh::{Indices, Mesh};
use bevy::render::render_resource::PrimitiveTopology;
use fast_surface_nets::SurfaceNetsBuffer;

/// Convert a Surface Nets buffer into one or more meshes split by material.
/// Single-material skeleton for now; `_vertex_materials` is reserved for future splitting.
pub fn buffer_to_meshes_per_material(
	buffer: &SurfaceNetsBuffer,
	_vertex_materials: Option<&[u8]>,
) -> Vec<Mesh> {
	if buffer.positions.is_empty() || buffer.indices.is_empty() {
		return Vec::new();
	}

	let mut mesh = Mesh::new(
		PrimitiveTopology::TriangleList,
		RenderAssetUsages::default(),
	);
	mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, buffer.positions.clone());
	if !buffer.normals.is_empty() {
		mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, buffer.normals.clone());
	}
	mesh.insert_indices(Indices::U32(buffer.indices.clone()));

	vec![mesh]
}
