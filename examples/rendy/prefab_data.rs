use amethyst::{
    animation::AnimationSetPrefab,
    assets::{AssetPrefab, Handle, Prefab, PrefabData, ProgressCounter},
    controls::ControlTagPrefab,
    core::{ecs::Entity, Transform},
    gltf::{GltfSceneAsset, GltfSceneFormat},
    utils::tag::Tag,
    Error,
};
use amethyst_rendy::{
    camera::CameraPrefab,
    formats::{mesh::MeshPrefab, mtl::MaterialPrefab},
    light::LightPrefab,
    rendy::{hal::Backend, mesh::PosNormTangTex},
    sprite::{
        prefab::{SpriteRenderPrefab, SpriteSheetPrefab},
        SpriteRender,
    },
    transparent::Transparent,
};
use derivative::Derivative;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct AnimationMarker;

/// Animation ids used in a AnimationSet
#[derive(
    Derivative, Eq, PartialOrd, PartialEq, Hash, Debug, Copy, Clone, Deserialize, Serialize,
)]
#[derivative(Default)]
pub enum SpriteAnimationId {
    #[derivative(Default)]
    Fly,
}

#[derive(Derivative)]
#[derivative(Default(bound = ""))]
pub struct Scene<B: Backend> {
    pub handle: Option<Handle<Prefab<ScenePrefabData<B>>>>,
    pub animation_index: usize,
}

#[derive(Derivative, Deserialize, Serialize)]
#[derivative(Default(bound = ""))]
#[serde(default, bound = "")]
pub struct ScenePrefabData<B: Backend> {
    transform: Option<Transform>,
    gltf: Option<AssetPrefab<GltfSceneAsset<B>, GltfSceneFormat>>,
    sprite_sheet: Option<SpriteSheetPrefab<B>>,
    animation_set: Option<AnimationSetPrefab<SpriteAnimationId, SpriteRender<B>>>,
    camera: Option<CameraPrefab>,
    light: Option<LightPrefab>,
    tag: Option<Tag<AnimationMarker>>,
    fly_tag: Option<ControlTagPrefab>,
    sprite: Option<SpriteRenderPrefab<B>>,
    mesh: Option<MeshPrefab<B, Vec<PosNormTangTex>>>,
    material: Option<MaterialPrefab<B>>,
    transparent: Option<Transparent>,
}

type PData<'a, T> = <T as PrefabData<'a>>::SystemData;
impl<'a, B: Backend> PrefabData<'a> for ScenePrefabData<B> {
    type SystemData = (
        PData<'a, Transform>,
        PData<'a, AssetPrefab<GltfSceneAsset<B>, GltfSceneFormat>>,
        PData<'a, SpriteSheetPrefab<B>>,
        PData<'a, AnimationSetPrefab<SpriteAnimationId, SpriteRender<B>>>,
        PData<'a, CameraPrefab>,
        PData<'a, LightPrefab>,
        PData<'a, Tag<AnimationMarker>>,
        PData<'a, ControlTagPrefab>,
        PData<'a, SpriteRenderPrefab<B>>,
        PData<'a, MeshPrefab<B, Vec<PosNormTangTex>>>,
        PData<'a, MaterialPrefab<B>>,
    );
    type Result = ();
    fn add_to_entity(
        &self,
        entity: Entity,
        d: &mut Self::SystemData,
        e: &[Entity],
        c: &[Entity],
    ) -> Result<(), Error> {
        self.transform
            .as_ref()
            .map(|p| p.add_to_entity(entity, &mut d.0, e, c))
            .transpose()?;
        self.gltf
            .as_ref()
            .map(|p| p.add_to_entity(entity, &mut d.1, e, c))
            .transpose()?;
        self.sprite_sheet
            .as_ref()
            .map(|p| p.add_to_entity(entity, &mut d.2, e, c))
            .transpose()?;
        self.animation_set
            .as_ref()
            .map(|p| p.add_to_entity(entity, &mut d.3, e, c))
            .transpose()?;
        self.camera
            .as_ref()
            .map(|p| p.add_to_entity(entity, &mut d.4, e, c))
            .transpose()?;
        self.light
            .as_ref()
            .map(|p| p.add_to_entity(entity, &mut d.5, e, c))
            .transpose()?;
        self.tag
            .as_ref()
            .map(|p| p.add_to_entity(entity, &mut d.6, e, c))
            .transpose()?;
        self.fly_tag
            .as_ref()
            .map(|p| p.add_to_entity(entity, &mut d.7, e, c))
            .transpose()?;
        self.sprite
            .as_ref()
            .map(|p| p.add_to_entity(entity, &mut d.8, e, c))
            .transpose()?;
        self.mesh
            .as_ref()
            .map(|p| p.add_to_entity(entity, &mut d.9, e, c))
            .transpose()?;
        self.material
            .as_ref()
            .map(|p| p.add_to_entity(entity, &mut d.10, e, c))
            .transpose()?;
        self.transparent
            .as_ref()
            .map(|p| p.add_to_entity(entity, &mut (d.10).1, e, c))
            .transpose()?;
        Ok(())
    }
    fn load_sub_assets(
        &mut self,
        pc: &mut ProgressCounter,
        d: &mut Self::SystemData,
    ) -> Result<bool, Error> {
        let mut ret = false;
        ret |= self
            .transform
            .as_mut()
            .map_or(Ok(false), |p| p.load_sub_assets(pc, &mut d.0))?;
        ret |= self
            .gltf
            .as_mut()
            .map_or(Ok(false), |p| p.load_sub_assets(pc, &mut d.1))?;
        ret |= self
            .sprite_sheet
            .as_mut()
            .map_or(Ok(false), |p| p.load_sub_assets(pc, &mut d.2))?;
        ret |= self
            .animation_set
            .as_mut()
            .map_or(Ok(false), |p| p.load_sub_assets(pc, &mut d.3))?;
        ret |= self
            .camera
            .as_mut()
            .map_or(Ok(false), |p| p.load_sub_assets(pc, &mut d.4))?;
        ret |= self
            .light
            .as_mut()
            .map_or(Ok(false), |p| p.load_sub_assets(pc, &mut d.5))?;
        ret |= self
            .tag
            .as_mut()
            .map_or(Ok(false), |p| p.load_sub_assets(pc, &mut d.6))?;
        ret |= self
            .fly_tag
            .as_mut()
            .map_or(Ok(false), |p| p.load_sub_assets(pc, &mut d.7))?;
        ret |= self
            .sprite
            .as_mut()
            .map_or(Ok(false), |p| p.load_sub_assets(pc, &mut d.8))?;
        ret |= self
            .mesh
            .as_mut()
            .map_or(Ok(false), |p| p.load_sub_assets(pc, &mut d.9))?;
        ret |= self
            .material
            .as_mut()
            .map_or(Ok(false), |p| p.load_sub_assets(pc, &mut d.10))?;
        ret |= self
            .transparent
            .as_mut()
            .map_or(Ok(false), |p| p.load_sub_assets(pc, &mut (d.10).1))?;
        Ok(ret)
    }
}
