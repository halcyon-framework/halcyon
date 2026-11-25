use halcyon::core::appearance::*;
use halcyon::core::event::{FocusManager, InteractionDomainId, WindowId};
use halcyon::core::focus_authority::FocusDomainId;
use halcyon::core::layout::Padding;
use halcyon::core::math::RectF;
use halcyon::core::paint::Color;
use halcyon::core::theme::Theme;

const PRODUCT: ProductAppearanceOwner = ProductAppearanceOwner(42);
const CONFIGURATION: AppearanceConfigurationId = AppearanceConfigurationId {
    owner: AppearanceConfigurationOwner::Product(PRODUCT),
    generation: 7,
};
const FRAMEWORK_CONFIGURATION: AppearanceConfigurationId = AppearanceConfigurationId {
    owner: AppearanceConfigurationOwner::Framework,
    generation: 7,
};
const FALLBACK: AppearanceFallbackId = AppearanceFallbackId {
    owner: PRODUCT,
    key: 1,
};

fn resolution_input(focused: bool) -> AppearanceResolutionInput {
    AppearanceResolutionInput::new(
        FrameworkAppearanceFacts::for_test(
            WindowId(3),
            InteractionDomainId(4),
            Some(FocusDomainId(5)),
            focused,
            false,
            false,
            false,
        ),
        ButtonAppearanceFacts::default(),
        None,
    )
}

fn request(selector: AppearanceSelector, focused: bool) -> AppearanceRequest {
    AppearanceRequest {
        configuration: CONFIGURATION,
        selector,
        input: resolution_input(focused),
        instance: ButtonInstanceAppearance::default(),
        fallback: AppearanceFallbackPolicy::Reject,
    }
}

fn configuration(
    contract: ButtonAppearanceContract,
    values: ButtonAppearanceValues,
) -> ButtonAppearanceConfiguration {
    let definitions = contract
        .selectors()
        .iter()
        .map(|selector| ButtonAppearanceDefinition {
            selector: selector.clone(),
            values: values.clone(),
        })
        .collect();
    ButtonAppearanceConfiguration::new(CONFIGURATION, contract, definitions, Vec::new()).unwrap()
}

fn resolved(
    config: &ButtonAppearanceConfiguration,
    request: &AppearanceRequest,
) -> ResolvedButtonAppearance {
    match config.resolve(request) {
        AppearanceValueResolution::Resolved(value) => value,
        AppearanceValueResolution::Unresolved(failure) => {
            panic!("unexpected unresolved appearance: {failure:?}")
        }
    }
}

fn problem(
    config: &ButtonAppearanceConfiguration,
    request: &AppearanceRequest,
) -> AppearanceProblem {
    match config.resolve(request) {
        AppearanceValueResolution::Unresolved(failure) => failure.problem,
        AppearanceValueResolution::Resolved(value) => {
            panic!("unexpected resolved appearance: {value:?}")
        }
    }
}

fn layer(value: &ResolvedButtonAppearance, role: AppearanceRole) -> Option<AppearancePrimitive> {
    value
        .composition()
        .iter()
        .find(|layer| layer.role == role)
        .map(|layer| layer.primitive.clone())
}

fn product_values(contract: &ButtonAppearanceContract) -> ButtonAppearanceValues {
    let mut values = Theme::dark().button_appearance_values(contract);
    values.metrics = Some(ButtonAppearanceMetrics {
        padding: Padding::symmetric(18.0, 9.0),
    });
    let paint = ButtonPaintValues {
        surface: AppearanceSurface::Image(AppearanceImage {
            source: AppearanceImageSource::Resource(ImageResourceReference {
                owner: PRODUCT,
                key: 19,
            }),
            uv_rect: RectF::new(0.25, 0.0, 0.5, 1.0),
            tint: Color::from_hex(0xF4DFC0),
        }),
        radius: 6.0,
        content_color: Color::from_hex(0x18283B),
        underlay: Some(AppearanceSurface::Solid(Color::from_hex(0xB89553))),
        border: Some(AppearanceStroke {
            color: Color::from_hex(0xEACD98),
            width: 2.0,
        }),
        overlay: Some(AppearanceStroke {
            color: Color::from_hex(0x795F32),
            width: 1.0,
        }),
    };
    values.resting = Some(paint.clone());
    values.hovered = Some(ButtonPaintValues {
        content_color: Color::from_hex(0x213D62),
        ..paint.clone()
    });
    values.pressed = Some(ButtonPaintValues {
        content_color: Color::from_hex(0x000000),
        ..paint.clone()
    });
    values.disabled = Some(ButtonPaintValues {
        content_color: Color::from_hex(0x727272),
        ..paint.clone()
    });
    values.selected = contract.selection_owner().map(|_| ButtonPaintValues {
        content_color: Color::from_hex(0x702000),
        ..paint
    });
    values.focus = Some(AppearanceStroke {
        color: Color::from_hex(0x00BBFF),
        width: 3.0,
    });
    values
}

