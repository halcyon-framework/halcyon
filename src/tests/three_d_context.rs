use crate::renderer::wgpu_renderer::WgpuRenderer;
use crate::three_d::{
    Appearance3DError, AppearanceSourceDescriptor, CANONICAL_VERTEX_STRIDE, Context3D,
    Context3DError, DEFAULT_LINEAR_VERTEX_COLOR, DeviceLimits3D, Geometry3DError, GeometryIndices,
    GeometrySemantics, GeometrySourceDescriptor, HardLimit3D, LimitProfile3D,
    LinearVertexColorRgba8, LocalPoint3, LocalVector3, PolicyBudget3D, PolicyLimits3D,
    ResourceKind3D, Sampler3D, SamplerAddressMode, SamplerFilterMode, Texture3D, Texture3DError,
    TextureCoordinate2, TextureFormat3D,
};
use std::sync::Arc;

fn local_point(x: f32, y: f32, z: f32) -> LocalPoint3 {
    LocalPoint3::try_new(x, y, z).unwrap()
}

/// Complete CPU producer for this private proof only. No runtime owner calls it.
pub(super) mod r028_fixture {
    use crate::three_d::mip_sampling::*;
    use std::sync::Arc;
    pub fn decode(b: u8) -> f64 {
        let x = f64::from(b) / 255.0;
        if x <= 0.04045 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    }
    fn encode(x: f64) -> f64 {
        if x <= 0.0031308 {
            12.92 * x
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        }
    }
    fn byte(x: f64) -> u8 {
        assert!(x.is_finite() && (0.0..=255.0).contains(&x));
        let n = x.floor();
        let f = x - n;
        (if f < 0.5 {
            n
        } else if f > 0.5 {
            n + 1.0
        } else if (n as u64) & 1 == 0 {
            n
        } else {
            n + 1.0
        }) as u8
    }
    fn ratio(n: u128, d: u128) -> u8 {
        let q = n / d;
        let r = n % d;
        (if 2 * r < d {
            q
        } else if 2 * r > d {
            q + 1
        } else if q & 1 == 0 {
            q
        } else {
            q + 1
        }) as u8
    }
    pub fn produce(
        role: MipRole3D,
        w: u32,
        h: u32,
        original: Vec<u8>,
        image: u64,
        revision: u64,
        full: bool,
    ) -> MipChainSource3D {
        let source = produce_bytes(role, w, h, original, image, revision, full);
        source.validate().unwrap();
        source
    }
    pub fn normal_candidate(
        w: u32,
        h: u32,
        original: Vec<u8>,
        image: u64,
        revision: u64,
        full: bool,
    ) -> MipChainSource3D {
        produce_bytes(MipRole3D::Normal, w, h, original, image, revision, full)
    }
    fn produce_bytes(
        role: MipRole3D,
        w: u32,
        h: u32,
        original: Vec<u8>,
        image: u64,
        revision: u64,
        full: bool,
    ) -> MipChainSource3D {
        assert_eq!(original.len(), w as usize * h as usize * 4);
        assert!(w > 0 && h > 0);
        let original: Arc<[u8]> = original.into();
        let mut raw = original.to_vec();
        let mut levels = vec![MipLevel3D {
            width: w,
            height: h,
            offset: 0,
            bytes: raw.len() as u64,
        }];
        let (mut pw, mut ph) = (w, h);
        let mut parent = original.to_vec();
        let mut work = u64::from(w) * u64::from(h);
        while full && (pw > 1 || ph > 1) {
            let cw = (pw / 2).max(1);
            let ch = (ph / 2).max(1);
            let mut child = vec![0u8; cw as usize * ch as usize * 4];
            for y in 0..ch {
                for x in 0..cw {
                    let mut numeric = [0u128; 4];
                    let mut linear = [0.0; 3];
                    let mut alpha_sum = 0u128;
                    let mut moments = [0i128; 3];
                    // Integer intersection lengths represent the EXACT normalized
                    // area weights dx*dy/(pw*ph), including both odd edge rows.
                    let ix0 = u64::from(x) * u64::from(pw) / u64::from(cw);
                    let ix1 =
                        (u64::from(x + 1) * u64::from(pw) + u64::from(cw) - 1) / u64::from(cw);
                    let iy0 = u64::from(y) * u64::from(ph) / u64::from(ch);
                    let iy1 =
                        (u64::from(y + 1) * u64::from(ph) + u64::from(ch) - 1) / u64::from(ch);
                    for j in iy0..iy1 {
                        for i in ix0..ix1 {
                            let dx = ((i + 1) * u64::from(cw))
                                .min(u64::from(x + 1) * u64::from(pw))
                                - (i * u64::from(cw)).max(u64::from(x) * u64::from(pw));
                            let dy = ((j + 1) * u64::from(ch))
                                .min(u64::from(y + 1) * u64::from(ph))
                                - (j * u64::from(ch)).max(u64::from(y) * u64::from(ph));
                            let weight = u128::from(dx) * u128::from(dy);
                            let p = &parent[(j as usize * pw as usize + i as usize) * 4..][..4];
                            for c in 0..4 {
                                numeric[c] += weight * u128::from(p[c]);
                            }
                            for c in 0..3 {
                                let mask = role == MipRole3D::MaskBase;
                                linear[c] += decode(p[c])
                                    * weight as f64
                                    * if mask { f64::from(p[3]) } else { 1.0 };
                                // Signed first moments, without per-parent normalization.
                                moments[c] += (2 * i128::from(p[c]) - 255) * weight as i128;
                            }
                            alpha_sum += weight * u128::from(p[3]);
                            work += 1;
                        }
                    }
                    let den = u128::from(pw) * u128::from(ph);
                    let out = &mut child[(y as usize * cw as usize + x as usize) * 4..][..4];
                    for c in 0..4 {
                        out[c] = ratio(numeric[c], den);
                    }
                    if role.format() == crate::three_d::TextureFormat3D::Rgba8UnormSrgb {
                        for c in 0..3 {
                            let v = if role == MipRole3D::MaskBase {
                                if alpha_sum == 0 {
                                    0.0
                                } else {
                                    linear[c] / alpha_sum as f64
                                }
                            } else {
                                linear[c] / den as f64
                            };
                            out[c] = byte(255.0 * encode(v));
                        }
                    } else if role == MipRole3D::Normal {
                        for c in 0..3 {
                            out[c] = ratio((moments[c] + 255 * den as i128) as u128, 2 * den);
                        }
                    }
                }
            }
            levels.push(MipLevel3D {
                width: cw,
                height: ch,
                offset: raw.len() as u64,
                bytes: child.len() as u64,
            });
            raw.extend(&child);
            parent = child;
            pw = cw;
            ph = ch;
        }
        let raw: Arc<[u8]> = raw.into();
        let provenance = Arc::new(ChainProvenance3D {
            image_identity: image,
            content_revision: revision,
            producer_identity: 28001,
            producer_revision: 1,
            generator_identity: 28002,
            generator_revision: 1,
            generator: Some(ChainGenerator3D::RecursiveStoredAreaR028V1),
            origin: Some(ImageOrigin3D::TopLeft),
            uv_set: 0,
            work,
            error_bound: 0.5,
            original,
            derived: Arc::downgrade(&raw),
        });
        let source = MipChainSource3D {
            role,
            format: role.format(),
            levels: levels.into(),
            texels: raw,
            provenance,
        };
        source
    }
    pub fn checker(role: MipRole3D, n: u32, image: u64) -> MipChainSource3D {
        let mut b = Vec::with_capacity(n as usize * n as usize * 4);
        for y in 0..n {
            for x in 0..n {
                let c = if (x + y) % 2 == 0 { 0 } else { 255 };
                b.extend([c, c, if role == MipRole3D::Normal { 128 } else { c }, 255]);
            }
        }
        produce(role, n, n, b, image, 1, true)
    }
    pub fn multiscale() -> MipChainSource3D {
        let mut bytes = Vec::with_capacity(1024 * 1024 * 4);
        let (mut lo, mut hi) = (255, 0);
        for y in 0..1024 {
            for x in 0..1024 {
                let bit = |t: u32, k: u32| 2 * ((t >> k) & 1) as i32 - 1;
                let c = 128
                    + 24 * bit(x, 0)
                    + 20 * bit(x, 2)
                    + 16 * bit(x, 4)
                    + 12 * bit(x, 6)
                    + 8 * bit(x, 8)
                    + 16 * bit(y, 1)
                    + 12 * bit(y, 3)
                    + 8 * bit(y, 5)
                    + 4 * bit(y, 7);
                lo = lo.min(c);
                hi = hi.max(c);
                bytes.extend([c as u8, c as u8, c as u8, 255]);
            }
        }
        assert_eq!((lo, hi), (8, 248));
        produce(MipRole3D::OpaqueBase, 1024, 1024, bytes, 28011, 1, true)
    }
    pub fn sampler(
        source: &MipChainSource3D,
        id: u64,
        min: MipMin3D,
        mag: crate::three_d::SamplerFilterMode,
        u: crate::three_d::SamplerAddressMode,
        v: crate::three_d::SamplerAddressMode,
    ) -> MipSamplerSource3D {
        MipSamplerSource3D {
            identity: id,
            revision: 1,
            image_identity: source.provenance.image_identity,
            image_revision: source.provenance.content_revision,
            u,
            v,
            mag,
            min,
            extent: [source.levels[0].width, source.levels[0].height],
        }
    }
}

pub(super) mod r028_normal_fixture {
    use crate::three_d::mip_sampling::*;
    use crate::three_d::{SamplerAddressMode as Address, SamplerFilterMode};
    use std::sync::Arc;

    pub fn constant(image: u64, w: u32, h: u32) -> MipChainSource3D {
        super::r028_fixture::normal_candidate(
            w,
            h,
            [255, 128, 127, 255].repeat((w * h) as usize),
            image,
            1,
            true,
        )
    }
    pub fn correlated(image: u64) -> MipChainSource3D {
        // A and -A occupy diagonal corners. C has Y+Z=2. Thus a real
        // bilinear response can vanish in Y+Z only at A/-A, where X=+/-1.
        // The stored1x1 first moment has positive Y+Z as well. This is a
        // complete source argument, not a conclusion from rendered averages.
        super::r028_fixture::normal_candidate(
            2,
            2,
            vec![
                255, 128, 127, 255, 128, 255, 255, 255, 128, 255, 255, 255, 0, 127, 128, 255,
            ],
            image,
            1,
            true,
        )
    }
    fn address(i: i64, w: u32, a: Address) -> i64 {
        let w = i64::from(w);
        match a {
            Address::ClampToEdge => i.clamp(0, w - 1),
            Address::Repeat => i.rem_euclid(w),
            Address::MirrorRepeat => {
                let q = i.rem_euclid(2 * w);
                if q < w { q } else { 2 * w - 1 - q }
            }
        }
    }
    fn axes(a: u32, b: u32, linear: bool, mode: Address, same: bool) -> u64 {
        let mut points = vec![(0u64, 1u64), (1, 1)];
        for w in [a, b] {
            if linear {
                for k in 0..w {
                    points.push((2 * u64::from(k) + 1, 2 * u64::from(w)));
                }
            } else {
                for k in 1..w {
                    points.push((u64::from(k), u64::from(w)));
                }
            }
        }
        points.sort_by(|x, y| {
            (u128::from(x.0) * u128::from(y.1)).cmp(&(u128::from(y.0) * u128::from(x.1)))
        });
        points.dedup_by(|x, y| {
            u128::from(x.0) * u128::from(y.1) == u128::from(y.0) * u128::from(x.1)
        });
        let intervals = (points.len() - 1) as u64;
        if linear {
            return intervals;
        }
        let mut total = intervals;
        for (n, d) in points {
            let choices = |w| {
                let k = n * u64::from(w);
                let i = (k / d) as i64;
                if k % d == 0 && address(i, w, mode) != address(i - 1, w, mode) {
                    2u64
                } else {
                    1
                }
            };
            total += if same {
                choices(a)
            } else {
                choices(a) * choices(b)
            };
        }
        total
    }
    pub fn cells(source: &MipChainSource3D, s: MipSamplerSource3D) -> u64 {
        let linear = matches!(
            s.min,
            MipMin3D::Linear | MipMin3D::LinearMipNearest | MipMin3D::LinearMipLinear
        );
        let mut pairs = vec![(0usize, 0usize, s.mag == SamplerFilterMode::Linear)];
        if !s.min.uses_mips() {
            pairs.push((0, 0, linear));
        } else if matches!(
            s.min,
            MipMin3D::NearestMipLinear | MipMin3D::LinearMipLinear
        ) && source.levels.len() > 1
        {
            for l in 0..source.levels.len() - 1 {
                pairs.push((l, l + 1, linear));
            }
        } else {
            for l in 0..source.levels.len() {
                pairs.push((l, l, linear));
            }
        }
        pairs
            .into_iter()
            .map(|(l, h, linear)| {
                let a = source.levels[l];
                let b = source.levels[h];
                axes(a.width, b.width, linear, s.u, l == h)
                    * axes(a.height, b.height, linear, s.v, l == h)
            })
            .sum()
    }
    pub fn certificate(
        source: &MipChainSource3D,
        sampler: &MipSamplerHandle3D,
        policy: MipSamplerSource3D,
        scale: f32,
        default: [f64; 3],
    ) -> NormalDomainCertificate3D {
        let mut c = NormalDomainCertificate3D {
            identity: 88001,
            revision: 1,
            source: Arc::new(source.clone()),
            sampler: sampler.clone(),
            policy,
            normal_scale_bits: scale.to_bits(),
            cells: cells(source, policy),
            maximum_work: 1_000_000,
            maximum_metadata_bytes: 65_536,
            default: NormalWitness3D {
                direction: default,
                lower: 0.003,
            },
            exceptions: Arc::from([]),
            digest: [0; 32],
        };
        c.digest = c.computed_digest();
        c
    }
    pub fn correlated_certificate(
        source: &MipChainSource3D,
        sampler: &MipSamplerHandle3D,
        policy: MipSamplerSource3D,
    ) -> NormalDomainCertificate3D {
        assert_eq!(policy.min, MipMin3D::LinearMipLinear);
        assert_eq!(policy.mag, SamplerFilterMode::Linear);
        assert_eq!(policy.u, Address::ClampToEdge);
        assert_eq!(policy.v, Address::ClampToEdge);
        assert_eq!(&source.level_bytes(1).unwrap()[..4], &[128, 191, 191, 255]);
        let mut c = certificate(source, sampler, policy, 1.0, [0.0, 1.0, 1.0]);
        let mut exceptions = Vec::new();
        for (mag, grid) in [
            (false, vec![0.0f64, 0.25, 0.5, 0.75, 1.0]),
            (true, vec![0.0f64, 0.25, 0.75, 1.0]),
        ] {
            for u in 0..grid.len() - 1 {
                for v in 0..grid.len() - 1 {
                    let key = NormalCell3D {
                        mag,
                        linear: true,
                        lower: 0,
                        upper: if mag { 0 } else { 1 },
                        u: u as u32,
                        v: v as u32,
                        u_choice: 0,
                        v_choice: 0,
                    };
                    let depth = if mag && u == 1 && v == 1 { 1 } else { 0 };
                    let mut leaves = Vec::new();
                    for path in 0..if depth == 0 { 1 } else { 8 } {
                        let cu = (grid[u] + grid[u + 1]) * 0.5
                            + if depth == 0 {
                                0.0
                            } else {
                                if path & 1 == 0 { -0.125 } else { 0.125 }
                            };
                        let cv = (grid[v] + grid[v + 1]) * 0.5
                            + if depth == 0 {
                                0.0
                            } else {
                                if path & 2 == 0 { -0.125 } else { 0.125 }
                            };
                        let x = ((cu - 0.25) / 0.5).clamp(0.0, 1.0);
                        let y = ((cv - 0.25) / 0.5).clamp(0.0, 1.0);
                        let weight_a = (1.0 - x) * (1.0 - y);
                        let weight_b = x * y;
                        let sign = if weight_a >= weight_b { 1.0 } else { -1.0 };
                        leaves.push(NormalBox3D {
                            path,
                            depth,
                            witness: NormalWitness3D {
                                direction: [sign, 1.0, 1.0],
                                lower: 0.1,
                            },
                        });
                    }
                    exceptions.push(NormalException3D {
                        cell: key,
                        leaves: leaves.into(),
                    });
                }
            }
        }
        exceptions.sort_by_key(|e| e.cell);
        c.exceptions = exceptions.into();
        assert_eq!(c.cells, 25);
        c.digest = c.computed_digest();
        c
    }
}

