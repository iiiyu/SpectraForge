// Log-Polar Bloom (audio-free) for SpectraForge.
//
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 @YoheiNishitsuji — https://opensource.org/licenses/MIT
// Adapted to SpectraForge's mainImage entry point (u_time -> iTime,
// u_resolution -> iResolution). Animates on iTime alone; pair with
// --duration-only.

vec3 hsv(float h, float s, float v) {
    vec4 t = vec4(1.0, 2.0/3.0, 1.0/3.0, 3.0);
    vec3 p = abs(fract(vec3(h) + t.xyz) * 6.0 - vec3(t.w));
    return v * mix(vec3(t.x), clamp(p - vec3(t.x), 0.0, 1.0), s);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2  r = iResolution.xy;
    float t = iTime;
    vec4  o = vec4(0.0, 0.0, 0.0, 1.0);

    float i = 0.0, e = 0.0, R = 0.0, s = 0.0;

    // RAY SETUP
    vec3  q = vec3(0.0), p,
          d = vec3((fragCoord - 0.5*r)/min(r.y, r.x)*0.5 + vec2(0, 1), 1);

    // RAY MARCH: 129 steps walking point q along d. q.yz -= 1.0 offsets the start.
    for (q.yz -= 1.0; i++ < 129.0; ) {

        o.rgb += hsv(-R/i, 0.4, min(R*e*s - 0.07, e)/7.0);

        s = 1.0;

        // ADAPTIVE STEP
        p = q += d*e*R*0.24;

        // DOMAIN WARP
        p = vec3(log2(R = length(p)) - t*0.5, exp(-p.z/R), atan(p.y, p.x));

        // fBm: sum sin/cos noise at octaves doubling each pass (s += s).
        for (e = (p.y -= 1.0); s < 5e2; s += s)
            e += dot(sin(p.yzx*s - t), vec3(0.2) - cos(p.yxy*s))/s*0.2;
    }

    fragColor = o;
}
