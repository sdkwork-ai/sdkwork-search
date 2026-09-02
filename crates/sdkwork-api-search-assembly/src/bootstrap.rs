//! Authored API assembly bootstrap for sdkwork-search.
//!
//! The assembly owns Search service construction (database pool, provider
//! registry, query/indexing/recommendation/promotion services, Drive document
//! upload adapter), business route composition (app-api + backend-api route
//! crates + the document upload endpoint), and the readiness set
//! (API_ASSEMBLY_SPEC §6.1). The thin standalone gateway calls
//! `assemble_api_router_from_env` and projects `.router` / `.readiness_check`.

use std::sync::Arc;

use async_trait::async_trait;
use axum::{
    extract::Multipart,
    response::{IntoResponse, Response},
    routing::post,
    Extension, Json, Router,
};
use sdkwork_drive_storage_local::LocalDriveObjectStore;
use sdkwork_drive_uploader_service::service::{
    DriveUploaderService, PrepareUploaderUploadCommand, UploadBytesCommand, UploaderActor,
    UploaderRetention, UploaderTarget,
};
use sdkwork_drive_workspace_service::infrastructure::sql::uploader_store::SqlUploaderStore;
use sdkwork_routes_search_app_api::SearchAppState;
use sdkwork_routes_search_backend_api::SearchBackendState;
use sdkwork_search_indexing_repository_sqlx::SearchRepositoryAdapter;
use sdkwork_search_indexing_service::ports::{
    DocumentUploadPort, UploadDocumentRequest, UploadedDocument,
};
use sdkwork_search_indexing_service::IndexingService;
use sdkwork_search_promotion_service::PromotionService;
use sdkwork_search_provider_spi::provider::{SearchProviderConfig, SearchProviderKind};
use sdkwork_search_provider_spi::registry::{
    SearchProviderRegistry, SearchProviderRegistryBuilder,
};
use sdkwork_search_provider_spi::{
    SearchProvider, SearchProviderContext, SearchProviderContextBuilder,
};
use sdkwork_search_query_service::QueryService;
use sdkwork_search_recommendation_service::RecommendationService;
use sdkwork_utils_rust::sha256_hash;
use sdkwork_web_bootstrap::{
    CompositeReadinessCheck, PgPoolReadinessCheck, ReadinessCheck, WebModule,
};
use sdkwork_web_core::HttpRouteManifest;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

pub use sdkwork_web_bootstrap::ApiAssemblyContribution;

/// Indivisible host-neutral API assembly contribution (web-bootstrap contract,
/// API_ASSEMBLY_SPEC.md section 4).
pub type ApiAssembly = ApiAssemblyContribution;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Server configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct SearchApiServerConfig {
    /// Socket address to bind the HTTP listener, e.g. `0.0.0.0:8080`.
    pub bind_addr: String,
    /// PostgreSQL connection URL.
    pub database_url: String,
    /// Provider configuration descriptors consumed by `SearchProviderRegistryBuilder`.
    pub provider_configs: Vec<SearchProviderConfig>,
    /// Drive 文档上传本地对象存储根目录（Phase 5 直传落地目录）。
    pub upload_root_dir: String,
}

