extern crate cgmath;
extern crate env_logger;
extern crate fbx_load;
extern crate fnv;
#[macro_use]
extern crate glium;
extern crate image;
#[macro_use]
extern crate log;
extern crate time;

use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use fnv::FnvHasher;

mod drawable;


pub type FnvHashMap<K, V> = HashMap<K, V, BuildHasherDefault<FnvHasher>>;

pub struct FormatConverter;

impl FormatConverter {
    pub fn new() -> Self {
        FormatConverter
    }
}

impl fbx_load::FormatConvert for FormatConverter {
    type ImageResult = image::ImageResult<image::DynamicImage>;

    fn binary_to_image(&mut self, binary: &[u8], path: &::std::path::Path) -> Self::ImageResult {
        debug!("Loading embedded texture: path=`{}`, binary size={}", path.display(), binary.len());
        if let Some(extension) = path.extension().and_then(|s| s.to_str()).map(|v| v.to_lowercase()) {
            match extension.as_ref() {
                "tga" => image::load_from_memory_with_format(binary, image::ImageFormat::TGA),
                "jpg" => image::load_from_memory_with_format(binary, image::ImageFormat::JPEG),
                "png" => image::load_from_memory_with_format(binary, image::ImageFormat::PNG),
                "gif" => image::load_from_memory_with_format(binary, image::ImageFormat::GIF),
                _ => {
                    info!("Unknown file extension in texture path: `{}`", path.display());
                    image::load_from_memory(binary)
                },
            }
        } else {
            warn!("Texture path without file extension: `{}`", path.display());
            image::load_from_memory(binary)
        }
    }
}