#[test]
fn appearance_contract_default_and_product_share_the_same_button_contract() {
    let contract = ButtonAppearanceContract::standard();
    let theme = Theme::dark();
    let default_values = theme.button_appearance_values(&contract);
    let default_resting = default_values.resting.as_ref().unwrap().clone();
    let default = ButtonAppearanceConfiguration::new(
        FRAMEWORK_CONFIGURATION,
        contract.clone(),
        vec![ButtonAppearanceDefinition {
            selector: AppearanceSelector::button(),
            values: default_values,
        }],
        vec![],
    )
    .unwrap();
    let product = configuration(contract.clone(), product_values(&contract));
    let request = request(AppearanceSelector::button(), true);
    let default_request = AppearanceRequest {
        configuration: FRAMEWORK_CONFIGURATION,
        ..request.clone()
    };
    let default_value = resolved(&default, &default_request);
    let product_value = resolved(&product, &request);
    assert_eq!(default.contract(), product.contract());
    assert_eq!(default.id().owner, AppearanceConfigurationOwner::Framework);
    assert_eq!(
        product.id().owner,
        AppearanceConfigurationOwner::Product(PRODUCT)
    );
    assert_eq!(default.id().generation, product.id().generation);
    assert_eq!(
        problem(&default, &request),
        AppearanceProblem::ConfigurationMismatch
    );
    assert_eq!(
        problem(&product, &default_request),
        AppearanceProblem::ConfigurationMismatch
    );
    assert_eq!(default_value, resolved(&default, &default_request));
    assert_eq!(product_value, resolved(&product, &request));
    assert_eq!(default_value.request().input, product_value.request().input);
    assert_ne!(default_value.metrics(), product_value.metrics());
    assert_ne!(default_value.composition(), product_value.composition());
    assert_eq!(default_value.radius(), default_resting.radius);
    assert_eq!(product_value.radius(), 6.0);
    assert!(
        matches!(
            layer(&default_value, AppearanceRole::Surface),
            Some(AppearancePrimitive::Surface(AppearanceSurface::Gradient(_)))
        ),
        "resting default Button must use the approved satin gradient"
    );
    assert_eq!(
        layer(&default_value, AppearanceRole::Border),
        default_resting.border.map(AppearancePrimitive::Stroke)
    );
    assert_eq!(default_value.content_color(), default_resting.content_color);
    assert_eq!(
        *default_value.metrics(),
        ButtonAppearanceMetrics {
            padding: Padding::symmetric(
                theme.geometry.button.padding_x,
                theme.geometry.button.padding_y
            ),
        }
    );
    assert_eq!(default_value.coverage(), AppearanceCoverage::Explicit);
    assert_eq!(
        product_value.admission(),
        AppearanceAdmission::PendingCapabilityAdmission
    );
    assert_eq!(
        product_value.composition().len(),
        MAX_BUTTON_COMPOSITION_LAYERS
    );
    assert_eq!(
        product_value
            .composition()
            .iter()
            .map(|layer| layer.role)
            .collect::<Vec<_>>(),
        [
            AppearanceRole::Underlay,
            AppearanceRole::Surface,
            AppearanceRole::Border,
            AppearanceRole::Overlay,
            AppearanceRole::FocusCue
        ]
    );
    assert_eq!(
        AppearanceRole::Border.content_side(),
        ContentSide::BeforeContent
    );
    assert_eq!(
        AppearanceRole::Overlay.content_side(),
        ContentSide::AfterContent
    );
    assert_eq!(
        AppearanceRole::FocusCue.content_side(),
        ContentSide::AfterContent
    );

    for mut theme in [
        Theme::light(),
        Theme::dark(),
        Theme::light_with_density(halcyon::core::theme::DensityMode::Regular),
        Theme::dark_with_density(halcyon::core::theme::DensityMode::Regular),
    ] {
        theme.focus_ring.focused_width = 3.0;
        let values = theme.button_appearance_values(&contract);
        let resting = values.resting.as_ref().unwrap();
        let AppearanceSurface::Gradient(gradient) = &resting.surface else {
            panic!("resting default Button must use the approved satin gradient");
        };
        let gradient = gradient.clone();
        assert!(
            gradient
                .definition()
                .stops
                .iter()
                .all(|stop| stop.color.a == 1.0)
        );
        let resting_radius = resting.radius;
        let resting_border = resting.border;
        assert!(resting.underlay.is_none() && resting.overlay.is_none());
        assert_eq!(
            values.metrics,
            Some(ButtonAppearanceMetrics {
                padding: Padding::symmetric(
                    theme.geometry.button.padding_x,
                    theme.geometry.button.padding_y,
                ),
            })
        );
        let config = configuration(contract.clone(), values.clone());
        for (facts, expected_case, paint) in [
            (
                ButtonAppearanceFacts {
                    hovered: true,
                    ..ButtonAppearanceFacts::default()
                },
                ButtonPaintCase::Hovered,
                values.hovered.as_ref().unwrap(),
            ),
            (
                ButtonAppearanceFacts {
                    pressed: true,
                    ..ButtonAppearanceFacts::default()
                },
                ButtonPaintCase::Pressed,
                values.pressed.as_ref().unwrap(),
            ),
            (
                ButtonAppearanceFacts {
                    enabled: false,
                    ..ButtonAppearanceFacts::default()
                },
                ButtonPaintCase::Disabled,
                values.disabled.as_ref().unwrap(),
            ),
        ] {
            assert!(matches!(&paint.surface, AppearanceSurface::Gradient(_)));
            let state_request = AppearanceRequest {
                input: AppearanceResolutionInput::new(
                    resolution_input(false).framework(),
                    facts,
                    None,
                ),
                ..request.clone()
            };
            let state_value = resolved(&config, &state_request);
            assert_eq!(state_value.paint_case(), expected_case);
            assert_eq!(state_value.request().input, state_request.input);
            assert_eq!(*state_value.metrics(), values.metrics.unwrap());
            assert_eq!(state_value.radius(), paint.radius);
            assert_eq!(state_value.content_color(), paint.content_color);
            assert_eq!(
                layer(&state_value, AppearanceRole::Surface),
                Some(AppearancePrimitive::Surface(paint.surface.clone()))
            );
            assert_eq!(
                layer(&state_value, AppearanceRole::Border),
                paint.border.map(AppearancePrimitive::Stroke)
            );
            assert!(paint.underlay.is_none() && paint.overlay.is_none());
            assert_eq!(layer(&state_value, AppearanceRole::FocusCue), None);
        }
        let focused = values.focus.unwrap();
        assert_eq!(focused.width, theme.focus_ring.focused_width);
        let unfocused_request = AppearanceRequest {
            input: resolution_input(false),
            ..request.clone()
        };
        let unfocused = resolved(&config, &unfocused_request);
        assert_eq!(
            layer(&unfocused, AppearanceRole::Surface),
            Some(AppearancePrimitive::Surface(AppearanceSurface::Gradient(
                gradient.clone()
            )))
        );
        assert_eq!(unfocused.radius(), resting_radius);
        assert_eq!(
            layer(&unfocused, AppearanceRole::Border),
            resting_border.map(AppearancePrimitive::Stroke)
        );
        assert_eq!(layer(&unfocused, AppearanceRole::FocusCue), None);
        let focused_value = resolved(&config, &request);
        assert_eq!(
            layer(&focused_value, AppearanceRole::Surface),
            layer(&unfocused, AppearanceRole::Surface)
        );
        assert_eq!(focused_value.metrics(), unfocused.metrics());
        assert_eq!(focused_value.radius(), unfocused.radius());
        assert_eq!(
            layer(&focused_value, AppearanceRole::Border),
            layer(&unfocused, AppearanceRole::Border)
        );
        assert_eq!(
            layer(&focused_value, AppearanceRole::FocusCue),
            Some(AppearancePrimitive::Stroke(focused))
        );
    }
}

