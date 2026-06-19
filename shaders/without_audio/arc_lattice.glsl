// Arc Lattice (audio-free) for SpectraForge.
//
// Originally twigl "geekest"/golf shorthand; expanded to standard GLSL and
// adapted to SpectraForge's mainImage entry point (T -> iTime, R -> iResolution,
// C -> fragCoord, O -> fragColor). O and p are initialized explicitly. Animates
// on iTime alone; pair with --duration-only.

void mainImage(out vec4 O, in vec2 C)
{
    O = vec4(0.0);
    vec2 p = vec2(0.0);
    for (float i = -1.; i < 1.; i += .1)
        O += (cos(i / .3 + vec4(0, 1, 2, 0)) + 1.)
           / (length(p * sin((p = ((2. * C.xy - iResolution.xy) / iResolution.y + i) / .2)
                             * mat2(cos(iTime - i - length(p) * .3 - vec4(0, 11, 33, 0)))))
              + i * i);
    O = tanh(O * O / 4e2);
}
