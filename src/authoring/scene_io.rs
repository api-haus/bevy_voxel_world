#[cfg(all(debug_assertions, feature = "editor"))]
pub fn demo_spawn_authoring(mut commands: Commands) {
    // Spawn a couple of authoring shapes so save has content in dev
    commands.spawn((
        super::components::SdfSphere {
            radius: 3.0,
            material: 1,
            op: super::components::CsgOp::Union,
            smooth_k: 1.0,
            priority: 0,
        },
        Transform::from_xyz(4.0, 6.0, 4.0),
        GlobalTransform::default(),
        Name::new("AuthorSphere"),
    ));

    commands.spawn((
        super::components::SdfBox {
            half_extents: Vec3::new(2.0, 1.0, 3.0),
            material: 2,
            op: super::components::CsgOp::Subtract,
            smooth_k: 0.5,
            priority: 0,
        },
        Transform::from_xyz(-3.0, 5.0, 2.0),
        GlobalTransform::default(),
        Name::new("AuthorBox"),
    ));
}

#[cfg(all(debug_assertions, feature = "editor"))]
pub fn save_authoring_scene_system(world: &mut World) {
    use bevy::scene::DynamicScene;

    // Only act on F5
    let key_input = world.resource::<ButtonInput<KeyCode>>();
    if !key_input.just_pressed(KeyCode::F5) {
        return;
    }

    // Build a DynamicScene containing only authoring shapes and their Transforms
    let mut scene_world = World::new();

    // Copy over the type registry so reflection-based serialization works
    if let Some(reg) = world.get_resource::<AppTypeRegistry>() {
        scene_world.insert_resource(reg.clone());
    }

    let mut q = world.query::<(
        Option<&super::components::SdfSphere>,
        Option<&super::components::SdfBox>,
        Option<&Transform>,
        Option<&Name>,
    )>();

    for (sphere, b, t, name) in q.iter(world) {
        if sphere.is_none() && b.is_none() {
            continue;
        }
        let mut e = scene_world.spawn(());
        if let Some(t) = t {
            e.insert(*t);
        }
        if let Some(name) = name {
            e.insert(name.clone());
        }
        if let Some(s) = sphere {
            e.insert(*s);
        }
        if let Some(bx) = b {
            e.insert(*bx);
        }
    }

    let scene = DynamicScene::from_world(&scene_world);
    let type_registry = scene_world
        .get_resource::<AppTypeRegistry>()
        .expect("AppTypeRegistry missing")
        .read();

    let ron = scene.serialize(&type_registry).expect("serialize scene");

    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::fs;
        let _ = fs::create_dir_all("assets/scenes");
        // Placeholder: will be replaced by editor-native persistence. Keeping this to aid
        // manual QA only; RON is not a shipping format for this project.
        fs::write("assets/scenes/authoring.debug.ron", ron).expect("write authoring.debug.ron");
    }
}

// No load path maintained; editor build will provide its own persistence.
