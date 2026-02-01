# KORE Shader Examples

This folder contains production-quality GPU shader examples written in KORE, demonstrating the language's expressiveness for graphics programming.

## Shaders

### `pbr_material.kr` - Physically-Based Rendering
Complete PBR material system with:
- **Cook-Torrance BRDF** - Industry-standard microfacet model
- **GGX/Trowbridge-Reitz** - Normal distribution function
- **Schlick-GGX Geometry** - Masking/shadowing term
- **Fresnel-Schlick** - Reflectance at grazing angles
- **Image-Based Lighting** - Environment maps with prefiltered specular
- **ACES Tonemapping** - Cinematic color response

### `volumetric_clouds.kr` - Raymarched Volumetrics
Real-time cloud rendering with:
- **3D Fractal Noise** - FBM-based density fields
- **Henyey-Greenstein Phase** - Realistic light scattering
- **Beer-Lambert Law** - Exponential light absorption
- **Powder Effect** - Multi-scattering approximation
- **Light Marching** - Sun shadow sampling

### `sdf_raymarcher.kr` - Signed Distance Fields
Complete SDF renderer with:
- **Primitive SDFs** - Sphere, box, torus, cylinder, capsule, plane
- **CSG Operations** - Union, subtract, intersect (hard and smooth)
- **Domain Operations** - Repeat, twist, bend
- **Soft Shadows** - Penumbra estimation
- **Ambient Occlusion** - Short-range raymarched AO
- **Material System** - Per-object material IDs

### `post_processing.kr` - Post-FX Stack
Full post-processing pipeline:
- **Bloom** - HDR bright-pass with Gaussian blur
- **Chromatic Aberration** - Lens distortion simulation
- **Film Grain** - Animated noise with luminance response
- **Vignette** - Configurable edge falloff
- **Color Grading** - Lift/Gamma/Gain, temperature, saturation
- **ACES Tonemapping** - Filmic tone curve
- **Temporal Anti-Aliasing** - Motion-aware history blending

## Compilation

These shaders target the legacy KORE compiler's SPIR-V backend:

```bash
# Compile to SPIR-V
cd legacy/compiler
cargo build --release
./target/release/kore ../shaders/pbr_material.kr --target spirv -o ../shaders/pbr_material.spv

# Convert to HLSL (for DirectX/UE5)
naga ../shaders/pbr_material.spv ../shaders/pbr_material.hlsl

# Convert to WGSL (for WebGPU)
naga ../shaders/pbr_material.spv ../shaders/pbr_material.wgsl
```

## Syntax Overview

KORE shader syntax is designed to be familiar to GLSL/HLSL users while being more concise:

```kore
// Shader declaration with typed inputs/outputs
shader vertex MyVertex(position: Vec4, uv: Vec2) -> Vec4:
    uniform mvp: Mat4 @0    // Uniform with binding point
    return mvp * position

shader fragment MyFragment(uv: Vec2) -> Vec4:
    uniform texture: Sampler2D @0
    let color = sample(texture, uv)
    return color
```

## Why KORE for Shaders?

1. **Type Safety** - Full type checking before IR generation
2. **Familiar Syntax** - Python-like indentation, Rust-like types
3. **Multi-Target** - Single source compiles to SPIR-V, WGSL, HLSL
4. **Effect Tracking** - GPU operations are tracked in the type system
5. **Composability** - Functions work like any other KORE code

---

*These examples are for reference and demonstration. For production use, consider the specific requirements of your rendering pipeline.*