impl SearchApiServerConfig {
    /// Load configuration from environment variables with safe development defaults.
    ///
    /// - `SEARCH_API_BIND_ADDR` (default `0.0.0.0:8080`)
    /// - `SDKWORK_DATABASE_*` (canonical workspace PostgreSQL profile)
    /// - `SEARCH_UPLOAD_ROOT_DIR` (default `var/search-uploads`)
    pub fn from_env() -> anyhow::Result<Self> {
        let bind_addr =
            std::env::var("SEARCH_API_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_owned());
        let database =
            sdkwork_database_config::DatabaseConfig::from_env("search").map_err(|error| {
                anyhow::anyhow!("invalid SDKWORK_DATABASE_* configuration: {error}")
            })?;
        if database.engine != sdkwork_database_config::DatabaseEngine::Postgres {
            anyhow::bail!(
                "search standalone gateway authoritative persistence requires PostgreSQL"
            );
        }
        let database_url = database.url;
        let upload_root_dir = std::env::var("SEARCH_UPLOAD_ROOT_DIR")
            .unwrap_or_else(|_| "var/search-uploads".to_owned());
        let provider_configs = vec![SearchProviderConfig {
            kind: SearchProviderKind::Memory,
            id: "memory-default".to_owned(),
            priority: 0,
            enabled: true,
            connection: serde_json::json!({}),
            options: serde_json::json!({}),
        }];
        Ok(Self {
            bind_addr,
            database_url,
            provider_configs,
            upload_root_dir,
        })
    }
}

// ---------------------------------------------------------------------------
// Database
// ---------------------------------------------------------------------------

/// Connect a PostgreSQL connection pool from the supplied URL.
async fn connect_database_pool(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
        .map_err(|err| anyhow::anyhow!("failed to connect to database: {err}"))?;
    tracing::info!("database pool connected");
    Ok(pool)
}

// ---------------------------------------------------------------------------
// Provider registry
// ---------------------------------------------------------------------------

/// Build the `SearchProviderRegistry` from the supplied provider configs.
///
/// The Memory provider is always registered as the default so the server can serve traffic
/// without an external search engine. PostgreSQL provider is registered when a config with
/// `kind = Postgresql` and a valid `connection.url` is supplied.
fn build_provider_registry(
    configs: &[SearchProviderConfig],
) -> anyhow::Result<Arc<SearchProviderRegistry>> {
    let mut builder = SearchProviderRegistryBuilder::default();

    // Always register Memory provider as default.
    let memory_provider: Arc<dyn SearchProvider> = Arc::new(
        sdkwork_search_provider_memory::MemorySearchProvider::new("memory-default"),
    );
    builder = builder
        .register_provider(memory_provider)
        .default_kind(SearchProviderKind::Memory);

    // Register PostgreSQL provider when configured.
    for cfg in configs
        .iter()
        .filter(|c| c.enabled && c.kind == SearchProviderKind::Postgresql)
    {
        let factory = sdkwork_search_provider_postgresql::factory();
        if let Ok(provider) = factory(cfg) {
            builder = builder.register_provider(provider);
        }
    }

    // Register Memory provider factory for future config-driven instantiation.
    builder = builder.register_factory(
        SearchProviderKind::Memory,
        sdkwork_search_provider_memory::factory(),
    );
    builder = builder.register_factory(
        SearchProviderKind::Postgresql,
        sdkwork_search_provider_postgresql::factory(),
    );

    let registry = builder.build();
    tracing::info!("search provider registry built with memory default");
    Ok(Arc::new(registry))
}

// ---------------------------------------------------------------------------
// Services
// ---------------------------------------------------------------------------

/// Construct the four search service instances from the database pool and provider registry.
///
/// `document_uploader` 注入到 indexing-service，供文档直传后建立 Drive 引用。
/// 返回的服务实例以 `Arc` 包装，可在多个 handler 任务间共享。
async fn build_services(
    pool: &PgPool,
    provider_registry: &Arc<SearchProviderRegistry>,
    document_uploader: &Arc<dyn DocumentUploadPort>,
) -> anyhow::Result<(
    Arc<QueryService>,
    Arc<IndexingService>,
    Arc<RecommendationService>,
    Arc<PromotionService>,
)> {
    let adapter = Arc::new(SearchRepositoryAdapter::new(pool.clone()));

    let query_service = Arc::new(QueryService::new(
        provider_registry.clone(),
        adapter.clone(),
    ));
    let indexing_service = Arc::new(IndexingService::new(
        provider_registry.clone(),
        adapter.clone(),
        document_uploader.clone(),
    ));
    let recommendation_service = Arc::new(RecommendationService::new(
        provider_registry.clone(),
        adapter.clone(),
    ));
    let promotion_service = Arc::new(PromotionService::new(adapter.clone()));

    tracing::info!("search services constructed with SQLx repository adapter");
    Ok((
        query_service,
        indexing_service,
        recommendation_service,
        promotion_service,
    ))
}

// ---------------------------------------------------------------------------
// Drive document upload adapter
// ---------------------------------------------------------------------------

/// Drive 文档上传适配器：封装 `DriveUploaderService`，实现 `DocumentUploadPort`。
///
/// indexing-service 仅依赖 `DocumentUploadPort` 抽象端口；本模块是基础设施层适配器，
/// 将 Drive uploader 的具体调用收敛在 assembly 装配层，保持服务层高内聚低耦合。
/// Phase 5 使用 `LocalDriveObjectStore` 作为对象存储实现。
struct DriveDocumentUploader {
    service: DriveUploaderService<SqlUploaderStore>,
    object_store: LocalDriveObjectStore,
    app_id: String,
}

impl DriveDocumentUploader {
    /// 构造适配器：传入 PgPool（Drive 仓库依赖）、本地对象存储与应用标识。
    fn new(pool: PgPool, object_store: LocalDriveObjectStore, app_id: impl Into<String>) -> Self {
        let store = SqlUploaderStore::new(pool);
        let service = DriveUploaderService::new(store);
        Self {
            service,
            object_store,
            app_id: app_id.into(),
        }
    }
}

#[async_trait]
impl DocumentUploadPort for DriveDocumentUploader {
    async fn upload_document(
        &self,
        ctx: &SearchProviderContext,
        request: &UploadDocumentRequest<'_>,
    ) -> Result<UploadedDocument, String> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let fingerprint = sha256_hash(request.bytes);
        let content_length = request.bytes.len() as i64;