#[test]
fn appearance_contract_independent_inputs_preserve_focus_and_metrics() {
    let contract =
        ButtonAppearanceContract::new(vec![AppearanceSelector::button()], Some(PRODUCT)).unwrap();
    for values in [
        Theme::dark().button_appearance_values(&contract),
        product_values(&contract),
    ] {
        let config = configuration(contract.clone(), values);
        let mut base = request(AppearanceSelector::button(), true);
        base.input = AppearanceResolutionInput::new(
            base.input.framework(),
            base.input.button(),
            Some(ProductSelectionFact {
                owner: PRODUCT,
                selected: false,
            }),
        );
        let resting = resolved(&config, &base);
        let focus = layer(&resting, AppearanceRole::FocusCue).unwrap();
        for (hovered, pressed, selected, enabled, expected) in [
            (true, false, false, true, ButtonPaintCase::Hovered),
            (false, true, false, true, ButtonPaintCase::Pressed),
            (false, false, true, true, ButtonPaintCase::Selected),
            (true, true, true, true, ButtonPaintCase::Pressed),
            (false, false, true, false, ButtonPaintCase::Disabled),
        ] {
            let mut request = base.clone();
            request.input = AppearanceResolutionInput::new(
                base.input.framework(),
                ButtonAppearanceFacts {
                    enabled,
                    hovered,
                    pressed,
                },
                Some(ProductSelectionFact {
                    owner: PRODUCT,
                    selected,
                }),
            );
            let value = resolved(&config, &request);
            assert_eq!(value.request().input, request.input);
            assert_eq!(resting.request().input, base.input);
            assert_eq!(base.input.button(), ButtonAppearanceFacts::default());
            assert!(!base.input.selection().unwrap().selected);
            assert_eq!(value.paint_case(), expected);
            assert_eq!(value.metrics(), resting.metrics());
            assert_eq!(layer(&value, AppearanceRole::FocusCue), Some(focus.clone()));
        }
    }
}

