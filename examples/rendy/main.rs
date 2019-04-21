//! Displays spheres with physically based materials.
//!
use amethyst::{
    animation::{
        get_animation_set, AnimationBundle, AnimationCommand, AnimationControlSet, AnimationSet,
        EndControl, VertexSkinningBundle,
    },
    assets::{
        AssetLoaderSystemData, Completion, PrefabLoader, PrefabLoaderSystem, Processor,
        ProgressCounter, RonFormat,
    },
    controls::{FlyControlBundle, FlyControlTag},
    core::{
        ecs::{
            Component, DenseVecStorage, Entities, Entity, Join, Read, ReadExpect, ReadStorage,
            Resources, System, SystemData, Write, WriteStorage,
        },
        math::{Unit, UnitQuaternion, Vector3},
        Time, Transform, TransformBundle,
    },
    gltf::GltfSceneLoaderSystem,
    input::{is_close_requested, is_key_down, Axis, Bindings, Button, InputBundle},
    prelude::*,
    utils::{application_root_dir, fps_counter::FPSCounterBundle, tag::TagFinder},
    window::{EventsLoopSystem, ScreenDimensions, WindowSystem},
    winit::{EventsLoop, Window},
    ui::{UiBundle, UiButtonBuilder},
};
use amethyst_rendy::{
    camera::{ActiveCamera, Camera, Projection},
    light::{Light, PointLight},
    mtl::{Material, MaterialDefaults},
    palette::{LinSrgba, Srgb},
    pass::{DrawFlat2DDesc, DrawPbrDesc, DrawUiDesc},
    rendy::{
        factory::Factory,
        graph::{
            present::PresentNode,
            render::{RenderGroupDesc, SimpleGraphicsPipelineDesc, SubpassBuilder},
            GraphBuilder,
        },
        hal::{
            command::{ClearDepthStencil, ClearValue},
            format::Format,
            pso, Backend,
        },
        mesh::PosNormTangTex,
        texture::palette::load_from_linear_rgba,
    },
    resources::Tint,
    shape::Shape,
    sprite::{SpriteRender, SpriteSheet},
    sprite_visibility::SpriteVisibilitySortingSystem,
    system::{GraphCreator, RendererSystem},
    types::{DefaultBackend, Mesh, Texture},
};
use std::{marker::PhantomData, path::Path, sync::Arc};

use prefab_data::{AnimationMarker, Scene, ScenePrefabData, SpriteAnimationId};

mod prefab_data;

struct Example<B: Backend> {
    entity: Option<Entity>,
    initialised: bool,
    progress: Option<ProgressCounter>,
    marker: PhantomData<B>,
}

impl<B: Backend> Example<B> {
    pub fn new() -> Self {
        Self {
            entity: None,
            initialised: false,
            progress: None,
            marker: PhantomData,
        }
    }
}

struct Orbit {
    axis: Unit<Vector3<f32>>,
    time_scale: f32,
    center: Vector3<f32>,
    radius: f32,
}

impl Component for Orbit {
    type Storage = DenseVecStorage<Self>;
}

struct OrbitSystem;

impl<'a> System<'a> for OrbitSystem {
    type SystemData = (
        Read<'a, Time>,
        ReadStorage<'a, Orbit>,
        WriteStorage<'a, Transform>,
    );

    fn run(&mut self, (time, orbits, mut transforms): Self::SystemData) {
        for (orbit, transform) in (&orbits, &mut transforms).join() {
            let angle = time.absolute_time_seconds() as f32 * orbit.time_scale;
            let cross = orbit.axis.cross(&Vector3::z()).normalize() * orbit.radius;
            let rot = UnitQuaternion::from_axis_angle(&orbit.axis, angle);
            let final_pos = (rot * cross) + orbit.center;
            transform.set_translation(final_pos);
        }
    }
}

struct CameraCorrectionSystem {
    last_aspect: f32,
}

impl CameraCorrectionSystem {
    pub fn new() -> Self {
        Self { last_aspect: 0.0 }
    }
}

impl<'a> System<'a> for CameraCorrectionSystem {
    type SystemData = (
        ReadExpect<'a, ScreenDimensions>,
        ReadExpect<'a, ActiveCamera>,
        WriteStorage<'a, Camera>,
    );

    fn run(&mut self, (dimensions, active_cam, mut cameras): Self::SystemData) {
        let current_aspect = dimensions.aspect_ratio();

        if current_aspect != self.last_aspect {
            self.last_aspect = current_aspect;

            let camera = cameras.get_mut(active_cam.entity).unwrap();
            *camera = Camera::from(Projection::perspective(
                current_aspect,
                std::f32::consts::FRAC_PI_3,
                0.1,
                100.0,
            ));
        }
    }
}

