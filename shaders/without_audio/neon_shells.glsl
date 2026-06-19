// Neon Shells (audio-free) for SpectraForge.
//
// SPDX-License-Identifier: CC-BY-NC-SA-4.0
// Copyright (c) 2026 @Frostbyte (https://fragcoord.xyz/s/sw3qqd9w)
// Remix of "Neon Shells" by Old Eclipse; glow orbs inspired by Diatribes.
// [LICENSE] https://creativecommons.org/licenses/by-nc-sa/4.0/
// Adapted to SpectraForge's mainImage entry point (u_time -> iTime,
// u_resolution -> iResolution). The original fft() sampled a u_audio texture;
// here it returns four iTime-driven band values so the shells still pulse.
// O and t are initialized explicitly. Animates on iTime alone; pair with
// --duration-only.

// Synthetic "spectrum": four bass-to-treble bands drifting over time.
vec4 fft(){
    float b = 0.5 + 0.5 * sin(iTime * 2.4);          // bass thump
    float lm = 0.5 + 0.5 * sin(iTime * 1.7 + 1.0);   // low-mid
    float hm = 0.5 + 0.5 * sin(iTime * 3.1 + 2.0);   // high-mid
    float tr = 0.5 + 0.5 * sin(iTime * 4.3 + 3.0);   // treble
    return vec4(b, lm, hm, tr) * 0.6;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    //Define variables and set r to screenspace
    vec2 R=iResolution.xy;
    float T=iTime,t=0.0,v;
    vec3 p,r=normalize(vec3(fragCoord.xy*2.-R.xy,R.x));
    vec4 O=vec4(0.0);

    //rotate screenspace XY with time
    r.xy*=mat2(cos(T*.2+vec4(0,11,33,0)));

    //150 Volumetric Raymarch steps
    for(int i=0;i<150;i++){

        //p set to screenspace * accumulated distance (volumetric)
        p=t*r;

        //Rotate along z
        p.xy*=mat2(cos(p.z*3.+vec4(0,11,33,0)));

        // Warping p.z with rotation (stylistic blur/warp with accumulated volume)
        p+=vec3(vec2(0.05,sin(p.z*.01+15.)*.42)*mat2(cos(5.*sin(T*.1+length(vec3(p.xy,p.z)))*.3*+vec4(0,11,33,0))),T*.2);

        //Shift Slightly
        p.x-=T*.1;

        //Repeat space XYZ
        p=fract(p.zxy-.5)-.5;

        //Fractal looping of space (Basically iterative mirroring, scaling, and shifting)
        for(int j=0;j<10;j++){
            p=abs(p.xzy);
            p*=1.6;
            p.x-=1.5;
        }

        //"t" accumulates volume
        t+=

        //"v" is instance of distance at each step
        v=

        //Abs allows positive interior and step into shape
        abs(

        //union of crossing cylinders that have no radius (crossing lines)
        min(length(p.xz*.5+.2-smoothstep(0.,1.,fft().y)*.5+p.x*.5),length(p.zy*.1+p.y*.5))

        //Adds softness to volumetrics as we force march through surface
        +.02)

        //reduce step size (smaller steps)
        /400.;

        //Color accumulated
        O.rgb+=

               //Palette Function and exp for interesting glow result
               exp(2.*(vec3(0.5,.41,.5)+vec3(.1,.8,0.1)*
               cos(6.28*(sin(length(p*.50)+T*.25)+p.z*.2+r*.7))))

               //Divide by intanced distance this creates glow as accumulated
               /v;
    }
    //Glow orbs inspired by Diatribes
    //Accumulated color divided to be brough into Tonemap range for Tanh and * for stylistic
    fragColor=tanh(O*O/1e14/pow(length(sin(r.xy*5.+iTime*.75)), 2.*fft().z))*150.;
}
