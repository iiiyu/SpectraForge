// Moonlit Ocean (audio-free) for SpectraForge.
//
// SPDX-License-Identifier: MIT
// Copyright (c) 2026 @ThatRenderMan
// [LICENSE] https://opensource.org/licenses/MIT
// Adapted to SpectraForge's mainImage entry point (u_time -> iTime,
// u_resolution -> iResolution). SpectraForge has no mouse, so the camera uses
// the shader's default look angle. Animates on iTime alone; pair with
// --duration-only.

#define DRAG_MULT           0.99
#define WATER_DEPTH         2.0
#define CAMERA_HEIGHT       3.5
#define ITERATIONS_RAYMARCH 12
#define ITERATIONS_NORMAL   36

// Waves
// - compute height and its derivative

vec2 wavedx(vec2 position, vec2 direction, float frequency, float timeshift) {
    float x    = dot(direction, position) * frequency + timeshift;
    float wave = exp(sin(x) - 1.0);
    return vec2(wave, -wave * cos(x));
}

// Gertsner wave-like model
float getwaves(vec2 position, int iterations) {
    float wavePhaseShift = length(position) * 0.1;
    float iter           = 0.0;
    float frequency      = 1.0;
    float timeMultiplier = 3.0;
    float weight         = 0.4;
    float sumOfValues    = 0.0;
    float sumOfWeights   = 0.0;
    for (int i = 0; i < iterations; i++) {
        vec2 p   = vec2(sin(iter), cos(iter));
        vec2 res = wavedx(position, p, frequency, iTime * timeMultiplier + wavePhaseShift);
        position       += p * res.y * weight * DRAG_MULT;
        sumOfValues    += res.x * weight;
        sumOfWeights   += weight;
        weight         =  mix(weight, 0.0, 0.2);
        frequency      *= 1.18;
        timeMultiplier *= 1.07;
        iter           += 1232.399963;
    }
    return sumOfValues / sumOfWeights;
}

// Ray marching

float raymarchwater(vec3 camera, vec3 start, vec3 end, float depth) {
    vec3 pos = start;
    vec3 dir = normalize(end - start);
    for (int i = 0; i < 64; i++) {
        float height = getwaves(pos.xz, ITERATIONS_RAYMARCH) * depth - depth;
        if (height + 0.01 > pos.y) return distance(pos, camera);
        pos += dir * (pos.y - height);
    }
    return distance(start, camera);
}

// Approximate normal at a point using finite differences
vec3 normal(vec2 pos, float e, float depth) {
    vec2  ex = vec2(e, 0.0);
    float H  = getwaves(pos.xy, ITERATIONS_NORMAL) * depth;
    vec3  a  = vec3(pos.x, H, pos.y);
    return normalize(cross(
        a - vec3(pos.x - e, getwaves(pos.xy - ex.xy, ITERATIONS_NORMAL) * depth, pos.y),
        a - vec3(pos.x,     getwaves(pos.xy + ex.yx, ITERATIONS_NORMAL) * depth, pos.y + e)
    ));
}

// Camera construction
//- Rodrigues equations

mat3 createRotationMatrixAxisAngle(vec3 axis, float angle) {
    float s  = sin(angle);
    float c  = cos(angle);
    float oc = 1.0 - c;
    return mat3(
        oc*axis.x*axis.x + c,          oc*axis.x*axis.y - axis.z*s,   oc*axis.z*axis.x + axis.y*s,
        oc*axis.x*axis.y + axis.z*s,   oc*axis.y*axis.y + c,           oc*axis.y*axis.z - axis.x*s,
        oc*axis.z*axis.x - axis.y*s,   oc*axis.y*axis.z + axis.x*s,   oc*axis.z*axis.z + c
    );
}

vec3 getRay(vec2 fragCoord) {
    vec2 uv = ((fragCoord / iResolution.xy) * 2.0 - 1.0)
            * vec2(iResolution.x / iResolution.y, 1.0);

    // Rotates the UV plane around the view axis; rocks the horizon gently
    float tiltAngle = sin(iTime * 0.25) * 0.3;
    float ct = cos(tiltAngle);
    float st = sin(tiltAngle);
    uv = mat2(ct, -st, st, ct) * uv;               // 2D rotation of screen space

    // Breathing zoom
    // Focal length > 1.5 narrows the FOV (zooms in); oscillates slowly
    float focal = 1.5 + sin(iTime * 0.18) * 0.25; // range [1.25, 1.75]
    vec3 proj = normalize(vec3(uv, focal));

    if (iResolution.x < 600.0) return proj;

    // No mouse in SpectraForge: use the shader's default look angle.
    float mouseX = 0.0;
    float mouseY = 0.27;

    return createRotationMatrixAxisAngle(vec3(0.0, -1.0, 0.0), 6.0 * mouseX)
         * createRotationMatrixAxisAngle(vec3(1.0,  0.0, 0.0), 3.0 * mouseY - 1.0)
         * proj;
}

float intersectPlane(vec3 origin, vec3 direction, float planeY) {
    return clamp((planeY - origin.y) / direction.y, -1.0, 9991999.0);
}

// Moon

vec3 getMoonDirection() {
    return normalize(vec3(0.0, 0.2, 0.5));
}