impl<B: Backend> SimpleState for Example<B> {
    fn on_start(&mut self, data: StateData<'_, GameData<'_, '_>>) {
        let StateData { world, .. } = data;

        let mat_defaults = world.read_resource::<MaterialDefaults<B>>().0.clone();

        self.progress = Some(ProgressCounter::default());

        world.exec(
            |(loader, mut scene): (PrefabLoader<'_, ScenePrefabData<B>>, Write<'_, Scene<B>>)| {
                scene.handle = Some(
                    loader.load(
                        Path::new("prefab")
                            .join("rendy_example_scene.ron")
                            .to_string_lossy(),
                        RonFormat,
                        (),
                        self.progress.as_mut().unwrap(),
                    ),
                );
            },
        );

        let (mesh, albedo) = {
            let mesh = world.exec(|loader: AssetLoaderSystemData<'_, Mesh<B>>| {
                loader.load_from_data(
                    Shape::Sphere(16, 16).generate::<Vec<PosNormTangTex>>(None),
                    self.progress.as_mut().unwrap(),
                )
            });
            let albedo = world.exec(|loader: AssetLoaderSystemData<'_, Texture<B>>| {
                loader.load_from_data(
                    load_from_linear_rgba(LinSrgba::new(1.0, 1.0, 1.0, 1.0)),
                    self.progress.as_mut().unwrap(),
                )
            });

