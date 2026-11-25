# SPEC-PM-026: Linear Dither Amplified by Display Transfer

**Status:** Recorded failure and prevention under SPEC-PM-000
**Owner:** Renderer / Gradient shader
**Recorded:** 2026-11-12
**Governing contract:** SPEC-HAL-085A section 6.8 and its premultiplied-alpha rule

## Failure class

The rectangular Gradient shader added a nominal half 8-bit step directly to
premultiplied **linear** RGB. That amplitude was described as anti-banding noise,
but an encoded display step is not a linear-light step. The display transfer
amplified it near black. A weak coordinate hash also made the grain spatially
repetitive. The same shared shader affected empty dark panes in a native
application, making the pattern visible across independently rendered regions.

The original shader was SHA-256
`54e7af285ab92db10ca18734e6944a2d32b26c3786dff29b0ed2fda1bf1b65da`.
Real Apple M4 Max Metal controls at 1x and 2x compared that exact source with
the same shader with only its dither block omitted. The original opaque dark
linear/radial outputs differed by up to seven encoded red steps. Unchanged
native AppKit application captures and a separately built no-dither control confirmed
that removing this block removed the repeating blank-pane pattern. Lossless
SCK pixel-buffer PNGs, actual color-space metadata and fixed physical crops
qualified this comparison; lossy video alone was insufficient.

This is a color-domain error in one shared renderer operation. It is separate
from the application's valid-transition sibling overlap, typography layout lag,
equal-Z composition, image identity and physical scanout.

## Prevention

1. State the domain of a noise/error budget. A fraction of an encoded 8-bit step
   must not be inserted as the same numerical fraction in linear light. Qualify
   the perturbation through the actual output transfer and target format.
2. Preserve linear-premultiplied stop interpolation and blending. Scale an
   admitted RGB perturbation by alpha, keep channels within the premultiplied
   bounds, and preserve the existing independent coverage operations and alpha.
3. A deterministic coordinate hash is not evidence of spatial decorrelation.
   Check the actual GPU output and blank native regions for residual amplitude
   and spectral concentration at each qualified physical scale.
4. Include zero-alpha, translucent composition, known-stop and rounded-coverage
   controls. Distinguish one draw's error from repeated framebuffer quantization.
5. Verify quiet surfaces **and** banding in the original native scenario.
   A no-dither diagnostic or a passing offscreen test alone does not establish
   the user-visible repair. Preserve raw capture scale and color-space identity.

## Forbidden recurrence

- Treating `0.5 / 255` as a universal linear-light dither amplitude.
- Compensating in application colors, gradients or decorative textures.
- Claiming white noise from a hash formula without output evidence.
- Changing alpha, clip coverage or stop interpolation to hide the grain.
- Presenting lossy/resampled images or headless tests as physical scanout proof.

This record adds no pipeline, material, output-stamp or Image API. It does not
adopt Design-status SPEC-R-012 or any inactive proposal. JC's visual acceptance
and physical scanout remain separate from renderer and compositor evidence.
