// Plasma Core (audio-free) for SpectraForge.
//
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 @diatribes — https://opensource.org/licenses/MIT
// Adapted to SpectraForge's mainImage entry point (u_time -> iTime,
// u_resolution -> iResolution). fragColor/i are initialized explicitly since
// the loop idiom relies on zero-init. Animates on iTime alone; pair with
// --duration-only.

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 u = fragCoord;

    vec3 r = vec3(iResolution.xy, 1.0);
    u = (u+u-r.xy)/r.y;

    float i = 0.0, d = 0.0, s, t = iTime, f = (.5+sin(t+sin(t*4e1+cos(t*6e1))));
    vec3 q, p,
         D = normalize(vec3(u, 1)); // <-- zoom here

    fragColor = vec4(0.0);
    for(; i++<1e2;) {
        q = p = D *d,
        p.xz *= mat2(cos(t/1e1+vec4(0,33,11,0))),
        p.z -= 6e1;
        p = p*4e4/dot(p,p); // <-- transform
        for(s = .01; s < 1.; s += s )
            p += cos(t/4e1+p.yzx/1e1),
            p -= abs(dot(sin(.02*p.x+p.y*.06+t+.1*p.z+p / s / 1e1), vec3(s+s)));
        d += s =.1+.35*abs(length(p)-6e1),
        fragColor += 1e1*vec4(6,2,1,0)/s*d +1e5*(1.+cos(i*.3+vec4(2,1,0,0)))/s;
    }
    fragColor += pow(abs(vec4(3, 2, 1, 0) / (cos(t+p.y*.025)+cos(p.x/1e1) * 4.)), vec4(f*abs(.7/s)));
    fragColor/=2e8*length(u-=.6);
    fragColor = fragColor / (fragColor + 0.155) * 1.019;
}
