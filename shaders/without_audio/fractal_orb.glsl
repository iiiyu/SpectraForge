// Fractal Orb (audio-free) for SpectraForge.
//
// Compact raymarched fractal in the XorDev idiom, adapted to SpectraForge's
// mainImage entry point (u_time -> iTime, u_resolution -> iResolution). Loop
// accumulators are initialized explicitly. Animates on iTime alone; pair with
// --duration-only.

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    fragColor = vec4(0.0);
    for(float i = 0.0, z = 0.0, d = 0.0, s = 0.0; i++ < 7e1; fragColor += vec4(z, 2, s, 1) / s/d)
    {
        vec3 p = z * normalize(gl_FragCoord.rgb * 2.0 - vec3(iResolution.xy, 1.0).xyy),
        a = vec3(0.0);
        p.z += 9.0;
        a = mix(dot(a -= 0.57, p) * a, p, cos(s -= iTime)) - sin(s) * cross(a, p);
        s = sqrt(length(a.xz - a.y));
        for(d = 1.0; d++ < 9.0; a += sin(a * d - iTime).yzx / d);
        z += d = length(sin(a) + dot(a, a / a) * 0.2) * s / 2e1;
    }
    fragColor = tanh(fragColor / 2e3);
}
