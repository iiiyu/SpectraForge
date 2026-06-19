// Bokeh Bloom (audio-free) for SpectraForge.
//
// Adapted to SpectraForge's mainImage entry point (u_time -> iTime,
// u_resolution -> iResolution). The original's tweakable uniforms are baked in
// as constants using the author's default values. Animates on iTime alone;
// pair with --duration-only.

const vec3  u_rgb     = vec3(0.0, 0.6, 1.2); // RGB phase shifts
const vec2  u_bokeh   = vec2(0.4, 0.8);      // bokeh radius
const float u_speed   = 0.4;                 // animation speed
const float u_spin    = 0.2;                 // spin range
const float u_scatter = 1.39;                // ring scatter strength
const int   u_colors  = 3;                   // number of colors
const float u_lines   = 30.0;                // number of lines

//Noise functions
float rand1(vec2 p)
{
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

float value_noise(vec2 p)
{
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    float a = rand1(i);
    float b = rand1(i + vec2(1.0, 0.0));
    float c = rand1(i + vec2(0.0, 1.0));
    float d = rand1(i + vec2(1.0, 1.0));
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y) * 2.0 - 1.0;
}

mat2 rotate2d(float r)
{
    return mat2(cos(r), sin(r), -sin(r), cos(r));
}

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 P = (fragCoord.xy - iResolution * 0.4) / iResolution.y * 6e2,
    u = vec2(dot(P, u_bokeh)) / 6e2;

    //Blank color
    vec3 col = vec3(0);

    //Fibonacci disk
    for (float i = 1.0;i < 16.0;i += 1.0 / i)
    {
        vec2 p = (P + u * i) * mat2(2, 1, -2, 4) / 4e1;
        float l = length(p);
        float d = cos(sin(ceil(log(l) * u_lines) * 1e2) * 2e2 + u_speed * iTime);
        float n = value_noise(0.5*vec2(l)) * value_noise(p * 5.0 * rotate2d(u_spin* d));
        u *= rotate2d(2.4);
        vec3 hue = cos(atan(p.y, p.x) * float(u_colors) + d * u_scatter + u_rgb) + 1.1;
        col += pow(max(n * hue, 0.0) * sqrt(l) * 0.1, vec3(3.0));
    }
    fragColor = vec4(sqrt(col / (1.0+col)), 1);
}
