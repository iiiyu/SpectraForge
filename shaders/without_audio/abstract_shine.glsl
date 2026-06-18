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

// Abstract Shine for SpectraForge.
//
// Based on "Abstract Shine" by @Frostbyte (CC-BY-NC-SA-4.0)
// https://fragcoord.xyz/s/agoj2i8q
// Adapted to SpectraForge's mainImage entry point and audio uniforms.
// SPDX-License-Identifier: CC-BY-NC-SA-4.0

// Compact 2D rotation built from cosine phase offsets (no sin()):
// cos(a+33) ~= -sin(a), cos(a+11) ~= sin(a).
#define R(a) mat2(cos((a) + vec4(0.0, 33.0, 11.0, 0.0)))

float sampleSpectrum(float x) {
    x = clamp(x, 0.0, 1.0);
    float a = texture(iSpectrum, vec2(x, 0.5)).r;
    float b = texture(iSpectrum, vec2(clamp(x + 0.025, 0.0, 1.0), 0.5)).r;
    return max(a, b * 0.65);
}

// IQ's continuous cosine palette (MIT). https://www.shadertoy.com/view/ll2GD3
vec3 palette(float i) {
    const vec3 a = vec3(0.50, 0.38, 0.26);
    const vec3 b = vec3(0.50, 0.35, 0.25);
    const vec3 c = vec3(1.00);
    const vec3 d = vec3(0.00, 0.12, 0.25);
    return a + b * cos(6.2831853 * (c * i + d));
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 u = fragCoord;
    vec2 uv = (u - 0.5 * iResolution.xy + 0.5) / iResolution.y;

    float t = iTime * (1.0 + iMid * 0.6);
    float bassPulse = 1.0 + iBass * 2.0;

    // Camera ray through pixel.
    vec3 d = normalize(vec3(2.0 * u - iResolution.xy, iResolution.y));
    vec3 p = vec3(0.0, 0.0, iTime);

    fragColor = vec4(0.0);
    float s;

    for (int i = 0; i < 20; i++) {
        // Depth-dependent rotation -> corkscrew tunnel motion.
        p.xy *= R(-p.z * 0.01 - iTime * 0.05);

        // Spectrum lookup along the tunnel, indexed by depth.
        float freq = fract(p.z * 0.02 + length(p.xy) * 0.03);
        float audio = sampleSpectrum(freq);

        s = 0.6;
        // Cylindrical confinement -> tunnel wall at radius ~10.
        s = max(s, 4.0 * (-length(p.xy) + 10.0));
        // Organic deformation field, energized by the spectrum.
        s += abs(
            p.y * 0.004 +
            sin(t - p.x * 0.5) * 0.9 +
            1.0 - audio * (0.8 + iTreble * 1.5)
        );

        p += d * s;
        fragColor += 1.0 / (s * 0.2);
    }

    // Depth-dependent coloration.
    fragColor *= vec4(palette(length(p) / (abs(sin(iTime * 0.02) * 50.0) + 6.0)), 1.0);

    // Beat-gated screen-space shimmer / interference layer.
    float gate = abs(sin(iTime * 5.0)) * (0.4 + iBass * 1.6);
    fragColor -= 20.0 * smoothstep(
        0.001,
        max(gate, 0.001),
        0.7 - length(sin(uv * 200.0) / 1.5) - abs(uv.y) + 0.2
    );

    fragColor /= 50.0;

    // Vignette.
    float l = length(uv);
    fragColor *= 1.2 - l;

    // Center glow.
    fragColor = mix(fragColor, palette(l - 0.23).rgbr, 1.0 - smoothstep(0.01, 0.95, l));

    // Soft highlight compression.
    fragColor = tanh(fragColor + fragColor);
}
