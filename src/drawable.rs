//! Contains types and functions for drawable objects.

use glium;

pub struct Mesh<T: Copy> {
    pub vertex_buffer: glium::VertexBuffer<T>,
    pub material_index: u32,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct LambertMaterial {
    pub ambient_color: [f32; 3],
    pub ambient_factor: f32,
    pub diffuse_color: [f32; 3],
    pub diffuse_factor: f32,
    pub emissive_color: [f32; 3],
    pub emissive_factor: f32,
    pub diffuse_texture_index: Option<u32>,
}

pub struct Texture {
    pub texture: glium::texture::Texture2d,
    pub sampler_behavior: Option<glium::uniforms::SamplerBehavior>,
}

pub struct ModelUniforms<'a, T: 'a + Copy, U: 'a + glium::uniforms::Uniforms> {
    base_uniforms: &'a U,
    model: &'a Model<T>,
    material_index: u32,
}

impl<'a, T: Copy, U: 'a + glium::uniforms::Uniforms> ModelUniforms<'a, T, U> {
    pub fn new(base_uniforms: &'a U, model: &'a Model<T>, material_index: u32) -> Self {
        ModelUniforms {
            base_uniforms: base_uniforms,
            model: model,
            material_index: material_index,
        }
    }
}

impl<'a, T: 'a + Copy, U: 'a + glium::uniforms::Uniforms> glium::uniforms::Uniforms for ModelUniforms<'a, T, U> {
    fn visit_values<'b, F: FnMut(&str, glium::uniforms::UniformValue<'b>)>(&'b self, mut fun: F) {
        use glium::uniforms::UniformValue;
        let ref material = self.model.materials[self.material_index as usize];
        fun("ambient_color", UniformValue::Vec3(material.ambient_color));
        fun("ambient_factor", UniformValue::Float(material.ambient_factor));
        fun("diffuse_color", UniformValue::Vec3(material.diffuse_color));
        fun("diffuse_factor", UniformValue::Float(material.diffuse_factor));
        fun("emissive_color", UniformValue::Vec3(material.emissive_color));
        fun("emissive_factor", UniformValue::Float(material.emissive_factor));
        fun("diffuse_texture_available", UniformValue::Bool(material.diffuse_texture_index.is_some()));
        if let Some(texture_index) = material.diffuse_texture_index {
            let ref texture = self.model.textures[texture_index as usize];
            fun("diffuse_texture", UniformValue::Texture2d(&texture.texture, texture.sampler_behavior));
        }
        self.base_uniforms.visit_values(fun);
    }
}

pub struct Model<T: Copy> {
    pub meshes: Vec<Mesh<T>>,
    pub materials: Vec<LambertMaterial>,
    pub textures: Vec<Texture>,
}

impl<T: Copy> Model<T> {
    pub fn draw<S, U>(&self, surface: &mut S, program: &glium::Program, base_uniforms: &U, draw_parameters: &glium::DrawParameters)
        where S: glium::Surface,
              U: glium::uniforms::Uniforms
    {
        let indices = glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList);
        for mesh in &self.meshes {
            let uniforms = ModelUniforms::new(base_uniforms, &self, mesh.material_index);
            surface.draw(&mesh.vertex_buffer, &indices, &program, &uniforms, &draw_parameters).unwrap();
        }
    }
}