// Full moon: sharp disc, soft inner corona, wide outer halo
vec3 moonDisc(vec3 dir) {
    vec3  moonDir = getMoonDirection();
    float mu      = dot(dir, moonDir);

    // Sharp limb using angular threshold
    float disc   = smoothstep(0.99920, 0.99960, mu);

    // Soft corona bleeds just outside the disc
    float corona = pow(max(0.0, mu), 180.0) * 0.55;

    // Wide atmospheric halo (thin ring of scattered moonlight)
    float halo   = pow(max(0.0, mu), 5.0) * 0.06;

    // Cream-white moon; slightly warmer than pure white, like a real full moon
    //vec3 color = vec3(0.88, 0.87, 0.98);
    //vec3 color = vec3(0.88, 0.47, 0.48);
      vec3 color = vec3(0.4, 0.4, 0.98);

    return color * (disc + corona + halo);
}

// Stars

float hash(vec2 p) {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

// Procedural star field - grid-jittered points with variable brightness
float stars(vec3 dir) {
    if (dir.y < 0.0) return 0.0;

    // Spherical UV: longitude / latitude mapped to [0,1]^2
    vec2 uv   = vec2(atan(dir.z, dir.x) / (2.0 * 3.14159) + 0.5,
                     asin(clamp(dir.y, -1.0, 1.0)) / 3.14159 + 0.5);
    vec2 grid = floor(uv * 250.0);

    // Jitter star from cell center
    vec2 jitter = vec2(hash(grid), hash(grid + 7.3));
    vec2 offset = fract(uv * 250.0) - 0.5 - (jitter - 0.5) * 0.7;
    float d     = length(offset);

    // Per-star random brightness - cube to create many dim and few bright
    float brightness = pow(hash(grid + 17.3), 3.5);

    // Stars fade toward the horizon (atmospheric extinction)
    float extinction = smoothstep(0.0, 0.12, dir.y);

    return brightness * smoothstep(0.06, 0.0, d) * extinction;
}

// Night sky

vec3 nightSky(vec3 dir) {
    vec3  moonDir = getMoonDirection();

    // Sky gradient: near-black indigo at zenith, deep navy at horizon
    float elev       = clamp(dir.y, 0.0, 1.0);
    vec3  zenith     = vec3(0.004, 0.006, 0.018) * 1.5;
    vec3  horizon    = vec3(0.010, 0.016, 0.040);
    vec3  sky        = mix(horizon, zenith, pow(elev, 0.6));

    // Thin horizon haze band: brightens the sea-sky boundary
    sky += vec3(0.008, 0.012, 0.030) * exp(-elev * 14.0);

    // Moonlight ambient: sky side facing the moon is very slightly brighter
    float moonAmbient = pow(max(0.0, dot(dir, moonDir)), 3.0);
    sky += vec3(0.010, 0.014, 0.025) * moonAmbient;

    // Stars: white with faint blue cast
    sky += vec3(0.85, 0.90, 1.00) * stars(dir) * 0.9;

    // Moon itself
    sky += moonDisc(dir);

    return sky;
}

// Tone mapping

vec3 aces_tonemap(vec3 color) {
    color *= 1.2;
    return pow(color / (vec3(1.0) + color), vec3(1.0 / 2.2));
}

// Main

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec3 ray = getRay(fragCoord);

    if (ray.y >= 0.0) {
        fragColor = vec4(aces_tonemap(nightSky(ray) * 2.0), 1.0);
        return;
    }

    vec3  origin = vec3(iTime * 0.2, CAMERA_HEIGHT, 1.0) - vec3(.0, 0.0, iTime * 5.0);

    float highPlaneHit = intersectPlane(origin, ray, 0.0);
    float lowPlaneHit  = intersectPlane(origin, ray, -WATER_DEPTH);
    vec3  highHitPos   = origin + ray * highPlaneHit;
    vec3  lowHitPos    = origin + ray * lowPlaneHit;

    float dist        = raymarchwater(origin, highHitPos, lowHitPos, WATER_DEPTH);
    vec3  waterHitPos = origin + ray * dist;

    vec3 N = normal(waterHitPos.xz, 0.01, WATER_DEPTH);
    N = mix(N, vec3(0.0, 1.0, 0.0), 0.8 * min(1.0, sqrt(dist * 0.01) * 1.1));

    float fresnel = 0.04 + 0.96 * pow(1.0 - max(0.0, dot(-N, ray)), 5.0);

    vec3 R = normalize(reflect(ray, N));
    R.y    = abs(R.y);

    // Sky reflection including moon disc and stars
    vec3 reflection = nightSky(R);

    // Specular moonlight path on water: sharp glint in the moon's direction
    vec3  moonDir  = getMoonDirection();
    float moonSpec = pow(max(0.0, dot(R, moonDir)), 270.0) * 1.0;
    reflection    += vec3(0.88, 0.90, 0.80) * moonSpec;

    // Deep ocean base: dark teal, lit softly by moonlight
    float seaDepthFactor = 0.2 + (waterHitPos.y + WATER_DEPTH) / WATER_DEPTH;
    vec3  scattering     = vec3(0.007, 0.016, 0.036) * seaDepthFactor;

    // Moon path: broad silvery shimmer along the moon's azimuth
    vec2  moonFlat   = normalize(moonDir.xz);
    vec2  reflFlat   = normalize(R.xz);
    float moonPath   = pow(max(0.0, dot(reflFlat, moonFlat)), 10.0);
    scattering      += vec3(0.04, 0.055, 0.08) * moonPath * 0.25;

    vec3 C = fresnel * reflection + scattering;
    fragColor = vec4(aces_tonemap(C * 2.0), 1.0);
}