        let prepare = PrepareUploaderUploadCommand {
            id: Uuid::new_v4().to_string(),
            task_id: Uuid::new_v4().to_string(),
            tenant_id: ctx.tenant_id.to_string(),
            organization_id: if ctx.organization_id > 0 {
                Some(ctx.organization_id.to_string())
            } else {
                None
            },
            actor: UploaderActor::System {
                operator_id: "sdkwork-search".to_string(),
            },
            app_id: self.app_id.clone(),
            app_resource_type: request.app_resource_type.to_string(),
            app_resource_id: request.app_resource_id.to_string(),
            scene: Some("search".to_string()),
            source: None,
            upload_profile_code: "document".to_string(),
            file_fingerprint: fingerprint,
            original_file_name: request.file_name.to_string(),
            content_type: request.content_type.to_string(),
            content_length,
            chunk_size_bytes: 5 * 1024 * 1024,
            target: UploaderTarget::AutoUploadSpace {
                parent_node_id: None,
            },
            retention: UploaderRetention::LongTerm,
            operator_id: "sdkwork-search".to_string(),
            now_epoch_ms: now_ms,
        };

        let command = UploadBytesCommand {
            prepare,
            body: request.bytes.to_vec(),
            uploaded_at_epoch_ms: now_ms,
        };

        // DriveServiceError 未实现 Display，使用 Debug 格式化错误信息。
        let item = self
            .service
            .upload_bytes(&self.object_store, command)
            .await
            .map_err(|e| format!("{e:?}"))?;

