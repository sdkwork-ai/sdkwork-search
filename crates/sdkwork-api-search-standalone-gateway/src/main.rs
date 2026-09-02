//! Process entry point for the SDKWork Search HTTP API server.
//!
//! The thin standalone gateway only loads process configuration, projects the
//! assembly contribution (`.router` / `.readiness_check`), mounts infrastructure
//! routes, and listens. Service construction, route composition, and readiness
//! are owned by `sdkwork-api-search-assembly` (API_ASSEMBLY_SPEC §6.1).

use std::net::SocketAddr;

use sdkwork_api_search_assembly::{web_module, SearchApiServerConfig};
use sdkwork_web_bootstrap::{
    init_tracing_from_env, mount_infra_routes, ApiModuleRegistry, ServiceRouterConfig,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing_from_env();

    let config = SearchApiServerConfig::from_env()?;
    let addr: SocketAddr = config.bind_addr.parse().map_err(|err| {
        anyhow::anyhow!("invalid SEARCH_API_BIND_ADDR {:?}: {err}", config.bind_addr)
    })?;

    let mut module_registry = ApiModuleRegistry::new();
    module_registry.add_module(
        web_module()
            .await
            .map_err(|error| anyhow::anyhow!("search API assembly failed: {error}"))?,
    );
    let assembly = module_registry
        .try_compose("SDKWork Search API")
        .map_err(|error| anyhow::anyhow!("search API composition failed: {error}"))?;
    let router = mount_infra_routes(
        assembly.router,
        ServiceRouterConfig::default().with_readiness_check(assembly.readiness_check.clone()),
    );

    tracing::info!(bind_addr = %config.bind_addr, "starting sdkwork-api-search-standalone-gateway");
    sdkwork_web_bootstrap::serve(router, addr).await?;
    Ok(())
}
