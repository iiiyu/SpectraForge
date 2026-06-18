// Aurora Paint for SpectraForge.
//
// Based on "Aurora Paint" by Noztol.
// Adapted to SpectraForge's mainImage entry point and audio uniforms.

const float pi = 3.14159;
const float pi2 = pi * 2.;

// Audio-reactive globals, set once at the top of mainImage so the helper
// functions (which take no audio args) can read them.
float aBass;    // low-end energy: aurora brightness + sway
float aMid;     // mids: wind sway on trees
float aTreble;  // highs: blotch-star twinkle

float opU( float d1, float d2 ){ return min(d1,d2); }
float opS( float d2, float d1 ){ return max(-d1,d2); }
float opI( float d1, float d2) { return max(d1,d2); }

float magicBox(vec3 p) {
    const int MAGIC_BOX_ITERS = 13;
    const float MAGIC_BOX_MAGIC = 0.55;
    p = 1.0 - abs(1.0 - mod(p, 2.0));
    float lastLength = length(p);
    float tot = 0.0;
    for (int i=0; i < MAGIC_BOX_ITERS; i++) {
      p = abs(p)/(lastLength*lastLength) - MAGIC_BOX_MAGIC;
      float newLength = length(p);
      tot += abs(newLength-lastLength);
      lastLength = newLength;
    }
    return tot;
}

float magicBox(vec2 uv){
    const mat3 M = mat3(0.28862355854826727, 0.6997227302779844, 0.6535170557707412,
                        0.06997493955670424, 0.6653237235314099, -0.7432683571499161,
                        -0.9548821651308448, 0.26025457467376617, 0.14306504491456504);
    vec3 p = 0.5*M*vec3(uv, 0.0);
    return magicBox(p);
}

mat2 rot2D(float r) {
    float c = cos(r), s = sin(r);
    return mat2(c, s, -s, c);
}
float nsin(float a){return .5+.5*sin(a);}
float ncos(float a){return .5+.5*cos(a);}
vec3 saturate(vec3 a){return clamp(a,0.,1.);}
float rand(vec2 co){ return fract(sin(dot(co.xy ,vec2(12.9898,78.233))) * 43758.5453); }
float rand(float n){ return fract(cos(n*89.42)*343.42); }
float dtoa(float d, float amount) { return clamp(1.0 / (clamp(d, 1.0/amount, 1.0)*amount), 0.,1.); }
float sdAxisAlignedRect(vec2 uv, vec2 tl, vec2 br) {
    vec2 d = max(tl-uv, uv-br);
    return length(max(vec2(0.0), d)) + min(0.0, max(d.x, d.y));
}
float sdCircle(vec2 uv, vec2 origin, float radius) { return length(uv - origin) - radius; }
float smoothstep4(float e1, float e2, float e3, float e4, float val) {
    return min(smoothstep(e1,e2,val), 1.-smoothstep(e3,e4,val));
}
float noise01(vec2 p) { return clamp((fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453)+.5)*.5, 0.,1.); }
float smoothf(float x) { return x*x*x*(x*(x*6. - 15.) + 10.); }

float sdTri(vec2 p, float bottom, float top, float halfWidth) {
    vec2 p0 = vec2(0.0, top);
    vec2 p1 = vec2(halfWidth, bottom);
    vec2 p2 = vec2(-halfWidth, bottom);
    vec2 e0 = p1 - p0; vec2 e1 = p2 - p1; vec2 e2 = p0 - p2;
    vec2 v0 = p - p0;  vec2 v1 = p - p1;  vec2 v2 = p - p2;
    vec2 pq0 = v0 - e0*clamp(dot(v0,e0)/dot(e0,e0), 0.0, 1.0);
    vec2 pq1 = v1 - e1*clamp(dot(v1,e1)/dot(e1,e1), 0.0, 1.0);
    vec2 pq2 = v2 - e2*clamp(dot(v2,e2)/dot(e2,e2), 0.0, 1.0);
    float s = sign( e0.x*e2.y - e0.y*e2.x );
    vec2 d = min( min( vec2( dot( pq0, pq0 ), s*(v0.x*e0.y-v0.y*e0.x) ),
                       vec2( dot( pq1, pq1 ), s*(v1.x*e1.y-v1.y*e1.x) )),
                       vec2( dot( pq2, pq2 ), s*(v2.x*e2.y-v2.y*e2.x) ));
    return -sqrt(d.x)*sign(d.y);
}