        Ok(UploadedDocument {
            drive_space_id: item.space_id,
            drive_node_id: item.node_id,
            object_bucket: item.object_bucket,
            object_key: item.object_key,
            content_length: item.content_length,
            checksum_sha256_hex: item.checksum_sha256_hex,
        })
    }
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

/// Runtime state assembled during bootstrap and shared across the HTTP server.
#[derive(Clone)]
struct ApplicationState {
    database_pool: PgPool,
    provider_registry: Arc<SearchProviderRegistry>,
    document_uploader: Arc<dyn DocumentUploadPort>,
    query_service: Arc<QueryService>,
    indexing_service: Arc<IndexingService>,
    recommendation_service: Arc<RecommendationService>,
    promotion_service: Arc<PromotionService>,
}

/// Build the full `ApplicationState` from the supplied configuration.
async fn build_application_state(
    config: &SearchApiServerConfig,
) -> anyhow::Result<ApplicationState> {
    let database_pool = connect_database_pool(&config.database_url).await?;
    let provider_registry = build_provider_registry(&config.provider_configs)?;

    // Drive 文档上传适配器：使用独立的 PgPool（Drive 仓库依赖）与本地对象存储。
    // PgPool 采用懒连接，避免在 Drive schema 未就绪时阻断 standalone-gateway 启动。
    let drive_pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect_lazy(&config.database_url)
        .map_err(|err| anyhow::anyhow!("failed to create drive upload pool: {err}"))?;
    let object_store = LocalDriveObjectStore::new(&config.upload_root_dir);
    let document_uploader: Arc<dyn DocumentUploadPort> = Arc::new(DriveDocumentUploader::new(
        drive_pool,
        object_store,
        "sdkwork-search",
    ));

    let (query_service, indexing_service, recommendation_service, promotion_service) =
        build_services(&database_pool, &provider_registry, &document_uploader).await?;

    Ok(ApplicationState {
        database_pool,
        provider_registry,
        document_uploader,
        query_service,
        indexing_service,
        recommendation_service,
        promotion_service,
    })
}

// ---------------------------------------------------------------------------
// Readiness
// ---------------------------------------------------------------------------

/// Build the composite readiness probe for the API server.
///
/// Currently checks PostgreSQL pool reachability. Additional probes (provider
/// health, cache, etc.) can be appended to the composite.
fn build_readiness_check(pool: PgPool) -> Arc<dyn ReadinessCheck> {
    let checks: Vec<Arc<dyn ReadinessCheck>> = vec![Arc::new(PgPoolReadinessCheck::new(pool))];
    Arc::new(CompositeReadinessCheck::new(checks))
}

// ---------------------------------------------------------------------------
// Router assembly
// ---------------------------------------------------------------------------

/// 上传成功的响应体。
#[derive(Debug, Serialize)]
struct UploadDocumentResponse {
    document_id: String,
    index_key: String,
}

/// 解析 multipart 表单得到的上传请求字段。
#[derive(Default)]
struct ParsedUploadForm {
    index_key: Option<String>,
    document_id: Option<String>,
    tenant_id: Option<i64>,
    organization_id: Option<i64>,
    file_name: Option<String>,
    content_type: Option<String>,
    bytes: Option<Vec<u8>>,
}

/// 处理 `/backend/search/documents/upload` 文档直传请求。
///
/// multipart 表单字段：
/// - `index_key`：目标索引键（必填）
/// - `document_id`：文档 ID（必填）
/// - `tenant_id`：租户 ID（可选，默认 0）
/// - `organization_id`：组织 ID（可选，默认 0）
/// - `file`：文件字段（必填，携带文件名与 content_type）
async fn upload_document(
    Extension(indexing_service): Extension<Arc<IndexingService>>,
    mut multipart: Multipart,
) -> Response {
    let parsed = match parse_upload_form(&mut multipart).await {
        Ok(parsed) => parsed,
        Err(message) => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                format!("invalid upload form: {message}"),
            )
                .into_response()
        }
    };

    let Some(index_key) = parsed.index_key else {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "missing required field: index_key".to_string(),
        )
            .into_response();
    };
    let Some(document_id) = parsed.document_id else {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "missing required field: document_id".to_string(),
        )
            .into_response();
    };
    let Some(file_name) = parsed.file_name else {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "missing required field: file".to_string(),
        )
            .into_response();
    };
    let Some(bytes) = parsed.bytes else {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            "missing required field: file".to_string(),
        )
            .into_response();
    };

    let content_type = parsed
        .content_type
        .clone()
        .unwrap_or_else(|| "application/octet-stream".to_string());

    let ctx: SearchProviderContext = SearchProviderContextBuilder::default()
        .tenant_id(parsed.tenant_id.unwrap_or(0))
        .organization_id(parsed.organization_id.unwrap_or(0))
        .build();

    let request = UploadDocumentRequest {
        file_name: &file_name,
        content_type: &content_type,
        bytes: &bytes,
        app_resource_type: "search_document",
        app_resource_id: &document_id,
    };

    match indexing_service
        .ingest_document_from_upload(&ctx, &index_key, &document_id, &request)
        .await
    {
        Ok(()) => Json(UploadDocumentResponse {
            document_id,
            index_key,
        })
        .into_response(),
        Err(err) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("ingest document failed: {err}"),
        )
            .into_response(),
    }
}

