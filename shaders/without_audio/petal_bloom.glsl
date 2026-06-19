// Petal Bloom (audio-free) for SpectraForge.
//
// SPDX-License-Identifier: CC-BY-4.0
// Copyright (c) 2026 @Xor — https://creativecommons.org/licenses/by/4.0/
// Originally twigl "geekest" shorthand; expanded to standard GLSL and adapted
// to SpectraForge's mainImage entry point (T -> iTime, R -> iResolution,
// C -> fragCoord, O -> fragColor). Animates on iTime alone; pair with
// --duration-only.

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 p = (fragCoord.xy * 2.0 - iResolution.xy) / iResolution.y / 0.6;
    float l = length(p) - 1.0, a = atan(p.y, p.x);
    fragColor = tanh(
        (cos(p.x + iTime / 2.0 + vec4(1, 2, 3, 0)) + 1.5)
        * max(0.2 / l, -0.02 - 0.02 / l)
        / (2.0 + cos(a * 8.0 + cos(a * 5.0 + iTime)) * sin(a * 4.0 - iTime))
    );
}
