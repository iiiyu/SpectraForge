// FFT Terrain for SpectraForge.
//
// Inspired by spectrogram height-field shaders, adapted to SpectraForge's
// current shader inputs. iSpectrum is a 64x1 texture containing the current
// frame's frequency magnitudes, so this shader synthesizes the receding
// "history" axis from time, phase offsets, and decay rather than sampling a
// true spectrogram texture.

float rand1(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

float spectrum(float f) {
    f = clamp(f, 0.0, 1.0);
    float a = texture(iSpectrum, vec2(f, 0.5)).r;
    float b = texture(iSpectrum, vec2(clamp(f + 0.018, 0.0, 1.0), 0.5)).r;
    float c = texture(iSpectrum, vec2(clamp(f - 0.018, 0.0, 1.0), 0.5)).r;
    return max(a, 0.35 * (b + c));
}

float terrainHeight(float f, float z) {
    float bassBias = exp(-f * 3.0);
    float raw = spectrum(f);

    // Fake time-history: nearby frequencies drift backward along z at
    // different rates, then decay into the distance.
    float driftA = spectrum(fract(f + 0.020 * sin(iTime * 0.35 - z * 0.90)));
    float driftB = spectrum(fract(f * 0.82 + 0.075 * sin(iTime * 0.18 + z * 1.30)));
    float ridge = max(raw, max(driftA * 0.72, driftB * 0.48));

    float scan = 0.55 + 0.45 * sin(iTime * (0.75 + f) - z * (2.4 + f * 3.2));
    float beat = 0.35 + 0.65 * smoothstep(0.02, 0.20, iBass + bassBias * iRMS);
    float falloff = exp(-z * 0.16);

    return pow(ridge, 0.72) * (0.35 + 1.25 * scan) * beat * falloff * 2.55;
}

vec3 palette(float f, float h, float z) {
    vec3 cold = vec3(0.04, 0.19, 0.34);
    vec3 cyan = vec3(0.05, 0.85, 0.95);
    vec3 magenta = vec3(0.92, 0.12, 0.68);
    vec3 gold = vec3(1.00, 0.70, 0.18);

    vec3 col = mix(cyan, magenta, smoothstep(0.18, 0.90, f));
    col = mix(col, gold, smoothstep(0.72, 1.90, h) * (0.25 + iTreble));
    col = mix(cold, col, 0.35 + 0.65 * exp(-z * 0.10));
    return col;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (fragCoord - 0.5 * iResolution.xy) / iResolution.y;

    float orbit = 1.35 + 0.22 * sin(iTime * 0.08);
    float pitch = 1.10 + 0.11 * sin(iTime * 0.11 + 0.8);
    float radius = 10.6 - 1.2 * smoothstep(0.02, 0.22, iBass);

    vec3 target = vec3(0.0, 0.72, 3.6);
    vec3 ro = target + radius * vec3(
        sin(pitch) * sin(orbit),
        cos(pitch),
       -sin(pitch) * cos(orbit)
    );

    vec3 ww = normalize(target - ro);
    vec3 uu = normalize(cross(ww, vec3(0.0, 1.0, 0.0)));
    vec3 vv = cross(uu, ww);
    vec3 rd = normalize(uv.x * uu + uv.y * vv + 1.95 * ww);

    vec3 col = vec3(0.006, 0.014, 0.025) + vec3(0.01, 0.025, 0.045) * (uv.y + 0.9);
    float t = 1.1 + 0.08 * rand1(fragCoord + iTime);

    for (int step = 0; step < 118; step++) {
        vec3 p = ro + rd * t;

        float f = abs(p.x) / 4.0; // mirrored frequency: bass at world x = 0
        float z = p.z / 8.0;      // now near the viewer, past into the scene

        if (f > 1.0 || z < 0.0 || z > 1.0) {
            t += 0.16;
        } else {
            float h = terrainHeight(f, p.z);
            float distToSurface = p.y - h;
            float fft = spectrum(f);
            vec3 terrainCol = palette(f, h, p.z);

            float glow = (fft + 0.012 + iRMS * 0.08)
                       / ((abs(distToSurface) + 0.075) * (abs(distToSurface) + 0.075))
                       / (1.0 + t * 1.45);
            col += terrainCol * glow * 0.22;

            float gridZ = min(fract(p.z * 3.0), 1.0 - fract(p.z * 3.0));
            float gridX = min(fract(abs(p.x) * 2.0), 1.0 - fract(abs(p.x) * 2.0));
            float wire = 1.0 - smoothstep(0.0, 0.035, gridZ);
            wire += 1.0 - smoothstep(0.0, 0.030, gridX);
            col += terrainCol * wire * 0.018 / (1.0 + t * 0.45);

            t += max(abs(distToSurface) * 0.075, 0.004);
        }

        if (t > 24.0) {
            break;
        }
    }

    // Center bass flare and subtle vignette.
    float center = exp(-dot(uv, uv) * 2.8);
    col += vec3(0.10, 0.32, 0.55) * center * (0.08 + iBass * 0.75);
    col *= 0.80 + 0.20 * (1.0 - smoothstep(0.1, 1.3, length(uv)));

    fragColor = vec4(tanh(col), 1.0);
}