            (mesh, albedo)
        };

        println!("Create spheres");
        const NUM_ROWS: usize = 25;
        const NUM_COLS: usize = 25;

        let mut mtls = Vec::with_capacity(100);

        for i in 0..10 {
            for j in 0..10 {
                if mtls.len() >= NUM_ROWS + NUM_COLS - 1 {
                    break;
                }

                let roughness = i as f32 / 9.0;
                let metallic = j as f32 / 9.0;

                let mtl = world.exec(
                    |(mtl_loader, tex_loader): (
                        AssetLoaderSystemData<'_, Material<B>>,
                        AssetLoaderSystemData<'_, Texture<B>>,
                    )| {
                        let metallic_roughness = tex_loader.load_from_data(
                            load_from_linear_rgba(LinSrgba::new(0.0, roughness, metallic, 0.0)),
                            self.progress.as_mut().unwrap(),
                        );

                        mtl_loader.load_from_data(
                            Material {
                                albedo: albedo.clone(),
                                metallic_roughness,
                                ..mat_defaults.clone()
                            },
                            self.progress.as_mut().unwrap(),
                        )
                    },
                );
                mtls.push(mtl);
            }
        }

        for i in 0..NUM_COLS {
            for j in 0..NUM_ROWS {
                let x = i as f32 / (NUM_COLS - 1) as f32;
                let y = j as f32 / (NUM_ROWS - 1) as f32;

                let center = Vector3::new(15.0 * (x - 0.5), 15.0 * (y - 0.5), -5.0);

                let mut pos = Transform::default();
                pos.set_translation(center);
                pos.set_scale(0.2, 0.2, 0.2);

                let mut builder = world
                    .create_entity()
                    .with(pos)
                    .with(mesh.clone())
                    .with(mtls[(j + i) % mtls.len()].clone())
                    .with(Orbit {
                        axis: Unit::new_normalize(Vector3::y()),
                        time_scale: 5.0 + y + 0.1 * x,
                        center,
                        radius: 0.2,
                    });

                // add some visible tint pattern
                if i > 10 && j > 10 && i < NUM_COLS - 10 && j < NUM_ROWS - 10 {
                    let xor_x = i - 10;
                    let xor_y = j - 10;
                    let c = ((xor_x ^ xor_y) & 0xFF) as f32 / 255.0;
                    builder = builder.with(Tint(Srgb::new(c, c, c).into()));
                }

                builder.build();
            }
        }

        println!("Create lights");
        let light1: Light = PointLight {
            intensity: 6.0,
            color: Srgb::new(0.8, 0.0, 0.0),
            ..PointLight::default()
        }
        .into();

        let mut light1_transform = Transform::default();
        light1_transform.set_translation_xyz(6.0, 6.0, 6.0);

        let light2: Light = PointLight {
            intensity: 5.0,
            color: Srgb::new(0.0, 0.3, 0.7),
            ..PointLight::default()
        }
        .into();

        let mut light2_transform = Transform::default();
        light2_transform.set_translation_xyz(6.0, -6.0, 6.0);

        let light3: Light = PointLight {
            intensity: 4.0,
            color: Srgb::new(0.5, 0.5, 0.5),
            ..PointLight::default()
        }
        .into();

        let mut light3_transform = Transform::default();
        light3_transform.set_translation_xyz(-3.0, 10.0, 2.0);

        world
            .create_entity()
            .with(light1)
            .with(light1_transform)
            .with(Orbit {
                axis: Unit::new_normalize(Vector3::x()),
                time_scale: 2.0,
                center: Vector3::new(6.0, -6.0, -6.0),
                radius: 5.0,
            })
            .build();

        world
            .create_entity()
            .with(light2)
            .with(light2_transform)
            .build();

        world
            .create_entity()
            .with(light3)
            .with(light3_transform)
            .build();

        let mut transform = Transform::default();
        transform.set_translation_xyz(0.0, 4.0, 8.0);
        transform.prepend_rotation_x_axis(std::f32::consts::PI * -0.0625);

        let camera = world
            .create_entity()
            .with(Camera::from(Projection::perspective(
                1.3,
                std::f32::consts::FRAC_PI_3,
                0.1,
                100.0,
            )))
            .with(transform)
            .with(FlyControlTag)
            .build();

        world.add_resource(ActiveCamera { entity: camera });

        let _ = UiButtonBuilder::<(), u32>::new("hello rendy")
            .with_size(200., 30.)
            .with_position(100., 15.)
            .build_from_world(&world);

        // let (width, height) = {
        //     let dim = world.read_resource::<ScreenDimensions>();
        //     (dim.width(), dim.height())
        // };

        // let mut camera_transform = Transform::default();
        // camera_transform.set_translation_z(1.0);

        // let sprite_camera = world
        //     .create_entity()
        //     .with(camera_transform)
        //     .with(Camera::from(Projection::orthographic(
        //         0.0, width, 0.0, height,
        //     )))
        //     .build();

        // world.add_resource(SpriteCamera {
        //     entity: sprite_camera,
        // });

        // println!("Create sprites");
        // // Sprites
        // let sprite_prefab_handle =
        //     world.exec(|loader: PrefabLoader<'_, SpriteScenePrefabData<B>>| {
        //         loader.load(
        //             "prefab/sprite_animation.ron",
        //             RonFormat,
        //             (),
        //             self.progress.as_mut().unwrap(),
        //         )
        //     });

        // // Creates new entities with components from MyPrefabData
        // world.create_entity().with(sprite_prefab_handle).build();
    }

    fn handle_event(
        &mut self,
        data: StateData<'_, GameData<'_, '_>>,
        event: StateEvent,
    ) -> SimpleTrans {
        let StateData { world, .. } = data;
        if let StateEvent::Window(event) = &event {
            if is_close_requested(&event) || is_key_down(&event, winit::VirtualKeyCode::Escape) {
                Trans::Quit
            } else if is_key_down(&event, winit::VirtualKeyCode::Space) {
                toggle_or_cycle_animation::<B>(
                    self.entity,
                    &mut world.write_resource(),
                    &world.read_storage(),
                    &mut world.write_storage(),
                );
                Trans::None
            } else {
                Trans::None
            }
        } else {
            Trans::None
        }
    }

    fn update(&mut self, data: &mut StateData<'_, GameData<'_, '_>>) -> SimpleTrans {
        if !self.initialised {
            let remove = match self.progress.as_ref().map(|p| p.complete()) {
                None | Some(Completion::Loading) => false,

                Some(Completion::Complete) => {
                    let scene_handle = data
                        .world
                        .read_resource::<Scene<B>>()
                        .handle
                        .as_ref()
                        .unwrap()
                        .clone();

                    data.world.create_entity().with(scene_handle).build();
                    true
                }

                Some(Completion::Failed) => {
                    println!("Error: {:?}", self.progress.as_ref().unwrap().errors());
                    return Trans::Quit;
                }
            };
            if remove {
                self.progress = None;
            }
            if self.entity.is_none() {
                if let Some(entity) = data
                    .world
                    .exec(|finder: TagFinder<'_, AnimationMarker>| finder.find())
                {
                    self.entity = Some(entity);
                    self.initialised = true;
                }
            }

            data.world.exec(
                |(entities, animation_sets, mut control_sets): (
                    Entities,
                    ReadStorage<AnimationSet<SpriteAnimationId, SpriteRender<B>>>,
                    WriteStorage<AnimationControlSet<SpriteAnimationId, SpriteRender<B>>>,
                )| {
                    // For each entity that has AnimationSet
                    for (entity, animation_set, _) in (&entities, &animation_sets, !&control_sets)
                        .join()
                        .collect::<Vec<_>>()
                    {
                        // Creates a new AnimationControlSet for the entity
                        let control_set = get_animation_set(&mut control_sets, entity).unwrap();
                        // Adds the `Fly` animation to AnimationControlSet and loops infinitely
                        control_set.add_animation(
                            SpriteAnimationId::Fly,
                            &animation_set.get(&SpriteAnimationId::Fly).unwrap(),
                            EndControl::Loop(None),
                            1.0,
                            AnimationCommand::Start,
                        );
                    }
                },
            );
        }
        Trans::None
    }
}

