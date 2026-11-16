/// The send shockwave's Metal source: ONE stitchable layer effect drawing the post's six layers over the pane's
/// snapshot. This string is the source of truth; `SendWaveShaderLibrary.swift` holds the metallib compiled from it
/// (`python3 checks/parity/par-sendwave.py --compile-shader`), and par-sendwave goes red when the two drift.
/// SwiftPM's command line compiles no .metal file and Package.swift is frozen, so the library ships as bytes.
enum SendWaveShaderSource {
    static let function = "sendWave"
    static let text = #"""
#include <metal_stdlib>
#include <SwiftUI/SwiftUI_Metal.h>
using namespace metal;

// Uniforms (points, the layer's own space):
//   size, origin      the pane and the send button's centre
//   t                 0..1 over SENDWAVE_MS
//   reach             distance from the origin to the farthest pane corner
//   dark              1 in dark mode (screen blend, subtler highlights), 0 in light (tint)
//   c0..c3            the scheme's rings, leading edge first
//   chan              red / blue offset as a share of green's (SENDWAVE_CHANNEL_OFFSET)
//   lens              lens displacement at its peak, points
//   band              one ring's width as a share of the reach
//   strength          ring opacity at its peak (light tint or dark screen)
//   edge              edge-highlight gain
//   hole              the send button's frame (x, y, w, h): left out, so the button draws live
//   fadeFrom          the share of the run after which the halo fades (send wave 0.8, landing 0.6)
static half luma(half4 c) { return dot(c.rgb, half3(0.299h, 0.587h, 0.114h)); }

// A sample pushed past the pane's edge is mirrored back inside: clamping would smear the edge row (a hairline)
// into a band.
static float2 inside(float2 p, float2 size) {
    float2 m = abs(p);
    m = size - abs(size - m);
    return clamp(m, float2(0.0), size);
}

[[ stitchable ]] half4 sendWave(float2 pos, SwiftUI::Layer layer, float2 size, float2 origin, float t,
                                float reach, float dark, half4 c0, half4 c1, half4 c2, half4 c3,
                                float chan, float lens, float band, float strength, float edge, float4 hole, float fadeFrom) {
    float2 d = pos - origin;
    float dist = length(d);
    float2 dir = dist > 0.001 ? d / dist : float2(0.0);

    // Pace (lead ruling, 2026-11-08): the front's speed is scaled to the reach so it arrives at the farthest corner
    // at the END of the run; it leaves fast and settles (ease-out quadratic, gentler than cubic so the whole window
    // is crossed over the whole run, not its first half); the halo fades over the last 20 %.
    float spread = 1.0 - (1.0 - t) * (1.0 - t);
    float front = spread * reach;
    float fade = 1.0 - smoothstep(fadeFrom, 1.0, t);
    float w = max(band * reach, 1.0);

    // (2) lens: just outside the front samples toward the origin (stretch), just inside away from it (compress).
    float x = (dist - front) / w;
    float lensProfile = 2.33 * x * exp(-x * x);
    float2 offset = -dir * lens * lensProfile * fade;

    // (5) edge refraction: as the front reaches a pane edge, content there stretches toward it. Samples stay
    // inside the pane, so the window frame never moves.
    float2 toEdge = float2(pos.x < size.x * 0.5 ? -1.0 : 1.0, pos.y < size.y * 0.5 ? -1.0 : 1.0);
    float2 edgeDist = float2(min(pos.x, size.x - pos.x), min(pos.y, size.y - pos.y));
    float2 nearEdge = exp(-edgeDist / max(w * 0.6, 1.0));
    float arrive = exp(-x * x * 0.35);
    offset -= toEdge * nearEdge * lens * 0.6 * arrive * fade;

    // (3) channel separation: red and blue ride +chan / -chan of green's offset; small, so text stays sharp.
    float2 pg = inside(pos + offset, size);
    float2 pr = inside(pos + offset * (1.0 + chan), size);
    float2 pb = inside(pos + offset * (1.0 - chan), size);
    half4 g = layer.sample(pg);                       // (1) the snapshot is the base
    half4 col = half4(layer.sample(pr).r, g.g, layer.sample(pb).b, g.a);

    // (4) four rings behind the front, leading edge first, fading as they expand.
    half4 rings[4] = { c0, c1, c2, c3 };
    half3 ringCol = half3(0.0h);
    float ringA = 0.0;
    for (int i = 0; i < 4; i++) {
        float ri = front - float(i) * w * 0.9;
        float xi = (dist - ri) / (w * 0.75);
        float a = exp(-xi * xi) * (1.0 - float(i) * 0.08) * (ri > 0.0 ? 1.0 : 0.0);
        ringCol += rings[i].rgb * half(a);
        ringA += a;
    }
    ringCol = ringA > 0.001 ? ringCol / half(ringA) : ringCol;
    float a = clamp(ringA, 0.0, 1.0) * strength * fade * (1.0 - 0.35 * spread);

    // (6) edge highlights: a divider or border is a gradient that continues 4 pt along itself (text strokes
    // rarely do); it brightens while a ring crosses it.
    float2 sx = float2(1.0, 0.0), sy = float2(0.0, 1.0), run = float2(4.0, 0.0), rise = float2(0.0, 4.0);
    half gyL = abs(luma(layer.sample(pos - run + sy)) - luma(layer.sample(pos - run - sy)));
    half gyC = abs(luma(layer.sample(pos + sy)) - luma(layer.sample(pos - sy)));
    half gyR = abs(luma(layer.sample(pos + run + sy)) - luma(layer.sample(pos + run - sy)));
    half gxT = abs(luma(layer.sample(pos - rise + sx)) - luma(layer.sample(pos - rise - sx)));
    half gxC = abs(luma(layer.sample(pos + sx)) - luma(layer.sample(pos - sx)));
    half gxB = abs(luma(layer.sample(pos + rise + sx)) - luma(layer.sample(pos + rise - sx)));
    float line = float(max(min(gyL, min(gyC, gyR)), min(gxT, min(gxC, gxB))));
    float glow = clamp(line * 6.0, 0.0, 1.0) * clamp(ringA, 0.0, 1.0) * edge * fade;

    half3 rgb = col.rgb;
    if (dark > 0.5) {
        rgb = 1.0h - (1.0h - rgb) * (1.0h - ringCol * half(a));          // screen
        rgb += ringCol * half(glow * 0.55);                                // subtler in dark
    } else {
        rgb = rgb * mix(half3(1.0h), ringCol, half(a));                    // tint: white takes the ring's hue, text stays dark
        rgb = mix(rgb, min(ringCol * 1.25h, half3(1.0h)), half(glow));
    }
    // Where nothing bends, tints or glows, the pixel is left transparent so the LIVE pane shows through: the
    // snapshot only stands in under the halo, so a control that changes during the run (the undo bar rising, a
    // button's fade) is never frozen elsewhere, and the hand-back at the end is seamless.
    float influence = max(clamp(ringA, 0.0, 1.0), max(abs(lensProfile), arrive * max(nearEdge.x, nearEdge.y)));
    influence = clamp(influence * 3.0, 0.0, 1.0) * fade;
    // The send button is never drawn from the picture: a capsule the size of its frame, feathered 1.5 pt.
    if (hole.z > 0.0 && hole.w > 0.0) {
        float r = 0.5 * min(hole.z, hole.w);
        float2 c = hole.xy + 0.5 * hole.zw;
        float2 q = abs(pos - c) - (0.5 * hole.zw - r);
        float sd = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - r;
        influence *= smoothstep(-0.5, 1.0, sd);
    }
    return half4(clamp(rgb, 0.0h, 1.0h) * col.a, col.a) * half(influence);
}
"""#
}