#[test]
fn r028_certified_negative_blue_chain_material_boundary() {
    use crate::three_d::mip_sampling::*;
    use crate::three_d::pbr::*;
    use r028_normal_fixture as f;
    use std::sync::Arc;
    fn summary_bits(s: NormalDomainSummary3D) -> [u64; 5] {
        [
            s.cells,
            s.leaves,
            s.work,
            s.minimum_projection.to_bits(),
            s.maximum_interval_width.to_bits(),
        ]
    }
    fn evaluate<const REUSE: bool>(
        c: &NormalDomainCertificate3D,
    ) -> Result<NormalDomainSummary3D, PbrError3D> {
        verify_normal_domain_for_test::<REUSE, REUSE, REUSE, REUSE>(
            &c.source,
            &c.policy,
            f32::from_bits(c.normal_scale_bits),
            c,
        )
    }
    fn without_quad(c: &NormalDomainCertificate3D) -> Result<NormalDomainSummary3D, PbrError3D> {
        // Previous production: identical default-corner and byte-table reuse.
        verify_normal_domain_for_test::<true, true, false, false>(
            &c.source,
            &c.policy,
            f32::from_bits(c.normal_scale_bits),
            c,
        )
    }
    fn without_axis(c: &NormalDomainCertificate3D) -> Result<NormalDomainSummary3D, PbrError3D> {
        // Previous production: default corners, byte tables and local texel quad.
        verify_normal_domain_for_test::<true, true, true, false>(
            &c.source,
            &c.policy,
            f32::from_bits(c.normal_scale_bits),
            c,
        )
    }
    fn compare(
        original: &Result<NormalDomainSummary3D, PbrError3D>,
        reused: &Result<NormalDomainSummary3D, PbrError3D>,
    ) -> Option<NormalDomainSummary3D> {
        match (original, reused) {
            (Ok(a), Ok(b)) => {
                assert_eq!(
                    summary_bits(*a),
                    summary_bits(*b),
                    "exact normal proof summary"
                );
                Some(*b)
            }
            (Err(a), Err(b)) => {
                assert_eq!(a, b, "exact first typed normal proof refusal");
                None
            }
            (a, b) => panic!("normal corner reuse changed admission: {a:?} versus {b:?}"),
        }
    }
    fn exact(c: &NormalDomainCertificate3D) -> Option<NormalDomainSummary3D> {
        let original = evaluate::<false>(c);
        let previous = without_quad(c);
        let quad = without_axis(c);
        let reused = evaluate::<true>(c);
        let _ = compare(&original, &previous);
        let _ = compare(&previous, &quad);
        compare(&quad, &reused)
    }
    // Every encoded byte, including both sides of BLUE127/128, retains exact
    // outward endpoint bits at signed zero, subnormal and admitted scale limits.
    for scale in [
        -1024.0,
        f32::from_bits((-1024.0f32).to_bits() - 1),
        -64.0,
        -1.0,
        -0.5,
        -f32::MIN_POSITIVE,
        -f32::from_bits(1),
        -0.0,
        0.0,
        f32::from_bits(1),
        f32::MIN_POSITIVE,
        0.5,
        1.0,
        64.0,
        f32::from_bits(1024.0f32.to_bits() - 1),
        1024.0,
    ] {
        check_normal_byte_intervals_for_test(scale);
    }
    let context =
        Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
    let source = f::constant(88010, 2, 2);
    assert!(source.validate().is_err());
    assert!(context.register_mip_chain(source.clone(), None).is_err());
    let mut ordinal = 0;
    for min in [
        MipMin3D::Nearest,
        MipMin3D::Linear,
        MipMin3D::NearestMipNearest,
        MipMin3D::LinearMipNearest,
        MipMin3D::NearestMipLinear,
        MipMin3D::LinearMipLinear,
    ] {
        for mag in [SamplerFilterMode::Nearest, SamplerFilterMode::Linear] {
            for u in [
                SamplerAddressMode::ClampToEdge,
                SamplerAddressMode::Repeat,
                SamplerAddressMode::MirrorRepeat,
            ] {
                for v in [
                    SamplerAddressMode::ClampToEdge,
                    SamplerAddressMode::Repeat,
                    SamplerAddressMode::MirrorRepeat,
                ] {
                    ordinal += 1;
                    let policy = r028_fixture::sampler(&source, 88020 + ordinal, min, mag, u, v);
                    let sampler = context.register_mip_sampler(policy, None).unwrap();
                    let cert = f::certificate(&source, &sampler, policy, 1.0, [1.0, 0.0, 0.0]);
                    // All 108 address/filter combinations include exact 0/1,
                    // seam/tie choices and same/adjacent-level default cells.
                    let expected = exact(&cert).unwrap();
                    if ordinal == 1 {
                        // Every one of the last eight projection visits still
                        // consumes its original work unit after UV reuse.
                        for missing in 0..=8 {
                            let mut bounded = cert.clone();
                            bounded.maximum_work = expected.work - missing;
                            bounded.digest = bounded.computed_digest();
                            assert_eq!(exact(&bounded).is_some(), missing == 0);
                        }
                        let before_cells = expected.work - 9 * expected.cells;
                        for allowance in [1, 2] {
                            let mut invalid = cert.clone();
                            invalid.default.direction = [-1.0, 0.0, 0.0];
                            invalid.maximum_work = before_cells + allowance;
                            invalid.digest = invalid.computed_digest();
                            let reused = evaluate::<true>(&invalid);
                            if allowance == 1 {
                                assert!(matches!(
                                    &reused,
                                    Err(PbrError3D::Sampling {
                                        field: "normal verification work",
                                        cause: "declared finite bound exceeded",
                                    })
                                ));
                            } else {
                                assert!(matches!(
                                    &reused,
                                    Err(PbrError3D::NormalCertificate { .. })
                                ));
                            }
                            assert!(compare(&evaluate::<false>(&invalid), &reused).is_none());
                            assert!(compare(&without_quad(&invalid), &reused).is_none());
                            assert!(compare(&without_axis(&invalid), &reused).is_none());
                        }
                    }
                    let chain = context
                        .register_certified_normal_chain(
                            source.clone(),
                            &sampler,
                            1.0,
                            Arc::new(cert),
                            None,
                        )
                        .unwrap();
                    assert_eq!(
                        summary_bits(expected),
                        summary_bits(
                            context
                                .lease_mip_chain(&chain)
                                .unwrap()
                                .normal_domain_summary()
                                .unwrap()
                        )
                    );
                    let mut material = PbrMaterialSource3D::defaults(90000 + ordinal, 1);
                    material.sampling = SamplingFamily3D::V2;
                    material.roles[1] = Some(PbrRoleBinding3D::V2 {
                        texture: chain.clone(),
                        sampler: sampler.clone(),
                    });
                    context
                        .register_pbr_material(material.clone(), None)
                        .unwrap();
                    material.normal_scale = 0.0;
                    assert!(context.register_pbr_material(material, None).is_err());
                }
            }
        }
    }
    assert_eq!(ordinal, 108);
    let source = f::correlated(88100);
    let policy = r028_fixture::sampler(
        &source,
        88101,
        MipMin3D::LinearMipLinear,
        SamplerFilterMode::Linear,
        SamplerAddressMode::ClampToEdge,
        SamplerAddressMode::ClampToEdge,
    );
    let sampler = context.register_mip_sampler(policy, None).unwrap();
    let cert = f::correlated_certificate(&source, &sampler, policy);
    let expected = exact(&cert).unwrap();
    let valid = context
        .register_certified_normal_chain(
            source.clone(),
            &sampler,
            1.0,
            Arc::new(cert.clone()),
            None,
        )
        .unwrap();
    let summary = context
        .lease_mip_chain(&valid)
        .unwrap()
        .normal_domain_summary()
        .unwrap();
    assert_eq!(summary.cells, 25);
    assert!(summary.minimum_projection >= 0.1);
    assert_eq!(summary_bits(summary), summary_bits(expected));
    for case in 0..8 {
        let mut bad = cert.clone();
        match case {
            0 => bad.cells -= 1,
            1 => {
                let mut e = bad.exceptions.to_vec();
                let middle = e.iter_mut().find(|e| e.leaves.len() == 8).unwrap();
                middle.leaves = middle.leaves[..7].to_vec().into();
                bad.exceptions = e.into();
            }
            2 => {
                let mut e = bad.exceptions.to_vec();
                Arc::make_mut(&mut e[0].leaves)[0].witness.lower = 2.0;
                bad.exceptions = e.into();
            }
            3 => bad.maximum_work = 0,
            4 => bad.maximum_metadata_bytes = 1,
            5 => bad.policy.min = MipMin3D::Linear,
            6 => bad.normal_scale_bits = (-0.0f32).to_bits(),
            _ => bad.revision = 0,
        }
        bad.digest = bad.computed_digest();
        let _ = exact(&bad);
        let before = context.resource_usage().unwrap();
        assert!(
            context
                .register_certified_normal_chain(source.clone(), &sampler, 1.0, Arc::new(bad), None)
                .is_err(),
            "case{case}"
        );
        assert_eq!(context.resource_usage().unwrap(), before);
    }
    let mut forged = cert.clone();
    forged.digest[0] ^= 1;
    assert!(exact(&forged).is_none());
    assert!(
        context
            .register_certified_normal_chain(source.clone(), &sampler, 1.0, Arc::new(forged), None)
            .is_err()
    );
    let mut alias = context.lease_mip_chain(&valid).unwrap();
    alias.source = Arc::new(source.clone());
    assert!(validate_pair(&alias, &context.lease_mip_sampler(&sampler).unwrap()).is_err());
    for kind in 0..4 {
        let mut changed = policy;
        changed.identity += 100 + kind;
        match kind {
            0 => changed.min = MipMin3D::Linear,
            1 => changed.mag = SamplerFilterMode::Nearest,
            2 => changed.u = SamplerAddressMode::Repeat,
            _ => changed.v = SamplerAddressMode::MirrorRepeat,
        }
        let replacement = context.register_mip_sampler(changed, None).unwrap();
        let mut m = PbrMaterialSource3D::defaults(91500 + kind, 1);
        m.sampling = SamplingFamily3D::V2;
        m.roles[1] = Some(PbrRoleBinding3D::V2 {
            texture: valid.clone(),
            sampler: replacement,
        });
        assert!(context.register_pbr_material(m, None).is_err());
    }
    for kind in 0..4 {
        let mut bad = source.clone();
        match kind {
            0 => {
                let mut levels = bad.levels.to_vec();
                levels.pop();
                bad.levels = levels.into();
            }
            1 => bad.role = MipRole3D::MetallicRoughness,
            2 => Arc::make_mut(&mut bad.provenance).generator_identity = 0,
            _ => {
                let mut bytes = bad.texels.to_vec();
                let last = bytes.len() - 2;
                bytes[last] ^= 1;
                bad.texels = bytes.into();
                Arc::make_mut(&mut bad.provenance).derived = Arc::downgrade(&bad.texels);
            }
        }
        let mut c = cert.clone();
        c.source = Arc::new(bad.clone());
        c.digest = c.computed_digest();
        assert!(
            context
                .register_certified_normal_chain(bad, &sampler, 1.0, Arc::new(c), None)
                .is_err()
        );
    }
    let other_sampler = context.register_mip_sampler(policy, None).unwrap();
    let mut m = PbrMaterialSource3D::defaults(91000, 1);
    m.sampling = SamplingFamily3D::V2;
    m.roles[1] = Some(PbrRoleBinding3D::V2 {
        texture: valid.clone(),
        sampler: other_sampler,
    });
    assert!(context.register_pbr_material(m, None).is_err());
    assert!(
        context
            .register_certified_normal_chain(
                source.clone(),
                &sampler,
                1.0,
                Arc::new(cert.clone()),
                Some(&valid)
            )
            .is_err()
    );
    let mut next = cert.clone();
    next.revision += 1;
    next.digest = next.computed_digest();
    let held = context.lease_mip_chain(&valid).unwrap();
    let before = context.resource_usage().unwrap();
    let successor = context
        .register_certified_normal_chain(
            source.clone(),
            &sampler,
            1.0,
            Arc::new(next),
            Some(&valid),
        )
        .unwrap();
    assert!(context.lease_mip_chain(&valid).is_err());
    assert!(
        context
            .resource_usage()
            .unwrap()
            .retained_cpu_descriptor_bytes
            > before.retained_cpu_descriptor_bytes
    );
    drop(held);
    context.retire_mip_chain(&successor).unwrap();
    let foreign =
        Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
    assert!(
        foreign
            .register_certified_normal_chain(
                source.clone(),
                &sampler,
                1.0,
                Arc::new(cert.clone()),
                None
            )
            .is_err()
    );
    let mut changed = source.clone();
    Arc::make_mut(&mut changed.provenance).content_revision += 1;
    assert!(
        context
            .register_certified_normal_chain(changed, &sampler, 1.0, Arc::new(cert), None)
            .is_err()
    );
    // A reachable exact zero, not merely a failed arbitrary raw hull: at
    // scale0,u=.5 the two BLUE127/128 texels have equal linear weights.
    let zero = r028_fixture::normal_candidate(
        2,
        1,
        vec![128, 128, 127, 255, 128, 128, 128, 255],
        88200,
        1,
        true,
    );
    let p = r028_fixture::sampler(
        &zero,
        88201,
        MipMin3D::LinearMipLinear,
        SamplerFilterMode::Linear,
        SamplerAddressMode::Repeat,
        SamplerAddressMode::MirrorRepeat,
    );
    let s = context.register_mip_sampler(p, None).unwrap();
    let z = f::certificate(&zero, &s, p, 0.0, [0.0, 0.0, 1.0]);
    assert!(exact(&z).is_none());
    assert!(
        context
            .register_certified_normal_chain(zero, &s, 0.0, Arc::new(z), None)
            .is_err()
    );
    // Exact signed-zero association remains significant even though either
    // source scale is mathematically0 and this constant negative Z is nonzero.
    let source = f::constant(88300, 3, 5);
    let p = r028_fixture::sampler(
        &source,
        88301,
        MipMin3D::LinearMipLinear,
        SamplerFilterMode::Linear,
        SamplerAddressMode::Repeat,
        SamplerAddressMode::MirrorRepeat,
    );
    let s = context.register_mip_sampler(p, None).unwrap();
    let c = f::certificate(&source, &s, p, 0.0, [0.0, 0.0, -1.0]);
    exact(&c).unwrap();
    let h = context
        .register_certified_normal_chain(source, &s, 0.0, Arc::new(c), None)
        .unwrap();
    let mut m = PbrMaterialSource3D::defaults(88302, 1);
    m.sampling = SamplingFamily3D::V2;
    m.normal_scale = -0.0;
    m.roles[1] = Some(PbrRoleBinding3D::V2 {
        texture: h,
        sampler: s,
    });
    assert!(context.register_pbr_material(m, None).is_err());

    // The original positive-BLUE V2 route needs no certificate, including at
    // signed zero scale. A present V1 negative-BLUE role still cannot hide
    // behind zero scale, and fails before defaults/material publication.
    let positive = r028_fixture::produce(
        MipRole3D::Normal,
        2,
        2,
        [255, 128, 128, 255].repeat(4),
        88310,
        1,
        true,
    );
    let policy = r028_fixture::sampler(
        &positive,
        88311,
        MipMin3D::LinearMipLinear,
        SamplerFilterMode::Linear,
        SamplerAddressMode::Repeat,
        SamplerAddressMode::MirrorRepeat,
    );
    let chain = context.register_mip_chain(positive, None).unwrap();
    let sampler = context.register_mip_sampler(policy, None).unwrap();
    let mut material = PbrMaterialSource3D::defaults(88312, 1);
    material.sampling = SamplingFamily3D::V2;
    material.normal_scale = -0.0;
    material.roles[1] = Some(PbrRoleBinding3D::V2 {
        texture: chain,
        sampler,
    });
    context.register_pbr_material(material, None).unwrap();
    let texture = context
        .register_texture(
            Texture3D::try_new(1, 1, TextureFormat3D::Rgba8Unorm, vec![255, 128, 127, 255])
                .unwrap(),
        )
        .unwrap();
    let sampler = context.register_sampler(Sampler3D::default()).unwrap();
    let mut material = PbrMaterialSource3D::defaults(88313, 1);
    material.normal_scale = 0.0;
    material.roles[1] = Some(PbrRoleBinding3D::V1 {
        texture: texture.handle().clone(),
        sampler: sampler.handle().clone(),
    });
    let before = context.resource_usage().unwrap();
    assert!(context.register_pbr_material(material, None).is_err());
    assert_eq!(context.resource_usage().unwrap(), before);

    // The verified capture owns its concrete sampler lease. Retiring both
    // registry names cannot release their charges while a chain lease survives.
    let retained =
        Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
    let source = f::constant(88400, 2, 2);
    let policy = r028_fixture::sampler(
        &source,
        88401,
        MipMin3D::LinearMipLinear,
        SamplerFilterMode::Linear,
        SamplerAddressMode::ClampToEdge,
        SamplerAddressMode::ClampToEdge,
    );
    let sampler = retained.register_mip_sampler(policy, None).unwrap();
    let certificate = f::certificate(&source, &sampler, policy, 1.0, [1.0, 0.0, 0.0]);
    let chain = retained
        .register_certified_normal_chain(source.clone(), &sampler, 1.0, Arc::new(certificate), None)
        .unwrap();
    let held = retained.lease_mip_chain(&chain).unwrap();
    let charged = retained.resource_usage().unwrap();
    retained.retire_mip_sampler(&sampler).unwrap();
    retained.retire_mip_chain(&chain).unwrap();
    assert!(retained.lease_mip_sampler(&sampler).is_err());
    assert!(retained.lease_mip_chain(&chain).is_err());
    assert_eq!(retained.resource_usage().unwrap(), charged);
    drop(held);
    assert_eq!(retained.resource_usage().unwrap().retained_generations, 0);
    assert_eq!(
        retained
            .resource_usage()
            .unwrap()
            .retained_cpu_descriptor_bytes,
        0
    );

    let mut policy_limits = PolicyLimits3D::conservative();
    policy_limits.max_cache_residency_bytes = charged.retained_cpu_descriptor_bytes - 1;
    let bounded = Context3D::new_cpu_only_for_test(policy_limits, u64::MAX).unwrap();
    let sampler = bounded.register_mip_sampler(policy, None).unwrap();
    let certificate = f::certificate(&source, &sampler, policy, 1.0, [1.0, 0.0, 0.0]);
    let before = bounded.resource_usage().unwrap();
    assert!(
        bounded
            .register_certified_normal_chain(source, &sampler, 1.0, Arc::new(certificate), None)
            .is_err()
    );
    assert_eq!(bounded.resource_usage().unwrap(), before);

    // Bounded same-binary kernel timing: test-owned negative-BLUE bytes, not
    // original external consumer input or a native click-to-display measurement. No timing
    // threshold changes test acceptance; exact summary equality remains required.
    let source = f::constant(88500, 256, 256);
    let policy = r028_fixture::sampler(
        &source,
        88501,
        MipMin3D::LinearMipLinear,
        SamplerFilterMode::Linear,
        SamplerAddressMode::Repeat,
        SamplerAddressMode::MirrorRepeat,
    );
    let sampler = context.register_mip_sampler(policy, None).unwrap();
    let mut certificate = f::certificate(&source, &sampler, policy, 1.0, [1.0, 0.0, 0.0]);
    certificate.maximum_work = 10_000_000;
    certificate.digest = certificate.computed_digest();
    let start = std::time::Instant::now();
    let original = evaluate::<false>(&certificate);
    let original_time = start.elapsed();
    let start = std::time::Instant::now();
    let scalar_corners = verify_normal_domain_for_test::<true, false, false, false>(
        &certificate.source,
        &certificate.policy,
        f32::from_bits(certificate.normal_scale_bits),
        &certificate,
    );
    let scalar_corner_time = start.elapsed();
    let start = std::time::Instant::now();
    let previous = without_quad(&certificate);
    let previous_time = start.elapsed();
    let start = std::time::Instant::now();
    let quad = without_axis(&certificate);
    let quad_time = start.elapsed();
    let start = std::time::Instant::now();
    let reused = evaluate::<true>(&certificate);
    let reused_time = start.elapsed();
    compare(&original, &scalar_corners).unwrap();
    compare(&scalar_corners, &previous).unwrap();
    compare(&previous, &quad).unwrap();
    let summary = compare(&quad, &reused).unwrap();
    std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
        "R028 EXACT LEAF AXIS REUSE test-owned256x256/full9levels/negativeBLUE127/LinearMipLinear+Linear/Repeat+Mirror/scale1: original_reference_ms={:.6} scalar_corner_reuse_ms={:.6} corner_and_byte_reuse_ms={:.6} local_quad_reuse_ms={:.6} leaf_axis_reuse_ms={:.6}; exact_summary_bits={:?}; eight checks/work per default cell unchanged; no native or external consumer speed claim\n",
        original_time.as_secs_f64() * 1000.0, scalar_corner_time.as_secs_f64() * 1000.0, previous_time.as_secs_f64() * 1000.0, quad_time.as_secs_f64() * 1000.0, reused_time.as_secs_f64() * 1000.0,
        summary_bits(summary),
    )).unwrap();
}

#[test]
fn r028_exact_stored_area_roles_npot_moments_and_multiscale_source_bytes() {
    use crate::three_d::mip_sampling::*;
    use r028_fixture as f;
    let mut rows = vec![0; 3 * 5 * 4];
    for x in 0..3 {
        rows[(4 * 3 + x) * 4] = 255;
    }
    let s = f::produce(MipRole3D::NumericMrAo, 3, 5, rows, 1, 1, true);
    assert_eq!(s.texels.len(), 72);
    assert_eq!(
        s.levels
            .iter()
            .map(|l| (l.width, l.height))
            .collect::<Vec<_>>(),
        [(3, 5), (1, 2), (1, 1)]
    );
    assert_eq!(s.level_bytes(1).unwrap()[0], 0);
    assert_eq!(s.level_bytes(1).unwrap()[4], 102);
    assert_eq!(s.level_bytes(2).unwrap()[0], 51);
    let mut center = vec![0; 3 * 5 * 4];
    center[(2 * 3 + 1) * 4] = 255;
    let center = f::produce(MipRole3D::NumericMrAo, 3, 5, center, 2, 1, true);
    assert_eq!(center.level_bytes(1).unwrap()[0], 17);
    assert_eq!(center.level_bytes(1).unwrap()[4], 17);
    assert_eq!(center.level_bytes(2).unwrap()[0], 17);
    let strip = f::produce(MipRole3D::Occlusion, 1, 8, vec![128; 32], 3, 1, true);
    assert_eq!(strip.texels.len(), 60);
    assert!(strip.texels.iter().all(|b| *b == 128));
    let color = f::checker(MipRole3D::OpaqueBase, 1024, 4);
    assert_eq!(color.texels.len(), 5_592_404);
    assert_eq!(5 * color.texels.len(), 27_962_020);
    assert_eq!(color.levels.len(), 11);
    for level in 1..11 {
        assert!(
            color
                .level_bytes(level)
                .unwrap()
                .chunks_exact(4)
                .all(|p| p == [188, 188, 188, 255])
        );
    }
    let numeric = f::checker(MipRole3D::NumericMrAo, 4, 5);
    assert_eq!(&numeric.level_bytes(1).unwrap()[..3], &[128; 3]);
    let mask = f::produce(
        MipRole3D::MaskBase,
        2,
        1,
        vec![255, 0, 0, 255, 0, 255, 0, 0],
        6,
        1,
        true,
    );
    assert_eq!(mask.level_bytes(1).unwrap(), [255, 0, 0, 128]);
    let opaque = f::produce(
        MipRole3D::OpaqueBase,
        2,
        1,
        vec![255, 0, 0, 255, 0, 255, 0, 0],
        7,
        1,
        true,
    );
    assert_eq!(opaque.level_bytes(1).unwrap(), [188, 188, 0, 128]);
    let normal = f::produce(
        MipRole3D::Normal,
        2,
        1,
        vec![255, 128, 128, 255, 128, 255, 255, 1],
        8,
        1,
        true,
    );
    assert_eq!(normal.level_bytes(1).unwrap(), [192, 192, 192, 128]);
    let boundary = f::checker(MipRole3D::Normal, 8, 9);
    assert!(boundary.texels.chunks_exact(4).all(|p| p[2] >= 128));
    let mr = f::produce(
        MipRole3D::NumericMrAo,
        2,
        1,
        vec![10, 0, 255, 255, 30, 255, 0, 1],
        10,
        1,
        true,
    );
    assert_eq!(mr.level_bytes(1).unwrap(), [20, 128, 128, 128]);
    let m = f::multiscale();
    for (l, expected) in [168, 147, 132, 115, 127, 144, 136, 125, 129]
        .into_iter()
        .enumerate()
    {
        let d = m.levels[l];
        let x = 101 >> l;
        let y = 307 >> l;
        assert_eq!(
            m.level_bytes(l).unwrap()[(y * d.width as usize + x) * 4],
            expected,
            "level{l}"
        );
    }
}

#[test]
fn r028_typed_chain_sampler_and_material_source_refusals_and_old_generation_leases() {
    use crate::three_d::mip_sampling::*;
    use crate::three_d::pbr::*;
    use r028_fixture as f;
    use std::sync::Arc;
    let context =
        Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
    let source = f::checker(MipRole3D::OpaqueBase, 4, 11);
    let handle = context.register_mip_chain(source.clone(), None).unwrap();
    let held = context.lease_mip_chain(&handle).unwrap();
    let sample = f::sampler(
        &source,
        1,
        MipMin3D::LinearMipLinear,
        SamplerFilterMode::Linear,
        SamplerAddressMode::ClampToEdge,
        SamplerAddressMode::ClampToEdge,
    );
    let sh = context.register_mip_sampler(sample, None).unwrap();
    assert!(
        context
            .register_mip_sampler(
                MipSamplerSource3D {
                    extent: [0, 4],
                    ..sample
                },
                None
            )
            .is_err()
    );
    let sampler = context.lease_mip_sampler(&sh).unwrap();
    validate_pair(&held, &sampler).unwrap();
    let base = f::produce(
        MipRole3D::OpaqueBase,
        4,
        4,
        source.level_bytes(0).unwrap().to_vec(),
        11,
        1,
        false,
    );
    let base = context.register_mip_chain(base, None).unwrap();
    assert!(validate_pair(&context.lease_mip_chain(&base).unwrap(), &sampler).is_err());
    for case in 0..5 {
        let mut bad = source.clone();
        match case {
            0 => {
                let mut levels = bad.levels.to_vec();
                levels.pop();
                bad.levels = levels.into();
            }
            1 => {
                let mut levels = bad.levels.to_vec();
                levels.push(*levels.last().unwrap());
                bad.levels = levels.into();
            }
            2 => bad.format = TextureFormat3D::Rgba8Unorm,
            3 => Arc::make_mut(&mut bad.provenance).origin = None,
            _ => Arc::make_mut(&mut bad.provenance).uv_set = 1,
        }
        assert!(context.register_mip_chain(bad, None).is_err(), "case{case}");
    }
    let mut normal = f::checker(MipRole3D::Normal, 4, 12);
    let mut bytes = normal.texels.to_vec();
    let at = normal.levels[1].offset as usize + 2;
    bytes[at] = 127;
    normal.texels = bytes.into();
    Arc::make_mut(&mut normal.provenance).derived = Arc::downgrade(&normal.texels);
    assert!(normal.validate().is_err());
    let mut material = PbrMaterialSource3D::defaults(28, 1);
    material.roles[0] = Some(PbrRoleBinding3D::V2 {
        texture: handle.clone(),
        sampler: sh.clone(),
    });
    assert!(
        context
            .register_pbr_material(material.clone(), None)
            .is_err()
    );
    material.sampling = SamplingFamily3D::V2;
    let m = context.register_pbr_material(material, None).unwrap();
    let model = context.lease_pbr_material(&m).unwrap();
    assert_eq!(model.data.source.sampling, SamplingFamily3D::V2);
    let foreign =
        Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
    assert!(foreign.lease_mip_chain(&handle).is_err());
    assert!(foreign.lease_mip_sampler(&sh).is_err());
    let mut next = source.clone();
    Arc::make_mut(&mut next.provenance).content_revision = 2;
    let new = context.register_mip_chain(next, Some(&handle)).unwrap();
    assert!(context.lease_mip_chain(&handle).is_err());
    assert_eq!(held.source.levels.len(), 3);
    context.retire_mip_chain(&new).unwrap();
    assert!(context.lease_mip_chain(&new).is_err());
    context.retire_mip_sampler(&sh).unwrap();
    assert!(context.lease_mip_sampler(&sh).is_err());
    assert_eq!(context.resource_usage().unwrap().resident_texture_bytes, 0);
    let mut policy = PolicyLimits3D::conservative();
    policy.max_texture_dimension_2d = 1;
    let limited = Context3D::new_cpu_only_for_test(policy, u64::MAX).unwrap();
    assert!(matches!(
        limited.register_mip_chain(source, None),
        Err(PbrError3D::Context(Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::TextureDimension2D,
            requested: 4,
            maximum: 1
        }))
    ));
}

#[test]
fn r026_packed_half_assets_required_roles_context_generations_and_source_evidence() {
    use super::three_d_viewport::r026_fixture as f;
    use crate::three_d::indirect::*;
    use std::sync::Arc;
    let context =
        Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
    let assets = f::assets(glam::DVec3::X, 0.0, [0.25, 0.5, 0.75]);
    let make_evidence = |bits: &[Arc<[u16]>; 3]| {
        f::evidence(
            26001,
            SourceDerivation3D::Environment {
                method: Arc::from(
                    "packed-domain validation of separately derived analytical fixture",
                ),
                original_field: vec![0.25, 0.5, 0.75].into(),
                reference_observations: vec![std::f64::consts::PI].into(),
                derived: bits.each_ref().map(Arc::downgrade),
            },
            1,
        )
    };
    let evidence = make_evidence(&assets);
    let roles = [
        EnvironmentRole3D::Diffuse,
        EnvironmentRole3D::Specular,
        EnvironmentRole3D::BrdfLut,
    ];
    let handles: Vec<_> = roles
        .into_iter()
        .zip(&assets)
        .map(|(r, b)| {
            context
                .register_environment_asset(
                    26010 + r as u64,
                    1,
                    r,
                    evidence.clone(),
                    b.clone(),
                    None,
                )
                .unwrap()
        })
        .collect();
    let sampler = context.register_sampler(Sampler3D::LINEAR_CLAMP).unwrap();
    let source = EnvironmentSource3D::from_handles(
        &context,
        evidence.clone(),
        std::array::from_fn(|i| Some(handles[i].clone())),
        Some(sampler.handle().clone()),
    )
    .unwrap();
    drop(source);
    for missing in 0..3 {
        let mut h: [Option<EnvironmentHandle3D>; 3] =
            std::array::from_fn(|i| Some(handles[i].clone()));
        h[missing] = None;
        assert!(
            EnvironmentSource3D::from_handles(
                &context,
                evidence.clone(),
                h,
                Some(sampler.handle().clone())
            )
            .is_err()
        );
    }
    for (role, index, bad) in [
        (EnvironmentRole3D::Diffuse, 0, 0x7e00),
        (EnvironmentRole3D::Diffuse, 0, 0x7c00),
        (EnvironmentRole3D::Diffuse, 0, 0xbc00),
        (EnvironmentRole3D::Specular, 1, 0x6c01),
        (EnvironmentRole3D::BrdfLut, 2, 0x3801),
    ] {
        let mut bytes = assets.clone();
        let mut v = bytes[index].to_vec();
        v[0] = bad;
        if role == EnvironmentRole3D::BrdfLut {
            v[1] = 0x3800;
        }
        bytes[index] = v.into();
        let proof = make_evidence(&bytes);
        assert!(
            context
                .register_environment_asset(26100, 1, role, proof, bytes[index].clone(), None)
                .is_err(),
            "bad packed half {bad:x}"
        );
    }
    let foreign =
        Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
    assert!(foreign.lease_environment_asset(&handles[0]).is_err());
    let held = context.lease_environment_asset(&handles[0]).unwrap();
    let usage = context.resource_usage().unwrap();
    let next = context
        .register_environment_asset(
            26010,
            2,
            roles[0],
            evidence.clone(),
            assets[0].clone(),
            Some(&handles[0]),
        )
        .unwrap();
    assert!(context.lease_environment_asset(&handles[0]).is_err());
    assert!(
        context
            .resource_usage()
            .unwrap()
            .retained_cpu_descriptor_bytes
            > usage.retained_cpu_descriptor_bytes
    );
    drop(held);
    context.retire_environment_asset(&next).unwrap();
    assert!(context.lease_environment_asset(&next).is_err());
    // Unknown original mapping and a copied, unattributed array refuse before GPU.
    let mut unknown = (*evidence).clone();
    unknown.original_to_world = None;
    assert!(
        context
            .register_environment_asset(1, 1, roles[0], Arc::new(unknown), assets[0].clone(), None)
            .is_err()
    );
    assert!(
        context
            .register_environment_asset(1, 1, roles[0], evidence, assets[0].to_vec().into(), None)
            .is_err()
    );
}

#[test]
fn r026_complete_bound_stale_world_and_g_dependency_refusals_and_no_registry_cycle() {
    use super::three_d_viewport::r026_fixture as f;
    use std::sync::Arc;
    let context =
        Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
    let scene = f::scene(&context, 1, 7);
    scene.validate(&context).unwrap();
    let mut low = f::successor(&scene, 2);
    low.hdr.maximum_rgb = [2.2; 3];
    low.direct.as_ref().unwrap().validate(&low).unwrap();
    assert!(
        matches!(
            low.validate(&context),
            Err(crate::three_d::pbr::PbrError3D::Indirect {
                field: "complete radiance certificate",
                ..
            })
        ),
        "D sub-bound is insufficient for E+G"
    );
    let e = f::environment(&context, 27000, glam::DVec3::X, 0.0, [0.5; 3]);
    let mut stale = f::successor(&scene, 2);
    let old = stale.indirect.as_ref().unwrap();
    stale.indirect = Some(Arc::new(crate::three_d::indirect::IndirectSource3D {
        identity: old.identity,
        revision: 2,
        world_identity: 26,
        environment: Some(e),
        gi: old.gi.clone(),
        visibility_revision: 1,
    }));
    assert!(
        stale.validate(&context).is_err(),
        "changed E cannot relabel stale G"
    );
    let old = scene.indirect.as_ref().unwrap();
    let mut wrong = f::successor(&scene, 2);
    wrong.indirect = Some(Arc::new(crate::three_d::indirect::IndirectSource3D {
        identity: old.identity,
        revision: 2,
        world_identity: 27,
        environment: old.environment.clone(),
        gi: old.gi.clone(),
        visibility_revision: 1,
    }));
    assert!(wrong.validate(&context).is_err());
    let weak_snapshot = Arc::downgrade(&scene.snapshot);
    let weak_model = Arc::downgrade(&scene.model);
    drop(low);
    drop(stale);
    drop(wrong);
    drop(scene);
    drop(context);
    assert!(weak_snapshot.upgrade().is_none());
    assert!(
        weak_model.upgrade().is_none(),
        "G registry may not retain context through its own snapshot/model"
    );
}

#[test]
fn r026_original_to_world_90_degree_producer_reintegration_requires_different_cube_bytes() {
    use super::three_d_viewport::r026_fixture as f;
    use crate::three_d::{Orientation3, indirect::SourceDerivation3D};
    use std::sync::Arc;
    let context =
        Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
    let original = f::environment(&context, 31001, glam::DVec3::X, 0.125, [0.5; 3]);
    let h = std::f32::consts::FRAC_1_SQRT_2;
    let q = Orientation3::try_from_xyzw(0.0, 0.0, h, h).unwrap();
    let rotated = f::environment_mapped(&context, 31002, glam::DVec3::X, 0.125, [0.5; 3], q);
    let SourceDerivation3D::Environment { derived: old, .. } =
        original.evidence.derivation.as_ref()
    else {
        panic!()
    };
    let SourceDerivation3D::Environment { derived: new, .. } = rotated.evidence.derivation.as_ref()
    else {
        panic!()
    };
    let a = old[0].upgrade().unwrap();
    let b = new[0].upgrade().unwrap();
    assert_ne!(&*a, &*b);
    assert!(!Arc::ptr_eq(&original.evidence, &rotated.evidence));
    assert_eq!(rotated.evidence.original_to_world, Some(q));
    // The shader gets the differently derived WORLD cube, never q.
    let plus_x = (16 * 32 + 16) * 4;
    let plus_y = (2 * 32 * 32 + 16 * 32 + 16) * 4;
    assert!(
        crate::three_d::indirect::decode_half(a[plus_x])
            > crate::three_d::indirect::decode_half(a[plus_y])
    );
    assert!(
        crate::three_d::indirect::decode_half(b[plus_y])
            > crate::three_d::indirect::decode_half(b[plus_x])
    );
}