#[test]
fn appearance_contract_framework_domains_and_capture_channels_are_independent() {
    let contract = ButtonAppearanceContract::standard();
    let config = configuration(
        contract.clone(),
        Theme::dark().button_appearance_values(&contract),
    );
    let mut request = request(AppearanceSelector::button(), true);
    request.input = AppearanceResolutionInput::new(
        request.input.framework(),
        ButtonAppearanceFacts {
            hovered: true,
            pressed: true,
            ..ButtonAppearanceFacts::default()
        },
        None,
    );
    let unheld = resolved(&config, &request);
    assert!(!unheld.request().input.framework().pointer_captured());
    assert!(!unheld.request().input.framework().keyboard_captured());
    request.input = AppearanceResolutionInput::new(
        FrameworkAppearanceFacts::for_test(
            WindowId(3),
            InteractionDomainId(4),
            Some(FocusDomainId(5)),
            true,
            false,
            true,
            false,
        ),
        request.input.button(),
        None,
    );
    let captured = resolved(&config, &request);
    let framework = captured.request().input.framework();
    assert_eq!(framework.window(), WindowId(3));
    assert_eq!(framework.interaction_domain(), InteractionDomainId(4));
    assert_eq!(framework.focus_domain(), Some(FocusDomainId(5)));
    assert!(framework.focused());
    assert!(framework.pointer_captured());
    assert!(!framework.keyboard_captured());
    assert!(!framework.scroll_captured());
    assert_eq!(captured.composition(), unheld.composition());
    request.input = AppearanceResolutionInput::new(
        FrameworkAppearanceFacts::for_test(
            WindowId(3),
            InteractionDomainId(4),
            Some(FocusDomainId(5)),
            true,
            true,
            false,
            false,
        ),
        request.input.button(),
        None,
    );
    assert_eq!(
        problem(&config, &request),
        AppearanceProblem::ButtonCannotCaptureKeyboard
    );
}

