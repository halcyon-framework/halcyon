use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use naga::proc::Layouter;
use naga::valid::{Capabilities, ModuleInfo, ValidationFlags, Validator};
use naga::{
    AddressSpace, Arena, Binding, Block, BuiltIn, EntryPoint, Expression, Function,
    FunctionArgument, FunctionResult, GlobalVariable, Handle, ImageClass, ImageDimension,
    Interpolation, MathFunction, Module, ResourceBinding, Sampling, Scalar, ScalarKind,
    ShaderStage, Span, Statement, StructMember, SwizzleComponent, Type, TypeInner, VectorSize,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::{AppearanceMode, AppearanceSourceDescriptor, GeometrySemantics};

pub const WGPU_ADAPTER_VERSION: &str = "30.0.2";
pub const NAGA_ADAPTER_VERSION: &str = "30.0.2";
pub const VIEW_BLOCK_SIZE: u64 = 80;
pub const OBJECT_BLOCK_SIZE: u64 = 112;
pub const APPEARANCE_BLOCK_SIZE: u64 = 16;

const SYNTHETIC_PREFIX: &str = "__halcyon355_";
const APPEARANCE_FUNCTION_NAME: &str = "halcyon_appearance";
pub(crate) const VERTEX_ENTRY_POINT_NAME: &str = "__halcyon355_vertex";
pub(crate) const FRAGMENT_ENTRY_POINT_NAME: &str = "__halcyon355_fragment";
const F32: Scalar = Scalar {
    kind: ScalarKind::Float,
    width: 4,
};
const BOOL: Scalar = Scalar {
    kind: ScalarKind::Bool,
    width: 1,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(crate) struct ViewBlockBytes {
    view_projection: [[f32; 4]; 4],
    camera_relative: [f32; 3],
    padding: f32,
}

impl ViewBlockBytes {
    pub(crate) const fn new(view_projection: [[f32; 4]; 4], camera_relative: [f32; 3]) -> Self {
        Self {
            view_projection,
            camera_relative,
            padding: 0.0,
        }
    }

    pub(crate) const fn view_projection(self) -> [[f32; 4]; 4] {
        self.view_projection
    }

    #[cfg(test)]
    pub(crate) const fn camera_relative(self) -> [f32; 3] {
        self.camera_relative
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(crate) struct ObjectBlockBytes {
    model_relative: [[f32; 4]; 4],
    normal_to_world: [[f32; 4]; 3],
}

impl ObjectBlockBytes {
    pub(crate) const fn new(model_relative: [[f32; 4]; 4], normal_to_world: [[f32; 4]; 3]) -> Self {
        Self {
            model_relative,
            normal_to_world,
        }
    }

    #[cfg(test)]
    pub(crate) const fn model_relative(self) -> [[f32; 4]; 4] {
        self.model_relative
    }

    #[cfg(test)]
    pub(crate) const fn normal_to_world(self) -> [[f32; 4]; 3] {
        self.normal_to_world
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(crate) struct AppearanceBlockBytes {
    color_factor: [f32; 4],
}

impl AppearanceBlockBytes {
    pub(crate) const fn new(color_factor: [f32; 4]) -> Self {
        Self { color_factor }
    }

    #[cfg(test)]
    pub(crate) const fn color_factor(self) -> [f32; 4] {
        self.color_factor
    }
}

const _: [(); VIEW_BLOCK_SIZE as usize] = [(); std::mem::size_of::<ViewBlockBytes>()];
const _: [(); OBJECT_BLOCK_SIZE as usize] = [(); std::mem::size_of::<ObjectBlockBytes>()];
const _: [(); APPEARANCE_BLOCK_SIZE as usize] = [(); std::mem::size_of::<AppearanceBlockBytes>()];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiLayout3D {
    view_block_size: u64,
    object_block_size: u64,
    appearance_block_size: u64,
    view_dynamic_stride: u64,
    object_dynamic_stride: u64,
    appearance_dynamic_stride: u64,
    min_uniform_buffer_offset_alignment: u32,
}

impl AbiLayout3D {
    pub const fn view_block_size(self) -> u64 {
        self.view_block_size
    }

    pub const fn object_block_size(self) -> u64 {
        self.object_block_size
    }

    pub const fn appearance_block_size(self) -> u64 {
        self.appearance_block_size
    }

    pub const fn view_dynamic_stride(self) -> u64 {
        self.view_dynamic_stride
    }

    pub const fn object_dynamic_stride(self) -> u64 {
        self.object_dynamic_stride
    }

    pub const fn appearance_dynamic_stride(self) -> u64 {
        self.appearance_dynamic_stride
    }

    pub const fn min_uniform_buffer_offset_alignment(self) -> u32 {
        self.min_uniform_buffer_offset_alignment
    }

    pub(crate) fn try_new(
        min_uniform_buffer_offset_alignment: u32,
    ) -> Result<Self, Appearance3DAdmissionError> {
        if min_uniform_buffer_offset_alignment == 0 {
            return Err(Appearance3DAdmissionError::InternalCompilerAdapter {
                stage: "ABI layout",
                detail: Arc::from("uniform-buffer offset alignment is zero"),
            });
        }
        let alignment = u64::from(min_uniform_buffer_offset_alignment);
        Ok(Self {
            view_block_size: VIEW_BLOCK_SIZE,
            object_block_size: OBJECT_BLOCK_SIZE,
            appearance_block_size: APPEARANCE_BLOCK_SIZE,
            view_dynamic_stride: checked_round_up(VIEW_BLOCK_SIZE, alignment)?,
            object_dynamic_stride: checked_round_up(OBJECT_BLOCK_SIZE, alignment)?,
            appearance_dynamic_stride: checked_round_up(APPEARANCE_BLOCK_SIZE, alignment)?,
            min_uniform_buffer_offset_alignment,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppearanceFallbackBindingContract {
    extent: [u32; 2],
    linear_rgba8: [u8; 4],
    filterable_float_2d: bool,
    filtering_sampler: bool,
}

impl AppearanceFallbackBindingContract {
    pub const OPAQUE_WHITE: Self = Self {
        extent: [1, 1],
        linear_rgba8: [255, 255, 255, 255],
        filterable_float_2d: true,
        filtering_sampler: true,
    };

    pub const fn extent(self) -> [u32; 2] {
        self.extent
    }

    pub const fn linear_rgba8(self) -> [u8; 4] {
        self.linear_rgba8
    }

    pub const fn is_filterable_float_2d(self) -> bool {
        self.filterable_float_2d
    }

    pub const fn uses_filtering_sampler(self) -> bool {
        self.filtering_sampler
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppearanceSourceLocation {
    line_number: u32,
    line_position: u32,
    offset: u32,
    length: u32,
}

impl AppearanceSourceLocation {
    pub const fn line_number(self) -> u32 {
        self.line_number
    }

    pub const fn line_position(self) -> u32 {
        self.line_position
    }

    pub const fn offset(self) -> u32 {
        self.offset
    }

    pub const fn length(self) -> u32 {
        self.length
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppearanceSourceDiagnosticLabel {
    location: AppearanceSourceLocation,
    message: Arc<str>,
}

impl AppearanceSourceDiagnosticLabel {
    pub const fn location(&self) -> AppearanceSourceLocation {
        self.location
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppearanceSourceDiagnostic {
    message: Arc<str>,
    labels: Arc<[AppearanceSourceDiagnosticLabel]>,
}

impl AppearanceSourceDiagnostic {
    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn labels(&self) -> &[AppearanceSourceDiagnosticLabel] {
        &self.labels
    }

    pub fn primary_location(&self) -> Option<AppearanceSourceLocation> {
        self.labels
            .first()
            .map(AppearanceSourceDiagnosticLabel::location)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppearanceAdmissionBudget {
    Functions,
    Nesting,
    IrNodes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppearanceSourceRule {
    EntryPoint,
    ModuleScopeVariable,
    Override,
    ReservedIdentifier,
    RequiredFunction,
    RequiredFunctionSignature,
    OptionalExtension,
    ForbiddenType,
    ForbiddenExpression,
    ForbiddenStatement,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum Appearance3DAdmissionError {
    #[error("appearance WGSL parsing failed: {diagnostic:?}")]
    SourceParse {
        diagnostic: AppearanceSourceDiagnostic,
    },
    #[error("appearance WGSL validation failed: {diagnostic:?}")]
    SourceValidation {
        diagnostic: AppearanceSourceDiagnostic,
    },
    #[error("appearance source violates {rule:?}: {detail}")]
    SourceRejected {
        rule: AppearanceSourceRule,
        detail: Arc<str>,
        location: Option<AppearanceSourceLocation>,
    },
    #[error("appearance reads undeclared geometry semantics: used {used:?}, declared {declared:?}")]
    UndeclaredSemanticRead {
        used: GeometrySemantics,
        declared: GeometrySemantics,
    },
    #[error("opaque appearance source may not execute discard")]
    OpaqueModeDiscards,
    #[error(
        "appearance admission budget {budget:?} exceeded: requested {requested}, maximum {maximum}"
    )]
    BudgetExceeded {
        budget: AppearanceAdmissionBudget,
        requested: u64,
        maximum: u64,
    },
    #[error("checked arithmetic overflow in the appearance compiler adapter")]
    ArithmeticOverflow,
    #[error("internal appearance compiler adapter failure during {stage}: {detail}")]
    InternalCompilerAdapter {
        stage: &'static str,
        detail: Arc<str>,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AppearanceCompilerLimits {
    pub max_functions: u32,
    pub max_nesting: u32,
    pub max_ir_nodes: u64,
}

pub(crate) struct CompiledFragmentAppearance {
    pub module: Module,
    pub source_hash: [u8; 32],
    pub required_semantics: GeometrySemantics,
    pub mode: AppearanceMode,
    pub source_ir_nodes: u64,
    pub final_ir_nodes: u64,
    pub source_spans_preserved: bool,
    pub synthetic_spans_are_undefined: bool,
}

pub(crate) struct CompiledCanonicalVertex {
    pub module: Module,
    pub ir_nodes: u64,
    pub synthetic_spans_are_undefined: bool,
}

pub(crate) fn compile_fragment_appearance(
    descriptor: &AppearanceSourceDescriptor,
    limits: AppearanceCompilerLimits,
) -> Result<CompiledFragmentAppearance, Appearance3DAdmissionError> {
    let source = descriptor.source();
    let mut module = naga::front::wgsl::parse_str(source).map_err(|error| {
        Appearance3DAdmissionError::SourceParse {
            diagnostic: parse_diagnostic(&error, source),
        }
    })?;

    inspect_source_directives(source)?;
    validate_module_budget(&module, limits, false)?;
    let appearance = inspect_source_module(&module, source)?;
    let source_info = Validator::new(ValidationFlags::all(), Capabilities::empty())
        .validate(&module)
        .map_err(|error| Appearance3DAdmissionError::SourceValidation {
            diagnostic: validation_diagnostic(&error, source),
        })?;

    if descriptor.mode() == AppearanceMode::Opaque && source_info[appearance].may_kill {
        return Err(Appearance3DAdmissionError::OpaqueModeDiscards);
    }
    let used_semantics =
        appearance_semantic_reads(&module.functions[appearance], &source_info[appearance]);
    if !descriptor.required_semantics().contains(used_semantics) {
        return Err(Appearance3DAdmissionError::UndeclaredSemanticRead {
            used: used_semantics,
            declared: descriptor.required_semantics(),
        });
    }

    let source_ir_nodes = module_ir_nodes(&module)?;
    let source_spans = SourceSpanSnapshot::capture(&module);
    let synthetic = append_fragment_wrapper(&mut module, appearance)?;
    validate_module_budget(&module, limits, true)?;
    let _final_info = validate_synthetic_module(&module, "fragment validation")?;
    validate_fragment_structure(&module, appearance)?;
    validate_fragment_layout(&module, synthetic.view_block, synthetic.appearance_block)?;
    let source_spans_preserved = source_spans.matches(&module);
    if !source_spans_preserved {
        return Err(internal_structure(
            "controlled composition changed an application-source span",
        ));
    }
    let synthetic_spans_are_undefined = synthetic.spans_are_undefined(&module);
    if !synthetic_spans_are_undefined {
        return Err(internal_structure(
            "controlled composition assigned an application-source span to a synthetic node",
        ));
    }

    Ok(CompiledFragmentAppearance {
        source_hash: Sha256::digest(source.as_bytes()).into(),
        required_semantics: descriptor.required_semantics(),
        mode: descriptor.mode(),
        source_ir_nodes,
        final_ir_nodes: module_ir_nodes(&module)?,
        source_spans_preserved,
        synthetic_spans_are_undefined,
        module,
    })
}

pub(crate) fn compile_canonical_vertex(
    limits: AppearanceCompilerLimits,
) -> Result<CompiledCanonicalVertex, Appearance3DAdmissionError> {
    let (module, synthetic) = build_vertex_module()?;
    validate_module_budget(&module, limits, true)?;
    let _info = validate_synthetic_module(&module, "vertex validation")?;
    validate_vertex_structure(&module)?;
    validate_vertex_layout(&module, synthetic.view_block, synthetic.object_block)?;
    let synthetic_spans_are_undefined = synthetic.spans_are_undefined(&module);
    if !synthetic_spans_are_undefined {
        return Err(internal_structure(
            "canonical vertex construction assigned a source span to a synthetic node",
        ));
    }
    Ok(CompiledCanonicalVertex {
        ir_nodes: module_ir_nodes(&module)?,
        synthetic_spans_are_undefined,
        module,
    })
}

#[cfg(test)]
pub(crate) fn validate_wgsl_roundtrip(module: &Module) -> Result<(), Appearance3DAdmissionError> {
    let info = validate_synthetic_module(module, "WGSL write validation")?;
    let emitted = naga::back::wgsl::write_string(
        module,
        &info,
        naga::back::wgsl::WriterFlags::EXPLICIT_TYPES,
    )
    .map_err(|error| internal_error("WGSL write", error))?;
    let reparsed = naga::front::wgsl::parse_str(&emitted)
        .map_err(|error| internal_error("WGSL reparse", error))?;
    let _ = validate_synthetic_module(&reparsed, "WGSL revalidation")?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn fragment_wrapper_proof(
    module: &Module,
) -> Result<(String, bool), Appearance3DAdmissionError> {
    let info = validate_synthetic_module(module, "fragment wrapper proof")?;
    let entry_may_kill = info.get_entry_point(0).may_kill;
    let emitted = naga::back::wgsl::write_string(
        module,
        &info,
        naga::back::wgsl::WriterFlags::EXPLICIT_TYPES,
    )
    .map_err(|error| internal_error("fragment wrapper proof WGSL write", error))?;
    Ok((emitted, entry_may_kill))
}

fn checked_round_up(value: u64, alignment: u64) -> Result<u64, Appearance3DAdmissionError> {
    let remainder = value % alignment;
    if remainder == 0 {
        return Ok(value);
    }
    value
        .checked_add(alignment - remainder)
        .ok_or(Appearance3DAdmissionError::ArithmeticOverflow)
}

fn parse_diagnostic(
    error: &naga::front::wgsl::ParseError,
    source: &str,
) -> AppearanceSourceDiagnostic {
    let labels = error
        .labels()
        .filter_map(|(span, message)| {
            source_location(span, source).map(|location| AppearanceSourceDiagnosticLabel {
                location,
                message: Arc::from(message),
            })
        })
        .collect::<Vec<_>>();
    AppearanceSourceDiagnostic {
        message: Arc::from(error.message()),
        labels: labels.into(),
    }
}

fn validation_diagnostic(
    error: &naga::WithSpan<naga::valid::ValidationError>,
    source: &str,
) -> AppearanceSourceDiagnostic {
    let labels = error
        .spans()
        .filter_map(|(span, message)| {
            source_location(*span, source).map(|location| AppearanceSourceDiagnosticLabel {
                location,
                message: Arc::from(message.as_str()),
            })
        })
        .collect::<Vec<_>>();
    AppearanceSourceDiagnostic {
        message: Arc::from(error.to_string()),
        labels: labels.into(),
    }
}

fn source_location(span: Span, source: &str) -> Option<AppearanceSourceLocation> {
    span.to_range()?;
    let location = span.location(source);
    Some(AppearanceSourceLocation {
        line_number: location.line_number,
        line_position: location.line_position,
        offset: location.offset,
        length: location.length,
    })
}

fn source_rejected(
    rule: AppearanceSourceRule,
    detail: impl Into<Arc<str>>,
    span: Span,
    source: &str,
) -> Appearance3DAdmissionError {
    Appearance3DAdmissionError::SourceRejected {
        rule,
        detail: detail.into(),
        location: source_location(span, source),
    }
}

fn inspect_source_directives(source: &str) -> Result<(), Appearance3DAdmissionError> {
    let bytes = source.as_bytes();
    let mut index = 0_usize;
    let mut block_comment_depth = 0_usize;
    while index < bytes.len() {
        if block_comment_depth != 0 {
            if bytes.get(index..index + 2) == Some(b"/*") {
                block_comment_depth += 1;
                index += 2;
            } else if bytes.get(index..index + 2) == Some(b"*/") {
                block_comment_depth -= 1;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if bytes.get(index..index + 2) == Some(b"//") {
            index += 2;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        if bytes.get(index..index + 2) == Some(b"/*") {
            block_comment_depth = 1;
            index += 2;
            continue;
        }
        if bytes[index].is_ascii_alphabetic() || bytes[index] == b'_' {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
            {
                index += 1;
            }
            let identifier = &source[start..index];
            if identifier == "enable" || identifier == "requires" {
                let start = u32::try_from(start)
                    .map_err(|_| Appearance3DAdmissionError::ArithmeticOverflow)?;
                let end = u32::try_from(index)
                    .map_err(|_| Appearance3DAdmissionError::ArithmeticOverflow)?;
                return Err(source_rejected(
                    AppearanceSourceRule::OptionalExtension,
                    format!("optional WGSL `{identifier}` directive is forbidden"),
                    Span::new(start, end),
                    source,
                ));
            }
            continue;
        }
        index += 1;
    }
    Ok(())
}

fn internal_error(
    stage: &'static str,
    error: impl std::fmt::Display,
) -> Appearance3DAdmissionError {
    Appearance3DAdmissionError::InternalCompilerAdapter {
        stage,
        detail: Arc::from(error.to_string()),
    }
}

fn validate_synthetic_module(
    module: &Module,
    stage: &'static str,
) -> Result<ModuleInfo, Appearance3DAdmissionError> {
    Validator::new(ValidationFlags::all(), Capabilities::empty())
        .validate(module)
        .map_err(
            |error| Appearance3DAdmissionError::InternalCompilerAdapter {
                stage,
                detail: Arc::from(format!("{error:#?}")),
            },
        )
}

fn validate_module_budget(
    module: &Module,
    limits: AppearanceCompilerLimits,
    include_entry_points: bool,
) -> Result<(), Appearance3DAdmissionError> {
    let function_count = u64::try_from(module.functions.len())
        .map_err(|_| Appearance3DAdmissionError::ArithmeticOverflow)?
        .checked_add(if include_entry_points {
            u64::try_from(module.entry_points.len())
                .map_err(|_| Appearance3DAdmissionError::ArithmeticOverflow)?
        } else {
            0
        })
        .ok_or(Appearance3DAdmissionError::ArithmeticOverflow)?;
    validate_budget(
        AppearanceAdmissionBudget::Functions,
        function_count,
        u64::from(limits.max_functions),
    )?;
    validate_budget(
        AppearanceAdmissionBudget::Nesting,
        u64::from(module_nesting(module)?),
        u64::from(limits.max_nesting),
    )?;
    validate_budget(
        AppearanceAdmissionBudget::IrNodes,
        module_ir_nodes(module)?,
        limits.max_ir_nodes,
    )
}

fn validate_budget(
    budget: AppearanceAdmissionBudget,
    requested: u64,
    maximum: u64,
) -> Result<(), Appearance3DAdmissionError> {
    if requested > maximum {
        return Err(Appearance3DAdmissionError::BudgetExceeded {
            budget,
            requested,
            maximum,
        });
    }
    Ok(())
}

fn inspect_source_module(
    module: &Module,
    source: &str,
) -> Result<Handle<Function>, Appearance3DAdmissionError> {
    if let Some(entry) = module.entry_points.first() {
        return Err(source_rejected(
            AppearanceSourceRule::EntryPoint,
            format!("source entry point `{}` is forbidden", entry.name),
            Span::UNDEFINED,
            source,
        ));
    }
    if let Some((handle, global)) = module.global_variables.iter().next() {
        return Err(source_rejected(
            AppearanceSourceRule::ModuleScopeVariable,
            format!(
                "module-scope variable `{}` is forbidden",
                global.name.as_deref().unwrap_or("<unnamed>")
            ),
            module.global_variables.get_span(handle),
            source,
        ));
    }
    if let Some((handle, pipeline_override)) = module.overrides.iter().next() {
        return Err(source_rejected(
            AppearanceSourceRule::Override,
            format!(
                "override `{}` is forbidden",
                pipeline_override.name.as_deref().unwrap_or("<unnamed>")
            ),
            module.overrides.get_span(handle),
            source,
        ));
    }

    inspect_source_identifiers(module, source)?;
    inspect_source_types(module, source)?;
    inspect_source_functions(module, source)?;

    let mut appearance = None;
    for (handle, function) in module.functions.iter() {
        if function.name.as_deref() == Some(APPEARANCE_FUNCTION_NAME) {
            if appearance.replace(handle).is_some() {
                return Err(source_rejected(
                    AppearanceSourceRule::RequiredFunction,
                    "more than one halcyon_appearance function is forbidden",
                    module.functions.get_span(handle),
                    source,
                ));
            }
        }
    }
    let appearance = appearance.ok_or_else(|| {
        source_rejected(
            AppearanceSourceRule::RequiredFunction,
            "exactly one halcyon_appearance function is required",
            Span::UNDEFINED,
            source,
        )
    })?;
    validate_appearance_signature(module, appearance, source)?;
    Ok(appearance)
}

fn inspect_source_identifiers(
    module: &Module,
    source: &str,
) -> Result<(), Appearance3DAdmissionError> {
    for (handle, ty) in module.types.iter() {
        if let Some(name) = ty
            .name
            .as_deref()
            .filter(|name| name.starts_with(SYNTHETIC_PREFIX))
        {
            return Err(source_rejected(
                AppearanceSourceRule::ReservedIdentifier,
                format!("identifier `{name}` uses the reserved prefix"),
                module.types.get_span(handle),
                source,
            ));
        }
        if let TypeInner::Struct { members, .. } = &ty.inner {
            if let Some(name) = members
                .iter()
                .filter_map(|member| member.name.as_deref())
                .find(|name| name.starts_with(SYNTHETIC_PREFIX))
            {
                return Err(source_rejected(
                    AppearanceSourceRule::ReservedIdentifier,
                    format!("identifier `{name}` uses the reserved prefix"),
                    module.types.get_span(handle),
                    source,
                ));
            }
        }
    }
    for (handle, constant) in module.constants.iter() {
        if let Some(name) = constant
            .name
            .as_deref()
            .filter(|name| name.starts_with(SYNTHETIC_PREFIX))
        {
            return Err(source_rejected(
                AppearanceSourceRule::ReservedIdentifier,
                format!("identifier `{name}` uses the reserved prefix"),
                module.constants.get_span(handle),
                source,
            ));
        }
    }
    for (handle, function) in module.functions.iter() {
        if let Some(name) = function
            .name
            .as_deref()
            .filter(|name| name.starts_with(SYNTHETIC_PREFIX))
        {
            return Err(source_rejected(
                AppearanceSourceRule::ReservedIdentifier,
                format!("identifier `{name}` uses the reserved prefix"),
                module.functions.get_span(handle),
                source,
            ));
        }
        if let Some(name) = function
            .arguments
            .iter()
            .filter_map(|argument| argument.name.as_deref())
            .find(|name| name.starts_with(SYNTHETIC_PREFIX))
        {
            return Err(source_rejected(
                AppearanceSourceRule::ReservedIdentifier,
                format!("identifier `{name}` uses the reserved prefix"),
                module.functions.get_span(handle),
                source,
            ));
        }
        if let Some((local_handle, name)) = function
            .local_variables
            .iter()
            .filter_map(|(local_handle, local)| {
                local.name.as_deref().map(|name| (local_handle, name))
            })
            .find(|(_, name)| name.starts_with(SYNTHETIC_PREFIX))
        {
            return Err(source_rejected(
                AppearanceSourceRule::ReservedIdentifier,
                format!("identifier `{name}` uses the reserved prefix"),
                function.local_variables.get_span(local_handle),
                source,
            ));
        }
    }
    Ok(())
}

fn inspect_source_types(module: &Module, source: &str) -> Result<(), Appearance3DAdmissionError> {
    for (handle, ty) in module.types.iter() {
        let forbidden = match &ty.inner {
            TypeInner::Scalar(scalar) => !allowed_scalar(*scalar),
            TypeInner::Vector { scalar, .. } => !allowed_scalar(*scalar),
            TypeInner::Matrix { scalar, .. } => *scalar != F32,
            TypeInner::Atomic(_) => true,
            TypeInner::Pointer { space, .. } | TypeInner::ValuePointer { space, .. } => {
                *space != AddressSpace::Function
            }
            TypeInner::Array { size, .. } => !matches!(size, naga::ArraySize::Constant(_)),
            TypeInner::Struct { members, .. } => {
                members.iter().any(|member| member.binding.is_some())
            }
            TypeInner::Image {
                dim,
                arrayed,
                class,
            } => {
                *dim != ImageDimension::D2
                    || *arrayed
                    || !matches!(
                        class,
                        ImageClass::Sampled {
                            kind: ScalarKind::Float,
                            multi: false
                        }
                    )
            }
            TypeInner::Sampler { comparison } => *comparison,
            TypeInner::AccelerationStructure { .. }
            | TypeInner::RayQuery { .. }
            | TypeInner::CooperativeMatrix { .. }
            | TypeInner::BindingArray { .. } => true,
        };
        if forbidden {
            let rule = if type_uses_optional_extension(&ty.inner) {
                AppearanceSourceRule::OptionalExtension
            } else {
                AppearanceSourceRule::ForbiddenType
            };
            return Err(source_rejected(
                rule,
                format!(
                    "type `{}` is outside the appearance allowlist",
                    ty.name.as_deref().unwrap_or("<unnamed>")
                ),
                module.types.get_span(handle),
                source,
            ));
        }
    }
    Ok(())
}

fn inspect_source_functions(
    module: &Module,
    source: &str,
) -> Result<(), Appearance3DAdmissionError> {
    for (function_handle, function) in module.functions.iter() {
        if function
            .arguments
            .iter()
            .any(|argument| argument.binding.is_some())
            || function
                .result
                .as_ref()
                .is_some_and(|result| result.binding.is_some())
        {
            return Err(source_rejected(
                AppearanceSourceRule::ForbiddenType,
                "interface bindings on source helper functions are forbidden",
                module.functions.get_span(function_handle),
                source,
            ));
        }
        for (expression_handle, expression) in function.expressions.iter() {
            let detail = match expression {
                Expression::Override(_) => Some("override expression"),
                Expression::ImageLoad { .. } => Some("textureLoad"),
                Expression::ImageQuery { .. } => Some("texture query"),
                Expression::ImageSample {
                    gather: Some(_), ..
                } => Some("texture gather"),
                Expression::ImageSample {
                    array_index: Some(_),
                    ..
                } => Some("arrayed texture sampling"),
                Expression::ImageSample {
                    depth_ref: Some(_), ..
                } => Some("depth comparison sampling"),
                Expression::AtomicResult { .. } => Some("atomic result"),
                Expression::CooperativeLoad { .. }
                | Expression::CooperativeMultiplyAdd { .. } => Some("cooperative matrix operation"),
                Expression::WorkGroupUniformLoadResult { .. } => {
                    Some("workgroup uniform-load result")
                }
                Expression::ArrayLength(_) => Some("runtime array length"),
                Expression::RayQueryVertexPositions { .. }
                | Expression::RayQueryProceedResult
                | Expression::RayQueryGetIntersection { .. } => Some("ray-query operation"),
                Expression::SubgroupBallotResult | Expression::SubgroupOperationResult { .. } => {
                    Some("subgroup operation")
                }
                _ => None,
            };
            if let Some(detail) = detail {
                return Err(source_rejected(
                    AppearanceSourceRule::ForbiddenExpression,
                    detail,
                    function.expressions.get_span(expression_handle),
                    source,
                ));
            }
        }
        inspect_source_block(
            &function.body,
            function,
            module.functions.get_span(function_handle),
            source,
        )?;
    }
    Ok(())
}

fn inspect_source_block(
    block: &Block,
    function: &Function,
    function_span: Span,
    source: &str,
) -> Result<(), Appearance3DAdmissionError> {
    for (statement, span) in block.span_iter() {
        let detail = match statement {
            Statement::ControlBarrier(_) | Statement::MemoryBarrier(_) => Some("barrier"),
            Statement::CooperativeStore { .. } => Some("cooperative matrix store"),
            Statement::RayPipelineFunction(_) => Some("ray-pipeline operation"),
            Statement::ImageStore { .. } => Some("texture store"),
            Statement::Atomic { .. } | Statement::ImageAtomic { .. } => Some("atomic operation"),
            Statement::WorkGroupUniformLoad { .. } => Some("workgroup uniform load"),
            Statement::RayQuery { .. } => Some("ray-query operation"),
            Statement::SubgroupBallot { .. }
            | Statement::SubgroupGather { .. }
            | Statement::SubgroupCollectiveOperation { .. } => Some("subgroup operation"),
            _ => None,
        };
        if let Some(detail) = detail {
            let source_span = if span.is_defined() {
                *span
            } else {
                function_span
            };
            return Err(source_rejected(
                AppearanceSourceRule::ForbiddenStatement,
                detail,
                source_span,
                source,
            ));
        }
        match statement {
            Statement::Block(nested) => {
                inspect_source_block(nested, function, function_span, source)?
            }
            Statement::If { accept, reject, .. } => {
                inspect_source_block(accept, function, function_span, source)?;
                inspect_source_block(reject, function, function_span, source)?;
            }
            Statement::Switch { cases, .. } => {
                for case in cases {
                    inspect_source_block(&case.body, function, function_span, source)?;
                }
            }
            Statement::Loop {
                body, continuing, ..
            } => {
                inspect_source_block(body, function, function_span, source)?;
                inspect_source_block(continuing, function, function_span, source)?;
            }
            _ => {
                let _ = function;
            }
        }
    }
    Ok(())
}

fn allowed_scalar(scalar: Scalar) -> bool {
    matches!(
        scalar,
        Scalar {
            kind: ScalarKind::Float | ScalarKind::Sint | ScalarKind::Uint,
            width: 4
        } | Scalar {
            kind: ScalarKind::Bool,
            width: 1
        }
    )
}

fn type_uses_optional_extension(inner: &TypeInner) -> bool {
    match inner {
        TypeInner::Scalar(scalar)
        | TypeInner::Atomic(scalar)
        | TypeInner::Vector { scalar, .. }
        | TypeInner::Matrix { scalar, .. } => scalar.width != 4 && scalar.kind != ScalarKind::Bool,
        _ => false,
    }
}

fn validate_appearance_signature(
    module: &Module,
    appearance: Handle<Function>,
    source: &str,
) -> Result<(), Appearance3DAdmissionError> {
    let function = &module.functions[appearance];
    let expected = [
        ExpectedType::Vec3F32,
        ExpectedType::Vec3F32,
        ExpectedType::Vec3F32,
        ExpectedType::Vec2F32,
        ExpectedType::Vec4F32,
        ExpectedType::Bool,
        ExpectedType::Vec4F32,
        ExpectedType::Texture2dF32,
        ExpectedType::FilteringSampler,
    ];
    let signature_matches = function.arguments.len() == expected.len()
        && function
            .arguments
            .iter()
            .zip(expected)
            .all(|(argument, expected)| {
                argument.binding.is_none() && expected.matches(module, argument.ty)
            })
        && function.result.as_ref().is_some_and(|result| {
            result.binding.is_none() && ExpectedType::Vec3F32.matches(module, result.ty)
        });
    if !signature_matches {
        return Err(source_rejected(
            AppearanceSourceRule::RequiredFunctionSignature,
            "halcyon_appearance must have the exact SPEC-HAL-355 section 9.3 signature",
            module.functions.get_span(appearance),
            source,
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum ExpectedType {
    Bool,
    Vec2F32,
    Vec3F32,
    Vec4F32,
    Texture2dF32,
    FilteringSampler,
}

impl ExpectedType {
    fn matches(self, module: &Module, handle: Handle<Type>) -> bool {
        match (self, &module.types[handle].inner) {
            (Self::Bool, TypeInner::Scalar(scalar)) => *scalar == BOOL,
            (
                Self::Vec2F32,
                TypeInner::Vector {
                    size: VectorSize::Bi,
                    scalar,
                },
            )
            | (
                Self::Vec3F32,
                TypeInner::Vector {
                    size: VectorSize::Tri,
                    scalar,
                },
            )
            | (
                Self::Vec4F32,
                TypeInner::Vector {
                    size: VectorSize::Quad,
                    scalar,
                },
            ) => *scalar == F32,
            (
                Self::Texture2dF32,
                TypeInner::Image {
                    dim: ImageDimension::D2,
                    arrayed: false,
                    class:
                        ImageClass::Sampled {
                            kind: ScalarKind::Float,
                            multi: false,
                        },
                },
            ) => true,
            (Self::FilteringSampler, TypeInner::Sampler { comparison: false }) => true,
            _ => false,
        }
    }
}

fn appearance_semantic_reads(
    function: &Function,
    info: &naga::valid::FunctionInfo,
) -> GeometrySemantics {
    let mut normal = false;
    let mut uv = false;
    let mut color = false;
    for (handle, expression) in function.expressions.iter() {
        if info[handle].ref_count == 0 {
            continue;
        }
        match expression {
            Expression::FunctionArgument(1) => normal = true,
            Expression::FunctionArgument(3) => uv = true,
            Expression::FunctionArgument(4) => color = true,
            _ => {}
        }
    }
    GeometrySemantics::new(normal, uv, color)
}

fn module_nesting(module: &Module) -> Result<u32, Appearance3DAdmissionError> {
    let mut maximum = 0_u32;
    for (_, function) in module.functions.iter() {
        maximum = maximum.max(block_nesting(&function.body, 1)?);
    }
    for entry in &module.entry_points {
        maximum = maximum.max(block_nesting(&entry.function.body, 1)?);
    }
    Ok(maximum)
}

fn block_nesting(block: &Block, depth: u32) -> Result<u32, Appearance3DAdmissionError> {
    let mut maximum = depth;
    let nested_depth = depth
        .checked_add(1)
        .ok_or(Appearance3DAdmissionError::ArithmeticOverflow)?;
    for statement in block.iter() {
        match statement {
            Statement::Block(nested) => {
                maximum = maximum.max(block_nesting(nested, nested_depth)?);
            }
            Statement::If { accept, reject, .. } => {
                maximum = maximum.max(block_nesting(accept, nested_depth)?);
                maximum = maximum.max(block_nesting(reject, nested_depth)?);
            }
            Statement::Switch { cases, .. } => {
                for case in cases {
                    maximum = maximum.max(block_nesting(&case.body, nested_depth)?);
                }
            }
            Statement::Loop {
                body, continuing, ..
            } => {
                maximum = maximum.max(block_nesting(body, nested_depth)?);
                maximum = maximum.max(block_nesting(continuing, nested_depth)?);
            }
            _ => {}
        }
    }
    Ok(maximum)
}

fn module_ir_nodes(module: &Module) -> Result<u64, Appearance3DAdmissionError> {
    let mut nodes = 0_u64;
    add_len(&mut nodes, module.types.len())?;
    for (_, ty) in module.types.iter() {
        if let TypeInner::Struct { members, .. } = &ty.inner {
            add_len(&mut nodes, members.len())?;
        }
    }
    add_len(&mut nodes, module.constants.len())?;
    add_len(&mut nodes, module.overrides.len())?;
    add_len(&mut nodes, module.global_variables.len())?;
    add_len(&mut nodes, module.global_expressions.len())?;
    add_len(&mut nodes, module.functions.len())?;
    add_len(&mut nodes, module.entry_points.len())?;
    add_len(&mut nodes, module.diagnostic_filters.len())?;
    for (_, function) in module.functions.iter() {
        add_function_nodes(&mut nodes, function)?;
    }
    for entry in &module.entry_points {
        add_function_nodes(&mut nodes, &entry.function)?;
    }
    Ok(nodes)
}

fn add_function_nodes(
    nodes: &mut u64,
    function: &Function,
) -> Result<(), Appearance3DAdmissionError> {
    add_len(nodes, function.arguments.len())?;
    if function.result.is_some() {
        *nodes = nodes
            .checked_add(1)
            .ok_or(Appearance3DAdmissionError::ArithmeticOverflow)?;
    }
    add_len(nodes, function.local_variables.len())?;
    add_len(nodes, function.expressions.len())?;
    add_len(nodes, function.named_expressions.len())?;
    add_block_nodes(nodes, &function.body)
}

fn add_block_nodes(nodes: &mut u64, block: &Block) -> Result<(), Appearance3DAdmissionError> {
    add_len(nodes, block.len())?;
    for statement in block.iter() {
        match statement {
            Statement::Block(nested) => add_block_nodes(nodes, nested)?,
            Statement::If { accept, reject, .. } => {
                add_block_nodes(nodes, accept)?;
                add_block_nodes(nodes, reject)?;
            }
            Statement::Switch { cases, .. } => {
                for case in cases {
                    add_block_nodes(nodes, &case.body)?;
                }
            }
            Statement::Loop {
                body, continuing, ..
            } => {
                add_block_nodes(nodes, body)?;
                add_block_nodes(nodes, continuing)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn add_len(nodes: &mut u64, length: usize) -> Result<(), Appearance3DAdmissionError> {
    let length =
        u64::try_from(length).map_err(|_| Appearance3DAdmissionError::ArithmeticOverflow)?;
    *nodes = nodes
        .checked_add(length)
        .ok_or(Appearance3DAdmissionError::ArithmeticOverflow)?;
    Ok(())
}

struct SourceSpanSnapshot {
    type_spans: Vec<Span>,
    constant_spans: Vec<Span>,
    override_spans: Vec<Span>,
    global_spans: Vec<Span>,
    global_expression_spans: Vec<Span>,
    function_spans: Vec<Span>,
}

impl SourceSpanSnapshot {
    fn capture(module: &Module) -> Self {
        Self {
            type_spans: module
                .types
                .iter()
                .map(|(handle, _)| module.types.get_span(handle))
                .collect(),
            constant_spans: module
                .constants
                .iter()
                .map(|(handle, _)| module.constants.get_span(handle))
                .collect(),
            override_spans: module
                .overrides
                .iter()
                .map(|(handle, _)| module.overrides.get_span(handle))
                .collect(),
            global_spans: module
                .global_variables
                .iter()
                .map(|(handle, _)| module.global_variables.get_span(handle))
                .collect(),
            global_expression_spans: module
                .global_expressions
                .iter()
                .map(|(handle, _)| module.global_expressions.get_span(handle))
                .collect(),
            function_spans: module
                .functions
                .iter()
                .map(|(handle, _)| module.functions.get_span(handle))
                .collect(),
        }
    }

    fn matches(&self, module: &Module) -> bool {
        type_spans_match(&self.type_spans, &module.types)
            && spans_match(&self.constant_spans, &module.constants)
            && spans_match(&self.override_spans, &module.overrides)
            && spans_match(&self.global_spans, &module.global_variables)
            && spans_match(&self.global_expression_spans, &module.global_expressions)
            && spans_match(&self.function_spans, &module.functions)
    }
}

fn type_spans_match(expected: &[Span], arena: &naga::UniqueArena<Type>) -> bool {
    expected.len() <= arena.len()
        && arena
            .iter()
            .take(expected.len())
            .zip(expected)
            .all(|((handle, _), expected)| arena.get_span(handle) == *expected)
}

fn spans_match<T>(expected: &[Span], arena: &Arena<T>) -> bool {
    expected.len() <= arena.len()
        && arena
            .iter()
            .take(expected.len())
            .zip(expected)
            .all(|((handle, _), expected)| arena.get_span(handle) == *expected)
}

struct FragmentSyntheticHandles {
    types: Vec<Handle<Type>>,
    globals: Vec<Handle<GlobalVariable>>,
    view_block: Handle<Type>,
    appearance_block: Handle<Type>,
}

impl FragmentSyntheticHandles {
    fn spans_are_undefined(&self, module: &Module) -> bool {
        self.types
            .iter()
            .all(|handle| module.types.get_span(*handle) == Span::UNDEFINED)
            && self
                .globals
                .iter()
                .all(|handle| module.global_variables.get_span(*handle) == Span::UNDEFINED)
            && module.entry_points.last().is_some_and(|entry| {
                entry.function.expressions.iter().all(|(handle, _)| {
                    entry.function.expressions.get_span(handle) == Span::UNDEFINED
                }) && entry
                    .function
                    .body
                    .span_iter()
                    .all(|(_, span)| *span == Span::UNDEFINED)
            })
    }
}

struct VertexSyntheticHandles {
    types: Vec<Handle<Type>>,
    globals: Vec<Handle<GlobalVariable>>,
    view_block: Handle<Type>,
    object_block: Handle<Type>,
}

impl VertexSyntheticHandles {
    fn spans_are_undefined(&self, module: &Module) -> bool {
        self.types
            .iter()
            .all(|handle| module.types.get_span(*handle) == Span::UNDEFINED)
            && self
                .globals
                .iter()
                .all(|handle| module.global_variables.get_span(*handle) == Span::UNDEFINED)
            && module.entry_points.first().is_some_and(|entry| {
                entry.function.expressions.iter().all(|(handle, _)| {
                    entry.function.expressions.get_span(handle) == Span::UNDEFINED
                }) && entry
                    .function
                    .body
                    .span_iter()
                    .all(|(_, span)| *span == Span::UNDEFINED)
            })
    }
}

fn append_fragment_wrapper(
    module: &mut Module,
    appearance: Handle<Function>,
) -> Result<FragmentSyntheticHandles, Appearance3DAdmissionError> {
    let mut synthetic_types = Vec::new();
    let f32_ty = insert_synthetic_type(
        module,
        "__halcyon355_fragment_f32",
        TypeInner::Scalar(F32),
        &mut synthetic_types,
    );
    let bool_ty = insert_synthetic_type(
        module,
        "__halcyon355_fragment_bool",
        TypeInner::Scalar(BOOL),
        &mut synthetic_types,
    );
    let vec2_ty = insert_synthetic_type(
        module,
        "__halcyon355_fragment_vec2_f32",
        TypeInner::Vector {
            size: VectorSize::Bi,
            scalar: F32,
        },
        &mut synthetic_types,
    );
    let vec3_ty = insert_synthetic_type(
        module,
        "__halcyon355_fragment_vec3_f32",
        TypeInner::Vector {
            size: VectorSize::Tri,
            scalar: F32,
        },
        &mut synthetic_types,
    );
    let vec4_ty = insert_synthetic_type(
        module,
        "__halcyon355_fragment_vec4_f32",
        TypeInner::Vector {
            size: VectorSize::Quad,
            scalar: F32,
        },
        &mut synthetic_types,
    );
    let mat4_ty = insert_synthetic_type(
        module,
        "__halcyon355_fragment_mat4_f32",
        TypeInner::Matrix {
            columns: VectorSize::Quad,
            rows: VectorSize::Quad,
            scalar: F32,
        },
        &mut synthetic_types,
    );
    let view_block = insert_synthetic_type(
        module,
        "__halcyon355_fragment_view_block",
        TypeInner::Struct {
            members: vec![
                StructMember {
                    name: Some("view_projection".into()),
                    ty: mat4_ty,
                    binding: None,
                    offset: 0,
                },
                StructMember {
                    name: Some("camera_relative".into()),
                    ty: vec3_ty,
                    binding: None,
                    offset: 64,
                },
                StructMember {
                    name: Some("padding".into()),
                    ty: f32_ty,
                    binding: None,
                    offset: 76,
                },
            ],
            span: u32::try_from(VIEW_BLOCK_SIZE)
                .map_err(|_| Appearance3DAdmissionError::ArithmeticOverflow)?,
        },
        &mut synthetic_types,
    );
    let appearance_block = insert_synthetic_type(
        module,
        "__halcyon355_fragment_appearance_block",
        TypeInner::Struct {
            members: vec![StructMember {
                name: Some("color_factor".into()),
                ty: vec4_ty,
                binding: None,
                offset: 0,
            }],
            span: u32::try_from(APPEARANCE_BLOCK_SIZE)
                .map_err(|_| Appearance3DAdmissionError::ArithmeticOverflow)?,
        },
        &mut synthetic_types,
    );
    let texture_ty = insert_synthetic_type(
        module,
        "__halcyon355_fragment_texture_2d_f32",
        TypeInner::Image {
            dim: ImageDimension::D2,
            arrayed: false,
            class: ImageClass::Sampled {
                kind: ScalarKind::Float,
                multi: false,
            },
        },
        &mut synthetic_types,
    );
    let sampler_ty = insert_synthetic_type(
        module,
        "__halcyon355_fragment_filtering_sampler",
        TypeInner::Sampler { comparison: false },
        &mut synthetic_types,
    );

    let view_global = module.global_variables.append(
        GlobalVariable {
            name: Some("__halcyon355_view".into()),
            space: AddressSpace::Uniform,
            binding: Some(ResourceBinding {
                group: 0,
                binding: 0,
            }),
            ty: view_block,
            init: None,
            memory_decorations: naga::MemoryDecorations::empty(),
        },
        Span::UNDEFINED,
    );
    let appearance_global = module.global_variables.append(
        GlobalVariable {
            name: Some("__halcyon355_appearance".into()),
            space: AddressSpace::Uniform,
            binding: Some(ResourceBinding {
                group: 2,
                binding: 0,
            }),
            ty: appearance_block,
            init: None,
            memory_decorations: naga::MemoryDecorations::empty(),
        },
        Span::UNDEFINED,
    );
    let texture_global = module.global_variables.append(
        GlobalVariable {
            name: Some("__halcyon355_texture".into()),
            space: AddressSpace::Handle,
            binding: Some(ResourceBinding {
                group: 2,
                binding: 1,
            }),
            ty: texture_ty,
            init: None,
            memory_decorations: naga::MemoryDecorations::empty(),
        },
        Span::UNDEFINED,
    );
    let sampler_global = module.global_variables.append(
        GlobalVariable {
            name: Some("__halcyon355_sampler".into()),
            space: AddressSpace::Handle,
            binding: Some(ResourceBinding {
                group: 2,
                binding: 2,
            }),
            ty: sampler_ty,
            init: None,
            memory_decorations: naga::MemoryDecorations::empty(),
        },
        Span::UNDEFINED,
    );

    let mut function = Function {
        name: Some(FRAGMENT_ENTRY_POINT_NAME.into()),
        arguments: vec![
            entry_argument("position_render_relative", vec3_ty, location_binding(0)),
            entry_argument("normal_world_interpolated", vec3_ty, location_binding(1)),
            entry_argument("uv", vec2_ty, location_binding(2)),
            entry_argument("vertex_color_linear", vec4_ty, location_binding(3)),
            entry_argument(
                "front_facing",
                bool_ty,
                Binding::BuiltIn(BuiltIn::FrontFacing),
            ),
        ],
        result: Some(FunctionResult {
            ty: vec4_ty,
            binding: Some(Binding::Location {
                location: 0,
                interpolation: None,
                sampling: None,
                blend_src: None,
                per_primitive: false,
            }),
        }),
        ..Function::default()
    };

    let position = append_expression(&mut function, Expression::FunctionArgument(0));
    let normal = append_expression(&mut function, Expression::FunctionArgument(1));
    let uv = append_expression(&mut function, Expression::FunctionArgument(2));
    let color = append_expression(&mut function, Expression::FunctionArgument(3));
    let front_facing = append_expression(&mut function, Expression::FunctionArgument(4));
    let view = append_expression(&mut function, Expression::GlobalVariable(view_global));
    let appearance_uniform =
        append_expression(&mut function, Expression::GlobalVariable(appearance_global));
    let texture = append_expression(&mut function, Expression::GlobalVariable(texture_global));
    let sampler = append_expression(&mut function, Expression::GlobalVariable(sampler_global));

    let mut emitter = naga::proc::Emitter::default();
    emitter.start(&function.expressions);
    let camera_pointer = append_expression(
        &mut function,
        Expression::AccessIndex {
            base: view,
            index: 1,
        },
    );
    let camera = append_expression(
        &mut function,
        Expression::Load {
            pointer: camera_pointer,
        },
    );
    let view_delta = append_expression(
        &mut function,
        Expression::Binary {
            op: naga::BinaryOperator::Subtract,
            left: camera,
            right: position,
        },
    );
    let view_direction = append_expression(
        &mut function,
        Expression::Math {
            fun: MathFunction::Normalize,
            arg: view_delta,
            arg1: None,
            arg2: None,
            arg3: None,
        },
    );
    let color_factor_pointer = append_expression(
        &mut function,
        Expression::AccessIndex {
            base: appearance_uniform,
            index: 0,
        },
    );
    let color_factor = append_expression(
        &mut function,
        Expression::Load {
            pointer: color_factor_pointer,
        },
    );
    function.body.extend(emitter.finish(&function.expressions));

    let call_result = append_expression(&mut function, Expression::CallResult(appearance));
    function.body.push(
        Statement::Call {
            function: appearance,
            arguments: vec![
                position,
                normal,
                view_direction,
                uv,
                color,
                front_facing,
                color_factor,
                texture,
                sampler,
            ],
            result: Some(call_result),
        },
        Span::UNDEFINED,
    );

    let zero = append_expression(&mut function, Expression::ZeroValue(vec3_ty));
    let one = append_expression(&mut function, Expression::Literal(naga::Literal::F32(1.0)));
    let largest_finite = append_expression(
        &mut function,
        Expression::Literal(naga::Literal::F32(f32::MAX)),
    );
    emitter.start(&function.expressions);
    let magnitude = append_expression(
        &mut function,
        Expression::Math {
            fun: MathFunction::Abs,
            arg: call_result,
            arg1: None,
            arg2: None,
            arg3: None,
        },
    );
    let largest_finite_vector = append_expression(
        &mut function,
        Expression::Splat {
            size: VectorSize::Tri,
            value: largest_finite,
        },
    );
    let is_finite = append_expression(
        &mut function,
        Expression::Binary {
            op: naga::BinaryOperator::LessEqual,
            left: magnitude,
            right: largest_finite_vector,
        },
    );
    let finite = append_expression(
        &mut function,
        Expression::Select {
            condition: is_finite,
            accept: call_result,
            reject: zero,
        },
    );
    let ones = append_expression(
        &mut function,
        Expression::Splat {
            size: VectorSize::Tri,
            value: one,
        },
    );
    let clamped = append_expression(
        &mut function,
        Expression::Math {
            fun: MathFunction::Clamp,
            arg: finite,
            arg1: Some(zero),
            arg2: Some(ones),
            arg3: None,
        },
    );
    let output = append_expression(
        &mut function,
        Expression::Compose {
            ty: vec4_ty,
            components: vec![clamped, one],
        },
    );
    function.body.extend(emitter.finish(&function.expressions));
    function.body.push(
        Statement::Return {
            value: Some(output),
        },
        Span::UNDEFINED,
    );

    module.entry_points.push(EntryPoint {
        name: FRAGMENT_ENTRY_POINT_NAME.into(),
        stage: ShaderStage::Fragment,
        early_depth_test: None,
        workgroup_size: [0, 0, 0],
        workgroup_size_overrides: None,
        function,
        mesh_info: None,
        task_payload: None,
        incoming_ray_payload: None,
    });

    Ok(FragmentSyntheticHandles {
        types: synthetic_types,
        globals: vec![
            view_global,
            appearance_global,
            texture_global,
            sampler_global,
        ],
        view_block,
        appearance_block,
    })
}

fn build_vertex_module() -> Result<(Module, VertexSyntheticHandles), Appearance3DAdmissionError> {
    let mut module = Module::default();
    let mut synthetic_types = Vec::new();
    let f32_ty = insert_synthetic_type(
        &mut module,
        "__halcyon355_vertex_f32",
        TypeInner::Scalar(F32),
        &mut synthetic_types,
    );
    let vec2_ty = insert_synthetic_type(
        &mut module,
        "__halcyon355_vertex_vec2_f32",
        TypeInner::Vector {
            size: VectorSize::Bi,
            scalar: F32,
        },
        &mut synthetic_types,
    );
    let vec3_ty = insert_synthetic_type(
        &mut module,
        "__halcyon355_vertex_vec3_f32",
        TypeInner::Vector {
            size: VectorSize::Tri,
            scalar: F32,
        },
        &mut synthetic_types,
    );
    let vec4_ty = insert_synthetic_type(
        &mut module,
        "__halcyon355_vertex_vec4_f32",
        TypeInner::Vector {
            size: VectorSize::Quad,
            scalar: F32,
        },
        &mut synthetic_types,
    );
    let mat3_ty = insert_synthetic_type(
        &mut module,
        "__halcyon355_vertex_mat3_f32",
        TypeInner::Matrix {
            columns: VectorSize::Tri,
            rows: VectorSize::Tri,
            scalar: F32,
        },
        &mut synthetic_types,
    );
    let mat4_ty = insert_synthetic_type(
        &mut module,
        "__halcyon355_vertex_mat4_f32",
        TypeInner::Matrix {
            columns: VectorSize::Quad,
            rows: VectorSize::Quad,
            scalar: F32,
        },
        &mut synthetic_types,
    );
    let view_block = insert_synthetic_type(
        &mut module,
        "__halcyon355_vertex_view_block",
        TypeInner::Struct {
            members: vec![
                StructMember {
                    name: Some("view_projection".into()),
                    ty: mat4_ty,
                    binding: None,
                    offset: 0,
                },
                StructMember {
                    name: Some("camera_relative".into()),
                    ty: vec3_ty,
                    binding: None,
                    offset: 64,
                },
                StructMember {
                    name: Some("padding".into()),
                    ty: f32_ty,
                    binding: None,
                    offset: 76,
                },
            ],
            span: u32::try_from(VIEW_BLOCK_SIZE)
                .map_err(|_| Appearance3DAdmissionError::ArithmeticOverflow)?,
        },
        &mut synthetic_types,
    );
    let object_block = insert_synthetic_type(
        &mut module,
        "__halcyon355_vertex_object_block",
        TypeInner::Struct {
            members: vec![
                StructMember {
                    name: Some("model_relative".into()),
                    ty: mat4_ty,
                    binding: None,
                    offset: 0,
                },
                StructMember {
                    name: Some("normal_to_world".into()),
                    ty: mat3_ty,
                    binding: None,
                    offset: 64,
                },
            ],
            span: u32::try_from(OBJECT_BLOCK_SIZE)
                .map_err(|_| Appearance3DAdmissionError::ArithmeticOverflow)?,
        },
        &mut synthetic_types,
    );
    let output_ty = insert_synthetic_type(
        &mut module,
        "__halcyon355_vertex_output",
        TypeInner::Struct {
            members: vec![
                StructMember {
                    name: Some("clip_position".into()),
                    ty: vec4_ty,
                    binding: Some(Binding::BuiltIn(BuiltIn::Position { invariant: false })),
                    offset: 0,
                },
                StructMember {
                    name: Some("position_render_relative".into()),
                    ty: vec3_ty,
                    binding: Some(location_binding(0)),
                    offset: 16,
                },
                StructMember {
                    name: Some("normal_world_interpolated".into()),
                    ty: vec3_ty,
                    binding: Some(location_binding(1)),
                    offset: 32,
                },
                StructMember {
                    name: Some("uv".into()),
                    ty: vec2_ty,
                    binding: Some(location_binding(2)),
                    offset: 48,
                },
                StructMember {
                    name: Some("vertex_color_linear".into()),
                    ty: vec4_ty,
                    binding: Some(location_binding(3)),
                    offset: 64,
                },
            ],
            span: 80,
        },
        &mut synthetic_types,
    );

    let view_global = module.global_variables.append(
        GlobalVariable {
            name: Some("__halcyon355_view".into()),
            space: AddressSpace::Uniform,
            binding: Some(ResourceBinding {
                group: 0,
                binding: 0,
            }),
            ty: view_block,
            init: None,
            memory_decorations: naga::MemoryDecorations::empty(),
        },
        Span::UNDEFINED,
    );
    let object_global = module.global_variables.append(
        GlobalVariable {
            name: Some("__halcyon355_object".into()),
            space: AddressSpace::Uniform,
            binding: Some(ResourceBinding {
                group: 1,
                binding: 0,
            }),
            ty: object_block,
            init: None,
            memory_decorations: naga::MemoryDecorations::empty(),
        },
        Span::UNDEFINED,
    );

    let mut function = Function {
        name: Some(VERTEX_ENTRY_POINT_NAME.into()),
        arguments: vec![
            vertex_argument("local_position", vec3_ty, 0),
            vertex_argument("local_normal", vec3_ty, 1),
            vertex_argument("uv", vec2_ty, 2),
            vertex_argument("vertex_color_linear", vec4_ty, 3),
        ],
        result: Some(FunctionResult {
            ty: output_ty,
            binding: None,
        }),
        ..Function::default()
    };
    let local_position = append_expression(&mut function, Expression::FunctionArgument(0));
    let local_normal = append_expression(&mut function, Expression::FunctionArgument(1));
    let uv = append_expression(&mut function, Expression::FunctionArgument(2));
    let color = append_expression(&mut function, Expression::FunctionArgument(3));
    let view = append_expression(&mut function, Expression::GlobalVariable(view_global));
    let object = append_expression(&mut function, Expression::GlobalVariable(object_global));
    let one = append_expression(&mut function, Expression::Literal(naga::Literal::F32(1.0)));

    let mut emitter = naga::proc::Emitter::default();
    emitter.start(&function.expressions);
    let view_projection_pointer = append_expression(
        &mut function,
        Expression::AccessIndex {
            base: view,
            index: 0,
        },
    );
    let view_projection = append_expression(
        &mut function,
        Expression::Load {
            pointer: view_projection_pointer,
        },
    );
    let model_pointer = append_expression(
        &mut function,
        Expression::AccessIndex {
            base: object,
            index: 0,
        },
    );
    let model = append_expression(
        &mut function,
        Expression::Load {
            pointer: model_pointer,
        },
    );
    let normal_matrix_pointer = append_expression(
        &mut function,
        Expression::AccessIndex {
            base: object,
            index: 1,
        },
    );
    let normal_matrix = append_expression(
        &mut function,
        Expression::Load {
            pointer: normal_matrix_pointer,
        },
    );
    let local_position4 = append_expression(
        &mut function,
        Expression::Compose {
            ty: vec4_ty,
            components: vec![local_position, one],
        },
    );
    let position4 = append_expression(
        &mut function,
        Expression::Binary {
            op: naga::BinaryOperator::Multiply,
            left: model,
            right: local_position4,
        },
    );
    let position3 = append_expression(
        &mut function,
        Expression::Swizzle {
            size: VectorSize::Tri,
            vector: position4,
            pattern: [
                SwizzleComponent::X,
                SwizzleComponent::Y,
                SwizzleComponent::Z,
                SwizzleComponent::W,
            ],
        },
    );
    let clip_position = append_expression(
        &mut function,
        Expression::Binary {
            op: naga::BinaryOperator::Multiply,
            left: view_projection,
            right: position4,
        },
    );
    let world_normal = append_expression(
        &mut function,
        Expression::Binary {
            op: naga::BinaryOperator::Multiply,
            left: normal_matrix,
            right: local_normal,
        },
    );
    let output = append_expression(
        &mut function,
        Expression::Compose {
            ty: output_ty,
            components: vec![clip_position, position3, world_normal, uv, color],
        },
    );
    function.body.extend(emitter.finish(&function.expressions));
    function.body.push(
        Statement::Return {
            value: Some(output),
        },
        Span::UNDEFINED,
    );

    module.entry_points.push(EntryPoint {
        name: VERTEX_ENTRY_POINT_NAME.into(),
        stage: ShaderStage::Vertex,
        early_depth_test: None,
        workgroup_size: [0, 0, 0],
        workgroup_size_overrides: None,
        function,
        mesh_info: None,
        task_payload: None,
        incoming_ray_payload: None,
    });

    Ok((
        module,
        VertexSyntheticHandles {
            types: synthetic_types,
            globals: vec![view_global, object_global],
            view_block,
            object_block,
        },
    ))
}

fn insert_synthetic_type(
    module: &mut Module,
    name: &'static str,
    inner: TypeInner,
    handles: &mut Vec<Handle<Type>>,
) -> Handle<Type> {
    let handle = module.types.insert(
        Type {
            name: Some(name.into()),
            inner,
        },
        Span::UNDEFINED,
    );
    handles.push(handle);
    handle
}

fn append_expression(function: &mut Function, expression: Expression) -> Handle<Expression> {
    function.expressions.append(expression, Span::UNDEFINED)
}

fn location_binding(location: u32) -> Binding {
    Binding::Location {
        location,
        interpolation: Some(Interpolation::Perspective),
        sampling: Some(Sampling::Center),
        blend_src: None,
        per_primitive: false,
    }
}

fn entry_argument(name: &'static str, ty: Handle<Type>, binding: Binding) -> FunctionArgument {
    FunctionArgument {
        name: Some(name.into()),
        ty,
        binding: Some(binding),
    }
}

fn vertex_argument(name: &'static str, ty: Handle<Type>, location: u32) -> FunctionArgument {
    FunctionArgument {
        name: Some(name.into()),
        ty,
        binding: Some(Binding::Location {
            location,
            interpolation: None,
            sampling: None,
            blend_src: None,
            per_primitive: false,
        }),
    }
}

fn validate_fragment_layout(
    module: &Module,
    view_block: Handle<Type>,
    appearance_block: Handle<Type>,
) -> Result<(), Appearance3DAdmissionError> {
    let mut layouter = Layouter::default();
    layouter
        .update(module.to_ctx())
        .map_err(|error| internal_error("fragment layout", error))?;
    if u64::from(layouter[view_block].size) != VIEW_BLOCK_SIZE
        || u64::from(layouter[appearance_block].size) != APPEARANCE_BLOCK_SIZE
    {
        return Err(Appearance3DAdmissionError::InternalCompilerAdapter {
            stage: "fragment layout",
            detail: Arc::from("Naga layout does not match the 80/16-byte binding ABI"),
        });
    }
    Ok(())
}

fn validate_vertex_layout(
    module: &Module,
    view_block: Handle<Type>,
    object_block: Handle<Type>,
) -> Result<(), Appearance3DAdmissionError> {
    let mut layouter = Layouter::default();
    layouter
        .update(module.to_ctx())
        .map_err(|error| internal_error("vertex layout", error))?;
    if u64::from(layouter[view_block].size) != VIEW_BLOCK_SIZE
        || u64::from(layouter[object_block].size) != OBJECT_BLOCK_SIZE
    {
        return Err(Appearance3DAdmissionError::InternalCompilerAdapter {
            stage: "vertex layout",
            detail: Arc::from("Naga layout does not match the 80/112-byte binding ABI"),
        });
    }
    Ok(())
}

fn validate_fragment_structure(
    module: &Module,
    appearance: Handle<Function>,
) -> Result<(), Appearance3DAdmissionError> {
    if module.entry_points.len() != 1 {
        return Err(internal_structure(
            "fragment module does not have one entry point",
        ));
    }
    let entry = &module.entry_points[0];
    if entry.name != FRAGMENT_ENTRY_POINT_NAME || entry.stage != ShaderStage::Fragment {
        return Err(internal_structure(
            "fragment entry point identity is incorrect",
        ));
    }
    if module.global_variables.len() != 4 {
        return Err(internal_structure(
            "fragment resource-global count is incorrect",
        ));
    }
    let expected = [
        (0, 0, AddressSpace::Uniform),
        (2, 0, AddressSpace::Uniform),
        (2, 1, AddressSpace::Handle),
        (2, 2, AddressSpace::Handle),
    ];
    for ((_, global), (group, binding, space)) in module.global_variables.iter().zip(expected) {
        if global.space != space || global.binding != Some(ResourceBinding { group, binding }) {
            return Err(internal_structure("fragment resource binding is incorrect"));
        }
    }
    let calls_appearance = entry
        .function
        .body
        .iter()
        .any(|statement| matches!(statement, Statement::Call { function, .. } if *function == appearance));
    if !calls_appearance {
        return Err(internal_structure(
            "fragment entry point does not call the admitted appearance handle",
        ));
    }
    if entry.function.arguments.len() != 5
        || entry
            .function
            .result
            .as_ref()
            .and_then(|result| result.binding.as_ref())
            != Some(&Binding::Location {
                location: 0,
                interpolation: None,
                sampling: None,
                blend_src: None,
                per_primitive: false,
            })
    {
        return Err(internal_structure("fragment interface is incorrect"));
    }
    Ok(())
}

fn validate_vertex_structure(module: &Module) -> Result<(), Appearance3DAdmissionError> {
    if module.entry_points.len() != 1 {
        return Err(internal_structure(
            "vertex module does not have one entry point",
        ));
    }
    let entry = &module.entry_points[0];
    if entry.name != VERTEX_ENTRY_POINT_NAME
        || entry.stage != ShaderStage::Vertex
        || entry.function.arguments.len() != 4
        || module.global_variables.len() != 2
    {
        return Err(internal_structure("vertex interface is incorrect"));
    }
    let expected = [(0, 0, AddressSpace::Uniform), (1, 0, AddressSpace::Uniform)];
    for ((_, global), (group, binding, space)) in module.global_variables.iter().zip(expected) {
        if global.space != space || global.binding != Some(ResourceBinding { group, binding }) {
            return Err(internal_structure("vertex resource binding is incorrect"));
        }
    }
    Ok(())
}

fn internal_structure(detail: &'static str) -> Appearance3DAdmissionError {
    Appearance3DAdmissionError::InternalCompilerAdapter {
        stage: "structural inspection",
        detail: Arc::from(detail),
    }
}
