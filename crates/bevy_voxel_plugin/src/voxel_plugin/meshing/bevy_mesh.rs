use bevy::asset::RenderAssetUsages;

use bevy::render::mesh::{Indices, Mesh};

use bevy::render::render_resource::PrimitiveTopology;

use fast_surface_nets::SurfaceNetsBuffer;

/// Convert a Surface Nets buffer into a single mesh.
/// If `vertex_colors` are provided, they are inserted as `Mesh::ATTRIBUTE_COLOR`.
pub fn buffer_to_meshes_per_material(
	buffer: &SurfaceNetsBuffer,
	vertex_colors: Option<&[[f32; 4]]>,
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

	if let Some(colors) = vertex_colors
		&& colors.len() == buffer.positions.len()
	{
		mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors.to_vec());
	}

	mesh.insert_indices(Indices::U32(buffer.indices.clone()));

	vec![mesh]
}