#[test]
fn r025_radiance_certificate_tighter_brdf_bound() {
    use crate::three_d::direct_shadow::{DirectSource3D, Directional3D, PointLight3D, SpotLight3D};
    use crate::three_d::hdr::HdrSource3D;
    use crate::three_d::pbr::{
        MaterialPacketSource3D, MaterialSceneSource3D, PbrError3D, PbrGeometrySource3D,
        PbrMaterialSource3D,
    };
    use crate::three_d::{
        ModelPlacement3, ModelRevision, Orientation3, PositiveScale3, RasterMode,
        StructuralRevision, WorldPoint3,
    };
    use std::sync::Arc;

    for samples in [1, 4] {
        let context =
            Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
        let geometry = context
            .register_pbr_geometry(
                PbrGeometrySource3D::try_new(
                    super::three_d_geometry::r024_vertices(),
                    GeometryIndices::Unindexed,
                )
                .unwrap(),
                None,
            )
            .unwrap();
        let mut material = PbrMaterialSource3D::defaults(97500, 1);
        material.roughness = 0.25;
        material.emissive_rgb = [0.5; 3];
        let material = context.register_pbr_material(material, None).unwrap();
        let snapshot = context
            .admit_material_snapshot(
                StructuralRevision::new(1),
                vec![MaterialPacketSource3D::PbrMaterial {
                    geometry,
                    material,
                    raster: RasterMode::SingleSided,
                    stable_ordinal: 0,
                }],
            )
            .unwrap();
        let model = context
            .admit_material_model(
                snapshot.clone(),
                ModelRevision::new(1),
                vec![
                    ModelPlacement3::try_new(
                        WorldPoint3::origin(),
                        Orientation3::identity(),
                        PositiveScale3::try_new(1.0, 1.0, 1.0).unwrap(),
                    )
                    .unwrap(),
                ],
            )
            .unwrap();
        let point = PointLight3D {
            // On-receiver emitters keep these one-ULP refusals meaningful when
            // distant receivers may use the tighter geometric certificate.
            position: WorldPoint3::origin(),
            intensity: [0.75; 3],
            radius_m: 0.5,
            range_m: 10.0,
        };
        let spot = SpotLight3D {
            point: PointLight3D {
                intensity: [2.625; 3],
                ..point.clone()
            },
            emitter_to_scene: [0.0, 0.0, -1.0],
            inner_radians: 0.2,
            outer_radians: 0.7,
        };
        let mut scene = MaterialSceneSource3D {
            direct: Some(Arc::new(DirectSource3D {
                identity: 97501,
                revision: 1,
                meters_per_scene_unit: 1.0,
                directional: Some(Directional3D {
                    surface_to_source: [0.0, 0.0, 1.0],
                    irradiance: [3.0; 3],
                    shadow_far_m: 8.0,
                }),
                points: Arc::from([point]),
                spots: vec![spot; 4].into(),
                snapshot: snapshot.clone(),
                model: model.clone(),
            })),
            snapshot,
            model,
            hdr: HdrSource3D {
                identity: 97502,
                revision: 1,
                exposure_ev: 16.0,
                maximum_rgb: [32816.5; 3],
            },
            samples,
            indirect: None,
            observe: false,
        };
        // Exact source arithmetic: incident=3+.75/.5^2+4*2.625/.5^2=48.
        // r_eff=1/4, so the new direct+emission envelope is
        // (1+4096/6)*48+.5=32816.5, exactly the declared channel ceiling.
        // The former envelope was (1+4096)*48+.5=196656.5: a false refusal.
        assert!(196656.5 > f64::from(scene.hdr.maximum_rgb[0]));
        assert_eq!(scene.validate(&context), Ok(()), "N{samples} exact ceiling");

        let radiance_refusal = |candidate: &MaterialSceneSource3D| {
            let result = candidate.validate(&context);
            assert!(
                matches!(
                    result,
                    Err(PbrError3D::Direct {
                        field: "radiance certificate",
                        ..
                    })
                ),
                "N{samples} expected the original typed radiance refusal: {result:?}"
            );
        };
        scene.hdr.maximum_rgb[1] = 32816.5f32.next_down();
        radiance_refusal(&scene);
        scene.hdr.maximum_rgb[1] = 32816.5;
        let direct = Arc::get_mut(scene.direct.as_mut().unwrap()).unwrap();
        Arc::make_mut(&mut direct.spots)[3].point.intensity[2] = 2.625f32.next_up();
        radiance_refusal(&scene);
        let direct = Arc::get_mut(scene.direct.as_mut().unwrap()).unwrap();
        Arc::make_mut(&mut direct.spots)[3].point.intensity[2] = 2.625;
        scene.hdr.maximum_rgb = [60000.0; 3];
        assert_eq!(scene.validate(&context), Ok(()));
        scene.hdr.maximum_rgb[0] = 60000.0f32.next_up();
        assert_eq!(
            scene.validate(&context),
            Err(PbrError3D::Association(
                "missing/invalid producer radiance certificate or exposure"
            ))
        );
    }
}

#[test]
fn r025_geometry_aware_incident_certificate() {
    use crate::three_d::direct_shadow::{DirectSource3D, PointLight3D, SpotLight3D};
    use crate::three_d::hdr::HdrSource3D;
    use crate::three_d::pbr::{
        MaterialPacketSource3D, MaterialSceneSource3D, PbrError3D, PbrGeometrySource3D,
        PbrMaterialSource3D,
    };
    use crate::three_d::{
        ModelPlacement3, ModelRevision, Orientation3, PositiveScale3, RasterMode,
        StructuralRevision, WorldPoint3,
    };
    use std::sync::Arc;

    let spots = |near: bool, intensity: f32| {
        [
            [16.0, 0.0, 32.0],
            [-16.0, 0.0, 32.0],
            [0.0, 16.0, 32.0],
            [0.0, -16.0, 32.0],
        ]
        .map(|far| {
            let p = if near { [0.0; 3] } else { far };
            let axis = if near {
                -glam::DVec3::Z
            } else {
                -glam::DVec3::from_array(p).normalize()
            };
            SpotLight3D {
                point: PointLight3D {
                    position: WorldPoint3::try_new(p[0], p[1], p[2]).unwrap(),
                    intensity: [intensity; 3],
                    radius_m: 0.5,
                    range_m: 50.0,
                },
                emitter_to_scene: axis.to_array(),
                inner_radians: 0.2,
                outer_radians: 0.7,
            }
        })
        .to_vec()
    };
    let fixture = |samples,
                   placement,
                   k,
                   roughness,
                   emission,
                   lights: Vec<SpotLight3D>,
                   unused: Option<[f32; 3]>| {
        let context =
            Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
        let mut vertices = super::three_d_geometry::r024_vertices();
        if let Some(p) = unused {
            let mut vertex = vertices[0].clone();
            vertex.position = LocalPoint3::try_new(p[0], p[1], p[2]).unwrap();
            vertices.push(vertex);
        }
        let geometry = context
            .register_pbr_geometry(
                PbrGeometrySource3D::try_new(vertices, GeometryIndices::from_u32(vec![0, 1, 2]))
                    .unwrap(),
                None,
            )
            .unwrap();
        let mut material = PbrMaterialSource3D::defaults(97600, 1);
        material.roughness = roughness;
        material.emissive_rgb = emission;
        let material = context.register_pbr_material(material, None).unwrap();
        let snapshot = context
            .admit_material_snapshot(
                StructuralRevision::new(1),
                vec![MaterialPacketSource3D::PbrMaterial {
                    geometry,
                    material,
                    raster: RasterMode::SingleSided,
                    stable_ordinal: 0,
                }],
            )
            .unwrap();
        let model = context
            .admit_material_model(snapshot.clone(), ModelRevision::new(1), vec![placement])
            .unwrap();
        let scene = MaterialSceneSource3D {
            direct: Some(Arc::new(DirectSource3D {
                identity: 97601,
                revision: 1,
                meters_per_scene_unit: k,
                directional: None,
                points: Arc::from([]),
                spots: lights.into(),
                snapshot: snapshot.clone(),
                model: model.clone(),
            })),
            snapshot,
            model,
            hdr: HdrSource3D {
                identity: 97602,
                revision: 1,
                exposure_ev: 16.0,
                maximum_rgb: [59000.0; 3],
            },
            samples,
            indirect: None,
            observe: false,
        };
        (context, scene)
    };
    let identity = ModelPlacement3::try_new(
        WorldPoint3::origin(),
        Orientation3::identity(),
        PositiveScale3::try_new(1.0, 1.0, 1.0).unwrap(),
    )
    .unwrap();
    let q = glam::Quat::from_rotation_y(0.7);
    let placed = ModelPlacement3::try_new(
        WorldPoint3::try_new(10.0, -7.0, 3.0).unwrap(),
        Orientation3::try_from_xyzw(q.x, q.y, q.z, q.w).unwrap(),
        PositiveScale3::try_new(2.0, 0.5, 1.0).unwrap(),
    )
    .unwrap();
    let radiance_refusal = |result| {
        assert!(
            matches!(
                result,
                Err(PbrError3D::Direct {
                    field: "radiance certificate",
                    ..
                })
            ),
            "expected the unchanged radiance refusal: {result:?}"
        );
    };
    for samples in [1, 4] {
        // Real f32 intensity supplied for the consumer target, but a synthetic
        // source-owned triangle fixture: this is not a Steel/Metal observation.
        let intensity = 0.08403114f32;
        let global = (1.0 + 1.0 / (6.0 * 0.045f64.powi(6))) * (16.0 * f64::from(intensity)) + 0.5;
        assert!(global > 59000.0);
        let (context, mut scene) = fixture(
            samples,
            identity,
            1.0,
            0.0,
            [0.5; 3],
            spots(false, intensity),
            None,
        );
        assert_eq!(
            scene.validate(&context),
            Ok(()),
            "N{samples} four distant spots"
        );

        let captured = scene.direct.as_ref().unwrap();
        let other_instance = DirectSource3D {
            identity: captured.identity,
            revision: captured.revision,
            meters_per_scene_unit: captured.meters_per_scene_unit,
            directional: captured.directional.clone(),
            points: captured.points.clone(),
            spots: captured.spots.clone(),
            snapshot: captured.snapshot.clone(),
            model: captured.model.clone(),
        };
        assert!(matches!(
            other_instance.validate(&scene),
            Err(PbrError3D::Direct {
                field: "source provenance",
                ..
            })
        ));
        scene.model = context
            .admit_material_model(scene.snapshot.clone(), ModelRevision::new(2), vec![placed])
            .unwrap();
        assert!(matches!(
            scene.validate(&context),
            Err(PbrError3D::Direct {
                field: "casters",
                ..
            })
        ));

        let (context, scene) = fixture(
            samples,
            placed,
            0.5,
            0.0,
            [0.5; 3],
            spots(false, intensity),
            None,
        );
        assert_eq!(
            scene.validate(&context),
            Ok(()),
            "rotated/scaled metric receiver"
        );
        let (context, scene) = fixture(
            samples,
            identity,
            1.0,
            0.0,
            [0.5; 3],
            spots(true, intensity),
            None,
        );
        radiance_refusal(scene.validate(&context));
        let (context, scene) = fixture(
            samples,
            identity,
            1.0,
            0.0,
            [0.5; 3],
            spots(false, intensity),
            Some([16.0, 0.0, 32.0]),
        );
        radiance_refusal(scene.validate(&context)); // unused vertex MUST widen the box

        let stretched = ModelPlacement3::try_new(
            WorldPoint3::origin(),
            Orientation3::identity(),
            PositiveScale3::try_new(2.0, 1.0, 1.0).unwrap(),
        )
        .unwrap();
        let (context, scene) = fixture(
            samples,
            stretched,
            1.0,
            0.0,
            [0.5; 3],
            spots(false, intensity),
            Some([f32::MAX, 0.0, 0.0]),
        );
        radiance_refusal(scene.validate(&context)); // unprovable box retains generic refusal
        let (context, scene) = fixture(
            samples,
            stretched,
            1.0,
            1.0,
            [0.5; 3],
            spots(false, 0.001),
            Some([f32::MAX, 0.0, 0.0]),
        );
        assert_eq!(
            scene.validate(&context),
            Ok(()),
            "original positive bypasses geometry"
        );

        let (context, mut boundary) = fixture(
            samples,
            identity,
            1.0,
            1.0,
            [0.5; 3],
            spots(true, 3.0),
            None,
        );
        // On-box emitters retain the original denominator: (7/6)*48+.5=56.5.
        boundary.hdr.maximum_rgb = [56.5; 3];
        assert_eq!(boundary.validate(&context), Ok(()));
        for c in 0..3 {
            let direct = Arc::get_mut(boundary.direct.as_mut().unwrap()).unwrap();
            Arc::make_mut(&mut direct.spots)[3].point.intensity[c] = 3.0f32.next_up();
            radiance_refusal(boundary.validate(&context));
            let direct = Arc::get_mut(boundary.direct.as_mut().unwrap()).unwrap();
            Arc::make_mut(&mut direct.spots)[3].point.intensity[c] = 3.0;
        }
        let (context, mut emission) = fixture(
            samples,
            identity,
            1.0,
            1.0,
            [0.5, 0.5f32.next_up(), 0.5],
            spots(true, 3.0),
            None,
        );
        emission.hdr.maximum_rgb = [56.5; 3];
        radiance_refusal(emission.validate(&context));
    }
}

fn hal372_extent_scene(
    context: &Context3D,
    identity: u64,
    indices: GeometryIndices,
    unused: bool,
    placements: Vec<crate::three_d::ModelPlacement3>,
) -> (
    crate::three_d::PbrScene3D,
    crate::three_d::pbr::PbrGeometryHandle3D,
) {
    use crate::three_d::{hdr::HdrSource3D, pbr::*, *};
    let mut vertices = super::three_d_geometry::r024_vertices();
    if unused {
        let mut v = vertices[0].clone();
        v.position = LocalPoint3::try_new(1000.0, 1000.0, 1000.0).unwrap();
        vertices.push(v);
    }
    let geometry = context
        .register_pbr_geometry(
            PbrGeometrySource3D::try_new(vertices, indices).unwrap(),
            None,
        )
        .unwrap();
    let mut material = PbrMaterialSource3D::defaults(identity + 1, 1);
    material.emissive_rgb = [1.0; 3];
    let material = context.register_pbr_material(material, None).unwrap();
    let snapshot = context
        .admit_material_snapshot(
            StructuralRevision::new(1),
            (0..placements.len())
                .map(|i| MaterialPacketSource3D::PbrMaterial {
                    geometry: geometry.clone(),
                    material: material.clone(),
                    raster: RasterMode::SingleSided,
                    stable_ordinal: i as u64,
                })
                .collect(),
        )
        .unwrap();
    let model = context
        .admit_material_model(snapshot.clone(), ModelRevision::new(1), placements)
        .unwrap();
    let source = MaterialSceneSource3D {
        snapshot,
        model,
        hdr: HdrSource3D {
            identity,
            revision: 1,
            exposure_ev: 0.0,
            maximum_rgb: [1.0; 3],
        },
        samples: 1,
        direct: None,
        indirect: None,
        observe: false,
    };
    source.validate(context).unwrap();
    (PbrScene3D(Arc::new(source)), geometry)
}

#[test]
fn hal372_cpu_extent_indexed_source_world_and_ticket() {
    use crate::three_d::*;
    use std::sync::atomic::AtomicBool;
    fn cpu<T: Send + Sync>() {}
    cpu::<PbrObjectRevealExtentCpuJob3D>();
    cpu::<PbrObjectRevealExtentCpuResult3D>();
    let placements = vec![
        ModelPlacement3::try_new(
            WorldPoint3::origin(),
            Orientation3::identity(),
            PositiveScale3::try_new(1.0, 1.0, 1.0).unwrap(),
        )
        .unwrap(),
        ModelPlacement3::try_new(
            WorldPoint3::try_new(10.0, 20.0, 5.0).unwrap(),
            Orientation3::try_from_xyzw(0.0, 0.0, 1.0, 0.0).unwrap(),
            PositiveScale3::try_new(2.0, 3.0, 1.0).unwrap(),
        )
        .unwrap(),
    ];
    for indices in [
        GeometryIndices::Unindexed,
        GeometryIndices::from_u16(vec![0, 1, 2]),
        GeometryIndices::from_u32(vec![0, 1, 2]),
    ] {
        let context =
            Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
        let indexed = indices.is_indexed();
        let (source, _) =
            hal372_extent_scene(&context, 372001, indices, indexed, placements.clone());
        assert!(context.begin_pbr_object_reveal_extent(&source, 1).is_err());
        let (job, pending) = context
            .begin_pbr_object_reveal_extent(&source, 1 << 20)
            .unwrap();
        let result = std::thread::spawn(move || job.verify(Arc::new(AtomicBool::new(false))))
            .join()
            .unwrap()
            .unwrap();
        let extent = context
            .finish_pbr_object_reveal_extent(pending, result, &source)
            .unwrap();
        assert_eq!(
            extent.reference_for_test(),
            ([-1.0, -1.0, 0.0], [12.0, 23.0, 5.0], 1)
        );
        drop(extent);
        assert_eq!(hal373_fixture::settled(&context), 0);
    }
}

#[test]
fn hal372_cpu_extent_cancel_escrows_original_retired_capacity() {
    use crate::three_d::*;
    use std::sync::atomic::AtomicBool;
    let context =
        Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
    let placement = ModelPlacement3::try_new(
        WorldPoint3::origin(),
        Orientation3::identity(),
        PositiveScale3::try_new(1.0, 1.0, 1.0).unwrap(),
    )
    .unwrap();
    let (source, geometry) = hal372_extent_scene(
        &context,
        372101,
        GeometryIndices::Unindexed,
        false,
        vec![placement],
    );
    let (job, pending) = context
        .begin_pbr_object_reveal_extent(&source, 1 << 20)
        .unwrap();
    drop(pending);
    context.retire_pbr_geometry(&geometry).unwrap();
    drop(source);
    let held = context
        .resource_usage()
        .unwrap()
        .retained_cpu_descriptor_bytes;
    assert!(
        hal373_fixture::settled(&context) > 0,
        "UI cancellation must not free a live worker's capacity"
    );
    let result = std::thread::spawn(move || job.verify(Arc::new(AtomicBool::new(true))))
        .join()
        .unwrap();
    assert!(matches!(
        result,
        Err(PbrObjectRevealExtentCpuError3D::Cancelled)
    ));
    assert_eq!(hal373_fixture::settled(&context), 0);
    assert!(
        context
            .resource_usage()
            .unwrap()
            .retained_cpu_descriptor_bytes
            < held
    );
}

#[test]
fn hal372_cpu_extent_source_context_and_result_mismatch() {
    use crate::three_d::*;
    use std::sync::atomic::AtomicBool;
    let context =
        Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
    let foreign =
        Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
    let placement = ModelPlacement3::try_new(
        WorldPoint3::origin(),
        Orientation3::identity(),
        PositiveScale3::try_new(1.0, 1.0, 1.0).unwrap(),
    )
    .unwrap();
    let (source, _) = hal372_extent_scene(
        &context,
        372201,
        GeometryIndices::Unindexed,
        false,
        vec![placement],
    );
    let (replacement, _) = hal372_extent_scene(
        &context,
        372203,
        GeometryIndices::Unindexed,
        false,
        vec![placement],
    );
    assert!(
        foreign
            .begin_pbr_object_reveal_extent(&source, 1 << 20)
            .is_err()
    );
    let (job, pending) = context
        .begin_pbr_object_reveal_extent(&source, 1 << 20)
        .unwrap();
    let result = job.verify(Arc::new(AtomicBool::new(false))).unwrap();
    assert!(
        context
            .finish_pbr_object_reveal_extent(pending, result, &replacement)
            .is_err()
    );
    let (a, pending_a) = context
        .begin_pbr_object_reveal_extent(&source, 1 << 20)
        .unwrap();
    let (b, pending_b) = context
        .begin_pbr_object_reveal_extent(&source, 1 << 20)
        .unwrap();
    let result = a.verify(Arc::new(AtomicBool::new(false))).unwrap();
    assert!(
        context
            .finish_pbr_object_reveal_extent(pending_b, result, &source)
            .is_err()
    );
    drop(b);
    drop(pending_a);
    assert_eq!(hal373_fixture::settled(&context), 0);
}

#[test]
fn hal372_cpu_extent_capacity_deduplicates_allocations_not_values() {
    use crate::three_d::{hdr::HdrSource3D, pbr::*, *};
    let context =
        Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
    let placement = ModelPlacement3::try_new(
        WorldPoint3::origin(),
        Orientation3::identity(),
        PositiveScale3::try_new(1.0, 1.0, 1.0).unwrap(),
    )
    .unwrap();
    let (shared, _) = hal372_extent_scene(
        &context,
        372401,
        GeometryIndices::Unindexed,
        false,
        vec![placement; 2],
    );
    let mut packets = shared.0.snapshot.source_packets().to_vec();
    let geometry = context
        .register_pbr_geometry(
            PbrGeometrySource3D::try_new(
                super::three_d_geometry::r024_vertices(),
                GeometryIndices::Unindexed,
            )
            .unwrap(),
            None,
        )
        .unwrap();
    let MaterialPacketSource3D::PbrMaterial {
        geometry: second, ..
    } = &mut packets[1]
    else {
        unreachable!()
    };
    *second = geometry;
    let snapshot = context
        .admit_material_snapshot(StructuralRevision::new(2), packets)
        .unwrap();
    let model = context
        .admit_material_model(snapshot.clone(), ModelRevision::new(2), vec![placement; 2])
        .unwrap();
    let distinct = MaterialSceneSource3D {
        snapshot,
        model,
        hdr: HdrSource3D {
            identity: 372403,
            revision: 1,
            exposure_ev: 0.0,
            maximum_rgb: [1.0; 3],
        },
        samples: 1,
        direct: None,
        indirect: None,
        observe: false,
    };
    distinct.validate(&context).unwrap();
    let distinct = PbrScene3D(Arc::new(distinct));
    let minimum = |source: &PbrScene3D| {
        let (mut lo, mut hi) = (0usize, 1usize << 20);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            match context.begin_pbr_object_reveal_extent(source, mid) {
                Ok((job, pending)) => {
                    drop(job);
                    drop(pending);
                    hi = mid;
                }
                Err(PbrSourceError3D::Context(Context3DError::PolicyBudgetExceeded { .. })) => {
                    lo = mid + 1
                }
                Err(error) => panic!("unexpected admission error: {error:?}"),
            }
        }
        lo
    };
    let one = minimum(&shared);
    let two = minimum(&distinct);
    assert_eq!(
        two - one,
        3 * std::mem::size_of::<PbrVertexSource3D>() + 2 * std::mem::size_of::<usize>()
    );
    assert_eq!(hal373_fixture::settled(&context), 0);
}

#[test]
fn hal372_cpu_extent_axis_fallback_and_render_frame_refusal() {
    use crate::three_d::*;
    use std::sync::atomic::AtomicBool;
    let context = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
    let huge = 2.0f64.powi(60);
    let q = glam::Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    for (expected, translation, orientation) in [
        (1, WorldPoint3::origin(), Orientation3::identity()),
        (
            0,
            WorldPoint3::try_new(0.0, huge, 0.0).unwrap(),
            Orientation3::identity(),
        ),
        (
            2,
            WorldPoint3::try_new(huge, huge, 0.0).unwrap(),
            Orientation3::try_from_xyzw(q.x, q.y, q.z, q.w).unwrap(),
        ),
    ] {
        // Exact f64 source-world additions collapse translated axes at this
        // supported large world coordinate; there is no epsilon axis policy.
        let (source, _) = hal372_extent_scene(
            &context,
            372400 + expected as u64,
            GeometryIndices::Unindexed,
            false,
            vec![
                ModelPlacement3::try_new(translation, orientation, PositiveScale3::one()).unwrap(),
            ],
        );
        let (job, pending) = context
            .begin_pbr_object_reveal_extent(&source, 1 << 20)
            .unwrap();
        let result = job.verify(Arc::new(AtomicBool::new(false))).unwrap();
        let extent = context
            .finish_pbr_object_reveal_extent(pending, result, &source)
            .unwrap();
        assert_eq!(extent.reference_for_test().2, expected);
        assert!(extent.parameters(0.5, translation).is_ok());
        assert!(
            extent
                .parameters(
                    0.5,
                    WorldPoint3::try_new(f64::MAX, f64::MAX, f64::MAX).unwrap()
                )
                .is_err()
        );
    }
    let direct = crate::tests::three_d_viewport::r025_source(&context, 1, 0, 2.0);
    direct.validate(&context).unwrap();
    let direct = PbrScene3D(direct);
    let (job, pending) = context
        .begin_pbr_object_reveal_extent(&direct, 1 << 20)
        .unwrap();
    let result = job.verify(Arc::new(AtomicBool::new(false))).unwrap();
    let extent = context
        .finish_pbr_object_reveal_extent(pending, result, &direct)
        .unwrap();
    let tail: [u32; 4] = bytemuck::cast(
        extent
            .parameters(0.25, WorldPoint3::try_new(0.0, 4.0, 0.0).unwrap())
            .unwrap(),
    );
    assert_eq!(
        tail,
        [0.25f32.to_bits(), (-10.0f32).to_bits(), 4.0f32.to_bits(), 1],
        "same source-world Y interval in the actual R025 metric/origin frame"
    );
    drop(extent);
    assert_eq!(hal373_fixture::settled(&context), 0);
}