float sdTree(vec2 p, vec2 pos, float scale) {
    vec2 q = (p - pos) / scale;
    // Wind sway, driven by the mids.
    q.x -= sin(iTime * 1.5 + pos.x * 10.0) * (0.02 + aMid * 0.04) * max(0.0, q.y);

    // Trunk
    float d = max(abs(q.x) - 0.03, max(-q.y, q.y - 0.2));

    // Jagged canopy layers
    d = min(d, sdTri(q, 0.1, 0.45, 0.25));
    d = min(d, sdTri(q, 0.25, 0.65, 0.2));
    d = min(d, sdTri(q, 0.45, 0.85, 0.15));
    d = min(d, sdTri(q, 0.65, 1.0, 0.1));
    return d * scale;
}

// Solid sloping landmasses framing the center
float getLand(vec2 uv, float aspect) {
    float leftShore = 0.45 * exp(-0.5 * pow(uv.x, 2.0));
    float rightShore = 0.45 * exp(-0.5 * pow(uv.x - aspect, 2.0));

    float terrainHeight = max(leftShore, rightShore);
    terrainHeight += (noise01(uv * vec2(20.0, 1.0)) * 0.015);

    return uv.y - terrainHeight;
}

// Trees planted along the slopes
float getTrees(vec2 uv, float aspect) {
    float d = 1.0;
    // Left shore trees
    d = min(d, sdTree(uv, vec2(aspect*0.02, 0.42), 0.18));
    d = min(d, sdTree(uv, vec2(aspect*0.08, 0.38), 0.22));
    d = min(d, sdTree(uv, vec2(aspect*0.14, 0.42), 0.14));
    d = min(d, sdTree(uv, vec2(aspect*0.18, 0.41), 0.09));
    d = min(d, sdTree(uv, vec2(aspect*0.22, 0.41), 0.08));
    d = min(d, sdTree(uv, vec2(aspect*0.24, 0.41), 0.08));
    d = min(d, sdTree(uv, vec2(aspect*0.29, 0.38), 0.08));

    // Right shore trees
    d = min(d, sdTree(uv, vec2(aspect*0.70, 0.38), 0.08));
    d = min(d, sdTree(uv, vec2(aspect*0.74, 0.40), 0.08));
    d = min(d, sdTree(uv, vec2(aspect*0.78, 0.42), 0.1));
    d = min(d, sdTree(uv, vec2(aspect*0.82, 0.41), 0.09));
    d = min(d, sdTree(uv, vec2(aspect*0.86, 0.42), 0.14));
    d = min(d, sdTree(uv, vec2(aspect*0.92, 0.38), 0.22));
    d = min(d, sdTree(uv, vec2(aspect*0.98, 0.42), 0.18));
    return d;
}

vec3 colorBrushStroke(vec2 uvLine, vec2 uvPaper, vec2 lineSize, float sdGeometry, vec3 inpColor, vec4 brushColor)
{
    float posInLineY = (uvLine.y / lineSize.y);

    if(posInLineY > 0.) {
        float smoothWave = 0.5 + 0.5 * sin(iTime * 1.5);
        float autoX = mix(0.15, 0.7, smoothWave);
        posInLineY = pow(posInLineY, (pow(autoX,2.) * 15.) + 1.5);
    }

    float strokeBoundary = dtoa(sdGeometry, 300.);
    float strokeTexture = 0.
        + noise01(uvLine * vec2(min(iResolution.y,iResolution.x)*0.2, 1.))
        + noise01(uvLine * vec2(79., 1.))
        + noise01(uvLine * vec2(14., 1.));
    strokeTexture *= 0.333 * strokeBoundary;
    strokeTexture = max(0.008, strokeTexture);

    float strokeAlpha = pow(strokeTexture, max(0.,posInLineY)+0.09);
    const float strokeAlphaBoost = 1.09;

    if(posInLineY > 0.) strokeAlpha = strokeAlphaBoost * max(0., strokeAlpha - pow(posInLineY,0.5));
    else strokeAlpha *= strokeAlphaBoost;

    strokeAlpha = smoothf(strokeAlpha);
    float paperBleedAmt = 60. + (rand(uvPaper.y) * 30.) + (rand(uvPaper.x) * 30.);
    float alpha = strokeAlpha * brushColor.a * dtoa(sdGeometry, paperBleedAmt);
    alpha = clamp(alpha, 0.,1.);
    return mix(inpColor, brushColor.rgb, alpha);
}

