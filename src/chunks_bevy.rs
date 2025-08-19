use bevy::asset::{Assets, RenderAssetUsages};
use bevy::color::Color;
use bevy::pbr::StandardMaterial;
use bevy::prelude::{Commands, Mesh, Mesh3d, MeshMaterial3d, ResMut, Transform, Vec3};

use crate::rayon_chunks::generate_chunks;

/// Generates chunk meshes using `generate_chunks` and spawns them into the Bevy world.
pub fn setup_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let chunk_buffers = generate_chunks();

    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.82, 0.74),
        perceptual_roughness: 0.8,
        metallic: 0.0,
        ..Default::default()
    });

    for (offset, sn_buffer) in chunk_buffers.into_iter() {
        // Build a mesh from the SurfaceNetsBuffer
        let mut mesh = Mesh::new(
            bevy::render::render_resource::PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, sn_buffer.positions.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, sn_buffer.normals.clone());
        mesh.insert_indices(bevy::render::mesh::Indices::U32(sn_buffer.indices.clone()));

        let mesh_handle = meshes.add(mesh);

        commands.spawn((
            Mesh3d(mesh_handle),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(Vec3::new(offset.x, offset.y, offset.z)),
        ));
    }
}
