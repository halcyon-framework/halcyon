use halcyon::three_d::{Context3DError, PolicyBudget3D, PolicyLimits3D};

fn pre_domain_policy_literal() -> PolicyLimits3D {
    PolicyLimits3D {
        max_live_views: 16,
        max_active_target_pixels: 67_108_864,
        max_target_dimension_2d: 8_192,
        max_single_target_attachment_bytes: 536_870_912,
        max_total_attachment_bytes: 1_073_741_824,
        max_mesh_bytes: 67_108_864,
        max_vertices_per_mesh: 1_000_000,
        max_indices_per_mesh: 3_000_000,
        max_resident_geometry_bytes: 536_870_912,
        max_texture_dimension_2d: 8_192,
        max_texture_bytes: 268_435_456,
        max_resident_texture_bytes: 536_870_912,
        max_draw_packets_per_snapshot: 100_000,
        max_upload_bytes_per_frame: 67_108_864,
        max_dynamic_storage_bytes_per_target: 67_108_864,
        max_total_dynamic_storage_bytes: 536_870_912,
        max_total_dynamic_object_capacity: 1_000_000,
        max_dynamic_storage_slots: 64,
        max_retained_dynamic_cpu_bytes: 1_073_741_824,
        max_dynamic_upload_bytes_per_frame: 67_108_864,
        max_retained_model_states: 256,
        max_resource_slots: 65_536,
        max_retained_generations: 131_072,
        max_appearance_source_bytes: 65_536,
        max_appearance_functions: 128,
        max_appearance_nesting: 32,
        max_appearance_ir_nodes: 262_144,
        max_registered_appearances: 256,
        max_shader_modules: 256,
        max_pipeline_variants: 2_048,
        max_pending_requests: 64,
        max_pending_publications: 16,
        max_cache_residency_bytes: 536_870_912,
    }
}

fn pre_domain_budget_name(budget: PolicyBudget3D) -> &'static str {
    match budget {
        PolicyBudget3D::LiveViews => "LiveViews",
        PolicyBudget3D::TargetDimension2D => "TargetDimension2D",
        PolicyBudget3D::ActiveTargetPixels => "ActiveTargetPixels",
        PolicyBudget3D::SingleTargetAttachmentBytes => "SingleTargetAttachmentBytes",
        PolicyBudget3D::TotalAttachmentBytes => "TotalAttachmentBytes",
        PolicyBudget3D::MeshBytes => "MeshBytes",
        PolicyBudget3D::MeshVertices => "MeshVertices",
        PolicyBudget3D::MeshIndices => "MeshIndices",
        PolicyBudget3D::ResidentGeometryBytes => "ResidentGeometryBytes",
        PolicyBudget3D::DrawPacketsPerSnapshot => "DrawPacketsPerSnapshot",
        PolicyBudget3D::UploadBytesPerFrame => "UploadBytesPerFrame",
        PolicyBudget3D::DynamicStorageBytesPerTarget => "DynamicStorageBytesPerTarget",
        PolicyBudget3D::TotalDynamicStorageBytes => "TotalDynamicStorageBytes",
        PolicyBudget3D::TotalDynamicObjectCapacity => "TotalDynamicObjectCapacity",
        PolicyBudget3D::DynamicStorageSlots => "DynamicStorageSlots",
        PolicyBudget3D::RetainedDynamicCpuBytes => "RetainedDynamicCpuBytes",
        PolicyBudget3D::DynamicUploadBytesPerFrame => "DynamicUploadBytesPerFrame",
        PolicyBudget3D::RetainedModelStates => "RetainedModelStates",
        PolicyBudget3D::ResourceSlots => "ResourceSlots",
        PolicyBudget3D::RetainedGenerations => "RetainedGenerations",
        PolicyBudget3D::AppearanceSourceBytes => "AppearanceSourceBytes",
        PolicyBudget3D::RegisteredAppearances => "RegisteredAppearances",
        PolicyBudget3D::ShaderModules => "ShaderModules",
        PolicyBudget3D::PipelineVariants => "PipelineVariants",
        PolicyBudget3D::PendingRequests => "PendingRequests",
        PolicyBudget3D::PendingPublications => "PendingPublications",
        PolicyBudget3D::TextureDimension2D => "TextureDimension2D",
        PolicyBudget3D::TextureBytes => "TextureBytes",
        PolicyBudget3D::ResidentTextureBytes => "ResidentTextureBytes",
        PolicyBudget3D::CacheResidencyBytes => "CacheResidencyBytes",
    }
}

fn pre_domain_context_error_name(error: Context3DError) -> &'static str {
    match error {
        Context3DError::InvalidConfiguration { .. } => "InvalidConfiguration",
        Context3DError::HardDeviceLimitExceeded { .. } => "HardDeviceLimitExceeded",
        Context3DError::PolicyBudgetExceeded { .. } => "PolicyBudgetExceeded",
        Context3DError::ContextMismatch => "ContextMismatch",
        Context3DError::WrongResourceKind { .. } => "WrongResourceKind",
        Context3DError::SlotOutOfRange { .. } => "SlotOutOfRange",
        Context3DError::FreedSlot { .. } => "FreedSlot",
        Context3DError::StaleGeneration { .. } => "StaleGeneration",
        Context3DError::GenerationExhausted { .. } => "GenerationExhausted",
        Context3DError::ArithmeticOverflow { .. } => "ArithmeticOverflow",
        Context3DError::GeometryPacking(_) => "GeometryPacking",
        Context3DError::AppearanceAdmission(_) => "AppearanceAdmission",
        Context3DError::MissingGeometrySemantics { .. } => "MissingGeometrySemantics",
        Context3DError::GpuMaterializationUnavailable => "GpuMaterializationUnavailable",
        Context3DError::RegistryUnavailable => "RegistryUnavailable",
        Context3DError::GeometryCacheUnavailable => "GeometryCacheUnavailable",
        Context3DError::AppearanceCacheUnavailable => "AppearanceCacheUnavailable",
        Context3DError::TextureCacheUnavailable => "TextureCacheUnavailable",
        Context3DError::SamplerCacheUnavailable => "SamplerCacheUnavailable",
        Context3DError::RasterCacheUnavailable => "RasterCacheUnavailable",
        Context3DError::PipelineCacheUnavailable => "PipelineCacheUnavailable",
        Context3DError::ViewportAccountingUnavailable => "ViewportAccountingUnavailable",
        Context3DError::DynamicStateAccountingUnavailable => "DynamicStateAccountingUnavailable",
        Context3DError::PendingWorkAccountingUnavailable => "PendingWorkAccountingUnavailable",
    }
}

#[test]
fn pre_domain_context_policy_surface_remains_source_compatible() {
    let policy = pre_domain_policy_literal();
    assert_eq!(policy.max_live_views, 16);
    assert_eq!(
        pre_domain_budget_name(PolicyBudget3D::RetainedModelStates),
        "RetainedModelStates"
    );
    assert_eq!(
        pre_domain_context_error_name(Context3DError::ContextMismatch),
        "ContextMismatch"
    );
}
