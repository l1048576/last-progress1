#version 330 core
#ifdef  GL_FRAGMENT_PRECISION_HIGH
precision highp	float;
#else
precision lowp	float;
#endif

layout(std140)	uniform;

uniform sampler2D	diffuse_texture;
uniform bool	diffuse_texture_available;

in VertexData {
	vec4	color;
	vec2	uv;
} VertexIn;

out vec4	color;

void main() {
	if(diffuse_texture_available) {
		color = texture(diffuse_texture, VertexIn.uv);
	} else {
		color = VertexIn.color;
	}
}