#[test]
fn appearance_contract_preserves_the_focus_authoritys_absent_active_domain() {
    let manager = FocusManager::new();
    assert_eq!(manager.active_focus_domain(), None);
    assert_eq!(manager.focused(), None);
    let contract = ButtonAppearanceContract::standard();
    let config = configuration(
        contract.clone(),
        Theme::dark().button_appearance_values(&contract),
    );
    let mut request = request(AppearanceSelector::button(), false);
    request.input = AppearanceResolutionInput::new(
        FrameworkAppearanceFacts::for_test(
            WindowId::PRIMARY,
            manager.interaction_domain_id(),
            manager.active_focus_domain(),
            false,
            false,
            false,
            false,
        ),
        ButtonAppearanceFacts {
            hovered: true,
            ..ButtonAppearanceFacts::default()
        },
        None,
    );
    let prepared = config.prepare(&[request.clone()]).unwrap();
    let value = &prepared.values()[0];
    assert_eq!(value.request().input, request.input);
    assert_eq!(value.request().input.framework().focus_domain(), None);
    assert!(!value.request().input.framework().focused());
    assert_eq!(value.paint_case(), ButtonPaintCase::Hovered);
    assert_eq!(layer(value, AppearanceRole::FocusCue), None);
    assert_eq!(value.coverage(), AppearanceCoverage::Explicit);
    assert_eq!(
        value.admission(),
        AppearanceAdmission::PendingCapabilityAdmission
    );
}

#[test]
fn appearance_contract_exact_selector_coverage_and_canonical_qualifier_order() {
    let a = AppearanceQualifier {
        owner: PRODUCT,
        key: 1,
        value: QualifierValue::Choice(2),
    };
    let b = AppearanceQualifier {
        owner: PRODUCT,
        key: 2,
        value: QualifierValue::Flag(true),
    };
    let variant = AppearanceVariant::Product {
        owner: PRODUCT,
        key: 3,
    };
    let selector =
        AppearanceSelector::new(AppearanceAddress::BUTTON_BODY, variant, vec![b, a]).unwrap();
    assert_eq!(
        selector,
        AppearanceSelector::new(AppearanceAddress::BUTTON_BODY, variant, vec![a, b]).unwrap()
    );
    assert_eq!(selector.qualifiers(), &[a, b]);
    assert_eq!(selector.address(), AppearanceAddress::BUTTON_BODY);
    assert_eq!(selector.variant(), variant);
    let contract =
        ButtonAppearanceContract::new(vec![selector.clone(), AppearanceSelector::button()], None)
            .unwrap();
    let values = Theme::dark().button_appearance_values(&contract);
    let definitions = vec![
        ButtonAppearanceDefinition {
            selector: selector.clone(),
            values: product_values(&contract),
        },
        ButtonAppearanceDefinition {
            selector: AppearanceSelector::button(),
            values,
        },
    ];
    let config = ButtonAppearanceConfiguration::new(
        CONFIGURATION,
        contract.clone(),
        definitions.clone(),
        vec![],
    )
    .unwrap();
    let reordered = ButtonAppearanceConfiguration::new(
        CONFIGURATION,
        contract,
        definitions.into_iter().rev().collect(),
        vec![],
    )
    .unwrap();
    let product_request = request(selector, false);
    assert_eq!(
        resolved(&config, &product_request),
        resolved(&reordered, &product_request)
    );
    // A product discriminator cannot manufacture the required framework cue.
    assert_eq!(
        layer(
            &resolved(&config, &product_request),
            AppearanceRole::FocusCue
        ),
        None
    );
    let undeclared = [
        AppearanceSelector::new(AppearanceAddress::BUTTON_BODY, variant, vec![a]).unwrap(),
        AppearanceSelector::new(
            AppearanceAddress::BUTTON_BODY,
            AppearanceVariant::Default,
            vec![a, b],
        )
        .unwrap(),
        AppearanceSelector::new(
            AppearanceAddress::BUTTON_BODY,
            variant,
            vec![
                a,
                AppearanceQualifier {
                    value: QualifierValue::Choice(1),
                    ..b
                },
            ],
        )
        .unwrap(),
    ];
    for selector in undeclared {
        assert_eq!(
            problem(&config, &request(selector, false)),
            AppearanceProblem::UndeclaredSelector
        );
    }
    let unknown_part = AppearanceSelector::new(
        AppearanceAddress {
            component: ComponentAddress::Button,
            part: VisualPart::Product {
                owner: PRODUCT,
                key: 1,
            },
        },
        AppearanceVariant::Default,
        vec![],
    )
    .unwrap();
    assert_eq!(
        problem(&config, &request(unknown_part, false)),
        AppearanceProblem::UnknownAddress
    );
    assert_eq!(
        AppearanceSelector::new(AppearanceAddress::BUTTON_BODY, variant, vec![a, a]),
        Err(AppearanceProblem::DuplicateQualifier)
    );
}

