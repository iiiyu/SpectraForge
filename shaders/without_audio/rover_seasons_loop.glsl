// Looping Rover Seasons for SpectraForge.
//
// A small off-road car stays centered while the road and scenery stream past.
// The whole scene loops every LOOP seconds, cycling through seasons and day/night.

const float TAU = 6.28318530718;
const float LOOP = 32.0;

float sat(float x) {
    return clamp(x, 0.0, 1.0);
}

vec3 sat(vec3 x) {
    return clamp(x, vec3(0.0), vec3(1.0));
}

float hash11(float x) {
    return fract(sin(x * 127.1) * 43758.5453123);
}

float sdBox(vec2 p, vec2 b) {
    vec2 d = abs(p) - b;
    return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0);
}

float sdRoundBox(vec2 p, vec2 b, float r) {
    vec2 q = abs(p) - b + r;
    return length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
}

float rectMask(vec2 p, vec2 center, vec2 halfSize, float radius) {
    float d = radius > 0.0
        ? sdRoundBox(p - center, halfSize, radius)
        : sdBox(p - center, halfSize);
    return 1.0 - smoothstep(0.0, 0.006, d);
}

float circleMask(vec2 p, vec2 center, float radius) {
    return 1.0 - smoothstep(radius, radius + 0.006, length(p - center));
}

float cyclicWeight(float x, float center) {
    float d = abs(x - center);
    d = min(d, 4.0 - d);
    return 1.0 - smoothstep(0.0, 1.0, d);
}

vec4 seasonWeights(float phase) {
    float s = fract(phase) * 4.0;
    vec4 w = vec4(
        cyclicWeight(s, 0.0), // spring
        cyclicWeight(s, 1.0), // summer
        cyclicWeight(s, 2.0), // autumn
        cyclicWeight(s, 3.0)  // winter
    );
    return w / max(dot(w, vec4(1.0)), 0.001);
}

vec3 seasonMix(vec4 w, vec3 spring, vec3 summer, vec3 autumn, vec3 winter) {
    return spring * w.x + summer * w.y + autumn * w.z + winter * w.w;
}

vec3 blend(vec3 base, vec3 paint, float mask) {
    return mix(base, paint, sat(mask));
}

float roadDepth(vec2 p, float horizon) {
    return sat((horizon - p.y) / (horizon + 0.78));
}

void addCloud(inout vec3 col, vec2 p, vec2 center, float scale, vec3 cloudColor) {
    float c = 0.0;
    c += circleMask(p, center + vec2(-0.05, 0.00) * scale, 0.055 * scale);
    c += circleMask(p, center + vec2( 0.00, 0.02) * scale, 0.075 * scale);
    c += circleMask(p, center + vec2( 0.07, 0.00) * scale, 0.060 * scale);
    c += rectMask(p, center + vec2(0.01, -0.02) * scale, vec2(0.12, 0.025) * scale, 0.03 * scale);
    col = blend(col, cloudColor, sat(c) * 0.55);
}