/// 解析 multipart 表单字段。
async fn parse_upload_form(multipart: &mut Multipart) -> Result<ParsedUploadForm, String> {
    let mut form = ParsedUploadForm::default();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| format!("read multipart field failed: {err}"))?
    {
        let name = field.name().unwrap_or_default().to_string();
        let file_name = field.file_name().map(|s| s.to_string());
        let content_type = field.content_type().map(|s| s.to_string());
        let bytes = field
            .bytes()
            .await
            .map_err(|err| format!("read multipart bytes failed: {err}"))?;

        match name.as_str() {
            "index_key" => {
                form.index_key = Some(String::from_utf8_lossy(&bytes).to_string());
            }
            "document_id" => {
                form.document_id = Some(String::from_utf8_lossy(&bytes).to_string());
            }
            "tenant_id" => {
                form.tenant_id = String::from_utf8_lossy(&bytes).parse().ok();
            }
            "organization_id" => {
                form.organization_id = String::from_utf8_lossy(&bytes).parse().ok();
            }
            "file" => {
                form.file_name = file_name;
                form.content_type = content_type;
                form.bytes = Some(bytes.to_vec());
            }
            _ => {}
        }
    }
    Ok(form)
}

fn combined_route_manifest() -> HttpRouteManifest {
    let routes = [
        sdkwork_routes_search_app_api::gateway_route_manifest(),
        sdkwork_routes_search_backend_api::gateway_route_manifest(),
    ]
    .into_iter()
    .flat_map(|manifest| manifest.iter().cloned())
    .collect();
    HttpRouteManifest::from_owned_routes(routes)
}

fn search_business_router(app_state: SearchAppState, backend_state: SearchBackendState) -> Router {
    Router::new()
        .merge(sdkwork_routes_search_app_api::gateway_mount(app_state))
        .merge(sdkwork_routes_search_backend_api::gateway_mount(
            backend_state,
        ))
}

pub fn assemble_api_router(
    app_state: SearchAppState,
    backend_state: SearchBackendState,
) -> ApiAssembly {
    ApiAssemblyContribution::from_manifest(
        "sdkwork-search",
        "SDKWork Search API",
        search_business_router(app_state, backend_state),
        combined_route_manifest(),
        Vec::new(),
        Arc::new(sdkwork_web_bootstrap::AlwaysReady),
    )
    .expect("search assembly contribution contract is valid")
}

/// Boots the Search services from `SDKWORK_DATABASE_*` environment, assembles
/// the business router (route crates + document upload endpoint), and returns
/// the contribution carrying the combined readiness set. The thin standalone
/// gateway projects `.router` / `.readiness_check` and mounts infra routes
/// (API_ASSEMBLY_SPEC §6.1).
pub async fn assemble_api_router_from_env() -> Result<ApiAssembly, String> {
    let config = SearchApiServerConfig::from_env()
        .map_err(|error| format!("load search server config failed: {error}"))?;
    let state = build_application_state(&config)
        .await
        .map_err(|error| format!("build search application state failed: {error}"))?;

    let app_state = SearchAppState::new(
        state.provider_registry.clone(),
        state.query_service.clone(),
        state.indexing_service.clone(),
        state.recommendation_service.clone(),
        state.promotion_service.clone(),
    );
    let backend_state = SearchBackendState::new(
        state.provider_registry.clone(),
        state.query_service.clone(),
        state.indexing_service.clone(),
        state.recommendation_service.clone(),
        state.promotion_service.clone(),
    );

    let router = search_business_router(app_state, backend_state)
        .route("/backend/search/documents/upload", post(upload_document))
        .layer(Extension(state.indexing_service));

    ApiAssemblyContribution::from_manifest(
        "sdkwork-search",
        "SDKWork Search API",
        router,
        combined_route_manifest(),
        Vec::new(),
        build_readiness_check(state.database_pool),
    )
    .map_err(|error| format!("search assembly contribution contract is invalid: {error}"))
}

/// Canonical Web Module definition for this application
/// (API_ASSEMBLY_SPEC §4.1.1): the complete HTTP surface — every route,
/// manifest, and OpenAPI document of this owner — as one installable module.
pub async fn web_module() -> Result<WebModule, String> {
    Ok(WebModule::from_contribution(
        assemble_api_router_from_env().await?,
    ))
}