#[test]
fn hal372_cpu_extent_context_quota_before_capture_and_mixed_source_refusal() {
    use crate::three_d::{pbr::*, *};
    let mut policy = PolicyLimits3D::conservative();
    policy.max_retained_dynamic_cpu_bytes = 1;
    let context = cpu_context(policy, u64::MAX);
    let identity = ModelPlacement3::try_new(
        WorldPoint3::origin(),
        Orientation3::identity(),
        PositiveScale3::one(),
    )
    .unwrap();
    let (source, _) = hal372_extent_scene(
        &context,
        372500,
        GeometryIndices::Unindexed,
        false,
        vec![identity],
    );
    let before = Arc::strong_count(&source.0);
    assert!(matches!(
        context.begin_pbr_object_reveal_extent(&source, 1 << 20),
        Err(PbrSourceError3D::Context(
            Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::RetainedDynamicCpuBytes,
                maximum: 1,
                ..
            }
        ))
    ));
    assert_eq!(Arc::strong_count(&source.0), before);
    assert_eq!(hal373_fixture::settled(&context), 0);

    let context = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
    let (source, pbr_geometry) = hal372_extent_scene(
        &context,
        372501,
        GeometryIndices::Unindexed,
        false,
        vec![identity],
    );
    let geometry = context.register_geometry(triangle(0.0)).unwrap();
    let appearance = context.register_appearance(appearance(r#"
fn halcyon_appearance(position_render_relative:vec3<f32>, normal_world_interpolated:vec3<f32>, view_direction_world:vec3<f32>, uv:vec2<f32>, vertex_color_linear:vec4<f32>, front_facing:bool, color_factor_linear:vec4<f32>, appearance_texture:texture_2d<f32>, appearance_sampler:sampler)->vec3<f32>{return vec3<f32>(1.0);}
"#)).unwrap();
    let pbr_material = context
        .register_pbr_material(PbrMaterialSource3D::defaults(372502, 1), None)
        .unwrap();
    let mut packets = vec![MaterialPacketSource3D::PbrMaterial {
        geometry: pbr_geometry,
        material: pbr_material,
        raster: RasterMode::SingleSided,
        stable_ordinal: 0,
    }];
    packets.push(MaterialPacketSource3D::LegacyAppearance(
        DrawPacketDescriptor::new(
            geometry.handle().clone(),
            appearance.handle().clone(),
            RasterMode::SingleSided,
            1,
        ),
    ));
    let snapshot = context
        .admit_material_snapshot(StructuralRevision::new(2), packets)
        .unwrap();
    let model = context
        .admit_material_model(snapshot.clone(), ModelRevision::new(2), vec![identity; 2])
        .unwrap();
    let mixed = MaterialSceneSource3D {
        snapshot,
        model,
        hdr: source.0.hdr.clone(),
        samples: 1,
        direct: None,
        indirect: None,
        observe: false,
    };
    mixed.validate(&context).unwrap(); // Ordinary mixed source remains eligible.
    assert!(matches!(
        context.begin_pbr_object_reveal_extent(&PbrScene3D(Arc::new(mixed)), 1 << 20),
        Err(PbrSourceError3D::Association(
            "reveal requires a complete PBR-only source"
        ))
    ));
    assert_eq!(hal373_fixture::settled(&context), 0);
}

#[test]
fn r025_complete_source_metric_interval_cone_near_coverage_and_certificate_refusals_are_pre_gpu() {
    use crate::three_d::{
        Orientation3, OrthographicProjection, RigidPose3, ViewDescriptor3, WorldPoint3,
    };
    use std::sync::Arc;
    let context =
        Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
    // The production shortcut must imply the complete original predicate. A
    // correlated triangle deliberately defeats the component hull while every
    // original corner still passes, so conservative grouping must fall back.
    {
        use crate::three_d::direct_shadow::triangle_depth_certificate_for_test;
        let point = |p: [f32; 3]| p.map(|v| [f64::from(v); 2]);
        let tiny = f32::from_bits(1);
        let cases = [
            (
                [0.125, 0.0625, 0.125, 0.5],
                [[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, 1.0, 0.0]],
                (true, true),
            ),
            (
                [0.25, -0.25, 0.0, 0.5],
                [[1.0, 1.0, 0.0], [2.0, 2.0, 0.0], [3.0, 3.0, 0.0]],
                (true, false),
            ),
            (
                [-0.0, 0.0, 0.0, 0.5],
                [[-tiny, tiny, 0.0], [tiny, -tiny, -0.0], [0.0, 0.0, tiny]],
                (true, true),
            ),
            (
                [f32::MIN_POSITIVE, 0.0, 0.0, 0.5],
                [
                    [-f32::MIN_POSITIVE, 0.0, 0.0],
                    [0.0, 0.0, 0.0],
                    [f32::MIN_POSITIVE.next_up(), 0.0, 0.0],
                ],
                (true, true),
            ),
            ([0.0, 0.0, 0.0, tiny], [[0.0; 3]; 3], (false, false)),
            ([0.0; 4], [[0.0; 3]; 3], (false, false)),
            ([0.0, 0.0, 0.0, 1.0], [[0.0; 3]; 3], (false, false)),
            (
                [f32::MAX, f32::MAX, 0.0, 0.5],
                [[1.0, 1.0, 0.0]; 3],
                (false, false),
            ),
        ];
        for (case, (row, points, expected)) in cases.into_iter().enumerate() {
            for widened in [false, true] {
                let inputs = if widened {
                    points.map(|p| p.map(|v| [f64::from(v.next_down()), f64::from(v.next_up())]))
                } else {
                    points.map(point)
                };
                let (hull, original, result) = triangle_depth_certificate_for_test(row, inputs);
                let original_accepts = original
                    .iter()
                    .all(|z| z.as_ref().is_ok_and(|z| z[0] > 0.0 && z[1] < 1.0));
                assert_eq!(result, expected, "depth hull branch case{case}");
                assert_eq!(result.0, original_accepts, "depth truth case{case}");
                assert_eq!(
                    result.1,
                    hull.as_ref().is_ok_and(|h| h[0] > 0.0 && h[1] < 1.0),
                );
                if let Ok(hull) = &hull {
                    for corner in &original {
                        let z = corner
                            .as_ref()
                            .expect("a successful hull encloses each original operation");
                        assert!(hull[0] <= z[0] && hull[1] >= z[1]);
                    }
                } else {
                    assert_eq!(
                        hull.as_ref().unwrap_err(),
                        original[0].as_ref().unwrap_err()
                    );
                }
            }
        }
    }
    {
        use crate::three_d::direct_shadow::{
            triangle_depth_certificate_for_test, triangle_groups_depth_certificate_for_test,
        };
        let point = |p: [f32; 3]| p.map(|v| [f64::from(v); 2]);
        let good = [[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, 1.0, 0.0]].map(point);
        let check = |row, triangles: &[[[[f64; 2]; 3]; 3]], expected| {
            let endpoints = triangles.iter().flatten().copied().collect::<Vec<_>>();
            let (result, counts, hulls) =
                triangle_groups_depth_certificate_for_test(row, &endpoints);
            let old = triangles
                .iter()
                .map(|triangle| triangle_depth_certificate_for_test(row, *triangle))
                .collect::<Vec<_>>();
            let first = old.iter().position(|(_, _, result)| !result.0);
            assert_eq!(result, first.map_or(Ok(()), Err));
            assert_eq!(result, expected);
            for (group, hull) in hulls.iter().enumerate() {
                if let Ok(hull) = hull {
                    for (_, original, _) in old.iter().skip(group * 16).take(16) {
                        for corner in original {
                            let z = corner
                                .as_ref()
                                .expect("finite group enclosure contains each original dot");
                            assert!(hull[0] <= z[0] && hull[1] >= z[1]);
                        }
                    }
                }
            }
            (counts, hulls)
        };
        let row = [0.125, 0.0625, 0.125, 0.5];
        for count in [1usize, 15, 16, 17, 32] {
            let (counts, _) = check(row, &vec![good; count], Ok(()));
            assert_eq!(counts[4], count.div_ceil(16) as u64);
            assert_eq!(counts[5], 0);
            assert_eq!(counts[6], count as u64);
            assert_eq!(counts[1] + counts[2], 0);
        }
        // Each triangle has a narrow exact response, but their joint X/Y hull
        // loses cross-triangle correlation and cannot certify the group.
        let correlated = (0..16)
            .map(|i| [point([1.0 + (i % 3) as f32, 1.0 + (i % 3) as f32, 0.0]); 3])
            .collect::<Vec<_>>();
        let (counts, hulls) = check([0.25, -0.25, 0.0, 0.5], &correlated, Ok(()));
        assert!(hulls[0].as_ref().is_ok_and(|z| z[0] <= 0.0 || z[1] >= 1.0));
        assert_eq!((counts[4], counts[5], counts[1], counts[2]), (0, 1, 16, 0));
        let mut invalid = vec![good; 32];
        invalid[17] = [point([8.0, 0.0, 0.0]); 3];
        invalid[31] = invalid[17];
        let (counts, _) = check(row, &invalid, Err(17));
        assert_eq!((counts[4], counts[5], counts[6]), (1, 1, 16));
        assert_eq!((counts[1], counts[2]), (1, 1));
        invalid[5] = invalid[17];
        let (counts, _) = check(row, &invalid, Err(5));
        assert_eq!((counts[4], counts[5], counts[6]), (0, 1, 0));
        assert_eq!((counts[1], counts[2]), (5, 1));
        for offset in [0.0, 1.0, f32::from_bits(1)] {
            let (counts, _) = check([0.0, 0.0, 0.0, offset], &[good; 17], Err(0));
            assert_eq!((counts[4], counts[5], counts[1], counts[2]), (0, 1, 0, 1));
        }
        let tiny = f32::from_bits(1);
        let subnormal = [[-tiny, tiny, -0.0], [0.0, -tiny, tiny], [tiny, 0.0, tiny]].map(point);
        let (counts, _) = check(
            [f32::MIN_POSITIVE, -f32::MIN_POSITIVE, tiny, 0.5],
            &[subnormal; 17],
            Ok(()),
        );
        assert_eq!((counts[4], counts[5], counts[6]), (2, 0, 17));
        for error_row in [
            [f32::INFINITY, 0.0, 0.0, 0.5],
            [f32::MAX, f32::MAX, 0.0, 0.5],
        ] {
            let overflow = [point([2.0, 2.0, 0.0]); 3];
            let (counts, hulls) = check(error_row, &[overflow; 17], Err(0));
            assert!(hulls[0].is_err());
            assert_eq!((counts[4], counts[5], counts[1], counts[2]), (0, 1, 0, 1));
            let (_, original, _) = triangle_depth_certificate_for_test(error_row, overflow);
            assert_eq!(
                hulls[0].as_ref().unwrap_err(),
                original[0].as_ref().unwrap_err()
            );
        }
    }
    for (near, far) in [(0.1, 8.0), (0.01, 50.0), (0.2, 16.0)] {
        let splits = crate::three_d::direct_shadow::cascade_splits(near, far).unwrap();
        for j in 1..=3 {
            let t = j as f64 / 3.0;
            let expected = (near * (far / near).powf(t) + near + (far - near) * t) * 0.5;
            assert_eq!(splits[j - 1], expected as f32);
        }
        assert_eq!(splits[2], far as f32);
        assert_eq!(splits[3], far as f32);
    }
    for case in 0..14 {
        let kind = if case == 7 || case == 8 { 2 } else { 1 };
        let mut scene = super::three_d_viewport::r025_source(&context, 1, kind, 1.0);
        let s = Arc::get_mut(&mut scene).unwrap();
        let d = Arc::get_mut(s.direct.as_mut().unwrap()).unwrap();
        match case {
            0 => d.meters_per_scene_unit = 0.0,
            1 => d.meters_per_scene_unit = f64::NAN,
            2 => Arc::make_mut(&mut d.points)[0].radius_m = 0.009,
            3 => {
                let p = &mut Arc::make_mut(&mut d.points)[0];
                p.range_m = p.radius_m;
            }
            4 => Arc::make_mut(&mut d.points)[0].intensity[0] = 4097.0,
            5 => Arc::make_mut(&mut d.points)[0].range_m = 50.1,
            6 => {
                let p = &mut Arc::make_mut(&mut d.points)[0];
                p.radius_m = 0.5;
                p.range_m = 0.500000001;
            }
            7 => {
                let spot = &mut Arc::make_mut(&mut d.spots)[0];
                spot.outer_radians = spot.inner_radians;
            }
            8 => {
                let spot = &mut Arc::make_mut(&mut d.spots)[0];
                spot.inner_radians = 0.2;
                spot.outer_radians = 0.2000000001;
            }
            9 => d.points = vec![d.points[0].clone(); 5].into(),
            10 => d.identity = 0,
            11 => d.revision = 0,
            12 => s.hdr.maximum_rgb = [0.0; 3],
            _ => {
                d.points = Arc::from([]);
                d.spots = Arc::from([]);
                d.directional = None;
            }
        }
        assert!(
            scene.validate(&context).is_err(),
            "invalid direct source case{case} silently admitted"
        );
    }
    let view = super::three_d_viewport::r025_view();
    let assert_allocating_refusal = |scene: &crate::three_d::pbr::MaterialSceneSource3D, view| {
        let direct = scene.direct.as_ref().unwrap();
        let current = direct
            .prepare_frame(scene, view, WorldPoint3::origin(), 32, 32)
            .err()
            .expect("the original coverage refusal must remain a refusal");
        let original = direct
            .prepare_frame_with_allocating_clip_for_test(
                scene,
                view,
                WorldPoint3::origin(),
                32,
                32,
                None,
                None,
            )
            .err()
            .expect("the allocating reference must refuse the same candidate");
        assert_eq!(current, original);
    };
    for (kind, cap) in [(1, "point"), (2, "spot")] {
        let mut scene = super::three_d_viewport::r025_source(&context, 1, kind, 1.0);
        let s = Arc::get_mut(&mut scene).unwrap();
        let d = Arc::get_mut(s.direct.as_mut().unwrap()).unwrap();
        if kind == 1 {
            Arc::make_mut(&mut d.points)[0].position =
                WorldPoint3::try_new(0.0, 0.0, 0.005).unwrap();
        } else {
            Arc::make_mut(&mut d.spots)[0].point.position =
                WorldPoint3::try_new(0.0, 0.0, 0.005).unwrap();
        }
        assert!(
            scene
                .direct
                .as_ref()
                .unwrap()
                .prepare_frame(&scene, view, WorldPoint3::origin(), 32, 32)
                .is_err(),
            "{cap} near-cap triangle interior was checked only at vertices"
        );
        assert_allocating_refusal(&scene, view);
    }
    let scene = super::three_d_viewport::r025_source(&context, 1, 0, 1.0);
    let zero_near = ViewDescriptor3::new(
        RigidPose3::new(
            WorldPoint3::try_new(0.0, 0.0, 2.0).unwrap(),
            Orientation3::identity(),
        ),
        OrthographicProjection::try_new(2.0, 0.0, 10.0)
            .unwrap()
            .into(),
    );
    assert!(
        scene
            .direct
            .as_ref()
            .unwrap()
            .prepare_frame(&scene, zero_near, WorldPoint3::origin(), 32, 32)
            .is_err()
    );
    assert_allocating_refusal(&scene, zero_near);
    let mut far = super::three_d_viewport::r025_source(&context, 1, 0, 1.0);
    Arc::get_mut(Arc::get_mut(&mut far).unwrap().direct.as_mut().unwrap())
        .unwrap()
        .directional
        .as_mut()
        .unwrap()
        .shadow_far_m = 1.0;
    assert!(
        far.direct
            .as_ref()
            .unwrap()
            .prepare_frame(&far, view, WorldPoint3::origin(), 32, 32)
            .is_err()
    );
    assert_allocating_refusal(&far, view);
    let mut spot = super::three_d_viewport::r025_source(&context, 1, 2, 1.0);
    {
        let s = &mut Arc::make_mut(
            &mut Arc::get_mut(Arc::get_mut(&mut spot).unwrap().direct.as_mut().unwrap())
                .unwrap()
                .spots,
        )[0];
        s.inner_radians = 0.05;
        s.outer_radians = 0.1;
    }
    assert!(
        spot.direct
            .as_ref()
            .unwrap()
            .prepare_frame(&spot, view, WorldPoint3::origin(), 32, 32)
            .is_err(),
        "clamped sampler cannot certify missing spot PCF footprint"
    );
    assert_allocating_refusal(&spot, view);
    // Retain only source-owned results. Compare a cached path against complete
    // source requalification, including every packed output and depth witness.
    {
        use crate::three_d::direct_shadow::{DirectFrame3D, DirectSource3D};
        use crate::three_d::pbr::MaterialSceneSource3D;
        let assert_frame = |fresh: &DirectFrame3D, retained: &DirectFrame3D| {
            assert_eq!(
                bytemuck::bytes_of(&fresh.light),
                bytemuck::bytes_of(&retained.light)
            );
            assert_eq!(
                bytemuck::bytes_of(&fresh.transforms),
                bytemuck::bytes_of(&retained.transforms)
            );
            assert_eq!(fresh.active_faces, retained.active_faces);
            assert_eq!(fresh.depth_proofs.len(), retained.depth_proofs.len());
            for (a, b) in fresh.depth_proofs.iter().zip(&retained.depth_proofs) {
                assert_eq!(a.t.to_bits(), b.t.to_bits());
                assert_eq!(a.row.map(f32::to_bits), b.row.map(f32::to_bits));
                assert_eq!(a.range.map(f64::to_bits), b.range.map(f64::to_bits));
                assert_eq!(
                    a.corners.map(|c| c.map(f64::to_bits)),
                    b.corners.map(|c| c.map(f64::to_bits))
                );
            }
        };
        let scene = super::three_d_viewport::r025_source(&context, 1, 0, 1.0);
        let direct = scene.direct.as_ref().unwrap();
        let origin = WorldPoint3::origin();
        let mut cold_timing = crate::three_d::PbrPreparationTiming3D::default();
        let (initial, qualified) = direct
            .prepare_frame_with_qualified_source(
                &scene,
                view,
                origin,
                32,
                32,
                None,
                Some(&mut cold_timing),
            )
            .unwrap();
        assert!(!cold_timing.direct_source_validation_reused);
        let qualified = qualified.expect("cold source must be fully qualified");
        assert_frame(
            &direct.prepare_frame(&scene, view, origin, 32, 32).unwrap(),
            &initial,
        );
        assert_frame(
            &direct
                .prepare_frame_without_depth_groups_for_test(&scene, view, origin, 32, 32)
                .unwrap(),
            &initial,
        );
        assert!(qualified.matches(&context, &scene, origin));
        assert!(qualified.matches_validated_radiance(direct, &context, &scene, origin));
        for degrees in [52.0f32, 48.0, 44.0, 48.0] {
            let changed_view = ViewDescriptor3::new(
                view.pose(),
                crate::three_d::PerspectiveProjection::try_new(
                    degrees.to_radians(),
                    0.1,
                    crate::three_d::PerspectiveFar::Finite(10.0),
                )
                .unwrap()
                .into(),
            );
            let fresh = direct
                .prepare_frame(&scene, changed_view, origin, 32, 32)
                .unwrap();
            assert_frame(
                &direct
                    .prepare_frame_without_depth_groups_for_test(
                        &scene,
                        changed_view,
                        origin,
                        32,
                        32,
                    )
                    .unwrap(),
                &fresh,
            );
            let mut timing = crate::three_d::PbrPreparationTiming3D::default();
            let (retained, replacement) = direct
                .prepare_frame_with_qualified_source(
                    &scene,
                    changed_view,
                    origin,
                    32,
                    32,
                    Some(&qualified),
                    Some(&mut timing),
                )
                .unwrap();
            assert!(
                replacement.is_none(),
                "FOV-only must reuse the exact source basis"
            );
            assert!(timing.direct_source_validation_reused);
            assert_frame(&fresh, &retained);
            assert_frame(
                &direct
                    .prepare_frame_with_full_validation_for_test(
                        &scene,
                        changed_view,
                        origin,
                        32,
                        32,
                        &qualified,
                    )
                    .unwrap(),
                &retained,
            );
        }
        // Compare the same complete frame implementation with only the clipping
        // allocator changed. The one-triangle source is a correctness fixture;
        // these paired CPU samples are not the full-scene consumer benchmark.
        for kind in 0..3 {
            let source = super::three_d_viewport::r025_source(&context, 1, kind, 1.0);
            let direct = source.direct.as_ref().unwrap();
            let (initial, basis) = direct
                .prepare_frame_with_qualified_source(&source, view, origin, 32, 32, None, None)
                .unwrap();
            let basis = basis.unwrap();
            assert_frame(
                &initial,
                &direct
                    .prepare_frame_with_allocating_clip_for_test(
                        &source, view, origin, 32, 32, None, None,
                    )
                    .unwrap(),
            );
            let mut samples = [Vec::new(), Vec::new()];
            for sample in 0..16 {
                let degrees = [52.0f32, 48.0, 44.0, 48.0][sample % 4];
                let changed_view = ViewDescriptor3::new(
                    view.pose(),
                    crate::three_d::PerspectiveProjection::try_new(
                        degrees.to_radians(),
                        0.1,
                        crate::three_d::PerspectiveFar::Finite(10.0),
                    )
                    .unwrap()
                    .into(),
                );
                let mut frames = [None, None];
                for mode in if sample % 2 == 0 { [0, 1] } else { [1, 0] } {
                    let mut timing = crate::three_d::PbrPreparationTiming3D::default();
                    let started = std::time::Instant::now();
                    let frame = if mode == 0 {
                        direct
                            .prepare_frame_with_allocating_clip_for_test(
                                &source,
                                changed_view,
                                origin,
                                32,
                                32,
                                Some(&basis),
                                Some(&mut timing),
                            )
                            .unwrap()
                    } else {
                        direct
                            .prepare_frame_with_qualified_source(
                                &source,
                                changed_view,
                                origin,
                                32,
                                32,
                                Some(&basis),
                                Some(&mut timing),
                            )
                            .map(|(frame, _)| frame)
                            .unwrap()
                    };
                    let elapsed = started.elapsed();
                    assert!(timing.direct_source_validation_reused);
                    samples[mode].push([
                        elapsed.as_nanos(),
                        timing.direct_receiver_clipping.as_nanos(),
                        timing.direct_light_other.as_nanos(),
                        timing.direct_projection_body.as_nanos(),
                    ]);
                    frames[mode] = Some(frame);
                }
                assert_frame(frames[0].as_ref().unwrap(), frames[1].as_ref().unwrap());
            }
            for (mode, rows) in samples.iter().enumerate() {
                let summaries = std::array::from_fn::<_, 4, _>(|phase| {
                    let mut values = rows.iter().map(|r| r[phase]).collect::<Vec<_>>();
                    values.sort_unstable();
                    // Sixteen observations: exact middle-pair median and the
                    // nearest-rank p95 (the final sample for this small fixture).
                    ((values[7] + values[8]) as f64 / 2.0, values[15])
                });
                std::io::Write::write_fmt(
                    &mut std::io::stderr(),
                    format_args!(
                        "R025_CLIP_CPU kind={kind} mode={mode} profile={} samples={} \
                     columns=outer_ns,receivers_ns,other_ns,body_ns \
                     rows={rows:?} median_p95={summaries:?}\n",
                        if cfg!(debug_assertions) {
                            "debug"
                        } else {
                            "release"
                        },
                        rows.len(),
                    ),
                )
                .unwrap();
            }
        }
        let reheader = |hdr, samples| MaterialSceneSource3D {
            snapshot: scene.snapshot.clone(),
            model: scene.model.clone(),
            hdr,
            samples,
            direct: scene.direct.clone(),
            indirect: scene.indirect.clone(),
            observe: false,
        };
        // HDR changes invalidate only the radiance receipt, not the immutable
        // geometry basis. Even a single numeric bit with unchanged IDs matters.
        let mut hdr_variants = Vec::new();
        for field in 0..6 {
            let mut hdr = scene.hdr.clone();
            let mut samples = scene.samples;
            match field {
                0 => hdr.identity += 1,
                1 => hdr.revision += 1,
                2 => hdr.exposure_ev = 1.0,
                3 => {
                    assert_eq!(hdr.exposure_ev.to_bits(), 0.0f32.to_bits());
                    hdr.exposure_ev = -0.0;
                }
                4 => hdr.maximum_rgb[1] = f32::from_bits(hdr.maximum_rgb[1].to_bits() + 1),
                5 => samples = 4,
                _ => unreachable!(),
            }
            hdr_variants.push(reheader(hdr, samples));
        }
        for changed in &hdr_variants {
            changed.validate(&context).unwrap();
            assert!(qualified.matches(&context, changed, origin));
            assert!(!qualified.matches_validated_radiance(direct, &context, changed, origin));
            let expected = direct
                .prepare_frame_with_full_validation_for_test(
                    changed, view, origin, 32, 32, &qualified,
                )
                .unwrap();
            let mut timing = crate::three_d::PbrPreparationTiming3D::default();
            let (actual, replacement) = direct
                .prepare_frame_with_qualified_source(
                    changed,
                    view,
                    origin,
                    32,
                    32,
                    Some(&qualified),
                    Some(&mut timing),
                )
                .unwrap();
            assert!(!timing.direct_source_validation_reused);
            assert!(
                replacement.is_none(),
                "HDR change retains compatible geometry"
            );
            assert_frame(&expected, &actual);
            let captured = Arc::new(reheader(changed.hdr.clone(), changed.samples));
            let proof = crate::three_d::pbr::ValidatedMaterialSceneSource3D::new(
                &context,
                captured.clone(),
            )
            .unwrap();
            assert!(proof.matches(&context, &captured));
            assert!(
                !proof.matches(&context, changed),
                "equal headers are not the same scene Arc"
            );
            let counts = DirectSource3D::full_validation_counts_for_test();
            let mut proven_timing = crate::three_d::PbrPreparationTiming3D::default();
            let (proven, replacement) = direct
                .prepare_frame_with_source_proof(
                    &captured,
                    view,
                    origin,
                    32,
                    32,
                    Some(&qualified),
                    Some(&proof),
                    Some(&mut proven_timing),
                )
                .unwrap();
            assert_eq!(DirectSource3D::full_validation_counts_for_test(), counts);
            assert!(proven_timing.direct_source_validation_reused);
            assert!(replacement.is_none());
            assert!(
                !qualified.matches_validated_radiance(direct, &context, &captured, origin),
                "new proof must not relabel the old HDR witness"
            );
            assert_frame(&expected, &proven);
            let (unproven, _) = direct
                .prepare_frame_with_source_proof(
                    changed,
                    view,
                    origin,
                    32,
                    32,
                    Some(&qualified),
                    Some(&proof),
                    None,
                )
                .unwrap();
            assert_eq!(
                DirectSource3D::full_validation_counts_for_test().0,
                counts.0 + 1
            );
            assert_frame(&expected, &unproven);
        }
        let mut invalid_hdr = scene.hdr.clone();
        invalid_hdr.maximum_rgb[1] = 0.0;
        let invalid = reheader(invalid_hdr, scene.samples);
        assert!(qualified.matches(&context, &invalid, origin));
        assert!(!qualified.matches_validated_radiance(direct, &context, &invalid, origin));
        let expected = direct.validate(&invalid).unwrap_err();
        assert_eq!(invalid.validate(&context).unwrap_err(), expected);
        assert_eq!(
            direct
                .prepare_frame_with_qualified_source(
                    &invalid,
                    view,
                    origin,
                    32,
                    32,
                    Some(&qualified),
                    None,
                )
                .err()
                .unwrap(),
            expected,
        );
        assert!(qualified.matches_validated_radiance(direct, &context, &scene, origin));
        // A different receiver instance must not use the scene's valid receipt.
        let other = super::three_d_viewport::r025_source(&context, 1, 0, 1.0);
        let other_direct = other.direct.as_ref().unwrap();
        assert!(!qualified.matches_validated_radiance(other_direct, &context, &scene, origin));
        assert_eq!(
            other_direct.validate(&scene).unwrap_err(),
            other_direct
                .prepare_frame_with_qualified_source(
                    &scene,
                    view,
                    origin,
                    32,
                    32,
                    Some(&qualified),
                    None,
                )
                .err()
                .unwrap(),
        );
        // A valid source basis never proves new-view receiver/near coverage.
        assert_eq!(
            direct
                .prepare_frame(&scene, zero_near, origin, 32, 32)
                .err()
                .unwrap(),
            direct
                .prepare_frame_with_qualified_source(
                    &scene,
                    zero_near,
                    origin,
                    32,
                    32,
                    Some(&qualified),
                    None,
                )
                .err()
                .unwrap(),
        );
        let recapture = |model, k, revision| {
            let captured = Arc::new(DirectSource3D {
                identity: direct.identity,
                revision,
                meters_per_scene_unit: k,
                directional: direct.directional.clone(),
                points: direct.points.clone(),
                spots: direct.spots.clone(),
                snapshot: scene.snapshot.clone(),
                model,
            });
            Arc::new(MaterialSceneSource3D {
                snapshot: scene.snapshot.clone(),
                model: captured.model.clone(),
                hdr: scene.hdr.clone(),
                samples: scene.samples,
                indirect: scene.indirect.clone(),
                direct: Some(captured),
                observe: false,
            })
        };
        let mut malformed = recapture(scene.model.clone(), 1.0, 2);
        Arc::get_mut(
            Arc::get_mut(&mut malformed)
                .unwrap()
                .direct
                .as_mut()
                .unwrap(),
        )
        .unwrap()
        .directional
        .as_mut()
        .unwrap()
        .irradiance[0] = -1.0;
        let malformed_direct = malformed.direct.as_ref().unwrap();
        assert!(!qualified.matches_validated_radiance(
            malformed_direct,
            &context,
            &malformed,
            origin,
        ));
        assert_eq!(
            malformed_direct.validate(&malformed).unwrap_err(),
            malformed_direct
                .prepare_frame_with_qualified_source(
                    &malformed,
                    view,
                    origin,
                    32,
                    32,
                    Some(&qualified),
                    None,
                )
                .err()
                .unwrap(),
        );
        let placements = vec![
            crate::three_d::ModelPlacement3::try_new(
                WorldPoint3::try_new(0.125, 0.0, 0.125).unwrap(),
                Orientation3::identity(),
                crate::three_d::PositiveScale3::try_new(1.0, 1.0, 1.0).unwrap(),
            )
            .unwrap(),
        ];
        let moved_model = context
            .admit_material_model(
                scene.snapshot.clone(),
                crate::three_d::ModelRevision::new(2),
                placements,
            )
            .unwrap();
        let foreign_context =
            Context3D::new_cpu_only_for_test(PolicyLimits3D::conservative(), u64::MAX).unwrap();
        assert!(!qualified.matches(&foreign_context, &scene, origin));
        let foreign_scene = super::three_d_viewport::r025_source(&foreign_context, 1, 0, 1.0);
        for (changed, changed_origin, owner) in [
            (recapture(moved_model, 1.0, 2), origin, &context),
            (
                scene.clone(),
                WorldPoint3::try_new(0.125, 0.25, 0.5).unwrap(),
                &context,
            ),
            (recapture(scene.model.clone(), 2.0, 2), origin, &context),
            (recapture(scene.model.clone(), 1.0, 2), origin, &context),
            (
                super::three_d_viewport::r025_source(&context, 1, 0, 1.0),
                origin,
                &context,
            ),
            (foreign_scene, origin, &foreign_context),
        ] {
            assert!(!qualified.matches(owner, &changed, changed_origin));
            let d = changed.direct.as_ref().unwrap();
            assert!(!qualified.matches_validated_radiance(d, owner, &changed, changed_origin));
            let fresh = d
                .prepare_frame(&changed, view, changed_origin, 32, 32)
                .unwrap();
            assert_frame(
                &d.prepare_frame_without_depth_groups_for_test(
                    &changed,
                    view,
                    changed_origin,
                    32,
                    32,
                )
                .unwrap(),
                &fresh,
            );
            let mut timing = crate::three_d::PbrPreparationTiming3D::default();
            let (retained, replacement) = d
                .prepare_frame_with_qualified_source(
                    &changed,
                    view,
                    changed_origin,
                    32,
                    32,
                    Some(&qualified),
                    Some(&mut timing),
                )
                .unwrap();
            assert!(!timing.direct_source_validation_reused);
            assert!(
                replacement.is_some(),
                "changed dependency must requalify complete source"
            );
            assert_frame(&fresh, &retained);
        }
        // A changed metric representation cannot use a previously finite basis.
        let invalid_origin = WorldPoint3::try_new(f64::MAX, 0.0, 0.0).unwrap();
        assert_eq!(
            direct
                .prepare_frame(&scene, view, invalid_origin, 32, 32)
                .err()
                .unwrap(),
            direct
                .prepare_frame_with_qualified_source(
                    &scene,
                    view,
                    invalid_origin,
                    32,
                    32,
                    Some(&qualified),
                    None,
                )
                .err()
                .unwrap(),
        );
    }
    let usage = context.resource_usage().unwrap();
    assert_eq!(usage.admitted_shader_modules, 0);
    assert_eq!(usage.resident_geometry_bytes, 0);
    assert_eq!(usage.resident_texture_bytes, 0);
    assert_eq!(context.target_charges_for_test(), (0, 0));
}

fn hal380_descriptor(mode: u8) -> crate::three_d::PbrGeometryDescriptor3D {
    use crate::three_d as a;
    let (render, input) =
        super::three_d_geometry::r024_abc_fixture::packet(if mode == 3 { 0 } else { mode }, false);
    let o = input.original;
    let identity = a::PbrOriginalIdentity3D {
        source: o.identity.source,
        revision: o.identity.revision,
        node: o.identity.node,
        mesh: o.identity.mesh,
        primitive: o.identity.primitive,
    };
    let frame = match &input.triangles[0].frame {
        crate::three_d::pbr::PbrFrameCertificate3D::OldCone => a::PbrFrameCertificate3D::OldCone,
        crate::three_d::pbr::PbrFrameCertificate3D::AuthoredSmooth { separator, lower } => {
            a::PbrFrameCertificate3D::AuthoredSmooth {
                separator: *separator,
                lower: *lower,
            }
        }
        crate::three_d::pbr::PbrFrameCertificate3D::SourceGeometric {
            separator, lower, ..
        } => a::PbrFrameCertificate3D::SourceGeometric {
            separator: *separator,
            lower: *lower,
            singularity: a::PbrSmoothSingularity3D::VertexInCone {
                normal_vertex: true,
                vertex: 1,
                basis: [0, 1, 0],
                count: 2,
                antiparallel: false,
            },
        },
        _ => unreachable!(),
    };
    a::PbrGeometryDescriptor3D {
        original: a::PbrOriginalGeometry3D {
            identity,
            vertices: o
                .vertices
                .iter()
                .map(|v| a::PbrOriginalVertex3D {
                    position: v.position,
                    normal: v.normal,
                    uv0: if mode == 3 { None } else { v.uv0 },
                    color: v.color,
                })
                .collect(),
            indices: o.indices,
            signed_tangents: o.signed_tangents,
            tangent_provenance: a::PbrTangentProvenance3D::Authored,
        },
        render: render
            .into_iter()
            .map(|v| a::PbrVertex3D {
                position: v.position,
                normal: v.normal,
                uv0: if mode == 3 { [0.0; 2] } else { v.uv0 },
                color: v.color,
                tangent: v.tangent,
            })
            .collect(),
        render_indices: GeometryIndices::Unindexed,
        render_vertex_sources: input.render_vertex_sources,
        triangles: Arc::from([a::PbrTriangleCertificate3D {
            original: identity,
            triangle: 0,
            certificate_identity: 901,
            certificate_revision: 1,
            frame: if mode == 3 {
                a::PbrFrameCertificate3D::NormalOnly {
                    separator: [0.0, 0.0, 1.0],
                    lower: [0.5; 3],
                }
            } else {
                frame
            },
        }]),
        mapped: mode != 3,
        maximum_certificate_bytes: 4096,
        maximum_verification_items: 16,
    }
}

fn hal380_run(
    context: &Context3D,
    input: crate::three_d::PbrGeometryDescriptor3D,
    previous: Option<&crate::three_d::PbrGeometry3D>,
) -> crate::three_d::PbrSourceResult3D<crate::three_d::PbrGeometry3D> {
    use crate::three_d::*;
    use std::sync::atomic::AtomicBool;
    let prepared = std::thread::spawn(move || {
        PbrGeometryCpuPrepared3D::prepare(input, 128 << 20, Arc::new(AtomicBool::new(false)))
    })
    .join()
    .unwrap()?;
    let (job, pending) = context.begin_pbr_geometry_cpu_admission(prepared, previous)?;
    let result = std::thread::spawn(move || job.verify(Arc::new(AtomicBool::new(false))))
        .join()
        .unwrap()?;
    context.finish_pbr_geometry_cpu_admission(pending, result, previous)
}

mod hal380_escrow {
    use super::*;
    use crate::three_d::{PbrGeometry3D, PbrGeometryCpuPrepared3D, PbrSourceError3D};
    use std::sync::{Weak, atomic::AtomicBool};

    enum WorkerState {
        Job,
        Result,
    }

    enum ContextLifetime {
        Live,
        Final,
    }

    fn cancel_token() -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(false))
    }

    fn prepare() -> PbrGeometryCpuPrepared3D {
        let input = hal380_descriptor(0);
        let cost = PbrGeometryCpuPrepared3D::cost_for_input(&input).unwrap();
        let limit = cost
            .input_cpu_bytes()
            .checked_add(cost.additional_peak_cpu_bytes())
            .unwrap();
        let visits = crate::three_d::cpu_geometry_visits_for_test();
        let prepared = PbrGeometryCpuPrepared3D::prepare(input, limit, cancel_token()).unwrap();
        assert_eq!(prepared.cpu_cost(), cost);
        assert_eq!(
            crate::three_d::cpu_geometry_visits_for_test(),
            [visits[0] + 1, visits[1] + 1, visits[2]]
        );
        prepared
    }

    fn admit(context: &Context3D, previous: Option<&PbrGeometry3D>) -> PbrGeometry3D {
        let (job, pending) = context
            .begin_pbr_geometry_cpu_admission(prepare(), previous)
            .unwrap();
        let result = job.verify(cancel_token()).unwrap();
        let geometry = context
            .finish_pbr_geometry_cpu_admission(pending, result, previous)
            .unwrap();
        let lease = context.lease_pbr_geometry(&geometry.0).unwrap();
        assert_eq!(lease.data.source.certified_frames().unwrap().len(), 1);
        geometry
    }

    fn exercise(with_predecessor: bool, worker: WorkerState, lifetime: ContextLifetime) {
        let context = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
        let previous = with_predecessor.then(|| admit(&context, None));
        let (core, record, accounting) = context
            .geometry_cpu_escrow_witness_for_test(previous.as_ref())
            .unwrap();
        let original_usage = context.resource_usage().unwrap();
        assert_eq!(
            original_usage.retained_generations,
            u32::from(with_predecessor)
        );
        assert_eq!(
            record.as_ref().map(Weak::strong_count),
            with_predecessor.then_some(1)
        );
        let (slots, bytes, release_revision) = accounting().unwrap();
        assert_eq!((slots, bytes), (0, 0));

        let (job, pending) = context
            .begin_pbr_geometry_cpu_admission(prepare(), previous.as_ref())
            .unwrap();
        let (ticket, candidate, prior, bound_predecessor) = job.escrow_witness_for_test();
        assert_eq!(
            bound_predecessor,
            previous.as_ref().map(PbrGeometry3D::resource_generation)
        );
        assert_eq!(core.strong_count(), 2);
        assert_eq!(candidate.strong_count(), 1);
        assert_eq!(ticket.strong_count(), 1);
        assert_eq!(
            prior.as_ref().map(Weak::strong_count),
            with_predecessor.then_some(2)
        );
        assert_eq!(
            record.as_ref().map(Weak::strong_count),
            with_predecessor.then_some(2)
        );
        let candidate_vertex = candidate.upgrade().unwrap()[0].clone();
        let prior_vertex = prior
            .as_ref()
            .map(|source| source.upgrade().unwrap()[0].clone());
        let charged = accounting().unwrap();
        assert_eq!((charged.0, charged.2), (1, release_revision));
        assert!(charged.1 > 0);
        let released = (0, 0, release_revision + 1);

        let verified = matches!(worker, WorkerState::Result);
        let (job, result) = match worker {
            WorkerState::Job => (Some(job), None),
            WorkerState::Result => (None, Some(job.verify(cancel_token()).unwrap())),
        };
        assert_eq!(
            prior.as_ref().map(Weak::strong_count),
            with_predecessor.then_some(if verified { 1 } else { 2 })
        );

        // Dropping Pending consumes the matching token. FINISH has no entry for a bare Result.
        match lifetime {
            ContextLifetime::Live => {
                drop(pending);
                assert_eq!(core.strong_count(), 1);
                assert_eq!(context.resource_usage().unwrap(), original_usage);
                assert_eq!(accounting(), Some(charged));
                assert_eq!(ticket.strong_count(), 1);
                assert_eq!(candidate.strong_count(), 1);
                assert_eq!(
                    record.as_ref().map(Weak::strong_count),
                    with_predecessor.then_some(2)
                );
                if let Some(previous) = &previous {
                    assert!(context.lease_pbr_geometry(&previous.0).is_ok());
                }

                drop(job);
                drop(result);
                assert!(ticket.upgrade().is_none());
                assert!(candidate.upgrade().is_none());
                assert_eq!(
                    prior.as_ref().map(Weak::strong_count),
                    with_predecessor.then_some(1)
                );
                assert_eq!(accounting(), Some(charged));
                assert_eq!(context.resource_usage().unwrap(), original_usage);
                assert_eq!(accounting(), Some(released));
                assert_eq!(
                    record.as_ref().map(Weak::strong_count),
                    with_predecessor.then_some(1)
                );
                assert_eq!(context.resource_usage().unwrap(), original_usage);
                assert_eq!(accounting(), Some(released));
            }
            ContextLifetime::Final => {
                let sibling = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
                assert!(!context.same_resource_family(&sibling));
                let (_sibling_job, _sibling_pending) = sibling
                    .begin_pbr_geometry_cpu_admission(prepare(), None)
                    .unwrap();
                let (_, _, sibling_accounting) =
                    sibling.geometry_cpu_escrow_witness_for_test(None).unwrap();
                let sibling_usage = sibling.resource_usage().unwrap();
                let sibling_charge = sibling_accounting().unwrap();
                assert_eq!(sibling_charge.0, 1);
                assert!(sibling_charge.1 > 0);

                // This real reservation owns only the accounting, so the terminal debit is readable.
                let model_reservation = context.reserve_model_state().unwrap();
                drop(context);
                assert_eq!(core.strong_count(), 1);
                assert_eq!(accounting(), Some(charged));
                drop(pending);
                assert!(core.upgrade().is_none());
                assert_eq!(accounting(), Some(released));
                assert_eq!(ticket.strong_count(), 1);
                assert_eq!(candidate.strong_count(), 1);
                assert_eq!(candidate.upgrade().unwrap()[0], candidate_vertex);
                assert_eq!(
                    record.as_ref().map(Weak::strong_count),
                    with_predecessor.then_some(0)
                );
                if let Some(prior) = &prior {
                    if verified {
                        assert!(prior.upgrade().is_none());
                    } else {
                        assert_eq!(prior.strong_count(), 1);
                        assert_eq!(prior.upgrade().unwrap()[0], prior_vertex.unwrap());
                    }
                }

                assert_eq!(sibling.resource_usage().unwrap(), sibling_usage);
                assert_eq!(sibling_accounting(), Some(sibling_charge));
                drop(job);
                drop(result);
                assert!(ticket.upgrade().is_none());
                assert!(candidate.upgrade().is_none());
                if let Some(prior) = &prior {
                    assert!(prior.upgrade().is_none());
                }
                assert_eq!(accounting(), Some(released));
                assert_eq!(sibling.resource_usage().unwrap(), sibling_usage);
                assert_eq!(sibling_accounting(), Some(sibling_charge));
                drop(model_reservation);
                assert!(accounting().is_none());
            }
        }
    }

    #[test]
    fn live_none_job() {
        exercise(false, WorkerState::Job, ContextLifetime::Live);
    }

    #[test]
    fn live_some_job() {
        exercise(true, WorkerState::Job, ContextLifetime::Live);
    }

    #[test]
    fn live_none_result() {
        exercise(false, WorkerState::Result, ContextLifetime::Live);
    }

    #[test]
    fn live_some_result() {
        exercise(true, WorkerState::Result, ContextLifetime::Live);
    }

    #[test]
    fn final_none_job() {
        exercise(false, WorkerState::Job, ContextLifetime::Final);
    }

    #[test]
    fn final_some_job() {
        exercise(true, WorkerState::Job, ContextLifetime::Final);
    }

    #[test]
    fn final_none_result() {
        exercise(false, WorkerState::Result, ContextLifetime::Final);
    }

    #[test]
    fn final_some_result() {
        exercise(true, WorkerState::Result, ContextLifetime::Final);
    }

    #[test]
    fn foreign_finish_refuses_a_matching_pair_and_preserves_current_sources() {
        let origin = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
        let foreign = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
        let previous = admit(&origin, None);
        let foreign_previous = admit(&foreign, None);
        assert_eq!(
            previous.resource_generation(),
            foreign_previous.resource_generation()
        );
        let origin_usage = origin.resource_usage().unwrap();
        let foreign_usage = foreign.resource_usage().unwrap();
        let (job, pending) = origin
            .begin_pbr_geometry_cpu_admission(prepare(), Some(&previous))
            .unwrap();
        let result = job.verify(cancel_token()).unwrap();
        assert_eq!(
            foreign
                .finish_pbr_geometry_cpu_admission(pending, result, Some(&previous))
                .unwrap_err(),
            PbrSourceError3D::Association("geometry CPU pending/result/predecessor mismatch")
        );
        assert_eq!(origin.resource_usage().unwrap(), origin_usage);
        assert_eq!(foreign.resource_usage().unwrap(), foreign_usage);
        assert!(origin.lease_pbr_geometry(&previous.0).is_ok());
        assert!(foreign.lease_pbr_geometry(&foreign_previous.0).is_ok());

        let next = admit(&origin, Some(&previous));
        let (slot, generation) = previous.resource_generation();
        assert_eq!(next.resource_generation(), (slot, generation + 1));
        assert!(origin.lease_pbr_geometry(&previous.0).is_err());
        assert_eq!(foreign.resource_usage().unwrap(), foreign_usage);
    }
}