void addTree(inout vec3 col, vec2 p, float laneSide, float z, vec4 w, float day, float phase) {
    float depth = z;
    float y = mix(0.10, -0.84, depth);
    float roadWidth = mix(0.14, 0.92, pow(depth, 1.35));
    float x = laneSide * (roadWidth + mix(0.10, 0.24, depth));
    float scale = mix(0.16, 0.78, depth);

    vec2 base = vec2(x, y);
    vec3 trunk = mix(vec3(0.20, 0.12, 0.07), vec3(0.07, 0.05, 0.08), 1.0 - day);
    vec3 leaf = seasonMix(
        w,
        vec3(0.95, 0.54, 0.72),
        vec3(0.13, 0.43, 0.15),
        vec3(0.92, 0.39, 0.08),
        vec3(0.82, 0.92, 0.96)
    );
    leaf *= mix(0.42, 1.0, day);

    float trunkM = rectMask(p, base + vec2(0.0, 0.04) * scale, vec2(0.018, 0.12) * scale, 0.01 * scale);
    col = blend(col, trunk, trunkM);

    float pine = w.w;
    float canopy = 0.0;
    if (pine > 0.35) {
        vec2 q = (p - (base + vec2(0.0, 0.22) * scale)) / scale;
        float tri1 = sat(1.0 - max(abs(q.x) * 3.2 + q.y * 3.0, -q.y * 5.0));
        q = (p - (base + vec2(0.0, 0.34) * scale)) / scale;
        float tri2 = sat(1.0 - max(abs(q.x) * 4.2 + q.y * 4.0, -q.y * 6.0));
        canopy = max(tri1, tri2);
    } else {
        canopy += circleMask(p, base + vec2(-0.05, 0.24) * scale, 0.10 * scale);
        canopy += circleMask(p, base + vec2( 0.04, 0.28) * scale, 0.13 * scale);
        canopy += circleMask(p, base + vec2( 0.11, 0.20) * scale, 0.09 * scale);
    }
    col = blend(col, leaf, canopy * 0.88);

    float sparkle = step(0.82, hash11(floor(depth * 25.0) + laneSide * 9.0 + floor(phase * 32.0)));
    vec3 flower = seasonMix(w, vec3(1.0, 0.70, 0.90), vec3(1.0, 0.90, 0.25), vec3(1.0, 0.45, 0.06), vec3(0.90, 0.98, 1.0));
    col = blend(col, flower, sparkle * canopy * 0.16 * (0.35 + 0.65 * day));
}

