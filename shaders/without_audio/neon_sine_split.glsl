// SpectraForge audio-free variant.
//
// Identical visuals to the with_audio version, but the former audio uniforms
// (iBass/iMid/iTreble/iRMS) and the iSpectrum texture are synthesized from
// iTime so the shader animates on its own — no input track drives it.
// Render any input with this shader; audio (if any) is only muxed for sound.
float sfPulse(float speed, float base, float amp) {
    return base + amp * (0.5 + 0.5 * sin(iTime * speed));
}
#define iBass   sfPulse(2.40, 0.10, 0.18)
#define iMid    sfPulse(1.70, 0.10, 0.22)
#define iTreble sfPulse(3.10, 0.04, 0.18)
#define iRMS    sfPulse(2.00, 0.10, 0.16)
// Bass-heavy, gently churning fake spectrum in normalized freq x (0..1).
float sfSpectrum(float x) {
    x = clamp(x, 0.0, 1.0);
    float env = exp(-x * 2.5);
    float wob = 0.5 + 0.5 * sin(iTime * (2.0 + x * 8.0) + x * 20.0);
    return clamp(env * (0.35 + 0.65 * wob), 0.0, 1.0);
}
// Every texture() call in these shaders samples iSpectrum along x; reroute them.
#define texture(tex, uv) vec4(sfSpectrum((uv).x))

// SPDX-License-Identifier: CC-BY-4.0
// Inspired by the CC BY 4.0 sine glow shader by @Tetane.
// Adapted for SpectraForge with audio-reactive neon ribbons.

#define PI 3.14159265359
#define TAU 6.28318530718

float saturate(float x) {
    return clamp(x, 0.0, 1.0);
}

float glow(float x, float strength, float spread) {
    return spread / pow(max(x, 0.0012), strength);
}

float sinSDF(vec2 st, float amp, float offset, float freq, float phase) {
    return abs((st.y - offset) + sin(st.x * freq + phase) * amp);
}

float spectrum(float x) {
    x = clamp(x, 0.0, 1.0);
    float a = texture(iSpectrum, vec2(x, 0.5)).r;
    float b = texture(iSpectrum, vec2(clamp(x + 0.035, 0.0, 1.0), 0.5)).r;
    float c = texture(iSpectrum, vec2(clamp(x - 0.035, 0.0, 1.0), 0.5)).r;
    return max(a, max(b, c) * 0.55);
}

vec3 palette(float t, float y, float time) {
    vec3 a = vec3(0.08, 0.85, 1.00);
    vec3 b = vec3(1.00, 0.12, 0.75);
    vec3 c = vec3(1.00, 0.78, 0.18);
    vec3 d = vec3(0.18, 0.35, 1.00);

    vec3 col = mix(a, b, smoothstep(0.05, 0.72, t));
    col = mix(col, c, smoothstep(0.58, 1.0, sin(y * 3.0 + time + t * TAU) * 0.5 + 0.5));
    col = mix(col, d, 0.20 + 0.20 * sin(time * 0.7 + t * 5.0));
    return col;
}

float splitMask(vec2 st, float time) {
    vec3 wave = cos(6.0 * st.y * vec3(1.0, 2.0, 3.0) - time * vec3(1.0, -1.0, 1.0)) * 0.5;
    float audioBend = iBass * sin(st.y * 18.0 - time * 2.0) * 0.035;
    float cut = st.x + (wave.x + wave.y + wave.z) / 33.0 + audioBend;
    return smoothstep(-0.018, 0.018, cut - 0.5);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 st = fragCoord / iResolution.xy;
    vec2 uv = (fragCoord - 0.5 * iResolution.xy) / iResolution.y;

    float time = iTime * (0.48 + iMid * 0.22);
    float side = sign(st.x - 0.5);
    side = side == 0.0 ? 1.0 : side;

    float bassPulse = smoothstep(0.025, 0.30, iBass);
    float rmsPulse = smoothstep(0.015, 0.20, iRMS);
    float lineEnergy = 0.0;
    vec3 neon = vec3(0.0);

    float amplitudeMod = 0.65 + 0.35 * cos((st.x - 0.5) * PI * (2.4 + iTreble));
    float centerOffset = 0.5
        + sin(st.x * 12.0 + time) * amplitudeMod * (0.040 + 0.020 * bassPulse)
        + sin(st.x * 31.0 - time * 1.7) * 0.008 * rmsPulse;

    const int ribbonCount = 7;
    for (int i = 0; i < ribbonCount; i++) {
        float fi = float(i);
        float band = fi / float(ribbonCount - 1);
        float audio = spectrum(band);

        float phase = -time * side * (1.0 + band * 0.55)
                    + fi * TAU / float(ribbonCount)
                    + audio * 2.6;
        float freq = 5.6 + band * 2.2 + iTreble * 1.8;
        float amp = amplitudeMod * (0.105 + audio * 0.115 + iBass * 0.055);
        float offset = centerOffset + (fi - 3.0) * (0.006 + audio * 0.008);

        float d = sinSDF(st, amp, offset, freq, phase);
        float g = glow(d, 0.62 + audio * 0.12, 0.012 + audio * 0.030 + iRMS * 0.006);
        vec3 tint = palette(band, st.y, time);

        lineEnergy += g;
        neon += tint * g * (0.75 + audio * 1.8);
    }

    float lines = tanh(lineEnergy * 0.55);
    neon = tanh(neon * 0.42);

    vec3 bg = vec3(0.008, 0.012, 0.020);
    bg += vec3(0.020, 0.015, 0.045) * (0.65 + uv.y);
    bg += vec3(0.05, 0.02, 0.08) * bassPulse * exp(-dot(uv, uv) * 2.2);

    vec3 color = mix(bg, neon, lines);
    float split = splitMask(st, time);
    vec3 inverted = 1.0 - color;
    color = mix(inverted, color, split);

    float seam = 1.0 - smoothstep(0.0, 0.022, abs(st.x - 0.5));
    color += vec3(1.0, 0.78, 0.35) * seam * (0.06 + iBass * 0.16);

    float scan = 0.96 + 0.04 * sin(fragCoord.y * PI);
    float vignette = smoothstep(1.25, 0.20, length(uv));
    color *= scan * (0.78 + 0.22 * vignette);

    fragColor = vec4(pow(max(color, vec3(0.0)), vec3(0.92)), 1.0);
}
