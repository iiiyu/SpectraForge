// Polar Tunnel for SpectraForge.
//
// Based on a compact cos/sin raymarch shader, adapted to SpectraForge's
// Shadertoy-style mainImage entry point and audio uniforms.

float sampleSpectrum(float x) {
    x = clamp(x, 0.0, 1.0);
    float a = texture(iSpectrum, vec2(x, 0.5)).r;
    float b = texture(iSpectrum, vec2(clamp(x + 0.025, 0.0, 1.0), 0.5)).r;
    return max(a, b * 0.65);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec3 rayBase = vec3(fragCoord, 1.0);
    vec3 resolution = vec3(iResolution, 1.0);
    vec4 color = vec4(0.0);

    float z = 0.0;
    float time = iTime * (0.72 + iMid * 0.65);
    float bassPulse = 1.0 + iBass * 2.4;

    for (int step = 0; step < 100; step++) {
        vec3 p = z * (rayBase * 2.0 - resolution.xyy) / resolution.y;
        p.z += 2.0 + iBass * 0.55;

        float lenP = max(length(p), 0.001);
        vec3 v = vec3(
            atan(p.x, p.z),
            atan(p.y, length(p.xz)),
            log(lenP)
        );

        float freq = fract(abs(v.x) * 0.15915494 + v.z * 0.045);
        float audio = sampleSpectrum(freq);
        float audioWarp = audio * (0.85 + iTreble * 1.6);

        vec3 q = v * (7.2 + bassPulse) + time;
        vec3 wave = cos(q + vec3(0.0, 1.7, 3.4) * audioWarp)
                  + sin(v.yzx + v + time - lenP + audioWarp * 3.0);

        float d = length(wave) * lenP * (0.020 + 0.015 * bassPulse);
        d = max(d, 0.004);
        z += d;

        float glow = (0.012 + audio * 0.080 + iRMS * 0.035) / d;
        vec4 tint = cos(v.z + lenP * vec4(3.0, 2.0, 1.0, 0.0) + vec4(0.0, 0.8, 1.6, 0.0)) + 1.0;
        color += tint * glow;

        if (z > 34.0) {
            break;
        }
    }

    vec2 uv = (fragCoord - 0.5 * iResolution.xy) / iResolution.y;
    float vignette = 1.0 - smoothstep(0.55, 1.45, length(uv));
    color.rgb *= 0.65 + vignette * 0.55;

    fragColor = vec4(tanh(color.rgb / 420.0), 1.0);
}
