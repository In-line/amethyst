//! Forward physically-based drawing pass.

use std::mem;

use amethyst_assets::AssetStorage;
use amethyst_core::cgmath::{Matrix4, One, SquareMatrix};
use amethyst_core::transform::Transform;
use gfx::pso::buffer::ElemStride;
use rayon::iter::ParallelIterator;
use rayon::iter::internal::UnindexedConsumer;
use specs::{Fetch, Join, ParJoin, ReadStorage};

use super::*;
use cam::{ActiveCamera, Camera};
use error::Result;
use light::{DirectionalLight, Light, PointLight};
use mesh::{Mesh, MeshHandle};
use mtl::{Material, MaterialDefaults};
use pipe::{DepthMode, Effect, NewEffect};
use pipe::pass::{Pass, PassApply, PassData, Supplier};
use resources::AmbientColor;
use tex::Texture;
use types::Encoder;
use vertex::{Normal, Position, Separate, Tangent, TexCoord, VertexFormat};

/// Draw mesh with physically based lighting
#[derive(Default, Clone, Debug, PartialEq)]
pub struct DrawPbmSeparate;

impl DrawPbmSeparate {
    /// Create instance of `DrawPbm` pass
    pub fn new() -> Self {
        Default::default()
    }
}

impl<'a> PassData<'a> for DrawPbmSeparate {
    type Data = (
        Option<Fetch<'a, ActiveCamera>>,
        ReadStorage<'a, Camera>,
        Fetch<'a, AmbientColor>,
        Fetch<'a, AssetStorage<Mesh>>,
        Fetch<'a, AssetStorage<Texture>>,
        Fetch<'a, MaterialDefaults>,
        ReadStorage<'a, MeshHandle>,
        ReadStorage<'a, Material>,
        ReadStorage<'a, Transform>,
        ReadStorage<'a, Light>,
    );
}

impl<'a> PassApply<'a> for DrawPbmSeparate {
    type Apply = DrawPbmSeparateApply<'a>;
}

impl Pass for DrawPbmSeparate {
    fn compile(&self, effect: NewEffect) -> Result<Effect> {
        effect
            .simple(VERT_SRC, FRAG_SRC)
            // Pos, norm, tangent, tex
            //.with_raw_vertex_buffer(V::QUERIED_ATTRIBUTES, V::size() as ElemStride, 0)
            .with_raw_vertex_buffer(
                Separate::<Position>::ATTRIBUTES,
                Separate::<Position>::size() as ElemStride,
                0,
            )
            .with_raw_vertex_buffer(
                Separate::<Normal>::ATTRIBUTES,
                Separate::<Normal>::size() as ElemStride,
                0,
            )
            .with_raw_vertex_buffer(
                Separate::<Tangent>::ATTRIBUTES,
                Separate::<Tangent>::size() as ElemStride,
                0,
            )
            .with_raw_vertex_buffer(
                Separate::<TexCoord>::ATTRIBUTES,
                Separate::<TexCoord>::size() as ElemStride,
                0,
            )
            .with_raw_constant_buffer("VertexArgs", mem::size_of::<VertexArgs>(), 1)
            .with_raw_constant_buffer("FragmentArgs", mem::size_of::<FragmentArgs>(), 1)
            .with_raw_constant_buffer("PointLights", mem::size_of::<PointLight>(), 512)
            .with_raw_constant_buffer("DirectionalLights", mem::size_of::<DirectionalLight>(), 16)
            .with_raw_global("ambient_color")
            .with_raw_global("camera_position")
            .with_texture("roughness")
            .with_texture("caveat")
            .with_texture("metallic")
            .with_texture("ambient_occlusion")
            .with_texture("emission")
            .with_texture("normal")
            .with_texture("albedo")
            .with_output("out_color", Some(DepthMode::LessEqualWrite))
            .build()
    }

    fn apply<'a, 'b: 'a>(
        &'a mut self,
        supplier: Supplier<'a>,
        (
            active,
            camera,
            ambient,
            mesh_storage,
            tex_storage,
            material_defaults,
            mesh,
            material,
            global,
            light,
        ): (
            Option<Fetch<'a, ActiveCamera>>,
            ReadStorage<'a, Camera>,
            Fetch<'a, AmbientColor>,
            Fetch<'a, AssetStorage<Mesh>>,
            Fetch<'a, AssetStorage<Texture>>,
            Fetch<'a, MaterialDefaults>,
            ReadStorage<'a, MeshHandle>,
            ReadStorage<'a, Material>,
            ReadStorage<'a, Transform>,
            ReadStorage<'a, Light>,
        ),
) -> DrawPbmSeparateApply<'a>{
        DrawPbmSeparateApply {
            active,
            camera,
            mesh_storage,
            tex_storage,
            material_defaults,
            mesh,
            material,
            global,
            ambient,
            light,
            supplier,
        }
    }
}

pub struct DrawPbmSeparateApply<'a> {
    active: Option<Fetch<'a, ActiveCamera>>,
    camera: ReadStorage<'a, Camera>,
    ambient: Fetch<'a, AmbientColor>,
    mesh_storage: Fetch<'a, AssetStorage<Mesh>>,
    tex_storage: Fetch<'a, AssetStorage<Texture>>,
    material_defaults: Fetch<'a, MaterialDefaults>,
    mesh: ReadStorage<'a, MeshHandle>,
    material: ReadStorage<'a, Material>,
    global: ReadStorage<'a, Transform>,
    light: ReadStorage<'a, Light>,
    supplier: Supplier<'a>,
}