#[test]
fn hal380_public_cpu_geometry_differential_frames_indices_and_refusals() {
    use crate::three_d::*;
    fn cpu<T: Send + Sync>() {}
    cpu::<PbrGeometryCpuPrepared3D>();
    cpu::<PbrGeometryCpuSuccessorJob3D>();
    cpu::<PbrGeometryCpuResult3D>();
    for mode in 0..4 {
        for index in 0..3 {
            let mut input = hal380_descriptor(mode);
            input.render_indices = match index {
                0 => GeometryIndices::Unindexed,
                1 => GeometryIndices::from_u16(vec![0, 1, 2]),
                _ => GeometryIndices::from_u32(vec![0, 1, 2]),
            };
            input.original.indices = input.render_indices.clone();
            let old = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
            let new = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
            let original = old.admit_pbr_geometry(input.clone(), None).unwrap();
            let split = hal380_run(&new, input.clone(), None).unwrap();
            let a = old.lease_pbr_geometry(&original.0).unwrap();
            let b = new.lease_pbr_geometry(&split.0).unwrap();
            assert_eq!(a.data.source, b.data.source, "mode{mode}/index{index}");
            let bits = |s: &crate::three_d::pbr::PbrGeometrySource3D| {
                s.certified_frames()
                    .unwrap()
                    .iter()
                    .map(|f| {
                        [
                            f.normal_lower,
                            f.tangent_lower,
                            f.cross_lower,
                            f.local_g_lower,
                            f.maximum_direction_pack_error,
                        ]
                        .map(f64::to_bits)
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(bits(&a.data.source), bits(&b.data.source));
            assert_eq!(
                old.resource_usage().unwrap().retained_cpu_descriptor_bytes,
                new.resource_usage().unwrap().retained_cpu_descriptor_bytes
            );
            let next_a = old
                .admit_pbr_geometry(input.clone(), Some(&original))
                .unwrap();
            let next_b = hal380_run(&new, input.clone(), Some(&split)).unwrap();
            assert_eq!(
                old.lease_pbr_geometry(&next_a.0).unwrap().data.source,
                new.lease_pbr_geometry(&next_b.0).unwrap().data.source
            );
            assert_eq!(hal373_fixture::settled(&new), 0);
            for corruption in 0..4 {
                let mut bad = input.clone();
                match corruption {
                    0 => Arc::make_mut(&mut bad.triangles)[0].original.source += 1,
                    1 => Arc::make_mut(&mut bad.render)[0].normal = [0.0, 0.0, 0.0],
                    2 => bad.maximum_verification_items = 1,
                    _ => Arc::make_mut(&mut bad.render_vertex_sources)[0] = 99,
                }
                let expected = old.admit_pbr_geometry(bad.clone(), None).unwrap_err();
                assert_eq!(
                    hal380_run(&new, bad, None).unwrap_err(),
                    expected,
                    "mode{mode}/corruption{corruption}"
                );
            }
        }
    }
}

#[test]
fn hal380_successor_source_revision_and_certificate_identity() {
    use crate::three_d::*;
    for case in 0..7 {
        let old = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
        let new = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
        let mut initial = hal380_descriptor(0);
        if case == 6 {
            initial.original.identity.revision = 2;
            Arc::make_mut(&mut initial.triangles)[0].original = initial.original.identity;
        }
        let a = old.admit_pbr_geometry(initial.clone(), None).unwrap();
        let b = hal380_run(&new, initial.clone(), None).unwrap();
        let mut next = initial;
        match case {
            0 => {}
            1 => {
                for v in Arc::make_mut(&mut next.original.vertices) {
                    v.position[0] += 0.125;
                }
                for v in Arc::make_mut(&mut next.render) {
                    v.position[0] += 0.125;
                }
            }
            2 => {
                next.original.identity.revision += 1;
                Arc::make_mut(&mut next.triangles)[0].original = next.original.identity;
                Arc::make_mut(&mut next.triangles)[0].certificate_revision += 1;
                for v in Arc::make_mut(&mut next.original.vertices) {
                    v.position[0] += 0.125;
                }
                for v in Arc::make_mut(&mut next.render) {
                    v.position[0] += 0.125;
                }
            }
            3 => {
                Arc::make_mut(&mut next.triangles)[0].frame =
                    PbrFrameCertificate3D::AuthoredSmooth {
                        separator: [0.0, 1.0, 0.0],
                        lower: [0.5; 9],
                    };
            }
            4 => {
                next.original.identity.source += 1;
                Arc::make_mut(&mut next.triangles)[0].original = next.original.identity;
            }
            5 => {
                next.original.tangent_provenance = PbrTangentProvenance3D::DefaultMikk {
                    producer: 1,
                    revision: 1,
                    version: [1, 0, 0],
                };
            }
            _ => {
                next.original.identity.revision = 1;
                Arc::make_mut(&mut next.triangles)[0].original = next.original.identity;
            }
        }
        let expected = old.admit_pbr_geometry(next.clone(), Some(&a));
        let actual = hal380_run(&new, next, Some(&b));
        match (expected, actual) {
            (Ok(a), Ok(b)) => assert_eq!(
                old.lease_pbr_geometry(&a.0).unwrap().data.source,
                new.lease_pbr_geometry(&b.0).unwrap().data.source
            ),
            (Err(a), Err(b)) => assert_eq!(a, b),
            (a, b) => panic!("case{case}: old{a:?}/new{b:?}"),
        }
    }
}

#[test]
fn hal380_quota_shared_allocations_cancel_and_retention_escrow() {
    use crate::three_d::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    let mut shared = hal380_descriptor(0);
    let mapping: Arc<[u32]> = Arc::from([0, 1, 2]);
    shared.original.indices = GeometryIndices::U32(mapping.clone());
    shared.render_indices = GeometryIndices::U32(mapping.clone());
    shared.render_vertex_sources = mapping;
    let mut separate = shared.clone();
    separate.original.indices = GeometryIndices::from_u32(vec![0, 1, 2]);
    separate.render_indices = GeometryIndices::from_u32(vec![0, 1, 2]);
    separate.render_vertex_sources = Arc::from([0, 1, 2]);
    let a = PbrGeometryCpuPrepared3D::prepare(shared, 1 << 20, Arc::new(AtomicBool::new(false)))
        .unwrap();
    let b = PbrGeometryCpuPrepared3D::prepare(separate, 1 << 20, Arc::new(AtomicBool::new(false)))
        .unwrap();
    assert_eq!(
        b.peak_for_test() - a.peak_for_test(),
        2 * (3 * 4 + 2 * std::mem::size_of::<usize>())
    );
    let mut policy = PolicyLimits3D::conservative();
    policy.max_retained_dynamic_cpu_bytes = 1;
    let tiny = cpu_context(policy, u64::MAX);
    assert!(matches!(
        tiny.begin_pbr_geometry_cpu_admission(a, None),
        Err(PbrSourceError3D::Context(
            Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::RetainedDynamicCpuBytes,
                ..
            }
        ))
    ));
    assert_eq!(hal373_fixture::settled(&tiny), 0);
    let context = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
    let old = context
        .admit_pbr_geometry(hal380_descriptor(0), None)
        .unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let (job, pending) = context
        .begin_pbr_geometry_cpu_admission(b, Some(&old))
        .unwrap();
    drop(pending);
    context.retire_pbr_geometry_source(&old).unwrap();
    assert!(hal373_fixture::settled(&context) > 0);
    let retained = context
        .resource_usage()
        .unwrap()
        .retained_cpu_descriptor_bytes;
    cancel.store(true, Ordering::Release);
    assert!(matches!(
        std::thread::spawn(move || job.verify(cancel))
            .join()
            .unwrap(),
        Err(PbrSourceError3D::Association(
            "geometry CPU operation cancelled"
        ))
    ));
    assert_eq!(hal373_fixture::settled(&context), 0);
    assert!(
        context
            .resource_usage()
            .unwrap()
            .retained_cpu_descriptor_bytes
            < retained
    );
    assert!(matches!(
        PbrGeometryCpuPrepared3D::prepare(
            hal380_descriptor(0),
            1,
            Arc::new(AtomicBool::new(false))
        ),
        Err(PbrSourceError3D::Context(
            Context3DError::PolicyBudgetExceeded { .. }
        ))
    ));
}

#[test]
fn hal380_binding_liveness_one_use_cancel_and_generation_exhaustion() {
    use crate::three_d::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    let context = cpu_context(PolicyLimits3D::conservative(), 2);
    let foreign = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
    let old = context
        .admit_pbr_geometry(hal380_descriptor(0), None)
        .unwrap();
    let prepare = || {
        PbrGeometryCpuPrepared3D::prepare(
            hal380_descriptor(0),
            1 << 20,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap()
    };
    let selected = Arc::new(AtomicBool::new(false));
    let obsolete =
        PbrGeometryCpuPrepared3D::prepare(hal380_descriptor(0), 1 << 20, selected.clone()).unwrap();
    selected.store(true, Ordering::Release);
    assert!(matches!(
        context.begin_pbr_geometry_cpu_admission(obsolete, None),
        Err(PbrSourceError3D::Association(
            "geometry CPU operation cancelled"
        ))
    ));
    assert!(foreign
        .begin_pbr_geometry_cpu_admission(prepare(), Some(&old))
        .is_err());
    let (a, pa) = context
        .begin_pbr_geometry_cpu_admission(prepare(), Some(&old))
        .unwrap();
    let (b, pb) = context
        .begin_pbr_geometry_cpu_admission(prepare(), Some(&old))
        .unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let ra = a.verify(cancel.clone()).unwrap();
    assert!(context
        .finish_pbr_geometry_cpu_admission(pb, ra, Some(&old))
        .is_err());
    let rb = b.verify(cancel.clone()).unwrap();
    assert!(foreign
        .finish_pbr_geometry_cpu_admission(pa, rb, Some(&old))
        .is_err());
    let (job, pending) = context
        .begin_pbr_geometry_cpu_admission(prepare(), Some(&old))
        .unwrap();
    let result = job.verify(cancel.clone()).unwrap();
    cancel.store(true, Ordering::Release);
    assert!(matches!(
        context.finish_pbr_geometry_cpu_admission(pending, result, Some(&old)),
        Err(PbrSourceError3D::Association(
            "geometry CPU operation cancelled"
        ))
    ));
    let (job, pending) = context
        .begin_pbr_geometry_cpu_admission(prepare(), Some(&old))
        .unwrap();
    let result = job.verify(Arc::new(AtomicBool::new(false))).unwrap();
    let next = context
        .admit_pbr_geometry(hal380_descriptor(0), Some(&old))
        .unwrap();
    assert!(context
        .finish_pbr_geometry_cpu_admission(pending, result, Some(&old))
        .is_err());
    assert!(matches!(
        hal380_run(&context, hal380_descriptor(0), Some(&next)),
        Err(PbrSourceError3D::Context(
            Context3DError::GenerationExhausted { .. }
        ))
    ));
    assert!(context.lease_pbr_geometry(&next.0).is_ok());
    assert_eq!(hal373_fixture::settled(&context), 0);
}

#[test]
fn hal380_large_geometry_math_runs_on_cpu_and_ui_has_zero_source_visits() {
    use crate::three_d::*;
    use std::sync::atomic::AtomicBool;
    use std::time::Instant;
    let mut input = hal380_descriptor(3);
    let triangles = 12000usize;
    let vertices = input.original.vertices.to_vec();
    let render = input.render.to_vec();
    let certificate = input.triangles[0].clone();
    input.original.vertices = (0..triangles)
        .flat_map(|_| vertices.iter().cloned())
        .collect();
    input.original.signed_tangents = Arc::from([]);
    input.render = (0..triangles)
        .flat_map(|_| render.iter().copied())
        .collect();
    input.render_vertex_sources = (0..triangles * 3).map(|i| i as u32).collect();
    input.triangles = (0..triangles)
        .map(|i| {
            let mut c = certificate.clone();
            c.triangle = i;
            c.certificate_identity += i as u64;
            c
        })
        .collect();
    input.maximum_verification_items = (13 * triangles) as u64;
    input.maximum_certificate_bytes = 64 << 20;
    let context = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
    let begin_visits = crate::three_d::cpu_geometry_visits_for_test();
    let ui = std::thread::current().id();
    let (prepared, worker, work, prepare_ms) = std::thread::spawn(move || {
        let before = crate::three_d::cpu_geometry_visits_for_test();
        let start = Instant::now();
        let p =
            PbrGeometryCpuPrepared3D::prepare(input, 128 << 20, Arc::new(AtomicBool::new(false)))
                .unwrap();
        let after = crate::three_d::cpu_geometry_visits_for_test();
        (
            p,
            std::thread::current().id(),
            [after[0] - before[0], after[1] - before[1]],
            start.elapsed().as_secs_f64() * 1000.0,
        )
    })
    .join()
    .unwrap();
    assert_ne!(ui, worker);
    assert_eq!(work, [1, 1]);
    let bytes = prepared.peak_for_test();
    let start = Instant::now();
    let (job, pending) = context
        .begin_pbr_geometry_cpu_admission(prepared, None)
        .unwrap();
    let begin_ms = start.elapsed().as_secs_f64() * 1000.0;
    let result = std::thread::spawn(move || job.verify(Arc::new(AtomicBool::new(false))).unwrap())
        .join()
        .unwrap();
    let start = Instant::now();
    let geometry = context
        .finish_pbr_geometry_cpu_admission(pending, result, None)
        .unwrap();
    let finish_ms = start.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(crate::three_d::cpu_geometry_visits_for_test(), begin_visits);
    assert_eq!(hal373_fixture::settled(&context), 0);
    assert_eq!(
        context
            .lease_pbr_geometry(&geometry.0)
            .unwrap()
            .data
            .source
            .certified_frames()
            .unwrap()
            .len(),
        triangles
    );
    std::io::Write::write_fmt(&mut std::io::stdout(),format_args!("HAL380 measured fixture triangles={triangles} ui={ui:?} worker={worker:?} kernel_calls={work:?} peak_cpu_bytes={bytes} prepare_ms={prepare_ms:.6} begin_ms={begin_ms:.6} finish_ms={finish_ms:.6} ui_geometry_visits=0; source-only CPU fixture, not native external consumer latency\n")).unwrap();
}

#[test]
fn hal380_actual_context_n1_n4_source_admission_and_device_failure() {
    use crate::three_d::*;
    use std::sync::atomic::AtomicBool;
    let renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
    let gpu = renderer.gpu_resources();
    let context = Context3D::new(gpu.clone()).unwrap();
    let geometry = hal380_run(&context, hal380_descriptor(3), None).unwrap();
    let mut material = PbrMaterialDescriptor3D::new(380, 1, PbrSamplingFamily3D::V1);
    material.emissive_rgb = [1.0; 3];
    let material = context.admit_pbr_material(material, None).unwrap();
    let snapshot = context
        .admit_pbr_snapshot(
            StructuralRevision::new(1),
            vec![PbrDrawPacket3D {
                geometry: geometry.clone(),
                material,
                raster: RasterMode::SingleSided,
                stable_ordinal: 0,
            }],
        )
        .unwrap();
    let model = context
        .admit_pbr_model(
            &snapshot,
            ModelRevision::new(1),
            vec![ModelPlacement3::try_new(
                WorldPoint3::origin(),
                Orientation3::identity(),
                PositiveScale3::one(),
            )
            .unwrap()],
        )
        .unwrap();
    for samples in [SampleCount3D::ONE, SampleCount3D::FOUR] {
        assert_eq!(
            context
                .admit_pbr_scene(PbrSceneDescriptor3D {
                    snapshot: snapshot.clone(),
                    model: model.clone(),
                    hdr: PbrHdrSource3D {
                        identity: 380,
                        revision: 1,
                        exposure_ev: 0.0,
                        maximum_rgb: [1.0; 3]
                    },
                    samples,
                    direct: None,
                    indirect: None
                })
                .unwrap()
                .samples(),
            samples.value()
        );
    }
    let prepared = PbrGeometryCpuPrepared3D::prepare(
        hal380_descriptor(3),
        1 << 20,
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    let (job, pending) = context
        .begin_pbr_geometry_cpu_admission(prepared, Some(&geometry))
        .unwrap();
    let result = job.verify(Arc::new(AtomicBool::new(false))).unwrap();
    gpu.device.destroy();
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    assert!(context
        .finish_pbr_geometry_cpu_admission(pending, result, Some(&geometry))
        .is_err());
    assert_eq!(hal373_fixture::settled(&context), 0);
}

#[test]
fn hal380_public_cpu_cost_preflight_matches_receipt_and_excludes_input() {
    use crate::three_d::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    fn scalar<T: Copy + Eq + Send + Sync>() {}
    scalar::<PbrGeometryCpuCost3D>();
    let mut input = hal380_descriptor(0);
    let shared: Arc<[u32]> = Arc::from([0, 1, 2]);
    input.original.indices = GeometryIndices::U32(shared.clone());
    input.render_indices = GeometryIndices::U32(shared.clone());
    input.render_vertex_sources = shared;
    let mut distinct = input.clone();
    distinct.original.indices = GeometryIndices::from_u32(vec![0, 1, 2]);
    distinct.render_indices = GeometryIndices::from_u32(vec![0, 1, 2]);
    distinct.render_vertex_sources = Arc::from([0, 1, 2]);
    let visits = crate::three_d::cpu_geometry_visits_for_test();
    let cost = PbrGeometryCpuPrepared3D::cost_for_input(&input).unwrap();
    let other = PbrGeometryCpuPrepared3D::cost_for_input(&distinct).unwrap();
    assert_eq!(
        crate::three_d::cpu_geometry_visits_for_test(),
        visits,
        "accounting is not source verification"
    );
    assert_eq!(
        other.input_cpu_bytes() - cost.input_cpu_bytes(),
        2 * (3 * 4 + 2 * std::mem::size_of::<usize>())
    );
    assert_eq!(
        other.additional_peak_cpu_bytes(),
        cost.additional_peak_cpu_bytes()
    );
    assert_eq!(
        other.additional_retained_cpu_bytes(),
        cost.additional_retained_cpu_bytes()
    );
    assert!(cost.additional_retained_cpu_bytes() <= cost.additional_peak_cpu_bytes());
    let total = cost
        .input_cpu_bytes()
        .checked_add(cost.additional_peak_cpu_bytes())
        .unwrap();
    assert!(
        matches!(PbrGeometryCpuPrepared3D::prepare(input.clone(), total - 1, Arc::new(AtomicBool::new(false))), Err(PbrSourceError3D::Context(Context3DError::PolicyBudgetExceeded { requested, maximum, .. })) if requested == total as u64 && maximum == total as u64 - 1)
    );
    assert_eq!(
        crate::three_d::cpu_geometry_visits_for_test(),
        visits,
        "total refusal precedes conversion/checking"
    );
    let cancel = Arc::new(AtomicBool::new(false));
    let prepared = PbrGeometryCpuPrepared3D::prepare(input, total, cancel.clone()).unwrap();
    assert_eq!(prepared.cpu_cost(), cost);
    assert_eq!(prepared.peak_for_test(), total);
    cancel.store(true, Ordering::Release);
    let context = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
    assert!(matches!(
        context.begin_pbr_geometry_cpu_admission(prepared, None),
        Err(PbrSourceError3D::Association(
            "geometry CPU operation cancelled"
        ))
    ));
    assert_eq!(hal373_fixture::settled(&context), 0);
    let mut invalid = distinct;
    Arc::make_mut(&mut invalid.triangles)[0].original.source += 1;
    assert!(PbrGeometryCpuPrepared3D::cost_for_input(&invalid).is_ok());
    assert!(matches!(
        PbrGeometryCpuPrepared3D::prepare(invalid, 1 << 20, Arc::new(AtomicBool::new(false))),
        Err(PbrSourceError3D::Frame { .. })
    ));
}

#[test]
fn hal380_one_request_budget_multiple_geometry_then_original_normal_proof() {
    use crate::three_d::*;
    use std::sync::atomic::AtomicBool;
    for count in [2usize, 4] {
        let (prepared, normal, remaining, retained) = std::thread::spawn(move || {
            let a = hal380_descriptor(0);
            let b = hal380_descriptor(3);
            assert!(!Arc::ptr_eq(&a.original.vertices, &b.original.vertices));
            let ca = PbrGeometryCpuPrepared3D::cost_for_input(&a).unwrap();
            let cb = PbrGeometryCpuPrepared3D::cost_for_input(&b).unwrap();
            let inputs: Vec<_> = (0..count)
                .map(|i| if i % 2 == 0 { a.clone() } else { b.clone() })
                .collect();
            if count == 4 {
                assert!(Arc::ptr_eq(&inputs[0].render, &inputs[2].render));
            }
            let raw_normal = hal373_fixture::input(380_900 + count as u64);
            // This is the caller's public input ledger, not private HAL geometry
            // sizes. A/B allocate disjoint input buffers; repeated descriptors
            // share those same Arcs and therefore do not duplicate the baseline.
            let p = &raw_normal.source.provenance;
            let header = 2 * std::mem::size_of::<usize>();
            let mut normal_input = std::mem::size_of_val(&raw_normal)
                + std::mem::size_of_val(raw_normal.source.as_ref())
                + header
                + std::mem::size_of_val(raw_normal.source.levels.as_ref())
                + header
                + std::mem::size_of_val(raw_normal.exceptions.as_ref())
                + header
                + raw_normal.source.texels.len()
                + header;
            if !Arc::ptr_eq(&p.original, &raw_normal.source.texels) {
                normal_input += p.original.len() + header;
            }
            let baseline = ca
                .input_cpu_bytes()
                .checked_add(cb.input_cpu_bytes())
                .unwrap()
                .checked_add(inputs.capacity() * std::mem::size_of::<PbrGeometryDescriptor3D>())
                .unwrap()
                .checked_add(normal_input)
                .unwrap();
            let maximum = 1usize << 20;
            let charged_peak = |retained: usize, additional: usize| {
                baseline
                    .checked_add(retained)
                    .and_then(|n| n.checked_add(additional))
            };
            // Test consumer of the selected aggregate rule, using only public
            // receipts. It must refuse before entering HAL preparation.
            let prepare_next = |input: PbrGeometryDescriptor3D, retained: &mut usize, maximum: usize, cancel: Arc<AtomicBool>| -> PbrSourceResult3D<PbrGeometryCpuPrepared3D> {
                let cost = PbrGeometryCpuPrepared3D::cost_for_input(&input)?;
                let peak = charged_peak(*retained, cost.additional_peak_cpu_bytes())
                    .ok_or(PbrSourceError3D::Association("request CPU size overflow"))?;
                if peak > maximum {
                    return Err(Context3DError::PolicyBudgetExceeded { budget: PolicyBudget3D::RetainedDynamicCpuBytes, requested: peak as u64, maximum: maximum as u64 }.into());
                }
                let output = PbrGeometryCpuPrepared3D::prepare(input, maximum, cancel)?;
                assert_eq!(output.cpu_cost(), cost);
                *retained = retained.checked_add(output.cpu_cost().additional_retained_cpu_bytes()).unwrap();
                Ok(output)
            };
            let mut retained = 0usize;
            let mut outputs = Vec::with_capacity(count);
            for (ordinal, input) in inputs.into_iter().enumerate() {
                let cost = PbrGeometryCpuPrepared3D::cost_for_input(&input).unwrap();
                let next_peak = charged_peak(retained, cost.additional_peak_cpu_bytes()).unwrap();
                assert!(next_peak <= maximum);
                if ordinal != 0 {
                    let insufficient = next_peak - 1;
                    assert!(cost.input_cpu_bytes().checked_add(cost.additional_peak_cpu_bytes()).unwrap() <= insufficient,
                        "the primitive alone would pass HAL's total guard; aggregate admission must still refuse");
                    let visits = crate::three_d::cpu_geometry_visits_for_test();
                    let before = retained;
                    assert!(matches!(prepare_next(input.clone(), &mut retained, insufficient, Arc::new(AtomicBool::new(false))),
                        Err(PbrSourceError3D::Context(Context3DError::PolicyBudgetExceeded { requested, maximum, .. })) if requested == next_peak as u64 && maximum == insufficient as u64));
                    assert!(matches!(prepare_next(input.clone(), &mut retained, maximum, Arc::new(AtomicBool::new(true))),
                        Err(PbrSourceError3D::Association("geometry CPU operation cancelled"))));
                    assert_eq!(retained, before, "refusal/cancellation never produces a retained debit");
                    assert_eq!(crate::three_d::cpu_geometry_visits_for_test(), visits);
                }
                // HAL independently checks its total, while this request ledger
                // reserves the additional peak over all earlier retained output.
                let output = prepare_next(input, &mut retained, maximum, Arc::new(AtomicBool::new(false))).unwrap();
                outputs.push(output);
            }
            let mut overflow = usize::MAX;
            assert!(matches!(prepare_next(a.clone(), &mut overflow, maximum, Arc::new(AtomicBool::new(false))),
                Err(PbrSourceError3D::Association("request CPU size overflow"))));
            assert_eq!(overflow, usize::MAX);
            let remaining = maximum
                .checked_sub(baseline)
                .unwrap()
                .checked_sub(retained)
                .unwrap();
            // No second quota: the existing normal preparation receives exactly
            // the same request's remainder after ALL geometry preparation.
            let normal = PbrNormalCpuProofPrepared3D::prepare(
                raw_normal,
                remaining as u64,
                &AtomicBool::new(false),
            )
            .unwrap();
            assert!(normal.storage_for_test().1 <= remaining as u64);
            (outputs, normal, remaining, retained)
        })
        .join()
        .unwrap();
        assert_eq!(prepared.len(), count);
        assert_eq!(
            prepared
                .iter()
                .map(|p| p.cpu_cost().additional_retained_cpu_bytes())
                .sum::<usize>(),
            retained
        );
        assert!(remaining > 0);
        let context = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
        let policy = PbrMipSamplerDescriptor3D {
            identity: 380_901 + count as u64,
            revision: 1,
            image_identity: 380_900 + count as u64,
            image_revision: 1,
            u: SamplerAddressMode::ClampToEdge,
            v: SamplerAddressMode::ClampToEdge,
            mag: SamplerFilterMode::Linear,
            min: PbrMipMin3D::LinearMipLinear,
            extent: [2, 2],
        };
        let sampler = context.admit_pbr_mip_sampler(policy, None).unwrap();
        let (job, pending) = context
            .begin_pbr_normal_cpu_proof(&normal, &sampler, 1.0, None)
            .unwrap();
        let result = hal373_fixture::verify(job);
        let _normal = context
            .finish_pbr_normal_cpu_proof(pending, result, &sampler, None)
            .unwrap();
        let mut geometry = Vec::new();
        for prepared in prepared {
            let (job, pending) = context
                .begin_pbr_geometry_cpu_admission(prepared, None)
                .unwrap();
            let result = std::thread::spawn(move || job.verify(Arc::new(AtomicBool::new(false))))
                .join()
                .unwrap()
                .unwrap();
            geometry.push(
                context
                    .finish_pbr_geometry_cpu_admission(pending, result, None)
                    .unwrap(),
            );
        }
        assert_eq!(geometry.len(), count);
        assert_eq!(hal373_fixture::settled(&context), 0);
    }
}

fn local_vector(x: f32, y: f32, z: f32) -> LocalVector3 {
    LocalVector3::try_new(x, y, z).unwrap()
}

fn triangle(offset: f32) -> GeometrySourceDescriptor {
    GeometrySourceDescriptor::try_new(
        vec![
            local_point(offset, 0.0, 0.0),
            local_point(offset + 1.0, 0.0, 0.0),
            local_point(offset, 1.0, 0.0),
        ],
        None,
        None,
        None,
        GeometryIndices::Unindexed,
    )
    .unwrap()
}

fn appearance(source: &str) -> AppearanceSourceDescriptor {
    AppearanceSourceDescriptor::try_new(source, GeometrySemantics::NONE).unwrap()
}

fn cpu_context(policy: PolicyLimits3D, generation_ceiling: u64) -> Context3D {
    Context3D::new_cpu_only_for_test(policy, generation_ceiling).unwrap()
}

mod hal373_fixture {
    use super::*;
    use crate::three_d::*;
    use std::sync::Arc;

    pub fn input(image: u64) -> PbrNormalCpuProofSource3D {
        let source = super::r028_normal_fixture::constant(image, 2, 2);
        let policy = PbrMipSamplerDescriptor3D {
            identity: image + 1,
            revision: 1,
            image_identity: image,
            image_revision: 1,
            u: SamplerAddressMode::ClampToEdge,
            v: SamplerAddressMode::ClampToEdge,
            mag: SamplerFilterMode::Linear,
            min: PbrMipMin3D::LinearMipLinear,
            extent: [2, 2],
        };
        let cells = super::r028_normal_fixture::cells(&source, policy.into());
        let provenance = &source.provenance;
        PbrNormalCpuProofSource3D {
            identity: image + 2,
            revision: 1,
            source: Arc::new(PbrMipChainDescriptor3D {
                role: PbrMipRole3D::Normal,
                format: source.format,
                levels: source
                    .levels
                    .iter()
                    .map(|l| PbrMipLevel3D {
                        width: l.width,
                        height: l.height,
                        offset: l.offset,
                        bytes: l.bytes,
                    })
                    .collect::<Vec<_>>()
                    .into(),
                texels: source.texels.clone(),
                provenance: PbrMipProvenance3D {
                    image_identity: provenance.image_identity,
                    content_revision: provenance.content_revision,
                    producer_identity: provenance.producer_identity,
                    producer_revision: provenance.producer_revision,
                    generator_identity: provenance.generator_identity,
                    generator_revision: provenance.generator_revision,
                    generator: PbrMipGenerator3D::RecursiveStoredAreaR028V1,
                    origin: PbrImageOrigin3D::TopLeft,
                    uv_set: provenance.uv_set,
                    work: provenance.work,
                    error_bound: provenance.error_bound,
                    original: provenance.original.clone(),
                    derived: provenance.derived.clone(),
                },
            }),
            policy,
            normal_scale_bits: 1.0f32.to_bits(),
            cells,
            maximum_work: 1_000_000,
            maximum_metadata_bytes: 65_536,
            default: PbrNormalWitness3D {
                direction: [1.0, 0.0, 0.0],
                lower: 0.003,
            },
            exceptions: Arc::from([]),
        }
    }
    pub fn legacy(
        raw: &PbrNormalCpuProofSource3D,
        sampler: &PbrMipSampler3D,
    ) -> PbrNormalCertificate3D {
        PbrNormalCertificate3D {
            identity: raw.identity,
            revision: raw.revision,
            source: raw.source.clone(),
            sampler: sampler.clone(),
            policy: raw.policy,
            normal_scale_bits: raw.normal_scale_bits,
            cells: raw.cells,
            maximum_work: raw.maximum_work,
            maximum_metadata_bytes: raw.maximum_metadata_bytes,
            default: raw.default,
            exceptions: raw.exceptions.clone(),
        }
    }
    // Independent copy of the pre-refactor canonical byte ordering. This is an
    // oracle in tests, never a producer credential or a production checker.
    pub fn original_digest(raw: &PbrNormalCpuProofSource3D, sampler: &PbrMipSampler3D) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(b"R028 common-UV domain corners v1");
        let p = &raw.source.provenance;
        let (slot, generation) = sampler.resource_generation();
        for n in [
            raw.identity,
            raw.revision,
            p.image_identity,
            p.content_revision,
            p.producer_identity,
            p.producer_revision,
            p.generator_identity,
            p.generator_revision,
            p.work,
            p.error_bound.to_bits(),
            u64::from(p.uv_set),
            u64::from(slot),
            generation,
            raw.policy.identity,
            raw.policy.revision,
            raw.policy.image_identity,
            raw.policy.image_revision,
            u64::from(raw.policy.extent[0]),
            u64::from(raw.policy.extent[1]),
            raw.policy.u as u64,
            raw.policy.v as u64,
            raw.policy.mag as u64,
            raw.policy.min as u64,
            u64::from(raw.normal_scale_bits),
            raw.cells,
            raw.maximum_work,
            raw.maximum_metadata_bytes,
        ] {
            h.update(n.to_le_bytes());
        }
        for level in raw.source.levels.iter() {
            for n in [
                u64::from(level.width),
                u64::from(level.height),
                level.offset,
                level.bytes,
            ] {
                h.update(n.to_le_bytes());
            }
        }
        h.update(&p.original);
        h.update(&raw.source.texels);
        let witness = |h: &mut Sha256, w: PbrNormalWitness3D| {
            for x in w.direction.into_iter().chain([w.lower]) {
                h.update(x.to_bits().to_le_bytes());
            }
        };
        witness(&mut h, raw.default);
        h.update((raw.exceptions.len() as u64).to_le_bytes());
        for e in raw.exceptions.iter() {
            let c = e.cell;
            for x in [
                c.mag as u64,
                c.linear as u64,
                u64::from(c.lower),
                u64::from(c.upper),
                u64::from(c.u),
                u64::from(c.v),
                u64::from(c.u_choice),
                u64::from(c.v_choice),
                e.leaves.len() as u64,
            ] {
                h.update(x.to_le_bytes());
            }
            for leaf in e.leaves.iter() {
                h.update(leaf.path.to_le_bytes());
                h.update([leaf.depth]);
                witness(&mut h, leaf.witness);
            }
        }
        h.finalize().into()
    }
    pub fn prepared(raw: PbrNormalCpuProofSource3D) -> PbrNormalCpuProofPrepared3D {
        PbrNormalCpuProofPrepared3D::prepare(
            raw,
            8 * 1024 * 1024,
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap()
    }
    pub fn verify(job: PbrNormalCpuProofJob3D) -> PbrNormalCpuProofResult3D {
        std::thread::spawn(move || job.verify(Arc::new(std::sync::atomic::AtomicBool::new(false))))
            .join()
            .unwrap()
            .unwrap()
    }
    pub fn settled(context: &Context3D) -> u64 {
        context.resource_usage().unwrap(); // Existing owner collects settled retirements.
        context
            .dynamic_state_accounting_for_test()
            .unwrap()
            .retained_cpu_bytes
    }
}

#[test]
fn hal373_cpu_normal_source_full_handoff_matches_original() {
    use crate::three_d::*;
    use hal373_fixture as f;
    fn cpu_only<T: Send + Sync>() {}
    cpu_only::<PbrNormalCpuProofSource3D>();
    cpu_only::<PbrNormalCpuProofPrepared3D>();
    cpu_only::<PbrNormalCpuProofJob3D>();
    cpu_only::<PbrNormalCpuProofResult3D>();
    cpu_only::<PbrNormalCpuProofError3D>();
    let context = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
    let raw = f::input(373001);
    let sampler = context.admit_pbr_mip_sampler(raw.policy, None).unwrap();
    let original = context
        .admit_pbr_certified_normal_chain(
            raw.source.clone(),
            &sampler,
            1.0,
            f::legacy(&raw, &sampler),
            None,
        )
        .unwrap();
    let expected = context.normal_cpu_chain_for_test(&original);
    let golden = f::original_digest(&raw, &sampler);
    assert_eq!(expected.1, golden);
    assert!(expected.3, "the fixture really contains negative blue");
    let prepared = f::prepared(raw);
    let before = context.resource_usage().unwrap();
    let started = std::time::Instant::now();
    let (job, pending) = context
        .begin_pbr_normal_cpu_proof(&prepared, &sampler, 1.0, None)
        .unwrap();
    let begin_time = started.elapsed();
    assert_eq!(
        before,
        context.resource_usage().unwrap(),
        "pending is not a registered source"
    );
    assert!(f::settled(&context) >= prepared.storage_for_test().1);
    let result = f::verify(job);
    let observed = result.observation_for_test();
    assert_eq!(
        observed.0, expected.0,
        "all integer/f64 summary bits and work"
    );
    assert_eq!(observed.1, golden, "original canonical byte stream");
    let started = std::time::Instant::now();
    let chain = context
        .finish_pbr_normal_cpu_proof(pending, result, &sampler, None)
        .unwrap();
    let finish_time = started.elapsed();
    let admitted = context.normal_cpu_chain_for_test(&chain);
    assert_eq!(
        (admitted.0, admitted.1, admitted.3),
        (expected.0, expected.1, expected.3)
    );
    assert_eq!(
        admitted.2, observed.2,
        "the actual private captures survived finish"
    );
    assert_eq!(f::settled(&context), 0);
    eprintln!(
        "HAL373 CPU fixture UI begin={begin_time:?} finish={finish_time:?}; no native responsiveness inference"
    );
}

#[test]
fn hal373_cpu_normal_source_nested_size_seal_and_quota() {
    use crate::three_d::*;
    use hal373_fixture as f;
    use std::sync::Arc;
    let mut raw = f::input(373010);
    raw.maximum_metadata_bytes = 0; // Untrusted claim is NOT the size seal.
    let leaf = PbrNormalBox3D {
        path: 0,
        depth: 0,
        witness: raw.default,
    };
    let cell = PbrNormalCell3D {
        mag: true,
        linear: true,
        lower: 0,
        upper: 0,
        u: 0,
        v: 0,
        u_choice: 0,
        v_choice: 0,
    };
    let shared: Arc<[PbrNormalBox3D]> = vec![leaf; 8].into();
    raw.exceptions = vec![
        PbrNormalException3D {
            cell,
            leaves: shared.clone(),
        },
        PbrNormalException3D {
            cell,
            leaves: shared.clone(),
        },
    ]
    .into();
    let same = f::prepared(raw.clone());
    let mut distinct = raw.clone();
    distinct.exceptions = vec![
        PbrNormalException3D {
            cell,
            leaves: shared.clone(),
        },
        PbrNormalException3D {
            cell,
            leaves: vec![leaf; 8].into(),
        },
    ]
    .into();
    let different = f::prepared(distinct);
    let allocation = 8 * std::mem::size_of::<PbrNormalBox3D>() + 2 * std::mem::size_of::<usize>();
    assert_eq!(
        different.storage_for_test().0 - same.storage_for_test().0,
        allocation as u64
    );
    assert_eq!(
        different.storage_for_test().1 - same.storage_for_test().1,
        allocation as u64
    );
    let limit = different.storage_for_test().1 * 2;
    let mut expanded = raw;
    let huge = vec![leaf; limit as usize / std::mem::size_of::<PbrNormalBox3D>() + 1];
    expanded.exceptions = vec![
        PbrNormalException3D {
            cell,
            leaves: shared,
        },
        PbrNormalException3D {
            cell,
            leaves: huge.into(),
        },
    ]
    .into();
    let error = PbrNormalCpuProofPrepared3D::prepare(
        expanded,
        limit,
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap_err();
    assert_eq!(error.kind(), PbrNormalCpuProofErrorKind3D::Capacity);
    assert_eq!(error.work(), 0);
    // A malformed producer Weak still holds its allocation after the last
    // strong reference dies. Size qualification precedes mathematical refusal.
    let mut weak_raw = f::input(373012);
    let ordinary = f::prepared(weak_raw.clone());
    let foreign: Arc<[u8]> = vec![0; 4096].into();
    let mut source = (*weak_raw.source).clone();
    source.provenance.derived = Arc::downgrade(&foreign);
    weak_raw.source = Arc::new(source);
    drop(foreign);
    let foreign_weak = f::prepared(weak_raw.clone());
    assert_eq!(
        foreign_weak.storage_for_test().0 - ordinary.storage_for_test().0,
        4096 + (2 * std::mem::size_of::<usize>()) as u64
    );
    let error = PbrNormalCpuProofPrepared3D::prepare(
        weak_raw,
        ordinary.storage_for_test().1,
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap_err();
    assert_eq!(error.kind(), PbrNormalCpuProofErrorKind3D::Capacity);
    let mut policy = PolicyLimits3D::conservative();
    policy.max_retained_dynamic_cpu_bytes = 1;
    let context = cpu_context(policy, u64::MAX);
    let raw = f::input(373011);
    let sampler = context.admit_pbr_mip_sampler(raw.policy, None).unwrap();
    let prepared = f::prepared(raw);
    let captures = prepared.storage_for_test().2;
    let error = match context.begin_pbr_normal_cpu_proof(&prepared, &sampler, 1.0, None) {
        Err(e) => e,
        Ok(_) => panic!("unqualified source custody"),
    };
    assert_eq!(error.kind(), PbrNormalCpuProofErrorKind3D::Capacity);
    assert_eq!(
        prepared.storage_for_test().2,
        captures,
        "refusal did not capture/destruct input"
    );
    assert_eq!(f::settled(&context), 0);
}

#[test]
fn hal373_cpu_normal_source_first_refusals_and_work() {
    use crate::three_d::*;
    use hal373_fixture as f;
    use std::sync::{Arc, atomic::AtomicBool};
    for case in 0..15 {
        let context = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
        let mut raw = f::input(373020 + case);
        match case {
            0 => raw.maximum_work = 0,
            1 => raw.maximum_metadata_bytes = 0,
            2 => raw.default.lower = 2.0,
            3 => raw.cells = 0,
            4 => raw.default.direction = [0.0; 3],
            5 => {
                let mut source = raw.source.as_ref().clone();
                source.provenance.producer_revision = 0;
                raw.source = Arc::new(source);
            }
            6 => raw.normal_scale_bits = 0.5f32.to_bits(),
            7 => {
                raw.default.direction = [0.0; 3];
                raw.maximum_work = 0;
            }
            8 => {
                let mut source = raw.source.as_ref().clone();
                source.provenance.original = vec![0; 16].into();
                raw.source = Arc::new(source);
            }
            9 => {
                let mut source = raw.source.as_ref().clone();
                let mut bytes = source.texels.to_vec();
                bytes[16] ^= 1; // Corrupt only the stored child first moment.
                source.texels = bytes.into();
                source.provenance.derived = Arc::downgrade(&source.texels);
                raw.source = Arc::new(source);
            }
            10 | 11 => {
                let cell = PbrNormalCell3D {
                    mag: false,
                    linear: true,
                    lower: 0,
                    upper: 1,
                    u: 0,
                    v: 0,
                    u_choice: 0,
                    v_choice: 0,
                };
                let leaf = PbrNormalBox3D {
                    path: 0,
                    depth: if case == 11 { 1 } else { 0 },
                    witness: raw.default,
                };
                let exception = PbrNormalException3D {
                    cell,
                    leaves: vec![leaf].into(),
                };
                raw.exceptions = if case == 10 {
                    vec![exception.clone(), exception].into()
                } else {
                    vec![exception].into()
                };
            }
            12 => {
                let mut source = raw.source.as_ref().clone();
                let mut levels = source.levels.to_vec();
                levels[0].width = 0;
                source.levels = levels.into();
                raw.source = Arc::new(source);
            }
            13 => {
                raw.maximum_metadata_bytes = 0;
                raw.default.direction = [f64::NAN; 3];
            }
            14 => raw.policy.mag = SamplerFilterMode::Nearest,
            _ => unreachable!(),
        }
        let mut actual_policy = raw.policy;
        if case == 14 {
            actual_policy.mag = SamplerFilterMode::Linear;
        }
        let sampler = context.admit_pbr_mip_sampler(actual_policy, None).unwrap();
        let expected = context
            .admit_pbr_certified_normal_chain(
                raw.source.clone(),
                &sampler,
                1.0,
                f::legacy(&raw, &sampler),
                None,
            )
            .unwrap_err();
        let prepared = f::prepared(raw);
        let (job, pending) = context
            .begin_pbr_normal_cpu_proof(&prepared, &sampler, 1.0, None)
            .unwrap();
        let error = std::thread::spawn(move || job.verify(Arc::new(AtomicBool::new(false))))
            .join()
            .unwrap()
            .unwrap_err();
        assert_eq!(
            error.source_error(),
            Some(&expected),
            "first refusal case{case}"
        );
        if case == 0 || case == 7 {
            assert_eq!(error.work(), 20, "original failing first debit");
        }
        drop(pending);
        assert_eq!(f::settled(&context), 0);
    }
}

#[test]
fn hal373_cpu_normal_source_predecessor_and_stale_binding() {
    use crate::three_d::*;
    use hal373_fixture as f;
    let context = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
    let raw = f::input(373040);
    let sampler = context.admit_pbr_mip_sampler(raw.policy, None).unwrap();
    let prepared = f::prepared(raw.clone());
    let (job, pending) = context
        .begin_pbr_normal_cpu_proof(&prepared, &sampler, 1.0, None)
        .unwrap();
    let first = context
        .finish_pbr_normal_cpu_proof(pending, f::verify(job), &sampler, None)
        .unwrap();
    let old = context.normal_cpu_chain_for_test(&first);
    let mut next_raw = raw;
    next_raw.revision += 1;
    let next = f::prepared(next_raw.clone());
    let (job, pending) = context
        .begin_pbr_normal_cpu_proof(&next, &sampler, 1.0, Some(&first))
        .unwrap();
    let result = f::verify(job);
    assert_eq!(
        result.observation_for_test().2,
        old.2,
        "unchanged source reuses exact private levels/provenance"
    );
    let second = context
        .finish_pbr_normal_cpu_proof(pending, result, &sampler, Some(&first))
        .unwrap();
    assert_eq!(
        second.resource_generation(),
        (
            first.resource_generation().0,
            first.resource_generation().1 + 1
        )
    );
    next_raw.revision += 1;
    let next = f::prepared(next_raw);
    let (job, pending) = context
        .begin_pbr_normal_cpu_proof(&next, &sampler, 1.0, Some(&second))
        .unwrap();
    let result = f::verify(job);
    context.retire_pbr_mip_chain(&second).unwrap();
    let error = context
        .finish_pbr_normal_cpu_proof(pending, result, &sampler, Some(&second))
        .unwrap_err();
    assert_eq!(error.kind(), PbrNormalCpuProofErrorKind3D::Binding);
    assert!(matches!(
        error.source_error(),
        Some(PbrSourceError3D::Stale { .. })
    ));
    assert!(
        f::settled(&context) > 0,
        "failed ready payload remains charged until CPU cleanup"
    );
    std::thread::spawn(move || drop(error)).join().unwrap();
    assert_eq!(f::settled(&context), 0);
}

#[test]
fn hal373_cpu_normal_source_cancellation_and_pending_lifetime() {
    use crate::three_d::*;
    use hal373_fixture as f;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let raw = f::input(373050);
    let cancelled = AtomicBool::new(true);
    assert_eq!(
        PbrNormalCpuProofPrepared3D::prepare(raw.clone(), 8 * 1024 * 1024, &cancelled)
            .unwrap_err()
            .kind(),
        PbrNormalCpuProofErrorKind3D::Cancelled
    );
    let context = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
    let sampler = context.admit_pbr_mip_sampler(raw.policy, None).unwrap();
    let prepared = f::prepared(raw);
    let (job, pending) = context
        .begin_pbr_normal_cpu_proof(&prepared, &sampler, 1.0, None)
        .unwrap();
    let charged = f::settled(&context);
    drop(pending); // Actual unmount/cancel custodian drop while CPU job is live.
    assert_eq!(f::settled(&context), charged, "no early charge release");
    let error = std::thread::spawn(move || job.verify(Arc::new(AtomicBool::new(true))))
        .join()
        .unwrap()
        .unwrap_err();
    assert_eq!(error.kind(), PbrNormalCpuProofErrorKind3D::Cancelled);
    assert_eq!(f::settled(&context), 0);
    let (job, pending) = context
        .begin_pbr_normal_cpu_proof(&prepared, &sampler, 1.0, None)
        .unwrap();
    let flag = Arc::new(AtomicBool::new(false));
    let result = job.verify(flag.clone()).unwrap();
    flag.store(true, Ordering::Release); // New selection after READY, before finish.
    let before = context.resource_usage().unwrap();
    let error = context
        .finish_pbr_normal_cpu_proof(pending, result, &sampler, None)
        .unwrap_err();
    assert_eq!(error.kind(), PbrNormalCpuProofErrorKind3D::Cancelled);
    assert_eq!(context.resource_usage().unwrap(), before);
    assert!(f::settled(&context) > 0);
    std::thread::spawn(move || drop(error)).join().unwrap();
    assert_eq!(f::settled(&context), 0);
}

#[test]
fn hal373_cpu_normal_source_foreign_and_mismatched_pair() {
    use crate::three_d::*;
    use hal373_fixture as f;
    let a = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
    let b = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
    let raw = f::input(373060);
    let sampler = a.admit_pbr_mip_sampler(raw.policy, None).unwrap();
    let foreign = b.admit_pbr_mip_sampler(raw.policy, None).unwrap();
    assert_eq!(
        sampler.resource_generation(),
        foreign.resource_generation(),
        "same scalar key is insufficient"
    );
    let prepared = f::prepared(raw);
    let error = match a.begin_pbr_normal_cpu_proof(&prepared, &foreign, 1.0, None) {
        Err(e) => e,
        Ok(_) => panic!("foreign Context accepted"),
    };
    assert!(matches!(
        error.source_error(),
        Some(PbrSourceError3D::Context(Context3DError::ContextMismatch))
    ));
    let (job_a, pending_a) = a
        .begin_pbr_normal_cpu_proof(&prepared, &sampler, 1.0, None)
        .unwrap();
    let (job_b, pending_b) = a
        .begin_pbr_normal_cpu_proof(&prepared, &sampler, 1.0, None)
        .unwrap();
    let result_a = f::verify(job_a);
    let result_b = f::verify(job_b);
    let before = a.resource_usage().unwrap();
    let error = a
        .finish_pbr_normal_cpu_proof(pending_a, result_b, &sampler, None)
        .unwrap_err();
    assert_eq!(error.kind(), PbrNormalCpuProofErrorKind3D::Binding);
    assert_eq!(a.resource_usage().unwrap(), before);
    drop(pending_b);
    std::thread::spawn(move || {
        drop(error);
        drop(result_a);
    })
    .join()
    .unwrap();
    assert_eq!(f::settled(&a), 0);
}

#[test]
fn hal373_cpu_normal_source_changed_capture_and_retired_sampler() {
    use crate::three_d::*;
    use hal373_fixture as f;
    use std::sync::Arc;
    let context = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
    let raw = f::input(373070);
    let sampler = context.admit_pbr_mip_sampler(raw.policy, None).unwrap();
    let prepared = f::prepared(raw.clone());
    let (job, pending) = context
        .begin_pbr_normal_cpu_proof(&prepared, &sampler, 1.0, None)
        .unwrap();
    let first = context
        .finish_pbr_normal_cpu_proof(pending, f::verify(job), &sampler, None)
        .unwrap();
    let old = context.normal_cpu_chain_for_test(&first);

    let mut changed = raw.clone();
    changed.revision = 2;
    let mut source = raw.source.as_ref().clone();
    source.texels = source.texels.to_vec().into(); // Equal bytes, different producer allocation.
    source.provenance.derived = Arc::downgrade(&source.texels);
    changed.source = Arc::new(source);
    let expected = context
        .admit_pbr_certified_normal_chain(
            changed.source.clone(),
            &sampler,
            1.0,
            f::legacy(&changed, &sampler),
            Some(&first),
        )
        .unwrap_err();
    let prepared_changed = f::prepared(changed.clone());
    let (job, pending) = context
        .begin_pbr_normal_cpu_proof(&prepared_changed, &sampler, 1.0, Some(&first))
        .unwrap();
    let result = f::verify(job);
    assert_ne!(result.observation_for_test().2[0], old.2[0]);
    assert_ne!(result.observation_for_test().2[2], old.2[2]);
    let before = context.resource_usage().unwrap();
    let error = context
        .finish_pbr_normal_cpu_proof(pending, result, &sampler, Some(&first))
        .unwrap_err();
    assert_eq!(error.source_error(), Some(&expected));
    assert_eq!(
        context.resource_usage().unwrap(),
        before,
        "refusal leaves current generation intact"
    );
    std::thread::spawn(move || drop(error)).join().unwrap();
    assert_eq!(f::settled(&context), 0);

    let mut source = changed.source.as_ref().clone();
    source.provenance.content_revision = 2;
    source.provenance.producer_revision = 2;
    changed.source = Arc::new(source);
    changed.policy.revision = 2;
    changed.policy.image_revision = 2;
    let next_sampler = context.admit_pbr_mip_sampler(changed.policy, None).unwrap();
    let next = f::prepared(changed);
    let (job, pending) = context
        .begin_pbr_normal_cpu_proof(&next, &next_sampler, 1.0, Some(&first))
        .unwrap();
    let result = f::verify(job);
    let next_chain = context
        .finish_pbr_normal_cpu_proof(pending, result, &next_sampler, Some(&first))
        .unwrap();
    assert_eq!(
        next_chain.resource_generation(),
        (
            first.resource_generation().0,
            first.resource_generation().1 + 1
        )
    );

    let (job, pending) = context
        .begin_pbr_normal_cpu_proof(&next, &next_sampler, 1.0, None)
        .unwrap();
    let result = f::verify(job);
    context.retire_pbr_mip_sampler(&next_sampler).unwrap();
    let error = context
        .finish_pbr_normal_cpu_proof(pending, result, &next_sampler, None)
        .unwrap_err();
    assert!(matches!(
        error.source_error(),
        Some(PbrSourceError3D::Stale { .. })
    ));
    std::thread::spawn(move || drop(error)).join().unwrap();
    assert_eq!(f::settled(&context), 0);
}

#[test]
fn hal373_cpu_normal_source_device_failure_is_live_at_finish() {
    use crate::three_d::*;
    use hal373_fixture as f;
    // A dedicated real Device, never a shared test Device or simulated loss.
    // This is a headless owner test, not App::run/native presentation evidence.
    let renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("HAL373 Device qualification requires an actual supported Device");
    let gpu = renderer.gpu_resources();
    let context = Context3D::new(gpu.clone()).unwrap();
    let raw = f::input(373080);
    let sampler = context.admit_pbr_mip_sampler(raw.policy, None).unwrap();
    let prepared = f::prepared(raw);
    let (job, pending) = context
        .begin_pbr_normal_cpu_proof(&prepared, &sampler, 1.0, None)
        .unwrap();
    let result = f::verify(job);
    gpu.device.destroy();
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    assert!(
        context.device_failure().is_some(),
        "actual renderer-owned callback observed loss"
    );
    let before = context.resource_usage().unwrap();
    let error = context
        .finish_pbr_normal_cpu_proof(pending, result, &sampler, None)
        .unwrap_err();
    assert!(matches!(
        error.source_error(),
        Some(PbrSourceError3D::Gpu(_))
    ));
    assert_eq!(context.resource_usage().unwrap(), before);
    std::thread::spawn(move || drop(error)).join().unwrap();
    assert_eq!(f::settled(&context), 0);
    let captures = prepared.storage_for_test().2;
    let error = match context.begin_pbr_normal_cpu_proof(&prepared, &sampler, 1.0, None) {
        Err(e) => e,
        Ok(_) => panic!("failed Device accepted a new source job"),
    };
    assert!(matches!(
        error.source_error(),
        Some(PbrSourceError3D::Gpu(_))
    ));
    assert_eq!(prepared.storage_for_test().2, captures);
}

#[test]
fn r024_typed_material_roles_defaults_generations_and_shared_slots_are_qualified() {
    use crate::three_d::pbr::*;
    let context = cpu_context(PolicyLimits3D::conservative(), 2);
    assert_eq!(context.resource_usage().unwrap(), Default::default());
    for role in ROLES {
        let mut m = PbrMaterialSource3D::defaults(1, 1);
        m.required[role as usize] = true;
        assert_eq!(
            context.register_pbr_material(m, None).unwrap_err(),
            PbrError3D::RequiredRole(role)
        );
    }
    assert_eq!(context.resource_usage().unwrap(), Default::default());
    let mat = context
        .register_pbr_material(PbrMaterialSource3D::defaults(1, 1), None)
        .unwrap();
    let held = context.lease_pbr_material(&mat).unwrap();
    assert_eq!(held.data.source.block().role_mode, [0, 0, 0, 0]);
    assert_eq!(held.data.source.metallic, 1.0);
    assert_eq!(held.data.source.emissive_rgb, [0.0; 3]);
    assert_eq!(context.resource_usage().unwrap().live_resource_slots, 11);
    for (i, role) in ROLES.into_iter().enumerate() {
        assert_eq!(
            held.data.textures[i].descriptor().format(),
            if matches!(role, PbrRole3D::Base | PbrRole3D::Emissive) {
                TextureFormat3D::Rgba8UnormSrgb
            } else {
                TextureFormat3D::Rgba8Unorm
            }
        );
    }
    let mut next = PbrMaterialSource3D::defaults(1, 2);
    next.roughness = 0.0;
    let next = context.register_pbr_material(next, Some(&mat)).unwrap();
    assert_eq!(
        context
            .lease_pbr_material(&next)
            .unwrap()
            .data
            .source
            .block()
            .mr_ao_cutoff[1],
        0.0
    );
    let mut near = PbrMaterialSource3D::defaults(4, 1);
    near.roughness = 0.045;
    let near = context.register_pbr_material(near, None).unwrap();
    assert_ne!(near, next);
    assert_eq!(
        context
            .lease_pbr_material(&near)
            .unwrap()
            .data
            .source
            .block()
            .mr_ao_cutoff[1],
        0.045
    );
    context.retire_pbr_material(&near).unwrap();
    assert!(context.lease_pbr_material(&mat).is_err());
    assert_eq!(held.data.source.roughness, 1.0);
    let mut third = PbrMaterialSource3D::defaults(1, 3);
    third.roughness = 0.045;
    assert!(matches!(
        context.register_pbr_material(third, Some(&next)),
        Err(PbrError3D::Resource(_))
    ));
    let foreign = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
    assert!(matches!(
        foreign.lease_pbr_material(&next),
        Err(PbrError3D::Resource(_))
    ));
    let numeric = context
        .register_texture(
            Texture3D::try_new(1, 1, TextureFormat3D::Rgba8Unorm, vec![128, 128, 127, 255])
                .unwrap(),
        )
        .unwrap();
    let sampler = context.register_sampler(Sampler3D::default()).unwrap();
    for role in [PbrRole3D::Base, PbrRole3D::Normal, PbrRole3D::Emissive] {
        let mut m = PbrMaterialSource3D::defaults(3, 1);
        m.roles[role as usize] = Some(PbrRoleBinding3D::V1 {
            texture: numeric.handle().clone(),
            sampler: sampler.handle().clone(),
        });
        assert!(matches!(
            context.register_pbr_material(m, None),
            Err(PbrError3D::Role { .. })
        ));
    }
    for x in [f32::NAN, -1024.01, 1024.01] {
        let mut m = PbrMaterialSource3D::defaults(3, 1);
        m.normal_scale = x;
        assert!(context.register_pbr_material(m, None).is_err());
    }
    context.retire_pbr_material(&next).unwrap();
    drop(held);
    context.resource_usage().unwrap();
    let mut policy = PolicyLimits3D::conservative();
    policy.max_resource_slots = 1;
    let c = cpu_context(policy, u64::MAX);
    let geometry = PbrGeometrySource3D::try_new(
        super::three_d_geometry::r024_vertices(),
        GeometryIndices::Unindexed,
    )
    .unwrap();
    let pbr = c.register_pbr_geometry(geometry, None).unwrap();
    assert!(matches!(
        c.register_geometry(triangle(0.0)),
        Err(Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::ResourceSlots,
            ..
        })
    ));
    c.retire_pbr_geometry(&pbr).unwrap();
    let legacy = c.register_geometry(triangle(0.0)).unwrap();
    assert_eq!(legacy.handle().generation(), 2);
    assert_eq!(c.resource_usage().unwrap().live_resource_slots, 1);
}

#[test]
fn hdr_complete_target_charge_splits_without_gap_or_duplicate_and_refuses_old_plus_new() {
    let mut policy = PolicyLimits3D::conservative();
    policy.max_total_attachment_bytes = 98_304;
    let context = cpu_context(policy, u64::MAX);
    let [active, scene, finals] = context
        .split_hdr_charges_for_test(1024, 81_920, 16_384)
        .unwrap();
    assert_eq!(context.target_charges_for_test(), (1024, 98_304));
    drop(active);
    assert_eq!(context.target_charges_for_test(), (0, 98_304));
    drop(scene);
    assert_eq!(context.target_charges_for_test(), (0, 16_384));
    let error = context
        .split_hdr_charges_for_test(1024, 81_920, 16_384)
        .err()
        .unwrap();
    assert_eq!(
        error,
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::TotalAttachmentBytes,
            requested: 114_688,
            maximum: 98_304
        }
    );
    assert_eq!(context.target_charges_for_test(), (0, 16_384));
    drop(finals);
    assert_eq!(context.target_charges_for_test(), (0, 0));
    let successor = context
        .split_hdr_charges_for_test(1024, 81_920, 16_384)
        .unwrap();
    assert_eq!(context.target_charges_for_test(), (1024, 98_304));
    drop(successor);
    assert_eq!(context.target_charges_for_test(), (0, 0));
}

#[test]
fn r027_mrt4_aggregate_accounting_refuses_overlap_without_releasing_old_custody() {
    let policy = PolicyLimits3D::conservative();
    assert_eq!(policy.max_single_target_attachment_bytes, 536_870_912);
    assert_eq!(policy.max_total_attachment_bytes, 1_073_741_824);
    let context = cpu_context(policy, u64::MAX);
    // Accounting-only oracle: two independently bounded old/retiring liabilities.
    // No giant GPU textures are allocated, nor is the independently oversized
    // candidate asserted to have passed the single-target admission check.
    let old_a = context
        .split_hdr_charges_for_test(0, 295_763_968, 0)
        .unwrap();
    let old_b = context
        .split_hdr_charges_for_test(0, 295_763_968, 0)
        .unwrap();
    assert_eq!(context.target_charges_for_test(), (0, 591_527_936));
    let error = context
        .split_hdr_charges_for_test(4097 * 2048, 860_028_928, 0)
        .err()
        .unwrap();
    assert_eq!(
        error,
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::TotalAttachmentBytes,
            requested: 1_451_556_864,
            maximum: 1_073_741_824,
        }
    );
    assert_eq!(context.target_charges_for_test(), (0, 591_527_936));
    drop((old_a, old_b));
    assert_eq!(context.target_charges_for_test(), (0, 0));
}

#[test]
fn geometry_source_is_canonical_checked_and_keeps_semantic_presence() {
    let descriptor = GeometrySourceDescriptor::try_new(
        vec![
            local_point(0.0, 0.0, 0.0),
            local_point(1.0, 0.0, 0.0),
            local_point(0.0, 1.0, 0.0),
        ],
        Some(vec![local_vector(0.0, 0.0, 2.0); 3]),
        Some(vec![
            TextureCoordinate2::try_new(0.0, 0.0).unwrap(),
            TextureCoordinate2::try_new(1.0, 0.0).unwrap(),
            TextureCoordinate2::try_new(0.0, 1.0).unwrap(),
        ]),
        Some(vec![
            LinearVertexColorRgba8::from_linear_unorm8(255, 0, 0, 255),
            LinearVertexColorRgba8::from_linear_unorm8(0, 255, 0, 255),
            LinearVertexColorRgba8::from_linear_unorm8(0, 0, 255, 255),
        ]),
        GeometryIndices::from_u16(vec![0, 1, 2]),
    )
    .unwrap();

    assert_eq!(descriptor.vertex_count(), 3);
    assert_eq!(descriptor.index_count(), 3);
    assert_eq!(
        descriptor.canonical_vertex_bytes(),
        3 * CANONICAL_VERTEX_STRIDE
    );
    assert_eq!(descriptor.canonical_index_bytes(), 6);
    assert_eq!(descriptor.canonical_total_bytes(), 114);
    assert_eq!(
        descriptor.normals().unwrap()[0],
        local_vector(0.0, 0.0, 1.0)
    );
    assert!(
        descriptor
            .semantics()
            .contains(GeometrySemantics::new(true, true, true))
    );
    assert_eq!(descriptor.uvs().unwrap()[2].v(), 1.0);
    assert_eq!(descriptor.colors().unwrap()[1].to_array(), [0, 255, 0, 255]);
    assert_eq!(DEFAULT_LINEAR_VERTEX_COLOR.to_array(), [255, 255, 255, 255]);
}

#[test]
fn geometry_source_rejects_every_invalid_cpu_boundary() {
    assert_eq!(
        GeometrySourceDescriptor::try_new(Vec::new(), None, None, None, GeometryIndices::Unindexed,),
        Err(Geometry3DError::EmptyGeometry)
    );
    assert_eq!(
        GeometrySourceDescriptor::try_new(
            vec![local_point(0.0, 0.0, 0.0); 3],
            Some(vec![local_vector(0.0, 0.0, 1.0); 2]),
            None,
            None,
            GeometryIndices::Unindexed,
        ),
        Err(Geometry3DError::SemanticLengthMismatch {
            semantic: "normal",
            positions: 3,
            actual: 2,
        })
    );
    assert_eq!(
        GeometrySourceDescriptor::try_new(
            vec![local_point(0.0, 0.0, 0.0); 3],
            Some(vec![local_vector(0.0, 0.0, 0.0); 3]),
            None,
            None,
            GeometryIndices::Unindexed,
        ),
        Err(Geometry3DError::InvalidNormal { vertex: 0 })
    );
    assert_eq!(
        TextureCoordinate2::try_new(f32::NAN, 0.0),
        Err(Geometry3DError::NonFiniteTextureCoordinate)
    );
    assert_eq!(
        GeometrySourceDescriptor::try_new(
            vec![local_point(0.0, 0.0, 0.0); 4],
            None,
            None,
            None,
            GeometryIndices::Unindexed,
        ),
        Err(Geometry3DError::InvalidTriangleListCount { count: 4 })
    );
    assert_eq!(
        GeometrySourceDescriptor::try_new(
            vec![local_point(0.0, 0.0, 0.0); 3],
            None,
            None,
            None,
            GeometryIndices::from_u32(vec![0, 1, 3]),
        ),
        Err(Geometry3DError::IndexOutOfRange {
            element: 2,
            index: 3,
            vertices: 3,
        })
    );
}

#[test]
fn appearance_source_registration_is_metadata_retention_not_shader_admission() {
    assert_eq!(
        AppearanceSourceDescriptor::try_new("  ", GeometrySemantics::NONE),
        Err(Appearance3DError::EmptySource)
    );
    let descriptor = AppearanceSourceDescriptor::try_new(
        "this is intentionally not admitted WGSL",
        GeometrySemantics::new(true, false, true),
    )
    .unwrap();
    assert_eq!(
        descriptor.source(),
        "this is intentionally not admitted WGSL"
    );
    assert_eq!(descriptor.source_byte_len(), 39);
    assert!(descriptor.required_semantics().has_normal());
    assert!(descriptor.required_semantics().has_color());
}

#[test]
fn context_identity_is_private_isolated_and_bound_to_exact_gpu_arcs() {
    let Some(renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    let gpu = renderer.gpu_resources();
    let registrations_before = gpu.pending_registrations.lock().unwrap().len();
    let unregistrations_before = gpu.pending_unregistrations.lock().unwrap().len();

    let first = Context3D::new(gpu.clone()).unwrap();
    let first_clone = first.clone();
    let isolated_sibling = Context3D::new(gpu.clone()).unwrap();

    assert!(first.same_resource_family(&first_clone));
    assert!(!first.same_resource_family(&isolated_sibling));
    assert!(first.matches_gpu_resources(&gpu));
    assert!(isolated_sibling.matches_gpu_resources(&gpu));
    assert_eq!(first.surface_format(), gpu.format);
    assert_eq!(first.limits().profile, LimitProfile3D::Conservative);
    assert!(first.limits().policy.max_mesh_bytes <= first.limits().hard.max_buffer_size);
    assert!(
        first.limits().policy.max_target_dimension_2d
            <= first.limits().hard.max_texture_dimension_2d
    );
    assert_eq!(
        gpu.pending_registrations.lock().unwrap().len(),
        registrations_before
    );
    assert_eq!(
        gpu.pending_unregistrations.lock().unwrap().len(),
        unregistrations_before
    );
    assert_eq!(first.resource_usage().unwrap().retained_generations, 0);
}

#[test]
fn every_context_limit_is_finite_nonzero_and_custom_hard_limits_fail_distinctly() {
    let context = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
    assert_eq!(context.limits().profile, LimitProfile3D::Custom);
    assert!(context.limits().policy.max_live_views > 0);
    assert!(context.limits().policy.max_active_target_pixels > 0);
    assert!(context.limits().policy.max_resource_slots > 0);
    assert!(context.limits().policy.max_retained_generations > 0);
    assert!(context.limits().policy.max_cache_residency_bytes > 0);

    let mut invalid = PolicyLimits3D::conservative();
    invalid.max_resource_slots = 0;
    assert_eq!(
        Context3D::new_cpu_only_for_test(invalid, u64::MAX).unwrap_err(),
        Context3DError::InvalidConfiguration {
            field: "max_resource_slots",
        }
    );

    let hard = context.limits().hard;
    let mut above_hard = PolicyLimits3D::conservative();
    above_hard.max_mesh_bytes = hard.max_buffer_size + 1;
    assert_eq!(
        Context3D::new_cpu_only_for_test(above_hard, u64::MAX).unwrap_err(),
        Context3DError::HardDeviceLimitExceeded {
            limit: HardLimit3D::BufferSize,
            requested: hard.max_buffer_size + 1,
            maximum: hard.max_buffer_size,
        }
    );
}

#[test]
fn typed_handles_reject_cross_context_wrong_kind_and_missing_slot() {
    let first = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
    let second = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
    let geometry = first.register_geometry(triangle(0.0)).unwrap();
    let appearance = first
        .register_appearance(appearance("appearance source"))
        .unwrap();

    assert_eq!(
        second.lease_geometry(geometry.handle()).unwrap_err(),
        Context3DError::ContextMismatch
    );
    let wrong_kind = Context3D::wrong_kind_geometry_handle_for_test(appearance.handle());
    assert_eq!(
        first.lease_geometry(&wrong_kind).unwrap_err(),
        Context3DError::WrongResourceKind {
            expected: ResourceKind3D::Geometry,
            actual: ResourceKind3D::Appearance,
        }
    );
    let missing = Context3D::geometry_handle_with_slot_for_test(geometry.handle(), u32::MAX);
    assert_eq!(
        first.lease_geometry(&missing).unwrap_err(),
        Context3DError::SlotOutOfRange { slot: u32::MAX }
    );
}

#[test]
fn texture_and_sampler_descriptors_are_exact_immutable_cpu_values() {
    assert_eq!(
        Texture3D::try_new(0, 1, TextureFormat3D::Rgba8Unorm, Vec::new()),
        Err(Texture3DError::EmptyExtent {
            width: 0,
            height: 1,
        })
    );
    assert_eq!(
        Texture3D::try_new(2, 1, TextureFormat3D::Rgba8UnormSrgb, vec![0; 7]),
        Err(Texture3DError::TexelByteLengthMismatch {
            expected: 8,
            actual: 7,
        })
    );
    let texture = Texture3D::try_new(2, 1, TextureFormat3D::Rgba8UnormSrgb, vec![1; 8]).unwrap();
    assert_eq!((texture.width(), texture.height()), (2, 1));
    assert_eq!(texture.format(), TextureFormat3D::Rgba8UnormSrgb);
    assert_eq!(texture.texel_byte_len(), 8);
    assert_eq!(texture.texels(), &[1; 8]);

    let sampler = Sampler3D::new(
        SamplerAddressMode::Repeat,
        SamplerAddressMode::MirrorRepeat,
        SamplerFilterMode::Nearest,
        SamplerFilterMode::Linear,
    );
    assert_eq!(sampler.address_mode_u(), SamplerAddressMode::Repeat);
    assert_eq!(sampler.address_mode_v(), SamplerAddressMode::MirrorRepeat);
    assert_eq!(sampler.mag_filter(), SamplerFilterMode::Nearest);
    assert_eq!(sampler.min_filter(), SamplerFilterMode::Linear);
    assert_eq!(Sampler3D::default(), Sampler3D::LINEAR_CLAMP);
}

#[test]
fn texture_sampler_handles_enforce_context_kind_generation_and_finite_source_budgets() {
    let context = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
    let other = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
    let texture = context
        .register_texture(
            Texture3D::try_new(1, 1, TextureFormat3D::Rgba8Unorm, vec![255, 0, 0, 255]).unwrap(),
        )
        .unwrap();
    let sampler = context.register_sampler(Sampler3D::LINEAR_CLAMP).unwrap();

    assert_eq!(
        other.lease_texture(texture.handle()).unwrap_err(),
        Context3DError::ContextMismatch
    );
    assert_eq!(
        context
            .lease_texture(&Context3D::wrong_kind_texture_handle_for_test(
                sampler.handle()
            ))
            .unwrap_err(),
        Context3DError::WrongResourceKind {
            expected: ResourceKind3D::Texture,
            actual: ResourceKind3D::Sampler,
        }
    );
    assert_eq!(
        context
            .lease_sampler(&Context3D::wrong_kind_sampler_handle_for_test(
                texture.handle()
            ))
            .unwrap_err(),
        Context3DError::WrongResourceKind {
            expected: ResourceKind3D::Sampler,
            actual: ResourceKind3D::Texture,
        }
    );

    let texture_handle = texture.handle().clone();
    let replacement = context
        .replace_texture(
            &texture_handle,
            Texture3D::try_new(1, 1, TextureFormat3D::Rgba8Unorm, vec![0, 0, 255, 255]).unwrap(),
        )
        .unwrap();
    assert_eq!(replacement.handle().generation(), 2);
    assert_eq!(
        context.lease_texture(&texture_handle).unwrap_err(),
        Context3DError::StaleGeneration {
            slot: texture_handle.slot(),
            requested: 1,
            live: Some(2),
        }
    );

    let mut dimension_policy = PolicyLimits3D::conservative();
    dimension_policy.max_texture_dimension_2d = 1;
    let dimension_context = cpu_context(dimension_policy, u64::MAX);
    assert_eq!(
        dimension_context
            .register_texture(
                Texture3D::try_new(2, 1, TextureFormat3D::Rgba8Unorm, vec![0; 8],).unwrap()
            )
            .unwrap_err(),
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::TextureDimension2D,
            requested: 2,
            maximum: 1,
        }
    );

    let mut byte_policy = PolicyLimits3D::conservative();
    byte_policy.max_texture_bytes = 3;
    let byte_context = cpu_context(byte_policy, u64::MAX);
    assert_eq!(
        byte_context
            .register_texture(
                Texture3D::try_new(1, 1, TextureFormat3D::Rgba8Unorm, vec![0; 4],).unwrap()
            )
            .unwrap_err(),
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::TextureBytes,
            requested: 4,
            maximum: 3,
        }
    );
}

#[test]
fn pending_request_and_publication_budgets_reserve_and_release_transactionally() {
    let mut policy = PolicyLimits3D::conservative();
    policy.max_pending_requests = 1;
    policy.max_pending_publications = 1;
    let context = cpu_context(policy, u64::MAX);

    let request = context.reserve_pending_request().unwrap();
    assert_eq!(context.resource_usage().unwrap().pending_requests, 1);
    let request_error = match context.reserve_pending_request() {
        Ok(_) => panic!("second pending request reservation exceeded its finite budget"),
        Err(error) => error,
    };
    assert_eq!(
        request_error,
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::PendingRequests,
            requested: 2,
            maximum: 1,
        }
    );
    assert_eq!(context.resource_usage().unwrap().pending_requests, 1);

    let publication = context.reserve_pending_publication().unwrap();
    assert_eq!(context.resource_usage().unwrap().pending_publications, 1);
    let publication_error = match context.reserve_pending_publication() {
        Ok(_) => panic!("second pending publication reservation exceeded its finite budget"),
        Err(error) => error,
    };
    assert_eq!(
        publication_error,
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::PendingPublications,
            requested: 2,
            maximum: 1,
        }
    );
    assert_eq!(context.resource_usage().unwrap().pending_publications, 1);

    drop(publication);
    drop(request);
    let usage = context.resource_usage().unwrap();
    assert_eq!(usage.pending_requests, 0);
    assert_eq!(usage.pending_publications, 0);
}

