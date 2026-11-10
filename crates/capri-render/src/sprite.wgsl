// Capri : un quad texturé, son « boil » et sa copie violette mal repérée.
// Couleurs prémultipliées de bout en bout (macOS et DirectComposition composent ainsi).

struct Uniforms {
    viewport: vec2f,   // taille de la surface, px physiques
    rect_min: vec2f,   // coin haut gauche du sprite, px
    rect_size: vec2f,  // taille du sprite, px
    ghost: vec2f,      // décalage de la copie violette, px
    seed: f32,         // dessin de boil courant
    boil: f32,         // amplitude du boil, px
    opacity: f32,
    _pad: f32,
}

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

struct VsOut {
    @builtin(position) pos: vec4f,
    @location(0) uv: vec2f,
}

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    // deux triangles ; le quad déborde pour que le boil et la copie violette ne soient pas rognés
    var corners = array<vec2f, 6>(
        vec2f(0.0, 0.0), vec2f(1.0, 0.0), vec2f(0.0, 1.0),
        vec2f(0.0, 1.0), vec2f(1.0, 0.0), vec2f(1.0, 1.0),
    );
    let margin = abs(u.ghost) + vec2f(u.boil + 2.0);
    let c = corners[i];
    let px = u.rect_min - margin + c * (u.rect_size + 2.0 * margin);
    let ndc = vec2f(px.x / u.viewport.x * 2.0 - 1.0, 1.0 - px.y / u.viewport.y * 2.0);
    var out: VsOut;
    out.pos = vec4f(ndc, 0.0, 1.0);
    out.uv = (px - u.rect_min) / u.rect_size;
    return out;
}

fn hash(p: vec2f) -> f32 {
    return fract(sin(dot(p, vec2f(127.1, 311.7))) * 43758.5453);
}

fn value_noise(p: vec2f) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let s = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash(i), hash(i + vec2f(1.0, 0.0)), s.x),
        mix(hash(i + vec2f(0.0, 1.0)), hash(i + vec2f(1.0, 1.0)), s.x),
        s.y,
    );
}

// Échantillon prémultiplié, transparent hors du sprite.
fn sample_at(uv: vec2f) -> vec4f {
    let inside = all(uv >= vec2f(0.0)) && all(uv <= vec2f(1.0));
    let c = textureSampleLevel(tex, samp, clamp(uv, vec2f(0.0), vec2f(1.0)), 0.0);
    return select(vec4f(0.0), c, inside);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4f {
    // Boil : un léger déplacement des pixels, retiré à chaque nouveau dessin.
    let p = in.uv * vec2f(14.0, 24.0) + vec2f(u.seed * 7.31, u.seed * 3.17);
    let wobble = vec2f(value_noise(p), value_noise(p + vec2f(19.7, 4.3))) - 0.5;
    let uv = in.uv + wobble * 2.0 * u.boil / u.rect_size;

    let body = sample_at(uv);
    let ghost_a = sample_at(uv - u.ghost / u.rect_size).a;
    let violet = vec4f(0.45, 0.16, 0.88, 1.0) * ghost_a * 0.55;

    // elle, par-dessus sa copie violette
    return (body + violet * (1.0 - body.a)) * u.opacity;
}
