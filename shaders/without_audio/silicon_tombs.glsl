// Silicon Tombs (audio-free) for SpectraForge.
//
// "Tomb World of Silicon Beats" by FragCoord. Adapted to SpectraForge's
// mainImage entry point (u_time -> iTime, u_resolution -> iResolution). The
// original drove a beat() / freq() pair off a u_audio texture; here both are
// synthesized from iTime so the scene pulses on its own. Animates on iTime
// alone; pair with --duration-only.

const vec4 hsv2rgb_K = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
#define HSV2RGB(c)  (c.z * mix(hsv2rgb_K.xxx, clamp(abs(fract(c.xxx + hsv2rgb_K.xyz) * 6.0 - hsv2rgb_K.www) - hsv2rgb_K.xxx, 0.0, 1.0), c.y))

vec3 hsv2rgb(vec3 c) {
  vec3 p = abs(fract(c.xxx + hsv2rgb_K.xyz) * 6.0 - hsv2rgb_K.www);
  return c.z * mix(hsv2rgb_K.xxx, clamp(p - hsv2rgb_K.xxx, 0.0, 1.0), c.y);
}

const float
  fft_limit = 0.5,
  fov = 2.0,
  motion_blur = 0.3,
  tomb_probability = 0.3,
  OFF = 0.7;

const vec2
  camera_direction = vec2(-0.5, 0.1),
  camera_pos = vec2(25.0, 3.3);

//change the color
const vec3
  BY = HSV2RGB(vec3(0.3 + OFF, 0.7, 0.8)),
  BG = HSV2RGB(vec3(0.95 + OFF, 0.6, 0.3)),
  BW = HSV2RGB(vec3(0.55 + OFF, 0.3, 2.0)),
  BF = HSV2RGB(vec3(0.82 + OFF, 0.6, 2.0)),
  FC = HSV2RGB(vec3(0.3, 0.7, 0.15)),
  LD = normalize(vec3(1.0, -0.5, 1.0)),
  RN = normalize(vec3(-0.2, 1.0, -1.1));

const vec4 GG = vec4(vec3(-700.0, 300.0, 1000.0), 400.0);

const float
  PI = 3.141592654,
  TAU = 2.0 * PI,
  PI_2 = 0.5 * PI,
  ZZ = 11.0;

const vec2
  PA = vec2(6.0, 1.41),
  PB = vec2(0.056, 0.035);

const mat2 R = mat2(1.2, 1.6, -1.6, 1.2);

float hash(vec2 co) {
  return fract(sin(dot(co.xy, vec2(12.9898, 58.233))) * 13758.5453);
}

