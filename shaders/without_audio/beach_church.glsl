// =====================================================================
// Beach Church (audio-free) for SpectraForge.
// By Noztol
// Takes the landscape layout of https://www.shadertoy.com/view/f3XSDS
// (Golden Gardens) and applies Voronoi layers to it for a stained-glass look.
// Adapted to SpectraForge's mainImage entry point (u_time -> iTime,
// u_resolution -> iResolution). Animates on iTime alone; pair with
// --duration-only.
// =====================================================================

#define time iTime
const float PI = 3.141592;

// ==========================================
// PART 1: VORONOI MATH
// ==========================================

vec2 rand(vec2 x) {
    return fract(sin(vec2(dot(x, vec2(1.2, 5.5)), dot(x, vec2(4.54, 2.41)))) * 4.45);
}

vec3 voro(vec2 uv) {
    vec2 uv_id = floor(uv);
    vec2 uv_st = fract(uv);
    vec2 m_diff; vec2 m_point; vec2 m_neighbor;
    float m_dist = 10.;

    for (int j = -1; j <= 1; j++) {
        for (int i = -1; i <= 1; i++) {
            vec2 neighbor = vec2(float(i), float(j));
            vec2 point = rand(uv_id + neighbor);
            point = 0.5 + 0.5 * sin(2. * PI * point + time * 0.5);
            vec2 diff = neighbor + point - uv_st;
            float dist = length(diff);
            if (dist < m_dist) {
                m_dist = dist; m_point = point; m_diff = diff; m_neighbor = neighbor;
            }
        }
    }

    m_dist = 10.;
    for (int j = -2; j <= 2; j++) {
        for (int i = -2; i <= 2; i++) {
            if (i == 0 && j == 0) continue;
            vec2 neighbor = m_neighbor + vec2(float(i), float(j));
            vec2 point = rand(uv_id + neighbor);
            point = 0.5 + 0.5 * sin(point * 2. * PI + time * 0.5);
            vec2 diff = neighbor + point - uv_st;
            float dist = dot(0.5 * (m_diff + diff), normalize(diff - m_diff));
            m_point = point; m_dist = min(m_dist, dist);
        }
    }
    return vec3(m_point, m_dist);
}

// ==========================================
// PART 2: LANDSCAPE MATH
// ==========================================

vec2 hash22(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * vec3(.1031, .1030, .0973));
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

float GradientNoise2D(vec2 xy) {
    float i0 = floor(xy.x), i1 = i0 + 1.0;
    float j0 = floor(xy.y), j1 = j0 + 1.0;
    float v00 = dot(2.0 * hash22(vec2(i0, j0)) - 1.0, xy - vec2(i0, j0));
    float v01 = dot(2.0 * hash22(vec2(i0, j1)) - 1.0, xy - vec2(i0, j1));
    float v10 = dot(2.0 * hash22(vec2(i1, j0)) - 1.0, xy - vec2(i1, j0));
    float v11 = dot(2.0 * hash22(vec2(i1, j1)) - 1.0, xy - vec2(i1, j1));
    float xf = xy.x - i0; xf = xf * xf * xf * (10.0 + xf * (-15.0 + xf * 6.0));
    float yf = xy.y - j0; yf = yf * yf * yf * (10.0 + yf * (-15.0 + yf * 6.0));
    return v00 + (v10 - v00) * xf + (v01 - v00) * yf + (v00 + v11 - v01 - v10) * xf * yf;
}

float sdTri(vec2 p, float bottom, float top, float halfWidth) {
    vec2 p0 = vec2(0.0, top); vec2 p1 = vec2(halfWidth, bottom); vec2 p2 = vec2(-halfWidth, bottom);
    vec2 e0 = p1 - p0; vec2 e1 = p2 - p1; vec2 e2 = p0 - p2;
    vec2 v0 = p - p0;  vec2 v1 = p - p1;  vec2 v2 = p - p2;
    vec2 pq0 = v0 - e0 * clamp(dot(v0, e0) / dot(e0, e0), 0.0, 1.0);
    vec2 pq1 = v1 - e1 * clamp(dot(v1, e1) / dot(e1, e1), 0.0, 1.0);
    vec2 pq2 = v2 - e2 * clamp(dot(v2, e2) / dot(e2, e2), 0.0, 1.0);
    float s = sign(e0.x * e2.y - e0.y * e2.x);
    vec2 d = min(min(vec2(dot(pq0, pq0), s * (v0.x * e0.y - v0.y * e0.x)),
                     vec2(dot(pq1, pq1), s * (v1.x * e1.y - v1.y * e1.x))),
                 vec2(dot(pq2, pq2), s * (v2.x * e2.y - v2.y * e2.x)));
    return -sqrt(d.x) * sign(d.y);
}

