// Feedback Bloom (audio-free, multipass) for SpectraForge.
//
// SPDX-License-Identifier: CC-BY-NC-SA-4.0
// Compositing pass by @Frostbyte (https://fragcoord.xyz/s/s6qf1xls)
// [LICENSE] https://creativecommons.org/licenses/by-nc-sa/4.0/
//
// Demonstrates SpectraForge multipass: passes are separated by a line starting
// with "//---pass". Each renders to its own texture; later passes sample
// earlier ones via iPass1, iPass2, ... The final pass is the output image.
// Animates on iTime alone; pair with --duration-only.
//
// Pass 1: bright drifting blobs (the "highlights" layer).

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec2 p = (2.0 * fragCoord - iResolution.xy) / iResolution.y;
    float v = 0.0;
    for (int i = 0; i < 5; i++) {
        float fi = float(i);
        vec2 c = 0.7 * vec2(sin(iTime * 0.7 + fi * 1.3), cos(iTime * 0.5 + fi * 2.1));
        v += 0.02 / (length(p - c) + 0.05);
    }
    fragColor = vec4(vec3(clamp(v, 0.0, 1.0)), 1.0);
}

//---pass---
// Pass 2: smooth animated color field (the "tint" layer).

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    vec3 col = 0.5 + 0.5 * cos(iTime + uv.xyx * 6.2831 + vec3(0.0, 2.0, 4.0));
    fragColor = vec4(col, 1.0);
}

//---pass---
// Pass 3: Frostbyte's compositor. u_pass1 -> iPass1, u_pass2 -> iPass2.

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 u = fragCoord / iResolution.xy;
    vec4 o = texture(iPass1, u);
    fragColor = mix(o, texture(iPass2, u) * 0.25, clamp(1.0 - o, 0.0, 1.0));
}
