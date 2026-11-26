import numpy as np
import math

def mul4(A, B):
    out = [0.0] * 16
    for c in range(4):
        for r in range(4):
            s = 0.0
            for k in range(4):
                s += A[k * 4 + r] * B[c * 4 + k]
            out[c * 4 + r] = s
    return out

def transform_point4(M, p):
    x, y, z = p
    return [
        M[0] * x + M[4] * y + M[8] * z + M[12],
        M[1] * x + M[5] * y + M[9] * z + M[13],
        M[2] * x + M[6] * y + M[10] * z + M[14],
        M[3] * x + M[7] * y + M[11] * z + M[15],
    ]

def camera(bmin, bmax, yaw, pitch):
    min_x, min_y, min_z = bmin
    max_x, max_y, max_z = bmax
    cx, cy, cz = (min_x + max_x) / 2.0, (min_y + max_y) / 2.0, (min_z + max_z) / 2.0
    radius = math.sqrt((max_x - min_x) ** 2 + (max_y - min_y) ** 2 + (max_z - min_z) ** 2) / 2.0
    radius = max(radius, 1e-6)
    distance = (radius / math.sin(math.radians(45) / 2.0)) * 1.2
    near = max(distance - radius * 2.0, 1e-3)
    far = max(distance + radius * 4.0, near * 10.0)
    return dict(target=(cx, cy, cz), yaw=yaw, pitch=pitch, distance=distance,
                fov_y=math.radians(45), near=near, far=far)

def eye(cam):
    cp, sp = math.cos(cam['pitch']), math.sin(cam['pitch'])
    cy, sy = math.cos(cam['yaw']), math.sin(cam['yaw'])
    o = np.array([cp * sy, sp, cp * cy]) * cam['distance']
    return np.array(cam['target']) + o

def view_matrix(cam):
    e = eye(cam)
    f = (np.array(cam['target']) - e) / np.linalg.norm(np.array(cam['target']) - e)
    up = np.array([0.0, 1.0, 0.0])
    s = np.cross(f, up); s = s / np.linalg.norm(s)
    u = np.cross(s, f)
    m03, m13, m23 = -np.dot(s, e), -np.dot(u, e), np.dot(f, e)
    return [s[0], u[0], -f[0], 0.0, s[1], u[1], -f[1], 0.0, s[2], u[2], -f[2], 0.0, m03, m13, m23, 1.0]

def projection(cam, aspect):
    f = 1.0 / math.tan(cam['fov_y'] / 2.0)
    nf = 1.0 / (cam['near'] - cam['far'])
    return [f / aspect, 0, 0, 0, 0, f, 0, 0, 0, 0, (cam['far'] + cam['near']) * nf, -1.0, 0, 0,
            2 * cam['far'] * cam['near'] * nf, 0]

def vp(cam, aspect):
    return mul4(projection(cam, aspect), view_matrix(cam))

def f32(x):
    return np.float32(x)

bmin = (0.0, 0.0, 0.0); bmax = (80.0, 40.0, 10.0)
corners = [(x, y, z) for x in (0.0, 80.0) for y in (0.0, 40.0) for z in (0.0, 10.0)]

views = [("Front", 0.0, 0.0), ("Back", math.pi, 0.0),
         ("Left", -math.pi / 2.0, 0.0), ("Right", math.pi / 2.0, 0.0),
         ("Top", 0.0, 1.54), ("Bottom", 0.0, -1.54),
         ("Iso", math.pi / 4.0, math.asin(1.0 / math.sqrt(3.0)))]

def old_ndc_f32(m, p):
    """Current CPU path: f64 clip/w, then cast each to f32."""
    c64 = transform_point4(m, p)
    w = c64[3]
    return [f32(c64[k] / w) for k in range(3)]

def new_shader_ndc_f32(m, p):
    """New GPU path: mvp f32 * world f32 -> clip f32, then .xyz/w in f32."""
    m32 = [f32(v) for v in m]
    w32 = [f32(v) for v in p] + [f32(1.0)]
    acc = []
    for r in range(4):
        # WGSL mat4x4 * vec4: for each row r, m[r][0]*v.x + m[r][1]*v.y + ...
        # column-major: m[r][c] = arr[c*4+r]
        acc.append(f32(f32(m32[0*4+r] * w32[0]) + f32(f32(m32[1*4+r] * w32[1]) + f32(f32(m32[2*4+r] * w32[2]) + f32(m32[3*4+r] * w32[3])))))
    ww = acc[3]
    return [f32(acc[k] / ww) for k in range(3)]

total_mismatch_vertices = 0
for name, yaw, pitch in views:
    cam = camera(bmin, bmax, yaw, pitch)
    m = vp(cam, 400.0 / 300.0)
    bad = 0
    for p in corners:
        o = old_ndc_f32(m, p)
        n = new_shader_ndc_f32(m, p)
        if o != n:
            bad += 1
    if bad:
        total_mismatch_vertices += bad
    print(f"{name}: vertices whose f32 NDC differs old-vs-new: {bad}/8")
print("TOTAL mismatch vertices:", total_mismatch_vertices)
