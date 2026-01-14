#version 330
layout (location = 0) in vec2 vPos;

layout (location = 1) in vec4 noteRect;
layout (location = 2) in uint noteMeta;

out vec2 uv;
out vec3 color;
out vec3 color2;

out float noteWidth;
out float noteHeight;

uniform sampler2D noteColorTexture;
uniform float keyboardHeight;
uniform float width;

void main() {
    vec3 n_color = texture2D(noteColorTexture, vec2(float(noteMeta & uint(0xF)) / 16.0, 0.5)).rgb;
    color2 = n_color * 0.5;

    n_color = mix(
        vec3(1.0),
        n_color,
        float((noteMeta & uint(0xFF0)) >> uint(4)) / 128.0
    );

    if ((noteMeta & uint(1 << 13)) != uint(0)) {
        n_color = vec3(1.0, 0.5, 0.5);
        color2 = vec3(0.9, 0.4, 0.4);
    }

    float grayFactor = float((noteMeta & (uint(3) << uint(14))) >> uint(14)) / 2.0;
    n_color = mix(n_color, vec3(0.5, 0.5, 0.5), grayFactor);
    color2 = mix(color2, vec3(0.5) / 2.0, grayFactor);

    if ((noteMeta & uint(1 << 12)) != uint(0)) {
        n_color += vec3(0.5);
        color2 += 0.25;
    }

    color = n_color;

    // color = noteColor;
    // color2 = noteColor2;
    vec2 uv_;
    float x_pos = 0.0f;
    float y_pos = 0.0f;

    float noteStart = noteRect.x;
    float noteLength = noteRect.y;
    float noteBottom = noteRect.z;
    float noteTop = noteRect.w;

    noteWidth = noteLength;
    noteHeight = noteTop - noteBottom;

    if (int(gl_VertexID % 4) == 0) {
        x_pos = noteStart;
        y_pos = noteBottom;
        uv_ = vec2(0.0, 0.0);
    } else if (int(gl_VertexID % 4) == 1) {
        x_pos = noteStart + noteLength;
        y_pos = noteBottom;
        uv_ = vec2(1.0, 0.0);
    } else if (int(gl_VertexID % 4) == 2) {
        x_pos = noteStart + noteLength;
        y_pos = noteTop;
        uv_ = vec2(1.0, 1.0);
    } else if (int(gl_VertexID % 4) == 3) {
        x_pos = noteStart;
        y_pos = noteTop;
        uv_ = vec2(0.0, 1.0);
    }

    uv = uv_;
    float kbWidth = keyboardHeight / width;
    x_pos = (x_pos * (1.0 - kbWidth)) + kbWidth;
    gl_Position = vec4(vec2(x_pos, y_pos) * 2.0 - 1.0, 0.0, 1.0);
}