float hash12(vec2 p)
{
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

vec3 filmGrain(vec2 uv)
{
    float n = hash12(uv + fract(iTime));
    return vec3(n - 0.5);
}

// --- Synthetic beat (no audio): a steady ~120bpm thump with a soft swing. ---
float beat() {
  float tempo = 2.0;                      // beats per second
  float phase = fract(iTime * tempo);
  float thump = exp(-phase * 6.0);        // sharp attack, quick decay
  float swing = 0.5 + 0.5 * sin(iTime * 0.7);
  return clamp(2.0 * thump * (0.6 + 0.4 * swing), 0.0, 2.0);
}

// --- Synthetic spectrum (no audio): bass-weighted, drifting over time. ---
float freq(float x) {
  x = fract(x) * 0.5 + 0.025;
  float env = exp(-x * 2.5);
  float wob = 0.5 + 0.5 * sin(iTime * (2.0 + x * 9.0) + x * 24.0);
  return clamp(env * (0.4 + 0.7 * wob), 0.0, 1.0) * (1.0 + 0.5 * x);
}

vec3 tanh_approx(vec3 x) {
  vec3 x2 = x * x;
  return clamp(x * (27.0 + x2) / (27.0 + 9.0 * x2), -1.0, 1.0);
}

float atan_approx(float y, float x) {
  float cosatan2 = x / (abs(x) + abs(y));
  float t = PI_2 - cosatan2 * PI_2;
  return y < 0.0 ? -t : t;
}

float acos_approx(float x) {
  return atan_approx(sqrt(max(0.0, 1.0 - x * x)), x);
}

vec3 to_spherical(vec3 p) {
  float r = length(p);
  return vec3(r, acos_approx(p.z / r), atan_approx(p.y, p.x));
}

vec3 stars(vec3 R) {
  float Z = TAU / 200.0;
  vec3 col = vec3(0.0);
  float a = 1.0;
  for (int i = 0; i < 3; ++i) {
    R = R.zxy;
    vec2 s = to_spherical(R).yz;
    vec2 n = floor(s / Z + 0.5);
    vec2 c = s - Z * n;
    float h = sin(s.x);
    float h0 = hash(n + 123.4 * float(i + 1));
    float h1 = fract(8887.0 * h0);
    float h2 = fract(9187.0 * h0);
    float h3 = fract(9677.0 * h0);
    c.y *= h;
    col += a * hsv2rgb(vec3(-0.4 * h1, sqrt(h3), step(h0, 0.1 * h) * h1 * vec3(7e-6) / max(7e-7, dot(c, c))));
    Z *= 0.5;
    a *= 0.5;
  }
  return col;
}

float ray_sphere(vec3 ro, vec3 rd, vec4 sph) {
  vec3 oc = ro - sph.xyz;
  float b = dot(oc, rd);
  float c = dot(oc, oc) - sph.w * sph.w;
  float h = b * b - c;
  if (h < 0.0) return -1.0;
  return -b - sqrt(h);
}

float ray_plane(vec3 ro, vec3 rd, vec4 p) {
  return -(dot(ro, p.xyz) + p.w) / dot(rd, p.xyz);
}

float doctahedron(vec3 p, float s) {
  p = abs(p);
  return (p.x + p.y + p.z - s) * 0.57735027;
}

vec3 path(float z) {
  return vec3(camera_pos + PA * cos(PB * z), z);
}

vec3 dpath(float z) {
  return vec3(-PA * PB * sin(PB * z), 1.0);
}

vec3 ddpath(float z) {
  return vec3(-PA * PB * PB * cos(PB * z), 0.0);
}

float dfbm(vec3 p) {
  float d = p.y + 0.6;
  float a = 1.0;
  vec2 D = vec2(0.0);
  vec2 P = 0.23 * p.xz;
  vec4 o;
  for (int j = 0; j < 7; ++j) {
    o = cos(P.xxyy + vec4(11.0, 0.0, 11.0, 0.0));
    p = o.yxx * o.zwz;
    D += p.xy;
    d -= a * (1.0 + p.z) / (1.0 + 3.0 * dot(D, D));
    P *= R;
    a *= 0.55;
  }
  return d;
}

float dpyramid(vec3 p, out vec3 oo) {
  vec2 n = floor(p.xz / ZZ + 0.5);
  p.xz -= n * ZZ;
  float h0 = hash(n);
  float h1 = fract(9677.0 * h0);
  float h  = 0.3 * ZZ * h0 * h0 + 0.1;
  float d  = doctahedron(p, h);
  oo = vec3(1e3, 0.0, 0.0);
  if (h1 > tomb_probability) return 1e3;
  oo = vec3(d, h0, h);
  return d;
}

float df(vec3 p, out vec3 oo) {
  p.y = abs(p.y);
  float d0 = dfbm(p);
  float d1 = dpyramid(p, oo);
  float d = d0;
  d = min(d, d1);
  return d;
}

float fbm(float x) {
  float a = 1.0;
  float h = 0.0;
  for (int i = 0; i < 5; ++i) {
    h += a * sin(x);
    x *= 2.03;
    x += 123.4;
    a *= 0.55;
  }
  return abs(h);
}

vec4 render(vec2 p2, vec2 q2) {
  float d = 1.0, z = 0.0;
  float T = 3.0 * iTime;
  float B = beat();
  float F, L;
  vec3 oo;
  vec3 O = vec3(0.0);
  vec3 p;
  vec3 P = path(T);
  vec3 ZZ_local = normalize(dpath(T) + vec3(camera_direction, 0.0));
  vec3 XX = normalize(cross(ZZ_local, vec3(0.0, 1.0, 0.0) + ddpath(T)));
  vec3 YY = cross(XX, ZZ_local);
  vec3 R_dir = normalize(-p2.x * XX + p2.y * YY + fov * ZZ_local);
  vec3 Y = (1.0 + R_dir.x) * BY;
  vec3 S_base = (1.0 + R_dir.y) * BW * Y;
  vec4 M;

  for (int i = 0; i < 50 && d > 1e-5 && z < 2e2; ++i) {
    p = z * R_dir + P;
    d = df(p, oo);
    if (p.y > 0.0) {
      O += BG + min(d, 9.0) * Y;
    } else {
      O += S_base;
      oo.x *= 9.0;
    }
    O += B * smoothstep(oo.z * 0.78, oo.z * 0.8, abs(p.y))
         / max(oo.x + oo.x * oo.x * oo.x * oo.x * 9.0, 3e-2)
         * BF;
    z += d * 0.7;
  }

  O *= 9e-3;

  if (R_dir.y > 0.0) {
    M = GG;
    vec3 S = M.xyz + P;
    M.xyz = S;
    z = d = ray_sphere(P, R_dir, M);
    F = smoothstep(0.0, 0.2, R_dir.y);
    Y = clamp(hsv2rgb(vec3(OFF - 0.4 * R_dir.y, 0.5 + 1.0 * R_dir.y, 3.0 / (1.0 + 800.0 * R_dir.y * R_dir.y * R_dir.y))), 0.0, 1.0);
    L = dot(vec3(0.2126, 0.7152, 0.0722), Y);
    if (z > 0.0) {
      p = P + R_dir * z;
      ZZ_local = normalize(p - M.xyz);
      Y += max(dot(LD, ZZ_local), 0.0)
           * F
           * smoothstep(1.0, 0.89, 1.0 + dot(R_dir, ZZ_local))
           * fbm(2e-2 * dot(p - S, RN));
    }
    M = vec4(RN, -dot(RN, S));
    z = ray_plane(P, R_dir, M);
    if (z > 0.0 && (d > 0.0 && z < d || d == -1.0)) {
      p = P + R_dir * z;
      z = distance(S, p);
      Y += F
           * smoothstep(GG.w * 1.41, GG.w * 1.46, z)
           * smoothstep(GG.w * 2.0, GG.w * 1.95, z)
           * (smoothstep(fft_limit, 1.01, freq(1.5 * abs(z - GG.w * 1.48) / GG.w))
              * hsv2rgb(vec3(OFF - 0.7 + z / GG.w, 0.9, 9.0))
              + abs(dot(LD, RN)) * fbm(0.035 * z));
    }
    if (d == -1.0) {
      Y += pow(max(0.0, 1.0 - L), 4.0) * stars(R_dir);
    }
    O *= Y;
  }

  O -= (length(-1.0 + 2.0 * q2) + 0.2) * FC;
  O = tanh_approx(O);
  O = max(O, 0.0);

      // 8-bit dithering
  float dither = hash12(gl_FragCoord.xy + iTime) - 0.5;
  O += dither / 255.0;

    // film grain before gamma
  O += filmGrain(gl_FragCoord.xy) / 255.0;

  O *= smoothstep(0.0, 6.0, iTime - p2.y * p2.y);
  O = sqrt(O);
  return vec4(O, 1.0);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
  vec2 r = iResolution.xy;
  fragColor = render((fragCoord + fragCoord - r) / r.y, fragCoord / r);
}