fn toggle_or_cycle_animation<B: Backend>(
    entity: Option<Entity>,
    scene: &mut Scene<B>,
    sets: &ReadStorage<'_, AnimationSet<usize, Transform>>,
    controls: &mut WriteStorage<'_, AnimationControlSet<usize, Transform>>,
) {
    if let Some((entity, Some(animations))) = entity.map(|entity| (entity, sets.get(entity))) {
        if animations.animations.len() > scene.animation_index {
            let animation = animations.animations.get(&scene.animation_index).unwrap();
            let set = get_animation_set::<usize, Transform>(controls, entity).unwrap();
            if set.has_animation(scene.animation_index) {
                set.toggle(scene.animation_index);
            } else {
                println!("Running animation {}", scene.animation_index);
                set.add_animation(
                    scene.animation_index,
                    animation,
                    EndControl::Normal,
                    1.0,
                    AnimationCommand::Start,
                );
            }
            scene.animation_index += 1;
            if scene.animation_index >= animations.animations.len() {
                scene.animation_index = 0;
            }
        }
    }
}

fn main() -> amethyst::Result<()> {
    amethyst::Logger::from_config(amethyst::LoggerConfig {
        log_file: Some("rendy_example.log".into()),
        level_filter: log::LevelFilter::Error,
        ..Default::default()
    })
    // .level_for("amethyst_utils::fps_counter", log::LevelFilter::Debug)
    // .level_for("rendy_memory", log::LevelFilter::Trace)
    // .level_for("rendy_factory", log::LevelFilter::Trace)
    // .level_for("rendy_resource", log::LevelFilter::Trace)
    // .level_for("rendy_graph", log::LevelFilter::Trace)
    // .level_for("rendy_node", log::LevelFilter::Trace)
    // .level_for("amethyst_rendy", log::LevelFilter::Trace)
    // .level_for("gfx_backend_metal", log::LevelFilter::Trace)
    .start();

    let app_root = application_root_dir()?;

    let path = app_root
        .join("examples")
        .join("rendy")
        .join("resources")
        .join("display_config.ron");
    let resources = app_root.join("examples").join("assets");

    let event_loop = EventsLoop::new();

    let mut bidnings = Bindings::new();
    bidnings.insert_axis(
        "vertical",
        Axis::Emulated {
            pos: Button::Key(winit::VirtualKeyCode::S),
            neg: Button::Key(winit::VirtualKeyCode::W),
        },
    )?;
    bidnings.insert_axis(
        "horizontal",
        Axis::Emulated {
            pos: Button::Key(winit::VirtualKeyCode::D),
            neg: Button::Key(winit::VirtualKeyCode::A),
        },
    )?;

    let game_data = GameDataBuilder::default()
        .with(
            WindowSystem::from_config_path(&event_loop, path),
            "window",
            &[],
        )
        .with(OrbitSystem, "orbit", &[])
        .with(CameraCorrectionSystem::new(), "cam", &[])
        // .with_bundle(TransformBundle::new().with_dep(&["orbit"]))?
        .with_bundle(FPSCounterBundle::default())?
        .with(
            PrefabLoaderSystem::<ScenePrefabData<DefaultBackend>>::default(),
            "scene_loader",
            &[],
        )
        .with(
            GltfSceneLoaderSystem::<DefaultBackend>::default(),
            "gltf_loader",
            &["scene_loader"], // This is important so that entity instantiation is performed in a single frame.
        )
        .with(
            Processor::<SpriteSheet<DefaultBackend>>::new(),
            "sprite_sheet_processor",
            &[],
        )
        .with_bundle(
            AnimationBundle::<usize, Transform>::new("animation_control", "sampler_interpolation")
                .with_dep(&["gltf_loader"]),
        )?
        .with_bundle(
            AnimationBundle::<SpriteAnimationId, SpriteRender<DefaultBackend>>::new(
                "sprite_animation_control",
                "sprite_sampler_interpolation",
            )
            .with_dep(&["gltf_loader"]),
        )?
        .with_bundle(InputBundle::<&'static str, &'static str>::new().with_bindings(bidnings))?
        .with_bundle(
            FlyControlBundle::<&'static str, &'static str>::new(
                Some("horizontal"),
                None,
                Some("vertical"),
            )
            .with_sensitivity(0.1, 0.1)
            .with_speed(5.),
        )?
        .with_bundle(TransformBundle::new().with_dep(&[
            "animation_control",
            "sampler_interpolation",
            "sprite_animation_control",
            "sprite_sampler_interpolation",
            "fly_movement",
            "orbit",
        ]))?
        .with_bundle(VertexSkinningBundle::new().with_dep(&[
            "transform_system",
            "animation_control",
            "sampler_interpolation",
        ]))?
        .with_bundle(UiBundle::<String, String>::new())?
        .with(
            SpriteVisibilitySortingSystem::new(),
            "sprite_visibility_system",
            &["fly_movement", "cam", "transform_system"],
        )
        .with_thread_local(EventsLoopSystem::new(event_loop))
        .with_thread_local(RendererSystem::<DefaultBackend, _>::new(ExampleGraph::new()));

    let mut game = Application::new(&resources, Example::<DefaultBackend>::new(), game_data)?;
    game.run();
    Ok(())
}