#[test]
fn appearance_contract_selection_requires_its_declared_owner() {
    let contract =
        ButtonAppearanceContract::new(vec![AppearanceSelector::button()], Some(PRODUCT)).unwrap();
    let config = configuration(
        contract.clone(),
        Theme::dark().button_appearance_values(&contract),
    );
    let mut request = request(AppearanceSelector::button(), true);
    assert_eq!(
        problem(&config, &request),
        AppearanceProblem::MissingSelectionFact
    );
    request.input = AppearanceResolutionInput::new(
        request.input.framework(),
        request.input.button(),
        Some(ProductSelectionFact {
            owner: ProductAppearanceOwner(43),
            selected: true,
        }),
    );
    assert_eq!(
        problem(&config, &request),
        AppearanceProblem::WrongSelectionOwner
    );
    request.input = AppearanceResolutionInput::new(
        request.input.framework(),
        request.input.button(),
        Some(ProductSelectionFact {
            owner: PRODUCT,
            selected: true,
        }),
    );
    assert_eq!(
        resolved(&config, &request).paint_case(),
        ButtonPaintCase::Selected
    );
    let standard = ButtonAppearanceContract::standard();
    let standard_config = configuration(
        standard.clone(),
        Theme::dark().button_appearance_values(&standard),
    );
    assert_eq!(
        problem(&standard_config, &request),
        AppearanceProblem::UndeclaredSelection
    );
}

#[test]
fn appearance_contract_explicit_padding_wins_without_modifying_configuration() {
    let contract = ButtonAppearanceContract::standard();
    let config = configuration(contract.clone(), product_values(&contract));
    let mut request = request(AppearanceSelector::button(), false);
    let before = resolved(&config, &request);
    let padding = Padding::new(1.0, 2.0, 3.0, 4.0);
    request.instance.padding = Some(padding);
    let overridden = resolved(&config, &request);
    assert_eq!(*overridden.metrics(), ButtonAppearanceMetrics { padding });
    assert_ne!(overridden.metrics(), before.metrics());
    assert_eq!(overridden.composition(), before.composition());
    request.instance.padding = None;
    assert_eq!(before, resolved(&config, &request));
}

#[test]
fn appearance_contract_optional_absence_differs_from_missing_required_values() {
    let contract = ButtonAppearanceContract::standard();
    let mut values = Theme::dark().button_appearance_values(&contract);
    values.resting.as_mut().unwrap().border = None;
    let request = request(AppearanceSelector::button(), false);
    let config = configuration(contract.clone(), values.clone());
    let complete = resolved(&config, &request);
    assert_eq!(
        complete.optional_absences(),
        &[
            AppearanceRole::Underlay,
            AppearanceRole::Border,
            AppearanceRole::Overlay
        ]
    );
    assert_eq!(complete.composition().len(), 1);
    assert_eq!(complete.coverage(), AppearanceCoverage::Explicit);
    values.resting = None;
    let missing = configuration(contract.clone(), values);
    assert_eq!(
        problem(&missing, &request),
        AppearanceProblem::Missing(MissingAppearanceValue::Paint(ButtonPaintCase::Resting))
    );
    let mut values = Theme::dark().button_appearance_values(&contract);
    values.metrics = None;
    assert_eq!(
        problem(&configuration(contract.clone(), values), &request),
        AppearanceProblem::Missing(MissingAppearanceValue::Metrics)
    );
    let mut values = Theme::dark().button_appearance_values(&contract);
    values.focus = None;
    assert_eq!(
        problem(&configuration(contract, values), &request),
        AppearanceProblem::Missing(MissingAppearanceValue::FocusCue)
    );
}