vec3 colorBrushStrokeLine(vec2 uv, vec3 inpColor, vec4 brushColor, vec2 p1_, vec2 p2_, float lineWidth)
{
    float lineAngle = pi-atan(p1_.x - p2_.x, p1_.y - p2_.y);
    mat2 rotMat = rot2D(lineAngle);
    float lineLength = distance(p2_, p1_);
    vec2 tl = (p1_ * rotMat);
    vec2 br = tl + vec2(0,lineLength);
    vec2 uvLine = uv * rotMat;

    lineWidth *= mix(1., .9, smoothstep(tl.y,br.y,uvLine.y));
    uvLine.x += (noise01(uvLine * 1.)-0.5) * 0.02;
    uvLine.x += cos(uvLine.y * 3.) * 0.009;
    uvLine.x += (noise01(uvLine * 5.)-0.5) * 0.005;

    float d = sdAxisAlignedRect(uvLine, tl, br) - lineWidth / 2.;
    uvLine = tl - uvLine;
    vec2 lineSize = vec2(lineWidth, lineLength);
    return colorBrushStroke(vec2(uvLine.x, -uvLine.y), uv, lineSize, d, inpColor, brushColor);
}

vec2 getuv_centerX(vec2 fragCoord, vec2 newTL, vec2 newSize)
{
    vec2 ret = vec2(fragCoord.x / iResolution.x, fragCoord.y / iResolution.y);
    ret *= newSize;
    float aspect = iResolution.x / iResolution.y;
    ret.x *= aspect;
    float newWidth = newSize.x * aspect;
    return ret + vec2(newTL.x - (newWidth - newSize.x) / 2.0, newTL.y);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    aBass   = iBass;
    aMid    = iMid;
    aTreble = iTreble;

    vec2 uv = getuv_centerX(fragCoord, vec2(-1,-1), vec2(2,2)); // Auroras (-1 to 1)
    vec2 landscapeUV = fragCoord / iResolution.y;               // Landscape SDFs (0 to Aspect)
    float aspect = iResolution.x / iResolution.y;

    float t = iTime * 0.4;

    // Aurora brightness/alpha boost driven by the bass.
    float glow = 1.0 + aBass * 1.2;

    float waterLevelLandscape = 0.34;
    float waterLevelUV = waterLevelLandscape * 2.0 - 1.0;
    bool isWater = landscapeUV.y < waterLevelLandscape;

    vec3 skyColor = mix(vec3(0.08, 0.15, 0.35), vec3(0.2, 0.4, 0.6), smoothstep(-0.2, 1.0, isWater ? (waterLevelUV + (waterLevelUV - uv.y)) : uv.y));
    vec3 col = skyColor;
    if (isWater) {
        vec3 waterBase = vec3(0.06, 0.18, 0.28);
        col = mix(col, waterBase, 0.85);
    }

    if (!isWater) {
        // 1. Paint Blotch Stars — twinkle with the treble.
        float blotchAmt = smoothstep(12.0, 25.0, magicBox((uv+12.0)*8.0));
        blotchAmt = pow(blotchAmt, 1.5);
        col += vec3(0.6, 0.85, 0.95) * blotchAmt * (0.6 + aTreble * 0.9) * (0.5 + 0.5 * sin(iTime*1.5 + uv.x*5.0));

        // 2. Aurora — sway amplitude grows slightly with the bass.
        float sway = 1.0 + aBass * 0.4;
        vec2 aUV1 = uv; aUV1.y += (sin(aUV1.x * 2.0 + 1.0 + t) * 0.35 + sin(aUV1.x * 4.0 - t*1.5) * 0.1) * sway;
        vec2 aUV2 = uv; aUV2.y += (cos(aUV2.x * 2.5 - 0.5 - t*0.8) * 0.4 + sin(aUV2.x * 1.5 + t*1.2) * 0.2) * sway;
        vec2 aUV3 = uv; aUV3.y += sin(aUV3.x * 1.8 + 2.0 + t*0.5) * 0.35 * sway;

        col = colorBrushStrokeLine(aUV3, col, vec4(vec3(0.1, 0.8, 0.5)*glow, 0.6), vec2(-2.5, 0.6), vec2(2.5, 0.6), 0.4);
        col = colorBrushStrokeLine(aUV1, col, vec4(vec3(0.3, 0.9, 0.7)*glow, 0.8), vec2(-2.5, 0.4), vec2(2.5, 0.4), 0.3);
        col = colorBrushStrokeLine(aUV2, col, vec4(vec3(0.0, 0.6, 0.5)*glow, 0.9), vec2(2.5, 0.2), vec2(-2.5, 0.2), 0.25);
        col = colorBrushStrokeLine(aUV1, col, vec4(vec3(0.6, 1.0, 0.8)*glow, 0.9), vec2(-2.5, 0.3), vec2(2.5, 0.3), 0.15);

    } else {
        // 3. Aurora reflections
        float depthUV = waterLevelUV - uv.y;
        vec2 refUV = uv;
        refUV.y = waterLevelUV + depthUV;
        refUV.x += sin(uv.y * 30.0 + iTime * 2.0) * depthUV * 0.15;
        refUV.y += cos(uv.x * 15.0 + iTime * 1.5) * depthUV * 0.025;

        vec2 rUV1 = refUV; rUV1.y += sin(rUV1.x * 2.0 + 1.0 + t) * 0.35 + sin(rUV1.x * 4.0 - t*1.5) * 0.1;
        vec2 rUV2 = refUV; rUV2.y += cos(rUV2.x * 2.5 - 0.5 - t*0.8) * 0.4 + sin(rUV2.x * 1.5 + t*1.2) * 0.2;
        vec2 rUV3 = refUV; rUV3.y += sin(rUV3.x * 1.8 + 2.0 + t*0.5) * 0.35;

        col = colorBrushStrokeLine(rUV3, col, vec4(vec3(0.1, 0.7, 0.5)*glow, 0.3), vec2(-2.5, 0.6), vec2(2.5, 0.6), 0.4);
        col = colorBrushStrokeLine(rUV1, col, vec4(vec3(0.2, 0.8, 0.6)*glow, 0.4), vec2(-2.5, 0.4), vec2(2.5, 0.4), 0.3);
        col = colorBrushStrokeLine(rUV2, col, vec4(vec3(0.0, 0.5, 0.4)*glow, 0.45), vec2(2.5, 0.2), vec2(-2.5, 0.2), 0.25);
        col = colorBrushStrokeLine(rUV1, col, vec4(vec3(0.4, 0.8, 0.7)*glow, 0.35), vec2(-2.5, 0.3), vec2(2.5, 0.3), 0.15);
    }

    // 4. Island Silhouettes
    if (!isWater) {
        float sceneDist = min(getLand(landscapeUV, aspect), getTrees(landscapeUV, aspect));
        col = mix(col, vec3(0.02, 0.04, 0.08), smoothstep(0.005, 0.0, sceneDist));
    } else {
        float depthLandscape = waterLevelLandscape - landscapeUV.y;
        vec2 refLandUV = landscapeUV;
        refLandUV.y = waterLevelLandscape + depthLandscape;
        refLandUV.x += sin(landscapeUV.y * 50.0 + iTime * 2.0) * depthLandscape * 0.1;

        float refSceneDist = min(getLand(refLandUV, aspect), getTrees(refLandUV, aspect));
        col = mix(col, mix(col, vec3(0.02, 0.04, 0.08), 0.8), smoothstep(0.005, 0.0, refSceneDist));

        vec2 surfaceUV = uv;
        surfaceUV.x += iTime * 0.05;
        col = colorBrushStrokeLine(surfaceUV, col, vec4(0.3, 0.8, 0.6, 0.15), vec2(2.5, waterLevelUV - 0.1), vec2(-2.5, waterLevelUV - 0.1), 0.08);
        col = colorBrushStrokeLine(surfaceUV, col, vec4(0.1, 0.6, 0.5, 0.2), vec2(-2.5, waterLevelUV - 0.3), vec2(2.5, waterLevelUV - 0.3), 0.12);
    }

    col.rgb += (rand(uv)-.5)*.06;
    col.rgb = saturate(col.rgb);

    vec2 uvScreen = (fragCoord / iResolution.xy * 2.)-1.;
    float vignetteAmt = 1.0 - dot(uvScreen*0.5, uvScreen* 0.62);
    col *= vignetteAmt;

    fragColor = vec4(col, 1.);
}
