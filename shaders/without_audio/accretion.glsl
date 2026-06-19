// Accretion (audio-free) for SpectraForge.
//
// "Accretion" by @XorDev — https://x.com/XorDev/status/1936884244128661986
// Adapted to SpectraForge's mainImage entry point (u_time -> iTime,
// u_resolution -> iResolution). fragColor/i/z/d are initialized explicitly
// since the loop idiom relies on zero-init. Animates on iTime alone; pair with
// --duration-only.

void mainImage(out vec4 fragColor, in vec2 I)
{
    //Raymarch depth
    float z = 0.0,
    //Step distance
    d = 0.0,
    //Raymarch iterator
    i = 0.0;
    //Clear fragColor and raymarch 20 steps
    fragColor = vec4(0.0);
    for(; i++<2e1; )
    {
        //Sample point (from ray direction)
        vec3 p = z*normalize(vec3(I+I,0)-iResolution.xyx)+.1;

        //Polar coordinates and additional transformations
        p = vec3(atan(p.y/.2,p.x)*2., p.z/3., length(p.xy)-5.-z*.2);

        //Apply turbulence and refraction effect
        for(d=0.; d++<7.;)
            p += sin(p.yzx*d+iTime+.3*i)/d;

        //Distance to cylinder and waves with refraction
        z += d = length(vec4(.4*cos(p)-.4, p.z));

        //Coloring and brightness
        fragColor += (1.+cos(p.x+i*.4+z+vec4(6,1,2,0)))/d;
    }
    //Tanh tonemap
    fragColor = tanh(fragColor*fragColor/4e2);
}