void addRover(inout vec3 col, vec2 p, float phase, float day) {
    float bounce = 0.010 * sin(TAU * phase * 24.0);
    vec2 q = p - vec2(0.0, -0.08 + bounce);

    vec3 tire = vec3(0.025, 0.025, 0.030);
    vec3 tireHi = vec3(0.16, 0.17, 0.18);
    vec3 body = mix(vec3(0.82, 0.34, 0.12), vec3(0.64, 0.20, 0.10), 1.0 - day);
    vec3 cabin = mix(vec3(0.12, 0.26, 0.34), vec3(0.03, 0.06, 0.10), 1.0 - day);
    vec3 trim = vec3(0.06, 0.07, 0.075);

    float shadow = 1.0 - smoothstep(0.0, 0.02, sdRoundBox(q - vec2(0.0, -0.19), vec2(0.45, 0.045), 0.04));
    col *= 1.0 - shadow * 0.18;

    vec2 wl = q - vec2(-0.26, -0.19);
    vec2 wr = q - vec2( 0.26, -0.19);
    float leftTire = circleMask(q, vec2(-0.26, -0.19), 0.095);
    float rightTire = circleMask(q, vec2( 0.26, -0.19), 0.095);
    col = blend(col, tire, max(leftTire, rightTire));
    col = blend(col, tireHi, max(circleMask(q, vec2(-0.26, -0.19), 0.048), circleMask(q, vec2(0.26, -0.19), 0.048)));

    float spin = TAU * phase * 64.0;
    float spokesL = (1.0 - smoothstep(0.0, 0.028, abs(sin(atan(wl.y, wl.x) * 4.0 + spin)))) * leftTire;
    float spokesR = (1.0 - smoothstep(0.0, 0.028, abs(sin(atan(wr.y, wr.x) * 4.0 + spin)))) * rightTire;
    col = blend(col, vec3(0.30, 0.32, 0.32), (spokesL + spokesR) * 0.55);

    col = blend(col, trim, rectMask(q, vec2(0.0, -0.21), vec2(0.42, 0.035), 0.018));
    col = blend(col, body, rectMask(q, vec2(0.0, -0.08), vec2(0.38, 0.125), 0.045));
    col = blend(col, body * 1.10, rectMask(q, vec2(0.0, 0.025), vec2(0.27, 0.100), 0.035));
    col = blend(col, cabin, rectMask(q, vec2(0.0, 0.035), vec2(0.205, 0.065), 0.025));
    col = blend(col, vec3(0.75, 0.88, 0.92) * mix(0.25, 0.85, day), rectMask(q, vec2(-0.10, 0.040), vec2(0.075, 0.044), 0.016));
    col = blend(col, vec3(0.75, 0.88, 0.92) * mix(0.25, 0.85, day), rectMask(q, vec2( 0.10, 0.040), vec2(0.075, 0.044), 0.016));

    // Roof rack and luggage.
    col = blend(col, trim, rectMask(q, vec2(0.0, 0.135), vec2(0.29, 0.010), 0.006));
    col = blend(col, trim, rectMask(q, vec2(-0.21, 0.110), vec2(0.010, 0.045), 0.004));
    col = blend(col, trim, rectMask(q, vec2( 0.21, 0.110), vec2(0.010, 0.045), 0.004));
    col = blend(col, vec3(0.55, 0.35, 0.16), rectMask(q, vec2(-0.10, 0.175), vec2(0.12, 0.050), 0.018));
    col = blend(col, vec3(0.18, 0.39, 0.42), rectMask(q, vec2( 0.09, 0.170), vec2(0.105, 0.045), 0.020));
    col = blend(col, vec3(0.76, 0.70, 0.50), rectMask(q, vec2( 0.00, 0.222), vec2(0.155, 0.026), 0.020));

    float headlight = 1.0 - day;
    vec3 light = vec3(1.0, 0.82, 0.42);
    col = blend(col, light, headlight * rectMask(q, vec2(-0.25, -0.075), vec2(0.040, 0.025), 0.015));
    col = blend(col, light, headlight * rectMask(q, vec2( 0.25, -0.075), vec2(0.040, 0.025), 0.015));
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 p = (fragCoord - 0.5 * iResolution.xy) / iResolution.y;
    float phase = fract(iTime / LOOP);
    vec4 w = seasonWeights(phase);

    float dayWave = 0.5 + 0.5 * sin(TAU * phase * 2.0 - 1.15);
    float day = smoothstep(0.18, 0.82, dayWave);
    float twilight = 1.0 - abs(dayWave * 2.0 - 1.0);

    vec3 skyDay = seasonMix(w,
        vec3(0.62, 0.82, 0.96),
        vec3(0.42, 0.72, 1.00),
        vec3(0.88, 0.54, 0.30),
        vec3(0.68, 0.82, 0.95)
    );
    vec3 skyNight = seasonMix(w,
        vec3(0.05, 0.08, 0.18),
        vec3(0.03, 0.07, 0.15),
        vec3(0.08, 0.04, 0.12),
        vec3(0.02, 0.04, 0.08)
    );
    vec3 col = mix(skyNight, skyDay, day);
    col += vec3(0.13, 0.06, 0.02) * twilight * 0.22 * (1.0 - day);
    col = mix(col, col * (0.55 + 0.45 * smoothstep(-0.4, 0.65, p.y)), 0.55);

    float sunAngle = TAU * phase * 2.0 - 1.15;
    vec2 orb = vec2(0.52 * cos(sunAngle), 0.40 + 0.25 * sin(sunAngle));
    float orbM = circleMask(p, orb, mix(0.035, 0.055, day));
    col = blend(col, mix(vec3(0.74, 0.82, 0.96), vec3(1.0, 0.72, 0.24), day), orbM * 0.95);

    addCloud(col, p, vec2(fract(phase * 2.0 + 0.10) * 2.4 - 1.2, 0.54), 1.0, vec3(0.90, 0.92, 0.95) * mix(0.45, 1.0, day));
    addCloud(col, p, vec2(fract(phase * 3.0 + 0.60) * 2.4 - 1.2, 0.36), 0.7, vec3(0.86, 0.88, 0.91) * mix(0.38, 0.88, day));

    // Distant mountains and seasonal ground.
    float mountain = 0.18 + 0.060 * sin(p.x * 4.0 + 0.4) + 0.035 * sin(p.x * 9.0 + phase * TAU);
    vec3 mountainCol = seasonMix(w, vec3(0.34, 0.54, 0.45), vec3(0.30, 0.48, 0.33), vec3(0.50, 0.32, 0.22), vec3(0.70, 0.78, 0.82));
    col = blend(col, mountainCol * mix(0.42, 0.90, day), 1.0 - smoothstep(mountain - 0.006, mountain + 0.012, p.y));

    float horizon = 0.06;
    vec3 field = seasonMix(w, vec3(0.35, 0.67, 0.34), vec3(0.25, 0.58, 0.22), vec3(0.68, 0.41, 0.13), vec3(0.80, 0.87, 0.88));
    col = blend(col, field * mix(0.38, 0.95, day), 1.0 - smoothstep(horizon - 0.015, horizon + 0.015, p.y));

    float depth = roadDepth(p, horizon);
    float roadWidth = mix(0.12, 1.05, pow(depth, 1.32));
    float roadMask = step(abs(p.x), roadWidth) * step(p.y, horizon);
    vec3 road = mix(vec3(0.05, 0.055, 0.060), vec3(0.33, 0.31, 0.28), day);
    road = mix(road, vec3(0.76, 0.78, 0.76), w.w * 0.45);
    col = blend(col, road, roadMask);

    float laneX = p.x / max(roadWidth, 0.001);
    float stripePhase = fract(depth * 12.0 + phase * 20.0);
    float dash = smoothstep(0.42, 0.47, stripePhase) * (1.0 - smoothstep(0.78, 0.84, stripePhase));
    float centerLine = (1.0 - smoothstep(0.018, 0.045, abs(laneX))) * dash * roadMask * smoothstep(0.04, 0.92, depth);
    col = blend(col, vec3(1.0, 0.86, 0.38), centerLine);

    float edgeLine = (1.0 - smoothstep(0.015, 0.032, abs(abs(laneX) - 0.86))) * roadMask;
    col = blend(col, vec3(0.86, 0.88, 0.82) * mix(0.55, 1.0, day), edgeLine * 0.75);

    for (int i = 0; i < 12; i++) {
        float fi = float(i);
        float z = fract(fi / 12.0 + phase * 5.0);
        addTree(col, p, -1.0, z, w, day, phase + fi * 0.02);
        addTree(col, p,  1.0, fract(z + 0.46), w, day, phase + fi * 0.03 + 0.4);
    }

    // Winter snow / spring petals / autumn leaves drifting past.
    vec3 particleCol = seasonMix(w, vec3(1.0, 0.72, 0.90), vec3(1.0, 0.96, 0.55), vec3(1.0, 0.43, 0.08), vec3(0.95, 0.98, 1.0));
    for (int i = 0; i < 26; i++) {
        float fi = float(i);
        vec2 pp = vec2(
            fract(hash11(fi + 4.0) + phase * (0.8 + hash11(fi) * 1.2)) * 2.4 - 1.2,
            fract(hash11(fi + 8.0) - phase * (2.0 + hash11(fi + 2.0))) * 1.55 - 0.72
        );
        float particle = circleMask(p, pp, mix(0.004, 0.011, hash11(fi + 12.0)));
        col = blend(col, particleCol, particle * (0.22 + w.x * 0.22 + w.z * 0.20 + w.w * 0.45));
    }

    addRover(col, p, phase, day);

    // Night headlight cones painted over the road.
    float night = 1.0 - day;
    float cone = (1.0 - smoothstep(0.02, 0.46, abs(p.x)))
               * smoothstep(-0.62, -0.35, p.y)
               * (1.0 - smoothstep(0.02, 0.06, p.y));
    col += vec3(0.95, 0.76, 0.38) * cone * night * 0.30;

    col *= 0.86 + 0.14 * (1.0 - smoothstep(0.45, 1.25, length(p)));
    fragColor = vec4(pow(sat(col), vec3(0.92)), 1.0);
}
