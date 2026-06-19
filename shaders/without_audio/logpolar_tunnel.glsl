// Log-Polar Tunnel (audio-free) for SpectraForge.
//
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 @YoheiNishitsuji
// [LICENSE] https://opensource.org/licenses/MIT
// Adapted to SpectraForge's mainImage entry point (u_time -> iTime,
// u_resolution -> iResolution). Animates on iTime alone; pair with
// --duration-only.

vec3 hsv(float h, float s, float v) {
    vec4 t = vec4(1., 2./3., 1./3., 3.);
    vec3 p = abs(fract(vec3(h) + t.xyz) * 6. - vec3(t.w));
    return v * mix(vec3(t.x), clamp(p - vec3(t.x), 0., 1.), s);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2  r  = iResolution.xy;
    vec2  FC = fragCoord.xy;
    float t  = iTime;
    vec4  o  = vec4(0, 0, 0, 1);
    float i, e, R, s;

    // Ray direction from screen UV.
    vec3  q, p, d = vec3((FC.xy - .5 * r) / r.y, .6);
    q.z--;  // Camera position: pulled back along -z

    // RAYMARCHING
    for (i = 0.; i++ < 97.; ) {
        // Accumulate color.
        o.rgb += hsv(.08, -e, e / 5e1) + .003;

        // Step forward. Step grows with R.
        p = q += d * max(e, .02) * R * .2;

        // LOG-POLAR WARP
        // log2(R) - t*0.5 creates the signature infinite-zoom tunnel:
        // moving outward is mathematically identical to time advancing,
        // so the fractal self-repeats at every doubling of distance.
        R = length(p);
        e = asin(-p.z / R - .001) - 1.5;
        p = vec3(log2(R) - t * .5, e, atan(p.x, p.y)) - 1.;

        // fBm
        // Octaves with s = 1, 2, 4, ... 512. Each doubles frequency
        // and halves amplitude (the /s term) - classic 1/f spectrum.
        // Swizzling .zyx vs .yxz between sin and cos decorrelates
        // the octaves so the noise looks organic, not gridded.
        // abs() folds negatives into ridges (Perlin-ridge style).
        for (s = 1.; s < 8e2; s += s) {
            e += abs(dot(sin(p.zyx * s), cos(p.yxz * s))) / s * .8;
        }
    }

    fragColor = o;
}
