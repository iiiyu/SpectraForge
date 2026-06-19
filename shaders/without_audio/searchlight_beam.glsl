// Searchlight Beam (audio-free) for SpectraForge.
//
// Sweeping volumetric searchlight, adapted to SpectraForge's mainImage entry
// point (u_time -> iTime, u_resolution -> iResolution). i/d are initialized
// explicitly since the for-loop idiom relies on zero-init. Animates on iTime
// alone; pair with --duration-only.

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 u = fragCoord;
    float i = 0.0, a, d = 0.0, s, t = iTime * 0.3;
    vec3 p;

    // Center & aspect-correct
    u = (u - iResolution.xy * 0.5) / iResolution.y;
    if (abs(u.y) > 0.4) { fragColor = vec4(0.0); return; }

    // 🔦 FIXED LIGHT SOURCE (top-center, like a helicopter)
    vec2 lightPos = vec2(0.0, 0.5);
    vec2 dirToPixel = u - lightPos;

    // 🚁 SWEEPING BEAM ANGLE (wide arc + mechanical wobble)
    float angle = sin(t * 1.3) * 0.5 + sin(t * 3.5) * 0.04;
    vec2 beamDir  = vec2(sin(angle), -cos(angle)); // Points downward

    // Raymarching (original volumetric noise preserved)
    for (fragColor = vec4(0.0); i++ < 100.0;
         d += s = 0.03 + abs(s) * 0.2, fragColor += 1.0 / s)
    {
        for (p = vec3(u * d, d + t),
             p.x *= 0.6, p.x += t * 4.0,
             s = 4.0 + p.y, a = 0.05; a < 3.0; a += a)
        {
            s -= abs(dot(sin(t + p * a * 8.0), 0.04 + p - p)) / a;
        }
    }

    // 🔦 CONICAL SEARCHLIGHT MASK
    // Angular alignment: how close the pixel is to the beam center
    float cone = pow(max(dot(normalize(dirToPixel), beamDir), 0.0), 12.0);
    // Distance falloff + angular mask combined
    float beamMask = cone / (dot(dirToPixel, dirToPixel) + 0.08);

    // Apply mask to accumulated volumetric light
    fragColor = tanh(vec4(2.159, 4.091, 11.25, 2.273) * fragColor * beamMask / 6000.0);
}