#[test]
fn appearance_contract_only_an_explicit_complete_fallback_can_resolve() {
    let contract = ButtonAppearanceContract::standard();
    let selector = AppearanceSelector::button();
    let values = Theme::dark().button_appearance_values(&contract);
    let config = ButtonAppearanceConfiguration::new(
        CONFIGURATION,
        contract.clone(),
        vec![],
        vec![ButtonAppearanceFallback {
            id: FALLBACK,
            selector: selector.clone(),
            values: values.clone(),
        }],
    )
    .unwrap();
    let mut request = request(selector.clone(), true);
    assert_eq!(
        problem(&config, &request),
        AppearanceProblem::Missing(MissingAppearanceValue::Definition)
    );
    request.fallback = AppearanceFallbackPolicy::Approve(FALLBACK);
    let complete = resolved(&config, &request);
    assert_eq!(
        complete.coverage(),
        AppearanceCoverage::Fallback {
            id: FALLBACK,
            primary_problem: AppearanceProblem::Missing(MissingAppearanceValue::Definition)
        }
    );
    assert!(layer(&complete, AppearanceRole::FocusCue).is_some());
    request.fallback =
        AppearanceFallbackPolicy::Approve(AppearanceFallbackId { key: 2, ..FALLBACK });
    assert_eq!(
        problem(&config, &request),
        AppearanceProblem::FallbackNotDeclared
    );
    request.fallback = AppearanceFallbackPolicy::Approve(FALLBACK);
    let mut incomplete = values;
    incomplete.focus = None;
    let invalid_fallback = ButtonAppearanceConfiguration::new(
        CONFIGURATION,
        contract,
        vec![],
        vec![ButtonAppearanceFallback {
            id: FALLBACK,
            selector,
            values: incomplete,
        }],
    )
    .unwrap();
    match invalid_fallback.resolve(&request) {
        AppearanceValueResolution::Unresolved(failure) => {
            assert_eq!(
                failure.primary_problem,
                Some(AppearanceProblem::Missing(
                    MissingAppearanceValue::Definition
                ))
            );
            assert_eq!(
                failure.problem,
                AppearanceProblem::Missing(MissingAppearanceValue::FocusCue)
            );
        }
        _ => panic!("incomplete fallback was accepted"),
    }
}

#[test]
fn appearance_contract_image_values_never_imply_resource_success_or_solid_fallback() {
    let contract = ButtonAppearanceContract::standard();
    for source in [
        AppearanceImageSource::Resource(ImageResourceReference {
            owner: PRODUCT,
            key: 9,
        }),
        AppearanceImageSource::CurrentBinding { texture_id: 9001 },
    ] {
        let mut values = product_values(&contract);
        let image = AppearanceImage {
            source,
            uv_rect: RectF::new(0.0, 0.0, 1.0, 1.0),
            tint: Color::WHITE,
        };
        values.resting.as_mut().unwrap().surface = AppearanceSurface::Image(image.clone());
        let config = ButtonAppearanceConfiguration::new(
            CONFIGURATION,
            contract.clone(),
            vec![ButtonAppearanceDefinition {
                selector: AppearanceSelector::button(),
                values,
            }],
            vec![ButtonAppearanceFallback {
                id: FALLBACK,
                selector: AppearanceSelector::button(),
                values: Theme::dark().button_appearance_values(&contract),
            }],
        )
        .unwrap();
        let mut request = request(AppearanceSelector::button(), false);
        request.fallback = AppearanceFallbackPolicy::Approve(FALLBACK);
        let value = resolved(&config, &request);
        assert_eq!(
            layer(&value, AppearanceRole::Surface),
            Some(AppearancePrimitive::Surface(AppearanceSurface::Image(
                image
            )))
        );
        assert_eq!(value.coverage(), AppearanceCoverage::Explicit);
        assert_eq!(
            value.admission(),
            AppearanceAdmission::PendingCapabilityAdmission
        );
    }
}