#[test]
fn replacement_is_immutable_and_old_generation_remains_only_through_its_lease() {
    let context = cpu_context(PolicyLimits3D::conservative(), u64::MAX);
    let first = context.register_geometry(triangle(0.0)).unwrap();
    let first_handle = first.handle().clone();
    let second = context
        .replace_geometry(&first_handle, triangle(10.0))
        .unwrap();

    assert_eq!(first.handle().slot(), second.handle().slot());
    assert_eq!(first.handle().generation(), 1);
    assert_eq!(second.handle().generation(), 2);
    assert_eq!(
        first.descriptor().positions()[0],
        local_point(0.0, 0.0, 0.0)
    );
    assert_eq!(
        second.descriptor().positions()[0],
        local_point(10.0, 0.0, 0.0)
    );
    assert_eq!(
        context.lease_geometry(&first_handle).unwrap_err(),
        Context3DError::StaleGeneration {
            slot: first_handle.slot(),
            requested: 1,
            live: Some(2),
        }
    );
    assert_eq!(context.resource_usage().unwrap().retained_generations, 2);
    drop(first);
    assert_eq!(context.resource_usage().unwrap().retained_generations, 1);
}

#[test]
fn retirement_blocks_reuse_until_final_lease_then_reuses_with_next_generation() {
    let mut policy = PolicyLimits3D::conservative();
    policy.max_resource_slots = 1;
    let context = cpu_context(policy, u64::MAX);
    let first = context.register_geometry(triangle(0.0)).unwrap();
    let first_handle = first.handle().clone();
    context.retire_geometry(&first_handle).unwrap();

    assert_eq!(
        context.lease_geometry(&first_handle).unwrap_err(),
        Context3DError::FreedSlot {
            slot: 0,
            generation: 1,
        }
    );
    assert_eq!(
        context.register_geometry(triangle(1.0)).unwrap_err(),
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::ResourceSlots,
            requested: 2,
            maximum: 1,
        }
    );
    drop(first);

    let second = context.register_geometry(triangle(2.0)).unwrap();
    assert_eq!(second.handle().slot(), 0);
    assert_eq!(second.handle().generation(), 2);
    assert_eq!(
        context.lease_geometry(&first_handle).unwrap_err(),
        Context3DError::StaleGeneration {
            slot: 0,
            requested: 1,
            live: Some(2),
        }
    );
}

