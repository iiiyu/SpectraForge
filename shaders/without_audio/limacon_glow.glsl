// Limacon Glow (audio-free) for SpectraForge.
//
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 @krisselden — https://opensource.org/licenses/MIT
// Adapted to SpectraForge's mainImage entry point (u_time -> iTime,
// u_resolution -> iResolution). Animates on iTime alone; pair with
// --duration-only.

#define PI 3.14159265359

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    float zoom = 6.;
    float pixel = 1./iResolution.y;
    vec2 uv = zoom*pixel*(2.*fragCoord.xy-iResolution.xy);
    uv += vec2(4.5,0);

    float a = 8.;
    float x0 = (a+1./a)/2.;
    float x1 = (x0+1.)/2.;
    float r1 = (a+1.)/2.-x1;

    float r;
    float d;
    vec2 p;

    vec3 col = vec3(0.);

    float step_size = 2. * PI / 24.;

    float damp=1.;

    // https://www.mathcurve.com/courbes2d.gb/limacon/limacon.shtml
    for (int i=0; i<24; i++) {
        float angle = iTime - float(i)*step_size;
        p = vec2(r1*cos(angle)+x1,r1*sin(angle));
        r = length(p-vec2(1.,0));
        d = length(uv-p)-r;

        vec3 glowCol = (d < 0. ? vec3(5.,1.,2.) : vec3(2.,1.,6.));
        col += damp*glowCol*tanh(zoom*pixel/abs(d));
        damp = damp*.9;
    }

    fragColor = vec4(tanh(col), 1);
}
