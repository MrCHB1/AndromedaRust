#version 330

out vec4 fragColor;

in vec2 uv;
in vec3 color;
in vec3 color2;

in float noteWidth;
in float noteHeight;

uniform float width;
uniform float height;

void main() {
    vec3 n_color = color;
    float border_width = 1.5;
    if (uv.x * noteWidth <= border_width / width || (1.0 - uv.x) * noteWidth <= border_width / width || uv.y * noteHeight <= border_width / height || (1.0 - uv.y) * noteHeight <= border_width / height) {
        n_color = color2 / 2.0;
    }

    fragColor = vec4(n_color, 1.0);
}