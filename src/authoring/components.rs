use bevy::prelude::*;

#[derive(Reflect, Clone, Copy, PartialEq, Default)]
pub enum CsgOp {
    #[default]
    Union,
    Intersect,
    Subtract,
    SmoothUnion {
        k: f32,
    },
    SmoothIntersect {
        k: f32,
    },
    SmoothSubtract {
        k: f32,
    },
}

#[derive(Component, Reflect, Default, Clone, Copy, PartialEq)]
#[reflect(Component)]
pub struct SdfSphere {
    pub radius: f32,
    pub material: u8,
    pub op: CsgOp,
    pub smooth_k: f32,
    pub priority: u8,
}

#[derive(Component, Reflect, Default, Clone, Copy, PartialEq)]
#[reflect(Component)]
pub struct SdfBox {
    pub half_extents: Vec3,
    pub material: u8,
    pub op: CsgOp,
    pub smooth_k: f32,
    pub priority: u8,
}
