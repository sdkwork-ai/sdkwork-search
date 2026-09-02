//! 统一搜索仓储适配器。
//!
//! `SearchRepositoryAdapter` 持有全部子 repository（index/document/user_event/
//! suggestion/promotion），实现 4 个 service port trait（query/indexing/
//! recommendation/promotion）。适配器只做委托与类型映射，不包含业务逻辑，
//! 遵循 `RUST_CODE_SPEC.md` 高内聚低耦合与开闭原则。

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sdkwork_search_indexing_service::{
    domain::{DocumentSummary, IndexJob, IndexJobStatus, IndexStats},
    ports::IndexingRepositoryPort,
};
use sdkwork_search_promotion_service::{
    domain::{
        CreatePromotionInput, Promotion, PromotionPlacement, PromotionRule, PromotionTarget,
        UpdatePromotionInput,
    },
    ports::PromotionRepositoryPort,
};
use sdkwork_search_provider_spi::{IndexDocument, SearchProviderContext};
use sdkwork_search_query_service::ports::SearchQueryRepositoryPort;
use sdkwork_search_recommendation_service::{
    domain::RecommendationItem, ports::RecommendationRepositoryPort,
};
use sqlx::PgPool;
use tracing::debug;

use crate::db::rows::{SearchDocumentRow, SearchIndexRow, SearchPromotionRow};
use crate::repository::{
    queries, CreateIndexJobParams, CreateIndexParams, CreatePromotionParams, RecordUserEventParams,
    SearchDocumentRepository, SearchIndexRepository, SearchPromotionRepository,
    SearchUserEventRepository, UpdateIndexParams, UpsertDocumentParams, UpsertRecentQueryParams,
};

/// 统一搜索仓储适配器：实现全部 service port trait，委托到具体 repository struct。
///
/// 字段集合仅包含当前 4 个 port trait 实际需要的子 repository；suggestion 仓储暂未
/// 被任何 port trait 引用，故不在此持有，遵循「保持简单直接」与开闭原则。
pub struct SearchRepositoryAdapter {
    index_repo: SearchIndexRepository,
    document_repo: SearchDocumentRepository,
    user_event_repo: SearchUserEventRepository,
    promotion_repo: SearchPromotionRepository,
}

impl SearchRepositoryAdapter {
    /// 从 PostgreSQL 连接池构造适配器，内部为每个子 repository 克隆一份池句柄。
    pub fn new(pool: PgPool) -> Self {
        Self {
            index_repo: SearchIndexRepository::new(pool.clone()),
            document_repo: SearchDocumentRepository::new(pool.clone()),
            user_event_repo: SearchUserEventRepository::new(pool.clone()),
            promotion_repo: SearchPromotionRepository::new(pool),
        }
    }
}

// =============================================================================
// SearchQueryRepositoryPort
// =============================================================================

#[async_trait]
impl SearchQueryRepositoryPort for SearchRepositoryAdapter {
    async fn list_recent_queries(
        &self,
        ctx: &SearchProviderContext,
        limit: u32,
    ) -> Result<Vec<String>, String> {
        let user_id = ctx.user_id.unwrap_or(0);
        let rows = self
            .user_event_repo
            .list_recent_queries(ctx.tenant_id, ctx.organization_id, user_id, limit)
            .await
            .map_err(|e| e.to_string())?;
        Ok(rows.into_iter().map(|r| r.q).collect())
    }

