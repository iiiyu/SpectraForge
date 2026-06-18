// SPDX-License-Identifier: CC-BY-NC-SA-4.0
// Inspired by the CC BY-NC-SA clouds shader by @teadrinker.
// Adapted for SpectraForge as a self-contained, audio-reactive GLSL shader.

float saturate(float x) {
    return clamp(x, 0.0, 1.0);
}

vec2 rot2(vec2 p, float a) {
    float s = sin(a);
    float c = cos(a);
    return vec2(c * p.x - s * p.y, s * p.x + c * p.y);
}

float hash13(vec3 p) {
    p = fract(p * 0.1031);
    p += dot(p, p.yzx + 33.33);
    return fract((p.x + p.y) * p.z);
}

float noise3(vec3 p) {
    vec3 i = floor(p);
    vec3 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);

    float n000 = hash13(i + vec3(0.0, 0.0, 0.0));
    float n100 = hash13(i + vec3(1.0, 0.0, 0.0));
    float n010 = hash13(i + vec3(0.0, 1.0, 0.0));
    float n110 = hash13(i + vec3(1.0, 1.0, 0.0));
    float n001 = hash13(i + vec3(0.0, 0.0, 1.0));
    float n101 = hash13(i + vec3(1.0, 0.0, 1.0));
    float n011 = hash13(i + vec3(0.0, 1.0, 1.0));
    float n111 = hash13(i + vec3(1.0, 1.0, 1.0));

    float nx00 = mix(n000, n100, f.x);
    float nx10 = mix(n010, n110, f.x);
    float nx01 = mix(n001, n101, f.x);
    float nx11 = mix(n011, n111, f.x);
    float nxy0 = mix(nx00, nx10, f.y);
    float nxy1 = mix(nx01, nx11, f.y);
    return mix(nxy0, nxy1, f.z);
}

float fbm(vec3 p) {
    float sum = 0.0;
    float amp = 0.5;
    mat3 warp = mat3(
         0.00,  0.80,  0.60,
        -0.80,  0.36, -0.48,
        -0.60, -0.48,  0.64
    );

    for (int i = 0; i < 5; i++) {
        sum += amp * noise3(p);
        p = warp * p * 2.03 + vec3(17.1, 3.7, 9.2);
        amp *= 0.52;
    }
    return sum;
}

float cloudDensity(vec3 p) {
    float audioLift = 0.28 * iRMS + 0.38 * iBass;
    float ceiling = smoothstep(-0.95, 0.34 + audioLift, p.y);
    float floorFade = 1.0 - smoothstep(1.35 + audioLift, 2.75, p.y);
    float layer = ceiling * floorFade;

    vec3 wind = vec3(0.09, 0.02, 0.26) * iTime;
    vec3 q = p * 0.48 + wind;
    q.xz += vec2(sin(iTime * 0.07), cos(iTime * 0.05)) * 0.85;

    float base = fbm(q);
    float detail = fbm(p * 1.35 + wind.yzx * 1.7);
    float wisps = fbm(p * 3.25 - wind.zxy * 2.1);
    float shaped = base * 0.92 + detail * 0.42 + wisps * 0.16;

    float cutoff = mix(0.77, 0.56, saturate(iBass * 2.2 + iRMS));
    return max(0.0, shaped - cutoff) * layer * (1.75 + iMid * 0.9);
}

vec3 skyColor(vec3 rd, vec3 sunDir) {
    float h = saturate(rd.y * 0.5 + 0.55);
    vec3 dusk = vec3(0.09, 0.06, 0.16);
    vec3 blue = vec3(0.14, 0.25, 0.46);
    vec3 horizon = vec3(0.95, 0.38, 0.22);
    vec3 sky = mix(dusk, blue, h);
    sky = mix(horizon, sky, smoothstep(0.02, 0.58, h));

    float sun = pow(saturate(dot(rd, sunDir)), 420.0);
    float halo = pow(saturate(dot(rd, sunDir)), 12.0);
    sky += vec3(1.0, 0.66, 0.35) * (sun * 3.2 + halo * 0.35 * (1.0 + iTreble));
    return sky;
}

vec4 marchClouds(vec3 ro, vec3 rd, vec3 sunDir, vec2 jitterSeed) {
    float t = 0.7 + hash13(vec3(jitterSeed, iTime)) * 0.28;
    vec3 accum = vec3(0.0);
    float alpha = 0.0;

    for (int i = 0; i < 96; i++) {
        vec3 p = ro + rd * t;
        float d = cloudDensity(p);

        if (d > 0.002) {
            float sunProbe = cloudDensity(p + sunDir * 0.28);
            float light = saturate(0.58 + (d - sunProbe) * 2.5);
            float silver = pow(saturate(dot(rd, sunDir)), 4.0) * 0.45;

            vec3 low = vec3(0.42, 0.34, 0.58);
            vec3 high = vec3(1.0, 0.78, 0.58);
            vec3 cloudCol = mix(low, high, light);
            cloudCol += vec3(0.65, 0.80, 1.0) * silver * (0.4 + iTreble);

            float sliceAlpha = saturate(d * 0.22) * (1.0 - alpha);
            accum += cloudCol * sliceAlpha;
            alpha += sliceAlpha;

            if (alpha > 0.96) {
                break;
            }
        }

        float fastAir = mix(0.19, 0.075, saturate(d * 4.0));
        t += fastAir * (1.0 + t * 0.035);
        if (t > 34.0) {
            break;
        }
    }

    return vec4(accum, alpha);
}

vec3 cameraRay(vec2 uv, vec3 ro, vec3 target, float fov) {
    vec3 ww = normalize(target - ro);
    vec3 uu = normalize(cross(ww, vec3(0.0, 1.0, 0.0)));
    vec3 vv = cross(uu, ww);
    return normalize(uv.x * uu + uv.y * vv + fov * ww);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (fragCoord - 0.5 * iResolution.xy) / iResolution.y;

    float orbit = 0.20 * sin(iTime * 0.045) + iBass * 0.18;
    float glide = iTime * (0.18 + iRMS * 0.12);
    vec3 target = vec3(0.0, 0.78 + iMid * 0.14, 3.8 + glide);
    vec3 ro = vec3(0.0, 0.42 + 0.10 * sin(iTime * 0.08), -4.8 + glide);
    ro.xz = rot2(ro.xz - target.xz, orbit) + target.xz;

    vec3 rd = cameraRay(uv, ro, target, 1.78);
    vec3 sunDir = normalize(vec3(-0.50, 0.18 + iTreble * 0.20, 0.86));

    vec3 sky = skyColor(rd, sunDir);
    vec4 clouds = marchClouds(ro, rd, sunDir, fragCoord);
    vec3 col = mix(sky, clouds.rgb, clouds.a);

    float bassPulse = smoothstep(0.03, 0.32, iBass);
    float vignette = smoothstep(1.32, 0.25, length(uv));
    col += vec3(0.20, 0.08, 0.35) * bassPulse * vignette * 0.22;

    col = pow(max(col, vec3(0.0)), vec3(0.92));
    col *= 0.78 + 0.22 * vignette;
    fragColor = vec4(col, 1.0);
}