#[test]
fn non_wrapping_generation_exhaustion_permanently_removes_the_slot() {
    let mut policy = PolicyLimits3D::conservative();
    policy.max_resource_slots = 1;
    let context = cpu_context(policy, 2);

    let first = context.register_geometry(triangle(0.0)).unwrap();
    let first_handle = first.handle().clone();
    context.retire_geometry(&first_handle).unwrap();
    drop(first);

    let second = context.register_geometry(triangle(1.0)).unwrap();
    assert_eq!(second.handle().generation(), 2);
    let second_handle = second.handle().clone();
    context.retire_geometry(&second_handle).unwrap();
    drop(second);

    assert_eq!(
        context.register_geometry(triangle(2.0)).unwrap_err(),
        Context3DError::GenerationExhausted { slot: 0 }
    );
    assert_eq!(context.resource_usage().unwrap().live_resource_slots, 0);
}

#[test]
fn replacement_generation_budget_tracks_explicit_leases_and_is_atomic() {
    let mut retained_policy = PolicyLimits3D::conservative();
    retained_policy.max_retained_generations = 1;
    let retained_context = cpu_context(retained_policy, u64::MAX);
    let first = retained_context.register_geometry(triangle(0.0)).unwrap();
    let first_handle = first.handle().clone();
    let second_lease = retained_context.lease_geometry(&first_handle).unwrap();

    assert_eq!(
        retained_context
            .replace_geometry(&first_handle, triangle(1.0))
            .unwrap_err(),
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::RetainedGenerations,
            requested: 2,
            maximum: 1,
        }
    );
    assert_eq!(
        retained_context
            .resource_usage()
            .unwrap()
            .retained_generations,
        1
    );
    assert_eq!(
        retained_context
            .lease_geometry(&first_handle)
            .unwrap()
            .descriptor()
            .positions()[0],
        local_point(0.0, 0.0, 0.0)
    );

    drop(second_lease);
    assert_eq!(
        retained_context
            .replace_geometry(&first_handle, triangle(2.0))
            .unwrap_err(),
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::RetainedGenerations,
            requested: 2,
            maximum: 1,
        }
    );
    drop(first);
    let replacement = retained_context
        .replace_geometry(&first_handle, triangle(3.0))
        .unwrap();
    assert_eq!(replacement.handle().generation(), 2);
    assert_eq!(
        replacement.descriptor().positions()[0],
        local_point(3.0, 0.0, 0.0)
    );
    assert_eq!(
        retained_context
            .resource_usage()
            .unwrap()
            .retained_generations,
        1
    );
}

