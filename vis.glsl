// Default SpectraForge visualizer.
// Available uniforms: iResolution, iTime, iRMS, iBass, iMid, iTreble,
// and iSpectrum (64x1 texture; sample .r for a bin magnitude in 0..1).
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;       // 0..1
    vec2 p = uv * 2.0 - 1.0;                     // -1..1
    p.x *= iResolution.x / iResolution.y;        // aspect correct

    // Radial spectrum ring that pulses with the bass.
    float r = length(p);
    float ang = atan(p.y, p.x) / 6.2831853 + 0.5; // 0..1
    float mag = texture(iSpectrum, vec2(ang, 0.5)).r;

    float ring = 0.45 + 0.35 * iBass;
    float band = smoothstep(0.04 + mag * 0.25, 0.0, abs(r - ring));

    // Color cycles over time; treble brightens, mid shifts hue.
    vec3 col = 0.5 + 0.5 * cos(iTime + uv.xyx * 3.0 + vec3(0.0, 2.0, 4.0) + iMid * 6.0);
    col *= band * (0.6 + 4.0 * iTreble);

    // Soft center glow driven by overall loudness.
    col += vec3(0.1, 0.2, 0.4) * iRMS / (r * r * 8.0 + 0.1);

    fragColor = vec4(col, 1.0);
}