float sdTree(vec2 p, vec2 pos, float scale) {
    vec2 q = (p - pos) / scale;
    float d = sdTri(q, 0.0, 0.5, 0.25);
    d = min(d, sdTri(q, 0.20, 0.7, 0.2));
    d = min(d, sdTri(q, 0.45, 0.9, 0.15));
    return d * scale;
}

float sdRock(vec2 p, vec2 center, vec2 radii) {
    vec2 q = p - center; return length(q / radii) - 1.0;
}

// Reverted Islands back to original
float getLand(vec2 uv) {
    float left = sdRock(uv, vec2(-0.8, -0.2), vec2(0.4, 0.15));
    left = min(left, sdRock(uv, vec2(-0.75, -0.18), vec2(0.3, 0.18)));
    float right = sdRock(uv, vec2(0.8, -0.22), vec2(0.35, 0.12));
    right = min(right, sdRock(uv, vec2(0.85, -0.2), vec2(0.25, 0.14)));
    return min(left, right);
}

float getTrees(vec2 uv) {
    float d = 1.0;
    d = min(d, sdTree(uv, vec2(-0.95, -0.15), 0.35));
    d = min(d, sdTree(uv, vec2(-0.75, -0.10), 0.45));
    d = min(d, sdTree(uv, vec2(-0.60, -0.15), 0.30));
    d = min(d, sdTree(uv, vec2(0.65, -0.18), 0.25));
    d = min(d, sdTree(uv, vec2(0.80, -0.12), 0.35));
    d = min(d, sdTree(uv, vec2(0.95, -0.16), 0.28));
    return d;
}

// Extracted Background Mountains
float getBgMount1(vec2 uv) {
    return uv.y - (sin(uv.x * 2.0)*0.1 + cos(uv.x * 0.8)*0.08 + 0.15);
}

float getBgMount2(vec2 uv) {
    return uv.y - (sin(uv.x * 2.5 - 2.0)*0.06 + 0.05);
}

vec4 drawTaperedClouds(vec2 uv, float offsetY, float freq, float amp, float driftSpeed, float maxThick, float spacing) {
    float driftX = (uv.x - iTime * driftSpeed) * spacing;
    float id = floor(driftX); float fractX = fract(driftX);
    float envelope = smoothstep(0.05, 0.3, fractX) * smoothstep(0.95, 0.7, fractX);
    float thick = maxThick * envelope * (0.6 + 0.4 * fract(sin(id * 111.1) * 222.2));
    float waveTime = iTime * 0.3;
    float wave = sin(uv.x * freq - waveTime) * amp + cos(uv.x * freq * 0.6 + waveTime * 0.8) * amp * 0.4;
    float dist = abs(uv.y - (offsetY + wave));
    if (dist < thick && thick > 0.001) {
        float t = dist / thick;
        vec3 col = vec3(0.92, 0.93, 0.95);
        if (t < 0.25) col = vec3(0.55, 0.58, 0.62);
        else if (t < 0.5) col = vec3(0.65, 0.68, 0.72);
        else if (t < 0.75) col = vec3(0.78, 0.80, 0.84);
        float aa = 2.0 / iResolution.y;
        float alpha = smoothstep(thick, thick - aa, dist);
        return vec4(col, alpha);
    }
    return vec4(0.0);
}