#[test]
fn replacement_cache_budget_tracks_explicit_leases_and_is_atomic() {
    let mut policy = PolicyLimits3D::conservative();
    policy.max_cache_residency_bytes = 108;
    let context = cpu_context(policy, u64::MAX);
    let first = context.register_geometry(triangle(0.0)).unwrap();
    let first_handle = first.handle().clone();
    let second_lease = context.lease_geometry(&first_handle).unwrap();

    assert_eq!(
        context
            .replace_geometry(&first_handle, triangle(1.0))
            .unwrap_err(),
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::CacheResidencyBytes,
            requested: 216,
            maximum: 108,
        }
    );
    assert_eq!(
        context
            .lease_geometry(&first_handle)
            .unwrap()
            .descriptor()
            .positions()[0],
        local_point(0.0, 0.0, 0.0)
    );
    drop(second_lease);
    assert_eq!(
        context
            .replace_geometry(&first_handle, triangle(2.0))
            .unwrap_err(),
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::CacheResidencyBytes,
            requested: 216,
            maximum: 108,
        }
    );
    drop(first);
    let replacement = context
        .replace_geometry(&first_handle, triangle(3.0))
        .unwrap();
    assert_eq!(replacement.handle().generation(), 2);
    assert_eq!(
        context
            .resource_usage()
            .unwrap()
            .retained_cpu_descriptor_bytes,
        108
    );
}

#[test]
fn cpu_geometry_generations_do_not_claim_gpu_residency() {
    let mut policy = PolicyLimits3D::conservative();
    policy.max_resident_geometry_bytes = 1;
    let context = cpu_context(policy, u64::MAX);
    let first = context.register_geometry(triangle(0.0)).unwrap();
    let first_handle = first.handle().clone();
    drop(first);
    let replacement = context
        .replace_geometry(&first_handle, triangle(1.0))
        .unwrap();
    assert_eq!(replacement.handle().generation(), 2);
    assert_eq!(context.resource_usage().unwrap().resident_geometry_bytes, 0);
}

#[test]
fn resource_payload_budgets_fail_without_partial_commit() {
    let mut mesh_policy = PolicyLimits3D::conservative();
    mesh_policy.max_mesh_bytes = 100;
    let mesh_context = cpu_context(mesh_policy, u64::MAX);
    assert_eq!(
        mesh_context.register_geometry(triangle(0.0)).unwrap_err(),
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::MeshBytes,
            requested: 108,
            maximum: 100,
        }
    );
    assert_eq!(
        mesh_context.resource_usage().unwrap().retained_generations,
        0
    );

    let mut appearance_policy = PolicyLimits3D::conservative();
    appearance_policy.max_appearance_source_bytes = 4;
    let appearance_context = cpu_context(appearance_policy, u64::MAX);
    assert_eq!(
        appearance_context
            .register_appearance(appearance("12345"))
            .unwrap_err(),
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::AppearanceSourceBytes,
            requested: 5,
            maximum: 4,
        }
    );
}

#[test]
fn registration_distinguishes_hard_buffer_ceiling_from_policy_budgets() {
    let mut hard = DeviceLimits3D::baseline_for_test();
    hard.max_buffer_size = 100;
    let mut policy = PolicyLimits3D::conservative();
    policy.max_mesh_bytes = 100;
    policy.max_vertices_per_mesh = 2;
    policy.max_indices_per_mesh = 2;
    let context = Context3D::new_cpu_only_with_hard_for_test(policy, hard, u64::MAX).unwrap();

    assert_eq!(
        context.register_geometry(triangle(0.0)).unwrap_err(),
        Context3DError::HardDeviceLimitExceeded {
            limit: HardLimit3D::BufferSize,
            requested: 108,
            maximum: 100,
        }
    );
    assert_eq!(context.resource_usage().unwrap().retained_generations, 0);
}

#[test]
fn each_live_registry_budget_is_checked_and_failure_is_atomic() {
    let mut vertex_policy = PolicyLimits3D::conservative();
    vertex_policy.max_vertices_per_mesh = 2;
    let vertex_context = cpu_context(vertex_policy, u64::MAX);
    assert_eq!(
        vertex_context.register_geometry(triangle(0.0)).unwrap_err(),
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::MeshVertices,
            requested: 3,
            maximum: 2,
        }
    );

    let indexed = GeometrySourceDescriptor::try_new(
        vec![
            local_point(0.0, 0.0, 0.0),
            local_point(1.0, 0.0, 0.0),
            local_point(0.0, 1.0, 0.0),
        ],
        None,
        None,
        None,
        GeometryIndices::from_u16(vec![0, 1, 2]),
    )
    .unwrap();
    let mut index_policy = PolicyLimits3D::conservative();
    index_policy.max_indices_per_mesh = 2;
    let index_context = cpu_context(index_policy, u64::MAX);
    assert_eq!(
        index_context.register_geometry(indexed).unwrap_err(),
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::MeshIndices,
            requested: 3,
            maximum: 2,
        }
    );

    let mut descriptor_cache_policy = PolicyLimits3D::conservative();
    descriptor_cache_policy.max_cache_residency_bytes = 150;
    let descriptor_cache_context = cpu_context(descriptor_cache_policy, u64::MAX);
    let retained_descriptor = descriptor_cache_context
        .register_geometry(triangle(0.0))
        .unwrap();
    assert_eq!(
        descriptor_cache_context
            .register_geometry(triangle(1.0))
            .unwrap_err(),
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::CacheResidencyBytes,
            requested: 216,
            maximum: 150,
        }
    );
    assert_eq!(
        descriptor_cache_context
            .resource_usage()
            .unwrap()
            .retained_cpu_descriptor_bytes,
        108
    );
    drop(retained_descriptor);

    let mut count_policy = PolicyLimits3D::conservative();
    count_policy.max_registered_appearances = 1;
    let count_context = cpu_context(count_policy, u64::MAX);
    let registered = count_context.register_appearance(appearance("a")).unwrap();
    assert_eq!(
        count_context
            .register_appearance(appearance("b"))
            .unwrap_err(),
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::RegisteredAppearances,
            requested: 2,
            maximum: 1,
        }
    );
    assert_eq!(
        count_context
            .resource_usage()
            .unwrap()
            .registered_appearances,
        1
    );
    drop(registered);

    let mut cache_policy = PolicyLimits3D::conservative();
    cache_policy.max_cache_residency_bytes = 110;
    let cache_context = cpu_context(cache_policy, u64::MAX);
    let geometry = cache_context.register_geometry(triangle(0.0)).unwrap();
    assert_eq!(
        cache_context
            .register_appearance(appearance("12345"))
            .unwrap_err(),
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::CacheResidencyBytes,
            requested: 113,
            maximum: 110,
        }
    );
    assert_eq!(
        cache_context
            .resource_usage()
            .unwrap()
            .retained_cpu_descriptor_bytes,
        108
    );
    drop(geometry);
}

#[test]
fn registered_appearance_budget_counts_only_current_lookup_generations() {
    let mut policy = PolicyLimits3D::conservative();
    policy.max_registered_appearances = 1;
    let context = cpu_context(policy, u64::MAX);
    let first = context.register_appearance(appearance("a")).unwrap();
    let first_handle = first.handle().clone();

    let second = context
        .replace_appearance(&first_handle, appearance("bb"))
        .unwrap();
    let second_handle = second.handle().clone();
    let usage = context.resource_usage().unwrap();
    assert_eq!(usage.registered_appearances, 1);
    assert_eq!(usage.retained_generations, 2);
    assert_eq!(usage.retained_appearance_source_bytes, 3);

    context.retire_appearance(&second_handle).unwrap();
    let retiring_usage = context.resource_usage().unwrap();
    assert_eq!(retiring_usage.registered_appearances, 0);
    assert_eq!(retiring_usage.retained_generations, 2);
    assert_eq!(retiring_usage.retained_appearance_source_bytes, 3);

    let third = context.register_appearance(appearance("ccc")).unwrap();
    let renewed_usage = context.resource_usage().unwrap();
    assert_eq!(renewed_usage.registered_appearances, 1);
    assert_eq!(renewed_usage.retained_generations, 3);
    assert_eq!(renewed_usage.retained_appearance_source_bytes, 6);

    drop(first);
    drop(second);
    drop(third);
}