fn main() {
    use glium::DisplayBuild;

    env_logger::init().expect("Cannot initialize env_logger");

    let format_converter = FormatConverter::new();
    let mut scene = fbx_load::load_from_file("assets/NakanoSisters_1_2_FBX/naka.fbx", format_converter).unwrap();
    scene.triangulate(fbx_load::utils::triangulate_polygon);

    // display: glium::backend::glutin_backend::GlutinFacade
    let display = glium::glutin::WindowBuilder::new()
        .with_dimensions(1024, 768)
        .with_title(format!("Last progress 1"))
        .with_gl_profile(glium::glutin::GlProfile::Core)
        .with_vsync()
        .with_multisampling(4)
        .with_depth_buffer(24)
        .build_glium()
        .expect("Cannot create window");
    debug!("window created");

    let vertex_shader_src = include_str!("default.vert");
    let fragment_shader_src = include_str!("default.frag");
    let program = glium::Program::from_source(&display, vertex_shader_src, fragment_shader_src, None).expect("Cannot create OpenGL shader program");

    #[derive(Clone, Copy)]
    struct Vertex {
        position: [f32; 3],
        normal: [f32; 3],
        uv: [f32; 2],
    }
    implement_vertex!(Vertex, position, normal, uv);

    let drawable_model = {
        // Load textures to VRAM.
        let textures = scene.objects.textures.iter()
            .map(|(&texture_id, texture_node)| {
                use std::borrow::Cow;

                if let Some(ref video) = texture_node.media.as_ref().and_then(|video_name| scene.objects.videos.iter().map(|(&_id, node)| node).find(|n| n.name == *video_name)) {
                    use std::borrow::Borrow;
                    use std::ops::Deref;
                    // Texture might be embedded.
                    use image::GenericImage;

                    // image: Cow<image::DynamicImage>
                    let image = match video.content {
                        Some(ref result) => Cow::Borrowed(result.as_ref().expect("Failed to load embedded texture image")),
                        None => {
                            // Texture is not embedded. Load from file.
                            Cow::Owned(image::open(&video.path).expect("Failed to load external texture image"))
                        },
                    };
                    let image_dimensions = image.dimensions();
                    // NOTE: `to_rgba()` is very heavy.
                    let image_ref = match *image.borrow() {
                        image::DynamicImage::ImageRgb8(ref rgb_image) => glium::texture::RawImage2d {
                            data: Cow::Borrowed(rgb_image.deref()),
                            width: image_dimensions.0,
                            height: image_dimensions.1,
                            format: glium::texture::ClientFormat::U8U8U8,
                        },
                        image::DynamicImage::ImageRgba8(ref rgba_image) => glium::texture::RawImage2d {
                            data: Cow::Borrowed(rgba_image.deref()),
                            width: image_dimensions.0,
                            height: image_dimensions.1,
                            format: glium::texture::ClientFormat::U8U8U8U8,
                        },
                        ref gray_image@image::DynamicImage::ImageLuma8(_) => glium::texture::RawImage2d {
                            data: Cow::Owned(gray_image.to_rgb().into_raw()),
                            width: image_dimensions.0,
                            height: image_dimensions.1,
                            format: glium::texture::ClientFormat::U8U8U8,
                        },
                        ref gray_image@image::DynamicImage::ImageLumaA8(_) => glium::texture::RawImage2d::from_raw_rgba(gray_image.to_rgba().into_raw(), image_dimensions),
                    };
                    drawable::Texture {
                        texture: glium::Texture2d::new(&display, image_ref).unwrap(),
                        sampler_behavior: None,
                    }
                } else {
                    // No texture corresponding to the mesh.
                    panic!("No texture data corresponding to the `Texture` node (id={})", texture_id);
                }
            })
            .collect::<Vec<_>>();
        let texture_id_to_texture_index = scene.objects.textures.iter()
            .enumerate()
            .map(|(texture_index, (&texture_id, _))| (texture_id, texture_index as u32))
            .collect::<FnvHashMap<_, _>>();

        // Load materials.
        let materials = scene.objects.materials.iter()
            .map(|(&material_id, fbx_material)| {
                use fbx_load::objects::{ShadingParameters, PhongParameters};
                let diffuse_texture_id = scene.connections.iter()
                    .filter(|c| c.parent == material_id && c.parent_is_property && !c.child_is_property)
                    .find(|c| c.has_attribute("DiffuseColor"))
                    .map(|c| c.child);
                let diffuse_texture_index = diffuse_texture_id.and_then(|i| texture_id_to_texture_index.get(&i).cloned());
                match fbx_material.shading_parameters {
                    ShadingParameters::Lambert(ref lambert)
                    | ShadingParameters::Phong(PhongParameters { ref lambert, .. })
                    => drawable::LambertMaterial {
                        ambient_color: lambert.ambient,
                        ambient_factor: lambert.ambient_factor,
                        diffuse_color: lambert.diffuse,
                        diffuse_factor: lambert.diffuse_factor,
                        emissive_color: lambert.emissive,
                        emissive_factor: lambert.emissive_factor,
                        diffuse_texture_index: diffuse_texture_index,
                    },
                    ref params => {
                        panic!("Unsupported shading parameter (material id={}): {:?}", material_id, params);
                    },
                }
            })
            .collect::<Vec<_>>();
        let material_id_to_material_index = scene.objects.materials.iter()
            .enumerate()
            .map(|(material_index, (&material_id, _))| (material_id, material_index as u32))
            .collect::<FnvHashMap<_, _>>();

        let meshes = scene.objects.geometry_meshes.iter()
            .flat_map(|(&mesh_id, mesh)| {
                let ref layer0 = mesh.layers[0];
                let ref normals = mesh.layer_element_normals[layer0.normal[0] as usize];
                let ref uvs = mesh.layer_element_uvs[layer0.uv[0] as usize];
                let ref materials = mesh.layer_element_materials[layer0.material[0] as usize];

                let model_id = scene.connections.iter()
                    .filter(|c| c.child == mesh_id && !c.parent_is_property && !c.child_is_property)
                    .filter_map(|c| scene.objects.model_meshes.get(&c.parent)).next()
                    .map(|m| m.id)
                    .unwrap();

                let material_ids = scene.connections.iter()
                    // Iterator<&Connection>
                    .filter(|c| c.parent == model_id && !c.parent_is_property && !c.child_is_property)
                    // Iterator<&Connection> -> Iterator<&Material>
                    .filter_map(|c| scene.objects.materials.get(&c.child))
                    // Iterator<&Material> -> Iterator<i64>
                    .map(|m| m.id)
                    // Iterator<i64> -> Vec<i64>
                    .collect::<Vec<_>>();

                if scene.objects.materials.is_empty() {
                    let _vertices = mesh.triangulated_index_list().iter().enumerate().map(|(pvi, &pv)| {
                            Vertex {
                                position: mesh.vertices[pv as usize],
                                normal: normals.element_of_polygon_vertex(mesh, pvi as usize),
                                uv: uvs.element_of_polygon_vertex(mesh, pvi as usize),
                            }
                        })
                        .collect::<Vec<_>>();
                    // FIXME: Materials may not be read when they have unsupported `ShadingModel`
                    //        (such as "unknown").
                    panic!("not yet unimplemented: Material load failure or unspecified: default/dummy material is required");
                } else {
                    let mut vertices_groups = vec![vec![]; scene.objects.materials.len()];
                    for (pvi, &pv) in mesh.triangulated_index_list().iter().enumerate() {
                        let mesh_local_material_index = materials.element_index_of_polygon_vertex(mesh, pvi as usize);
                        vertices_groups[mesh_local_material_index as usize].push(Vertex {
                            position: mesh.vertices[pv as usize],
                            normal: normals.element_of_polygon_vertex(mesh, pvi as usize),
                            uv: uvs.element_of_polygon_vertex(mesh, pvi as usize),
                        });
                    }
                    material_ids.into_iter().zip(vertices_groups)
                        .map(|(material_id, vertices)| {
                            let vertex_buffer = glium::VertexBuffer::new(&display, &vertices).unwrap();
                            let material_index = material_id_to_material_index.get(&material_id).cloned().unwrap();

                            drawable::Mesh {
                                vertex_buffer: vertex_buffer,
                                material_index: material_index,
                            }
                        })
                }

            })
            .collect::<Vec<_>>();

        drawable::Model {
            meshes: meshes,
            materials: materials,
            textures: textures,
        }
    };

    'window_loop: loop {
        use glium::Surface;

        {
            use cgmath::Angle;

            let bg_color = (0.1, 0.1, 0.1, 1.0);
            // surface: glium::Frame
            let mut surface = display.draw();
            surface.clear_color_and_depth(bg_color, 1.0);

            // Get aspect ratio.
            let aspect_ratio: f64 = {
                let (width, height) = surface.get_dimensions();
                (width as f64) / (height as f64)
            };

            let time_sec = time::precise_time_s();

            // Generate projection matrix.
            let far = 1000f32;
            let near = 0.1f32;
            let perspective_fov = cgmath::PerspectiveFov::<f32> {
                fovy: cgmath::deg(60f32).into(),
                aspect: aspect_ratio as f32,
                near: near,
                far: far,
            };
            let projection_mat = cgmath::Matrix4::from(perspective_fov);

            // Generate view matrix.
            let view = {
                let distance = 200.0;
                let eye_height = 70.0;
                let (sin_t, cos_t) = cgmath::rad(time_sec as f32).sin_cos();
                cgmath::Matrix4::<f32>::look_at(
                    // eye
                    cgmath::Point3::new(distance*cos_t, eye_height, distance*sin_t),
                    // centerPoint
                    cgmath::Point3::new(0.0, eye_height, 0.0),
                    // up
                    cgmath::Vector3::new(0.0, 1.0, 0.0))
            };
            let view_mat = cgmath::Matrix4::from(view);

            // Generate model matrix.
            let model = {
                let scale = 1.0;
                let rot = cgmath::rad(0.0_f32);
                let displacement = cgmath::Vector3::<f32>::new(0.0, 0.0, 0.0);
                cgmath::Decomposed::<_, cgmath::Basis3<f32>> {
                    scale: scale,
                    rot: cgmath::Rotation3::from_angle_y(rot),
                    disp: displacement,
                }
            };
            let model_mat = cgmath::Matrix4::from(model);

            let uniforms = uniform! {
                model: Into::<[[f32; 4]; 4]>::into(model_mat),
                view: Into::<[[f32; 4]; 4]>::into(view_mat),
                projection: Into::<[[f32; 4]; 4]>::into(projection_mat),
            };

            let draw_parameters = glium::DrawParameters {
                depth : glium::Depth {
                    test: glium::DepthTest::IfLess,
                    write: true,
                    .. Default::default()
                },
                .. Default::default()
            };

            // Draw the mesh.
            drawable_model.draw(&mut surface, &program, &uniforms, &draw_parameters);

            surface.finish().expect("surface.finish() failed");
        }

        // Get window events
        for ev in display.poll_events() {
            use glium::glutin::{Event, ElementState, VirtualKeyCode};
            match ev {
                // The window has been closed by the user
                Event::Closed => break 'window_loop,
                Event::KeyboardInput(ElementState::Pressed, _, Some(VirtualKeyCode::Escape)) => break 'window_loop,
                _ => {},
            }
            debug!("event: {:?}", ev);
        }
    }
}