vec3 getColorCycle(float t, vec3 cNight, vec3 cMorn, vec3 cDay, vec3 cSun, vec3 cDusk) {
    vec3 col = mix(cNight, cMorn, smoothstep(0.05, 0.20, t));
    col = mix(col, cDay, smoothstep(0.25, 0.40, t));
    col = mix(col, cSun, smoothstep(0.55, 0.70, t));
    col = mix(col, cDusk, smoothstep(0.75, 0.85, t));
    return mix(col, cNight, smoothstep(0.90, 0.98, t));
}

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 uv = (fragCoord - 0.5 * iResolution.xy) / iResolution.y;
    uv *= 2.0;

    float cycleSpeed = 0.04;
    float t = fract(iTime * cycleSpeed);
    vec3 ambient = getColorCycle(t, vec3(0.15,0.2,0.35), vec3(0.99, 0.94, 0.94), vec3(1.0), mix(vec3(0.98, 0.80, 0.04), vec3(0.90, 0.21, 0.32), 0.5), mix(vec3(0.67, 0.44, 0.66), vec3(0.35, 0.54, 0.82), 0.5));

    float waterLevel = -0.15;
    float shoreBase = -0.75 + sin(uv.x * 1.5) * 0.05 + cos(uv.x * 3.0) * 0.03;
    float tideAmplitude = 0.14;
    float activeWaterLine = shoreBase + sin(iTime * 1.5 + uv.x * 2.0) * tideAmplitude + cos(iTime * 0.8) * 0.03 + GradientNoise2D(vec2(uv.x * 5.0, iTime * 0.5)) * 0.04;

    // 1. Sky
    vec3 colSky = mix(getColorCycle(t, vec3(0.04, 0.08, 0.15), vec3(0.96, 0.92, 0.90), vec3(0.68, 0.81, 0.88), vec3(0.98, 0.80, 0.04), vec3(0.67, 0.44, 0.66)),
                      getColorCycle(t, vec3(0.01, 0.02, 0.05), vec3(0.84, 0.80, 0.86), vec3(0.50, 0.70, 0.85), vec3(0.90, 0.21, 0.32), vec3(0.35, 0.54, 0.82)), smoothstep(-0.2, 0.8, uv.y));
    float starFade = smoothstep(0.1, 0.0, t) + smoothstep(0.9, 1.0, t);
    colSky += vec3(step(0.992, fract(sin(dot(uv * 120.0, vec2(12.9898, 78.233))) * 43758.5453)) * (0.5 + 0.5 * sin(iTime * 3.0 + uv.x * 200.0))) * starFade;

    // 2. Clouds
    vec4 cloudAll = vec4(0.0);
    if (uv.y >= waterLevel) {
        vec4 cloud1 = drawTaperedClouds(uv, 0.85, 2.0, 0.12, 0.10, 0.08, 0.4);
        vec4 cloud2 = drawTaperedClouds(uv, 0.65, 2.5, 0.15, 0.07, 0.06, 0.5);
        vec4 cloud3 = drawTaperedClouds(uv, 0.45, 1.8, 0.08, 0.12, 0.05, 0.3);
        cloudAll = cloud1;
        if (cloud2.a > 0.0) cloudAll = mix(cloudAll, cloud2, cloud2.a);
        if (cloud3.a > 0.0) cloudAll = mix(cloudAll, cloud3, cloud3.a);
    }

    // 3. Foreground Islands & Trees
    float dLand = getLand(uv);
    vec3 rockCol = mix(vec3(0.35, 0.38, 0.42), vec3(0.50, 0.53, 0.57), smoothstep(-0.5, 0.5, sin(uv.y * 250.0 + sin(uv.x * 15.0)))) * ambient;
    float dTrees = getTrees(uv);
    vec3 treeCol = vec3(0.48, 0.60, 0.56) * ambient;

    // 4. Background Mountains
    float dMount1 = getBgMount1(uv);
    float dMount2 = getBgMount2(uv);

    // 5. Water
    float wt = clamp((waterLevel - uv.y) / (waterLevel - -1.0), 0.0, 1.0);
    vec3 midBand = getColorCycle(t, vec3(0.04, 0.075, 0.14), vec3(0.29, 0.45, 0.58), vec3(0.335, 0.505, 0.56), mix(vec3(0.90, 0.21, 0.32), vec3(0.45, 0.23, 0.43), 0.5), mix(vec3(0.67, 0.44, 0.66), vec3(0.18, 0.16, 0.36), 0.5));
    vec3 colWater = (wt < 0.5) ? mix(getColorCycle(t, vec3(0.06, 0.10, 0.18), vec3(0.68, 0.85, 0.90), vec3(0.62, 0.66, 0.70), vec3(0.90, 0.21, 0.32), vec3(0.67, 0.44, 0.66)), midBand, wt * 2.0)
                               : mix(midBand, getColorCycle(t, vec3(0.02, 0.05, 0.10), vec3(0.16, 0.23, 0.30), vec3(0.05, 0.35, 0.42), vec3(0.45, 0.23, 0.43), vec3(0.18, 0.16, 0.36)), (wt - 0.5) * 2.0);
    float foamAlpha = (uv.y - activeWaterLine > 0.0 && uv.y - activeWaterLine < 0.08) ? smoothstep(0.08, 0.0, uv.y - activeWaterLine) * (0.5 + 0.5 * GradientNoise2D(vec2(uv.x * 15.0, uv.y * 15.0 - iTime))) : 0.0;

    // 6. Sand
    vec3 sandCol = mix(vec3(0.75, 0.70, 0.65), vec3(0.70, 0.65, 0.58), smoothstep(0.5, 0.9, sin(uv.y * 120.0 + cos(uv.x * 5.0) * 2.0)) * 0.4) * mix(1.0, 0.60 + 0.05 * GradientNoise2D(vec2(uv.x * 40.0, uv.y * 40.0)), smoothstep(0.18, 0.0, shoreBase + tideAmplitude - uv.y)) * ambient;

    bool isTree = dTrees < 0.0;
    bool isIsland = dLand < 0.0;
    bool isCloud = uv.y >= waterLevel && cloudAll.a > 0.2;
    bool isMount2 = uv.y >= waterLevel && dMount2 < 0.0; // Front background mountain
    bool isMount1 = uv.y >= waterLevel && dMount1 < 0.0; // Rear background mountain

    float layerScale = 6.0; vec2 layerOffset = vec2(0.0); vec3 layerColor = vec3(0.0); float shapeEdge = 1.0;

    if (isTree) {
        layerScale = 35.0; layerOffset = vec2(7.0, 8.0); layerColor = treeCol;
    } else if (isIsland) {
        layerScale = 20.0; layerOffset = vec2(5.0, 6.0); layerColor = rockCol;
    } else if (isCloud) {
        layerScale = 40.0; layerOffset = vec2(3.0, 4.0); layerColor = cloudAll.rgb * ambient * 1.1;
    } else if (isMount2) {
        layerScale = 12.0; layerOffset = vec2(15.0, 16.0); layerColor = vec3(0.55, 0.63, 0.70) * ambient;
    } else if (isMount1) {
        layerScale = 12.0; layerOffset = vec2(17.0, 18.0); layerColor = vec3(0.60, 0.68, 0.75) * ambient;
    } else if (uv.y >= waterLevel) {
        layerScale = 8.0; layerOffset = vec2(1.0, 2.0); layerColor = colSky;
    } else if (uv.y >= activeWaterLine) {
        layerScale = 15.0; layerOffset = vec2(9.0, 10.0); layerColor = colWater;
        if (foamAlpha > 0.3) layerColor = mix(colWater, vec3(0.9, 0.95, 1.0) * ambient, 0.8);
    } else {
        layerScale = 12.0; layerOffset = vec2(11.0, 12.0); layerColor = sandCol;
    }


    // Evaluates from front to back to draw lines around each specific shape cutoff
    if (dTrees < 0.015) {
        shapeEdge = smoothstep(0.0, 0.015, abs(dTrees));
    } else if (dLand < 0.015) {
        shapeEdge = smoothstep(0.0, 0.015, abs(dLand));
    } else if (uv.y >= waterLevel && abs(cloudAll.a - 0.2) < 0.02) {
        shapeEdge = smoothstep(0.0, 0.02, abs(cloudAll.a - 0.2));
    } else if (uv.y >= waterLevel && abs(dMount2) < 0.012 && !isCloud) {
        shapeEdge = smoothstep(0.0, 0.012, abs(dMount2));
    } else if (uv.y >= waterLevel && abs(dMount1) < 0.012 && !isMount2 && !isCloud) {
        shapeEdge = smoothstep(0.0, 0.012, abs(dMount1));
    } else if (abs(uv.y - waterLevel) < 0.015) {
        shapeEdge = smoothstep(0.0, 0.015, abs(uv.y - waterLevel));
    } else if (abs(uv.y - activeWaterLine) < 0.015) {
        shapeEdge = smoothstep(0.0, 0.015, abs(uv.y - activeWaterLine));
    } else if (uv.y < waterLevel && uv.y >= activeWaterLine && abs(foamAlpha - 0.3) < 0.05) {
        shapeEdge = smoothstep(0.0, 0.05, abs(foamAlpha - 0.3));
    }

    vec3 v = voro((uv + layerOffset) * layerScale);
    float finalMask = min(smoothstep(0.04, 0.08, v.z), shapeEdge);

    vec3 glassTint = clamp(vec3(v.x * 0.8 + 0.3, v.y * 0.8 + 0.3, 1.0), 0.0, 1.0);
    vec3 finalCol = layerColor * mix(vec3(1.0), glassTint, 0.15);
    finalCol += (smoothstep(0.4, 0.8, v.z) * 0.15) * glassTint;

    fragColor = vec4(finalCol * finalMask * (1.0 - dot(uv*0.25, uv*0.25)), 1.0);
}
