#version 330 core
layout(std140)	uniform;

layout(location = 0) in vec3	position;
layout(location = 1) in vec3	normal;
layout(location = 2) in vec2	uv;

out VertexData {
	vec4	color;
	vec2	uv;
} VertexOut;

uniform mat4	model;
uniform mat4	view;
uniform mat4	projection;
uniform vec3	diffuse_color;

void main() {
	gl_Position = projection * view * model * vec4(position, 1.0);
	VertexOut.color = vec4(diffuse_color, 1.0);
	VertexOut.uv = uv;
}