#[test]
fn appearance_contract_preparation_rejects_an_incomplete_declared_use_atomically() {
    let primary = AppearanceSelector::button();
    let alternate = AppearanceSelector::new(
        AppearanceAddress::BUTTON_BODY,
        AppearanceVariant::Product {
            owner: PRODUCT,
            key: 1,
        },
        vec![],
    )
    .unwrap();
    let contract =
        ButtonAppearanceContract::new(vec![primary.clone(), alternate.clone()], None).unwrap();
    let values = Theme::dark().button_appearance_values(&contract);
    let config = ButtonAppearanceConfiguration::new(
        CONFIGURATION,
        contract.clone(),
        vec![ButtonAppearanceDefinition {
            selector: primary.clone(),
            values: values.clone(),
        }],
        vec![],
    )
    .unwrap();
    let requests = [
        request(primary.clone(), true),
        request(alternate.clone(), true),
    ];
    let unchanged = config.clone();
    match config.prepare(&requests) {
        Err(AppearancePreparationFailure::Unresolved {
            request_index,
            failure,
        }) => {
            assert_eq!(request_index, 1);
            assert_eq!(failure.request, requests[1]);
            assert_eq!(
                failure.problem,
                AppearanceProblem::Missing(MissingAppearanceValue::Definition)
            );
        }
        outcome => panic!("incomplete declared use was prepared: {outcome:?}"),
    }
    assert_eq!(config, unchanged);
    let complete = configuration(contract, values);
    let prepared = complete.prepare(&requests).unwrap();
    assert_eq!(prepared.configuration(), CONFIGURATION);
    assert_eq!(prepared.values().len(), 2);
    assert_eq!(
        prepared.admission(),
        AppearanceAdmission::PendingCapabilityAdmission
    );
    assert_eq!(prepared.values()[0], resolved(&complete, &requests[0]));
    assert_eq!(prepared.values()[1], resolved(&complete, &requests[1]));
}

#[test]
fn appearance_contract_invalid_declared_values_and_generation_are_explicit() {
    let contract = ButtonAppearanceContract::standard();
    let mut values = Theme::dark().button_appearance_values(&contract);
    values.focus.as_mut().unwrap().color = Color::TRANSPARENT;
    let request = request(AppearanceSelector::button(), true);
    assert_eq!(
        problem(&configuration(contract.clone(), values), &request),
        AppearanceProblem::Invalid(InvalidAppearanceValue::FocusCue)
    );
    let mut values = Theme::dark().button_appearance_values(&contract);
    values.metrics.as_mut().unwrap().padding.top = f32::NAN;
    assert_eq!(
        problem(&configuration(contract.clone(), values), &request),
        AppearanceProblem::Invalid(InvalidAppearanceValue::Metrics)
    );
    let config = configuration(
        contract.clone(),
        Theme::dark().button_appearance_values(&contract),
    );
    let mut stale = request;
    stale.configuration.generation -= 1;
    assert_eq!(
        problem(&config, &stale),
        AppearanceProblem::ConfigurationMismatch
    );
}
#[test]
fn appearance_contract_prepared_metrics_need_no_interaction_projection() {
    let contract = ButtonAppearanceContract::standard();
    let config = configuration(contract.clone(), product_values(&contract));
    let style = config
        .prepare_style(
            AppearanceSelector::button(),
            AppearanceFallbackPolicy::Reject,
        )
        .unwrap();
    assert_eq!(style.metrics(), product_values(&contract).metrics.unwrap());
    for focused in [false, true] {
        let request = request(AppearanceSelector::button(), focused);
        assert_eq!(style.resolve(request.input), config.resolve(&request));
        let value = resolved(&config, &request);
        assert_eq!(*value.metrics(), style.metrics());
    }
}
