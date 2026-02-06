Keeping some build artifacts around for reference! ultimate_shader.kr compiled cleanly into ultimate_shader.hlsl and ultimate_shader.usf

This successful compilation acts as a stress test for the backend, proving it can handle complex control flow, heavy mathematical operations, and multi-stage processing without breaking. It confirms that the HLSL and USF generation logic is robust enough to translate high-level Kore syntax into valid, optimized shader code for real-world scenarios.

To serve as a comprehensive testing reference, ultimate_shader.kr includes:

Parallax occlusion mapping

Full PBR with Cook-Torrance BRDF

GGX distribution

Schlick-GGX geometry

Fresnel-Schlick

Normal mapping with TBN matrix

Detail textures

ACES tone mapping

Gamma correction

Chromatic aberration

Vignette

Film grain

Color grading (contrast + saturation)

Animated distortion

8 texture samplers

14 uniform parameters