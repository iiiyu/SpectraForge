// Ribbon Flow (audio-free) for SpectraForge.
//
// Scrolling perspective ribbon shader, adapted to SpectraForge's mainImage
// entry point (u_time -> iTime, u_resolution -> iResolution). Animates on
// iTime alone; pair with --duration-only.

#define NUM 9.0

#define SCROLL 0.3
#define SPEED 0.5

float ncos(float x)
{
    return cos(x)/(.5+.4*abs(cos(x)));
}

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 s = (fragCoord*2.0-iResolution.xy) / iResolution.y;
    float v = (s.y+1.0)*(s.y+1.0)*0.25;
    s.y -= 1.2;
    float per = 2.0/abs(s.y);

    vec3 col = vec3(0);
    for(float z=0.0; z<1.0; z+=0.1)
    {
        float d = 1.0+0.4*z;
        vec2 p = vec2(s.x*d,s.y+d)*per;
        vec2 s = p;
        s.y += SCROLL * iTime;
        vec2 c = s - 0.02*iTime + sin(s*4.1+.01*iTime);

        //s.x += 0.1*v*v*cos(sin(dot(round(c)+z,vec2(97,79)))*4e4);

        float shift = cos(z/.1);
        float wave = ncos(s.y)+ncos(s.y*0.6);
        s.x += shift+(wave)/(1.0+0.01*per*per);

        float w = s.x;
        float l = sin(s.y*.5+z/.1+SPEED*iTime*sign(shift));
        col += exp(min(l,-l/.3/(1.+4.*w*w)))*
        mix(vec3(1,.2,.2), vec3(1,1,.9), tanh(shift/.1)*.5+.5)
        / (abs(w) + 0.01*per) * per;
    }
    col = tanh(col/2e1);
    fragColor = vec4(col*col,1);
}