impl<'a> ParallelIterator for DrawPbmSeparateApply<'a> {
    type Item = ();

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: UnindexedConsumer<Self::Item>,
    {
        let DrawPbmSeparateApply {
            active,
            camera,
            mesh,
            mesh_storage,
            tex_storage,
            material_defaults,
            material,
            global,
            ambient,
            light,
            supplier,
            ..
        } = self;

        let camera: Option<(&Camera, &Transform)> = active
            .and_then(|a| {
                let cam = camera.get(a.entity);
                let transform = global.get(a.entity);
                cam.into_iter().zip(transform.into_iter()).next()
            })
            .or_else(|| (&camera, &global).join().next());

        let ambient = &ambient;
        let light = &light;
        let mesh_storage = &mesh_storage;
        let tex_storage = &tex_storage;
        let material_defaults = &material_defaults;

        supplier
            .supply((&mesh, &material, &global).par_join().map(
                |(mesh, material, global)| {
                    move |encoder: &mut Encoder, effect: &mut Effect| if let Some(mesh) =
                        mesh_storage.get(mesh)
                    {
                        for attrs in [
                            Separate::<Position>::ATTRIBUTES,
                            Separate::<Normal>::ATTRIBUTES,
                            Separate::<Tangent>::ATTRIBUTES,
                            Separate::<TexCoord>::ATTRIBUTES,
                        ].iter()
                        {
                            match mesh.buffer(attrs) {
                                Some(vbuf) => effect.data.vertex_bufs.push(vbuf.clone()),
                                None => return,
                            }
                        }

                        let vertex_args = camera
                            .as_ref()
                            .map(|&(ref cam, ref transform)| {
                                VertexArgs {
                                    proj: cam.proj.into(),
                                    view: transform.0.invert().unwrap().into(),
                                    model: *global.as_ref(),
                                }
                            })
                            .unwrap_or_else(|| {
                                VertexArgs {
                                    proj: Matrix4::one().into(),
                                    view: Matrix4::one().into(),
                                    model: *global.as_ref(),
                                }
                            });

                        effect.update_constant_buffer("VertexArgs", &vertex_args, encoder);

                        let point_lights: Vec<PointLightPod> = light
                            .join()
                            .filter_map(|light| if let Light::Point(ref light) = *light {
                                Some(PointLightPod {
                                    position: pad(light.center.into()),
                                    color: pad(light.color.into()),
                                    intensity: light.intensity,
                                    _pad: [0.0; 3],
                                })
                            } else {
                                None
                            })
                            .collect();

                        let directional_lights: Vec<DirectionalLightPod> = light
                            .join()
                            .filter_map(|light| if let Light::Directional(ref light) = *light {
                                Some(DirectionalLightPod {
                                    color: pad(light.color.into()),
                                    direction: pad(light.direction.into()),
                                })
                            } else {
                                None
                            })
                            .collect();

                        let fragment_args = FragmentArgs {
                            point_light_count: point_lights.len() as i32,
                            directional_light_count: directional_lights.len() as i32,
                        };

                        effect.update_constant_buffer("FragmentArgs", &fragment_args, encoder);
                        effect.update_buffer("PointLights", &point_lights[..], encoder);
                        effect.update_buffer("DirectionalLights", &directional_lights[..], encoder);

                        effect.update_global(
                            "ambient_color",
                            Into::<[f32; 3]>::into(*ambient.as_ref()),
                        );
                        effect.update_global(
                            "camera_position",
                            camera
                                .as_ref()
                                .map(|&(_, ref trans)| {
                                    [trans.0[3][0], trans.0[3][1], trans.0[3][2]]
                                })
                                .unwrap_or([0.0; 3]),
                        );

                        let albedo = tex_storage
                            .get(&material.albedo)
                            .or_else(|| tex_storage.get(&material_defaults.0.albedo))
                            .unwrap();

                        let roughness = tex_storage
                            .get(&material.roughness)
                            .or_else(|| tex_storage.get(&material_defaults.0.roughness))
                            .unwrap();

                        let emission = tex_storage
                            .get(&material.emission)
                            .or_else(|| tex_storage.get(&material_defaults.0.emission))
                            .unwrap();

                        let caveat = tex_storage
                            .get(&material.caveat)
                            .or_else(|| tex_storage.get(&material_defaults.0.caveat))
                            .unwrap();

                        let metallic = tex_storage
                            .get(&material.metallic)
                            .or_else(|| tex_storage.get(&material_defaults.0.metallic))
                            .unwrap();

                        let ambient_occlusion = tex_storage
                            .get(&material.ambient_occlusion)
                            .or_else(|| tex_storage.get(&material_defaults.0.ambient_occlusion))
                            .unwrap();

                        let normal = tex_storage
                            .get(&material.normal)
                            .or_else(|| tex_storage.get(&material_defaults.0.normal))
                            .unwrap();

                        effect.data.textures.push(roughness.view().clone());
                        effect.data.samplers.push(roughness.sampler().clone());
                        effect.data.textures.push(caveat.view().clone());
                        effect.data.samplers.push(caveat.sampler().clone());
                        effect.data.textures.push(metallic.view().clone());
                        effect.data.samplers.push(metallic.sampler().clone());
                        effect.data.textures.push(ambient_occlusion.view().clone());
                        effect
                            .data
                            .samplers
                            .push(ambient_occlusion.sampler().clone());
                        effect.data.textures.push(emission.view().clone());
                        effect.data.samplers.push(emission.sampler().clone());
                        effect.data.textures.push(normal.view().clone());
                        effect.data.samplers.push(normal.sampler().clone());
                        effect.data.textures.push(albedo.view().clone());
                        effect.data.samplers.push(albedo.sampler().clone());

                        effect.draw(mesh.slice(), encoder);
                    }
                },
            ))
            .drive_unindexed(consumer)
    }
}