struct ExampleGraph {
    last_dimensions: Option<ScreenDimensions>,
    dirty: bool,
}

impl ExampleGraph {
    pub fn new() -> Self {
        Self {
            last_dimensions: None,
            dirty: true,
        }
    }
}

impl<B: Backend> GraphCreator<B> for ExampleGraph {
    fn rebuild(&mut self, res: &Resources) -> bool {
        // Rebuild when dimensions change, but wait until at least two frames have the same.
        let new_dimensions = res.try_fetch::<ScreenDimensions>();
        use std::ops::Deref;
        if self.last_dimensions.as_ref() != new_dimensions.as_ref().map(|d| d.deref()) {
            self.dirty = true;
            self.last_dimensions = new_dimensions.map(|d| d.clone());
            return false;
        }
        return self.dirty;
    }

    fn builder(&mut self, factory: &mut Factory<B>, res: &Resources) -> GraphBuilder<B, Resources> {
        self.dirty = false;

        let window = <ReadExpect<'_, Arc<Window>>>::fetch(res);

        let surface = factory.create_surface(window.clone());

        let mut graph_builder = GraphBuilder::new();

        let color = graph_builder.create_image(
            surface.kind(),
            1,
            factory.get_surface_format(&surface),
            Some(ClearValue::Color([0.34, 0.36, 0.52, 1.0].into())),
        );

        let depth = graph_builder.create_image(
            surface.kind(),
            1,
            Format::D16Unorm,
            Some(ClearValue::DepthStencil(ClearDepthStencil(1.0, 0))),
        );

        let subpass = SubpassBuilder::new()
            .with_group(
                DrawPbrDesc::default()
                    .with_vertex_skinning()
                    .with_transparency(
                        pso::ColorBlendDesc(pso::ColorMask::ALL, pso::BlendState::ALPHA),
                        Some(pso::DepthStencilDesc {
                            depth: pso::DepthTest::On {
                                fun: pso::Comparison::Less,
                                write: true,
                            },
                            depth_bounds: false,
                            stencil: pso::StencilTest::Off,
                        }),
                    )
                    .builder(),
            )
            .with_group(
                DrawFlat2DDesc::default()
                    .with_transparency(
                        pso::ColorBlendDesc(pso::ColorMask::ALL, pso::BlendState::ALPHA),
                        Some(pso::DepthStencilDesc {
                            depth: pso::DepthTest::On {
                                fun: pso::Comparison::Less,
                                write: true,
                            },
                            depth_bounds: false,
                            stencil: pso::StencilTest::Off,
                        }),
                    )
                    .builder(),
            )
            .with_group(DrawUiDesc::default().builder())
            .with_color(color)
            .with_depth_stencil(depth);

        let pass = graph_builder.add_node(subpass.into_pass());
        let present_builder = PresentNode::builder(factory, surface, color).with_dependency(pass);

        graph_builder.add_node(present_builder);

        graph_builder
    }
}