    async fn record_query_audit(
        &self,
        ctx: &SearchProviderContext,
        query_text: &str,
        index_key: &str,
    ) -> Result<(), String> {
        let user_id = ctx.user_id.unwrap_or(0);
        let params = UpsertRecentQueryParams {
            id: now_id(),
            tenant_id: ctx.tenant_id,
            organization_id: ctx.organization_id,
            user_id,
            index_key,
            q: query_text,
            result_count: 0,
        };
        self.user_event_repo
            .upsert_recent_query(&params)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn record_user_event(
        &self,
        ctx: &SearchProviderContext,
        event_kind: &str,
        payload: &serde_json::Value,
    ) -> Result<(), String> {
        let user_id = ctx.user_id.unwrap_or(0);
        let params = RecordUserEventParams {
            id: now_id(),
            tenant_id: ctx.tenant_id,
            organization_id: ctx.organization_id,
            user_id,
            event_type: event_kind.to_string(),
            surface: "app".to_string(),
            index_key: None,
            document_id: None,
            placement: None,
            q: None,
            result_position: None,
            request_id: ctx.request_id.clone(),
            metadata_json: payload.clone(),
        };
        self.user_event_repo
            .record_event(&params)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

// =============================================================================
// IndexingRepositoryPort
// =============================================================================

#[async_trait]
impl IndexingRepositoryPort for SearchRepositoryAdapter {
    async fn upsert_document(
        &self,
        ctx: &SearchProviderContext,
        doc: &IndexDocument,
    ) -> Result<(), String> {
        // 查找 index_id（search_document.index_id 为 NOT NULL）。
        let index_row = self
            .index_repo
            .get_index_by_key(ctx.tenant_id, ctx.organization_id, &doc.index_key)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("index not found for index_key: {}", doc.index_key))?;

        let params = UpsertDocumentParams {
            id: now_id(),
            tenant_id: ctx.tenant_id,
            organization_id: ctx.organization_id,
            index_id: index_row.id,
            index_key: doc.index_key.clone(),
            document_id: doc.document_id.clone(),
            capability: None,
            scope: "global".to_string(),
            group_key: None,
            group_title: None,
            source_ref: None,
            title: doc.title.clone().unwrap_or_else(|| doc.document_id.clone()),
            body_text: doc.content.clone(),
            keyword_text: None,
            payload_json: doc.source.clone(),
            token_json: serde_json::json!([]),
            embedding_json: serde_json::json!({ "vector": doc.embedding }),
            status: 1,
            data_scope: 0,
        };
        self.document_repo
            .upsert_document(&params)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn delete_document(
        &self,
        ctx: &SearchProviderContext,
        index_key: &str,
        document_id: &str,
    ) -> Result<(), String> {
        self.document_repo
            .delete_document(
                ctx.tenant_id,
                ctx.organization_id,
                index_key,
                document_id,
                ctx.user_id,
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn get_index(
        &self,
        ctx: &SearchProviderContext,
        index_key: &str,
    ) -> Result<Option<IndexJob>, String> {
        let row = self
            .index_repo
            .get_index_by_key(ctx.tenant_id, ctx.organization_id, index_key)
            .await
            .map_err(|e| e.to_string())?;
        Ok(row.map(|r| index_row_to_job(&r)))
    }

    async fn create_index(
        &self,
        ctx: &SearchProviderContext,
        index_key: &str,
        schema: &serde_json::Value,
    ) -> Result<(), String> {
        let params = CreateIndexParams {
            id: now_id(),
            tenant_id: ctx.tenant_id,
            organization_id: ctx.organization_id,
            index_key: index_key.to_string(),
            title: index_key.to_string(),
            description: None,
            config_json: schema.clone(),
        };
        self.index_repo
            .create_index(&params)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn update_index(
        &self,
        ctx: &SearchProviderContext,
        index_key: &str,
        title: Option<&str>,
        description: Option<&str>,
        status: Option<i32>,
        config_json: Option<&serde_json::Value>,
    ) -> Result<bool, String> {
        // 通过 index_key + tenant_id 定位索引行；不存在返回 false。
        let row = self
            .index_repo
            .get_index_by_key(ctx.tenant_id, ctx.organization_id, index_key)
            .await
            .map_err(|e| e.to_string())?;
        let Some(row) = row else {
            return Ok(false);
        };

        // sqlx repository 的 update_index 为全量替换，port 为部分更新；
        // 在适配层用现有值补齐 None 字段，完成 Option→全量 的转换。
        let params = UpdateIndexParams {
            id: row.id,
            tenant_id: ctx.tenant_id,
            title: title
                .map(|s| s.to_string())
                .unwrap_or_else(|| row.title.clone()),
            description: description
                .map(|s| s.to_string())
                .or(row.description.clone()),
            status: status.unwrap_or(row.status),
            config_json: config_json
                .cloned()
                .unwrap_or_else(|| row.config_json.clone()),
        };
        self.index_repo
            .update_index(&params)
            .await
            .map_err(|e| e.to_string())?;
        Ok(true)
    }

    async fn create_index_job(
        &self,
        ctx: &SearchProviderContext,
        index_key: &str,
        job_type: &str,
        payload: &serde_json::Value,
    ) -> Result<String, String> {
        let params = CreateIndexJobParams {
            id: now_id(),
            tenant_id: ctx.tenant_id,
            organization_id: ctx.organization_id,
            index_key: index_key.to_string(),
            job_type: job_type.to_string(),
            payload_json: payload.clone(),
        };
        let row = self
            .index_repo
            .create_index_job(&params)
            .await
            .map_err(|e| e.to_string())?;
        Ok(row.uuid)
    }

    async fn get_index_job(
        &self,
        ctx: &SearchProviderContext,
        job_uuid: &str,
    ) -> Result<Option<IndexJobStatus>, String> {
        let row = self
            .index_repo
            .get_index_job(ctx.tenant_id, job_uuid)
            .await
            .map_err(|e| e.to_string())?;
        Ok(row.map(|r| job_status_from_i32(r.status)))
    }

    async fn update_index_job_status(
        &self,
        ctx: &SearchProviderContext,
        job_uuid: &str,
        status: i32,
        started_at: Option<DateTime<Utc>>,
        finished_at: Option<DateTime<Utc>>,
        error_summary: Option<&str>,
    ) -> Result<bool, String> {
        let affected = self
            .index_repo
            .update_index_job_status(
                ctx.tenant_id,
                job_uuid,
                status,
                started_at,
                finished_at,
                error_summary,
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(affected > 0)
    }

    async fn list_indexes(&self, ctx: &SearchProviderContext) -> Result<Vec<String>, String> {
        let rows = self
            .index_repo
            .list_indexes(ctx.tenant_id, ctx.organization_id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(rows.into_iter().map(|r| r.index_key).collect())
    }

    async fn list_documents(
        &self,
        ctx: &SearchProviderContext,
        index_key: &str,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<DocumentSummary>, String> {
        debug!(%index_key, page, page_size, "listing documents");
        let rows = self
            .document_repo
            .list_documents(
                ctx.tenant_id,
                ctx.organization_id,
                index_key,
                page,
                page_size,
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(rows.iter().map(document_row_to_summary).collect())
    }

    async fn count_documents(
        &self,
        ctx: &SearchProviderContext,
        index_key: &str,
    ) -> Result<u64, String> {
        debug!(%index_key, "counting documents");
        let row: DocumentCountRow =
            sqlx::query_as::<sqlx::Postgres, DocumentCountRow>(queries::DOCUMENT_COUNT)
                .bind(ctx.tenant_id)
                .bind(ctx.organization_id)
                .bind(index_key)
                .fetch_one(self.document_repo.pool())
                .await
                .map_err(|e| e.to_string())?;
        Ok(row.count as u64)
    }

    async fn get_document(
        &self,
        ctx: &SearchProviderContext,
        index_key: &str,
        document_id: &str,
    ) -> Result<Option<DocumentSummary>, String> {
        debug!(%index_key, %document_id, "getting document");
        let row = self
            .document_repo
            .get_document(ctx.tenant_id, ctx.organization_id, index_key, document_id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(row.as_ref().map(document_row_to_summary))
    }

    async fn get_index_stats(
        &self,
        ctx: &SearchProviderContext,
        index_key: &str,
    ) -> Result<IndexStats, String> {
        debug!(%index_key, "getting index stats");
        let row: DocumentStatsRow =
            sqlx::query_as::<sqlx::Postgres, DocumentStatsRow>(queries::DOCUMENT_STATS)
                .bind(ctx.tenant_id)
                .bind(ctx.organization_id)
                .bind(index_key)
                .fetch_one(self.document_repo.pool())
                .await
                .map_err(|e| e.to_string())?;
        Ok(IndexStats {
            index_key: index_key.to_string(),
            document_count: row.document_count as u64,
            active_document_count: row.active_document_count as u64,
            last_updated: row.last_updated.map(|dt| dt.to_rfc3339()),
        })
    }

    async fn bulk_delete_documents(
        &self,
        ctx: &SearchProviderContext,
        index_key: &str,
        document_ids: &[String],
    ) -> Result<u64, String> {
        debug!(%index_key, count = document_ids.len(), "bulk deleting documents");
        let mut deleted: u64 = 0;
        for doc_id in document_ids {
            let result = sqlx::query(queries::DOCUMENT_DELETE)
                .bind(ctx.user_id)
                .bind(ctx.tenant_id)
                .bind(ctx.organization_id)
                .bind(index_key)
                .bind(doc_id)
                .execute(self.document_repo.pool())
                .await
                .map_err(|e| e.to_string())?;
            deleted += result.rows_affected();
        }
        Ok(deleted)
    }
}

// =============================================================================
// RecommendationRepositoryPort
// =============================================================================

#[async_trait]
impl RecommendationRepositoryPort for SearchRepositoryAdapter {
    async fn get_user_events(
        &self,
        ctx: &SearchProviderContext,
        user_id: i64,
        limit: u32,
    ) -> Result<Vec<RecommendationItem>, String> {
        let rows = self
            .user_event_repo
            .list_user_events(ctx.tenant_id, ctx.organization_id, user_id, None, limit)
            .await
            .map_err(|e| e.to_string())?;

        // 按位置衰减打分（最近事件得分最高），仅保留有 document_id 的事件。
        let items = rows
            .iter()
            .enumerate()
            .filter_map(|(index, event)| {
                event
                    .document_id
                    .as_ref()
                    .map(|document_id| RecommendationItem {
                        document_id: document_id.clone(),
                        score: 1.0 / (index as f64 + 1.0),
                        source: serde_json::json!({
                            "event_type": event.event_type,
                            "occurred_at": event.occurred_at.to_rfc3339(),
                        }),
                        reason: Some("recent_user_event".to_string()),
                    })
            })
            .collect();
        Ok(items)
    }

    async fn get_trending(
        &self,
        ctx: &SearchProviderContext,
        index_key: &str,
        limit: u32,
    ) -> Result<Vec<RecommendationItem>, String> {
        let rows = self
            .user_event_repo
            .get_trending(ctx.tenant_id, ctx.organization_id, index_key, limit)
            .await
            .map_err(|e| e.to_string())?;

        let max_count = rows.first().map(|r| r.event_count).unwrap_or(1).max(1) as f64;
        let items = rows
            .iter()
            .map(|row| RecommendationItem {
                document_id: row.document_id.clone(),
                score: row.event_count as f64 / max_count,
                source: serde_json::json!({ "event_count": row.event_count }),
                reason: Some("trending".to_string()),
            })
            .collect();
        Ok(items)
    }

    async fn get_similar_documents(
        &self,
        ctx: &SearchProviderContext,
        index_key: &str,
        document_ids: &[String],
        limit: u32,
    ) -> Result<Vec<RecommendationItem>, String> {
        let rows = self
            .user_event_repo
            .get_similar_documents(
                ctx.tenant_id,
                ctx.organization_id,
                index_key,
                document_ids,
                limit,
            )
            .await
            .map_err(|e| e.to_string())?;

        // 按共同用户数归一化到 [0,1]，与其它策略分数在同一量纲便于混合融合。
        let max_score = rows.first().map(|r| r.similarity_score).unwrap_or(1).max(1) as f64;
        let items = rows
            .iter()
            .map(|row| RecommendationItem {
                document_id: row.document_id.clone(),
                score: row.similarity_score as f64 / max_score,
                source: serde_json::json!({ "similarity_score": row.similarity_score }),
                reason: Some("collaborative_filtering".to_string()),
            })
            .collect();
        Ok(items)
    }

    async fn get_user_profile(
        &self,
        ctx: &SearchProviderContext,
        user_id: i64,
    ) -> Result<Option<serde_json::Value>, String> {
        self.user_event_repo
            .get_user_profile(ctx.tenant_id, ctx.organization_id, user_id)
            .await
            .map_err(|e| e.to_string())
    }

    async fn record_recommendation(
        &self,
        ctx: &SearchProviderContext,
        user_id: i64,
        items: &[RecommendationItem],
    ) -> Result<(), String> {
        self.user_event_repo
            .record_recommendation(ctx.tenant_id, ctx.organization_id, user_id, items)
            .await
            .map_err(|e| e.to_string())
    }
}

// =============================================================================
// PromotionRepositoryPort
// =============================================================================

#[async_trait]
impl PromotionRepositoryPort for SearchRepositoryAdapter {
    async fn list_active_promotions(
        &self,
        ctx: &SearchProviderContext,
        index_key: &str,
        placement: PromotionPlacement,
    ) -> Result<Vec<Promotion>, String> {
        let rows = self
            .promotion_repo
            .list_active_promotions(
                ctx.tenant_id,
                ctx.organization_id,
                index_key,
                placement_to_str(placement),
                Utc::now(),
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(rows.iter().map(promotion_row_to_domain).collect())
    }

    async fn record_delivery(
        &self,
        ctx: &SearchProviderContext,
        promotion_id: &str,
        user_id: i64,
    ) -> Result<(), String> {
        self.promotion_repo
            .record_delivery(
                now_id(),
                ctx.tenant_id,
                ctx.organization_id,
                promotion_id,
                user_id,
            )
            .await
            .map_err(|e| e.to_string())
    }

    async fn record_click(
        &self,
        ctx: &SearchProviderContext,
        promotion_id: &str,
        user_id: i64,
    ) -> Result<(), String> {
        self.promotion_repo
            .record_click(
                now_id(),
                ctx.tenant_id,
                ctx.organization_id,
                promotion_id,
                user_id,
            )
            .await
            .map_err(|e| e.to_string())
    }

    async fn get_promotion_stats(
        &self,
        ctx: &SearchProviderContext,
        promotion_id: &str,
    ) -> Result<serde_json::Value, String> {
        self.promotion_repo
            .get_promotion_stats(ctx.tenant_id, ctx.organization_id, promotion_id)
            .await
            .map_err(|e| e.to_string())
    }

    async fn create_promotion(
        &self,
        ctx: &SearchProviderContext,
        input: &CreatePromotionInput,
    ) -> Result<Promotion, String> {
        debug!(promotion_key = %input.promotion_key, "creating promotion");
        let rule_json = serde_json::to_value(&input.rules).unwrap_or(serde_json::json!([]));
        let params = CreatePromotionParams {
            id: now_id(),
            tenant_id: ctx.tenant_id,
            organization_id: ctx.organization_id,
            promotion_key: input.promotion_key.clone(),
            placement: placement_to_str(input.placement).to_string(),
            index_key: input.index_key.clone(),
            document_id: input.document_id.clone(),
            priority: input.priority,
            rule_json,
        };
        let row = self
            .promotion_repo
            .create_promotion(&params)
            .await
            .map_err(|e| e.to_string())?;
        Ok(promotion_row_to_domain(&row))
    }

    async fn update_promotion(
        &self,
        ctx: &SearchProviderContext,
        promotion_id: &str,
        patch: &UpdatePromotionInput,
    ) -> Result<Promotion, String> {
        debug!(%promotion_id, "updating promotion");
        let placement = patch.placement.map(|p| placement_to_str(p).to_string());
        let rule_json = patch
            .rules
            .as_ref()
            .map(|r| serde_json::to_value(r).unwrap_or(serde_json::json!([])));
        let status = patch.enabled.map(|e| if e { 1 } else { 0 });
        let row = sqlx::query_as::<sqlx::Postgres, SearchPromotionRow>(queries::PROMOTION_UPDATE)
            .bind(placement)
            .bind(patch.document_id.as_deref())
            .bind(patch.priority)
            .bind(rule_json)
            .bind(status)
            .bind(patch.start_at)
            .bind(patch.end_at)
            .bind(ctx.tenant_id)
            .bind(ctx.organization_id)
            .bind(promotion_id)
            .fetch_one(self.promotion_repo.pool())
            .await
            .map_err(|e| e.to_string())?;
        Ok(promotion_row_to_domain(&row))
    }

    async fn delete_promotion(
        &self,
        ctx: &SearchProviderContext,
        promotion_id: &str,
    ) -> Result<(), String> {
        debug!(%promotion_id, "deleting promotion");
        sqlx::query(queries::PROMOTION_DELETE)
            .bind(ctx.user_id)
            .bind(ctx.tenant_id)
            .bind(ctx.organization_id)
            .bind(promotion_id)
            .execute(self.promotion_repo.pool())
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn list_all_promotions(
        &self,
        ctx: &SearchProviderContext,
        index_key: &str,
        page: u32,
        page_size: u32,
    ) -> Result<Vec<Promotion>, String> {
        debug!(%index_key, page, page_size, "listing all promotions");
        let page = page.max(1);
        let page_size = page_size.max(1);
        let limit = page_size as i64;
        let offset = ((page - 1) as i64) * limit;
        let rows =
            sqlx::query_as::<sqlx::Postgres, SearchPromotionRow>(queries::PROMOTION_LIST_ALL)
                .bind(ctx.tenant_id)
                .bind(ctx.organization_id)
                .bind(index_key)
                .bind(limit)
                .bind(offset)
                .fetch_all(self.promotion_repo.pool())
                .await
                .map_err(|e| e.to_string())?;
        Ok(rows.iter().map(promotion_row_to_domain).collect())
    }

    async fn get_promotion(
        &self,
        ctx: &SearchProviderContext,
        promotion_id: &str,
    ) -> Result<Option<Promotion>, String> {
        debug!(%promotion_id, "getting promotion");
        let row = sqlx::query_as::<sqlx::Postgres, SearchPromotionRow>(queries::PROMOTION_GET)
            .bind(ctx.tenant_id)
            .bind(ctx.organization_id)
            .bind(promotion_id)
            .fetch_optional(self.promotion_repo.pool())
            .await
            .map_err(|e| e.to_string())?;
        Ok(row.as_ref().map(promotion_row_to_domain))
    }
}

// =============================================================================
// 行 -> 领域模型映射与辅助函数
// =============================================================================

/// 生成基于毫秒时间戳的 BIGINT id（保持简单直接，遵循任务约定）。
fn now_id() -> i64 {
    Utc::now().timestamp_millis()
}

/// 将 `SearchIndexRow` 映射为 `IndexJob`。
///
/// `search_index` 表不直接存储 job 计数字段；索引存在即视为已就绪（Completed），
/// 计数字段填 0，真实 job 跟踪留待后续扩展。
fn index_row_to_job(row: &SearchIndexRow) -> IndexJob {
    IndexJob {
        job_id: row.index_key.clone(),
        index_key: row.index_key.clone(),
        status: IndexJobStatus::Completed,
        document_count: 0,
        success_count: 0,
        failure_count: 0,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

/// 将 `search_index_job.status` 整数编码映射为 `IndexJobStatus` 枚举。
///
/// 约定：0=pending、1=running、2=completed、3=failed；未知值回退为 `Pending`。
fn job_status_from_i32(status: i32) -> IndexJobStatus {
    match status {
        1 => IndexJobStatus::Running,
        2 => IndexJobStatus::Completed,
        3 => IndexJobStatus::Failed,
        _ => IndexJobStatus::Pending,
    }
}

/// 将 `SearchPromotionRow` 映射为领域模型 `Promotion`。
///
/// `search_promotion` 表无 title 列，复用 `promotion_key` 作为标题占位；
/// `rule_json` 为数组时反序列化为规则列表，否则返回空规则集。
fn promotion_row_to_domain(row: &SearchPromotionRow) -> Promotion {
    let rules = if row.rule_json.is_array() {
        serde_json::from_value::<Vec<PromotionRule>>(row.rule_json.clone()).unwrap_or_default()
    } else {
        Vec::new()
    };
    Promotion {
        promotion_id: row.promotion_key.clone(),
        title: row.promotion_key.clone(),
        target: PromotionTarget {
            index_key: row.index_key.clone(),
            placement: parse_placement(&row.placement),
            filters: HashMap::new(),
        },
        rules,
        start_at: row.active_from,
        end_at: row.active_until,
        enabled: row.status == 1,
    }
}

/// 将 `PromotionPlacement` 序列化为 `search_promotion.placement` 列存储的小写字符串。
fn placement_to_str(placement: PromotionPlacement) -> &'static str {
    match placement {
        PromotionPlacement::Top => "top",
        PromotionPlacement::Inline => "inline",
        PromotionPlacement::Banner => "banner",
        PromotionPlacement::Sidebar => "sidebar",
    }
}

/// 将 `placement` 列的字符串解析回 `PromotionPlacement`，未知值回退为 `Top`。
fn parse_placement(value: &str) -> PromotionPlacement {
    match value {
        "inline" => PromotionPlacement::Inline,
        "banner" => PromotionPlacement::Banner,
        "sidebar" => PromotionPlacement::Sidebar,
        _ => PromotionPlacement::Top,
    }
}

/// 将 `SearchDocumentRow` 映射为 `DocumentSummary`，仅保留摘要字段。
fn document_row_to_summary(row: &SearchDocumentRow) -> DocumentSummary {
    DocumentSummary {
        document_id: row.document_id.clone(),
        index_key: row.index_key.clone(),
        title: row.title.clone(),
        payload_json: row.payload_json.clone(),
        status: row.status,
        updated_at: Some(row.updated_at.to_rfc3339()),
    }
}

/// 文档计数查询行（`SELECT COUNT(*)`）。
#[derive(Debug, Clone, sqlx::FromRow)]
struct DocumentCountRow {
    count: i64,
}

/// 文档统计聚合行：文档总数、活跃文档数、最后更新时间。
#[derive(Debug, Clone, sqlx::FromRow)]
struct DocumentStatsRow {
    document_count: i64,
    active_document_count: i64,
    last_updated: Option<DateTime<Utc>>,
}
