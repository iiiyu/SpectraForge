// Radial Beam (audio-free) for SpectraForge.
//
// Polar rainbow-beam shader, adapted to SpectraForge's mainImage entry point
// (u_time -> iTime, u_resolution -> iResolution). Animates on iTime alone;
// pair with --duration-only.

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 p = (2.0*fragCoord.xy-iResolution.xy)/iResolution.y;
    float tau = 3.1415926535*2.0;
    float a = atan(p.x,p.y);
    float r = length(p)*0.75;
    vec2 uv = vec2(a/tau,r);

    //get the color
    float xCol = (uv.x - (iTime / 5.0)) * 3.0;
    xCol = mod(xCol, 3.0);
    vec3 horColour = vec3(0.25, 0.25, 0.25);

    if (xCol < 1.0) {

        horColour.r += 1.0 - xCol;
        horColour.g += xCol;
    }
    else if (xCol < 2.0) {

        xCol -= 1.0;
        horColour.g += 1.0 - xCol;
        horColour.b += xCol;
    }
    else {

        xCol -= 2.0;
        horColour.b += 1.0 - xCol;
        horColour.r += xCol;
    }

    // draw color beam
    uv = (2.0 * uv) - 1.0;
    float beamWidth = (0.75+0.1*cos(uv.x*10.0*tau*0.15)) * abs(1.5 / (45.0 * uv.y));
    vec3 horBeam = vec3(beamWidth);
    horBeam = horBeam * horColour;
    fragColor = vec4(horBeam.b,horBeam.b,horBeam.b, 1.0);
}
