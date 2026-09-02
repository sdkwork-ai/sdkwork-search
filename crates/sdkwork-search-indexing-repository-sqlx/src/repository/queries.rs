//! SQL query text constants for the search indexing repository.
//!
//! All queries use named parameters (`:name`). sqlx rewrites named parameters to
//! positional placeholders (`$1`, `$2`, ...) in order of first appearance; callers
//! MUST bind values in that same first-appearance order.
//!
//! Every query filters by `tenant_id` and `organization_id` to enforce tenant
//! isolation per `RUST_CODE_SPEC.md` §6.

/// Builds a `SELECT` over `search_document` excluding the generated
/// `search_vector` TSVECTOR column, which sqlx cannot decode into a native type.
macro_rules! document_select {
    ($tail:literal) => {
        concat!(
            "SELECT id, uuid, tenant_id, organization_id, index_id, index_key, \
             document_id, capability, scope, group_key, group_title, source_ref, \
             title, body_text, keyword_text, payload_json, token_json, \
             embedding_json, status, data_scope, indexed_at, created_at, \
             updated_at, version, deleted_at, deleted_by ",
            $tail
        )
    };
}

/// Builds a `SELECT` over `search_promotion` 列，与 `document_select!` 同构，
/// 用于保证 SELECT 列表与 `SearchPromotionRow` 字段顺序一致。
macro_rules! promotion_select {
    ($tail:literal) => {
        concat!(
            "SELECT id, uuid, tenant_id, organization_id, promotion_key, \
             placement, index_key, document_id, priority, rule_json, status, \
             active_from, active_until, created_at, updated_at, version, \
             deleted_at, deleted_by ",
            $tail
        )
    };
}

// ===========================================================================
// search_index
// ===========================================================================

/// bind order: tenant_id, organization_id
pub const INDEX_LIST_INDEXES: &str = "SELECT id, uuid, tenant_id, organization_id, \
     index_key, title, description, status, data_scope, config_json, created_at, \
     updated_at, version, deleted_at, deleted_by \
     FROM search_index \
     WHERE tenant_id = :tenant_id AND organization_id = :organization_id \
       AND deleted_at IS NULL \
     ORDER BY updated_at DESC";

/// bind order: tenant_id, organization_id, index_key
pub const INDEX_GET_BY_KEY: &str = "SELECT id, uuid, tenant_id, organization_id, \
     index_key, title, description, status, data_scope, config_json, created_at, \
     updated_at, version, deleted_at, deleted_by \
     FROM search_index \
     WHERE tenant_id = :tenant_id AND organization_id = :organization_id \
       AND index_key = :index_key AND deleted_at IS NULL";

/// bind order: id, uuid, tenant_id, organization_id, index_key, title, description, config_json
pub const INDEX_CREATE: &str = "INSERT INTO search_index \
     (id, uuid, tenant_id, organization_id, index_key, title, description, \
      status, data_scope, config_json, version) \
     VALUES (:id, :uuid, :tenant_id, :organization_id, :index_key, :title, \
             :description, 1, 0, :config_json, 0) \
     RETURNING id, uuid, tenant_id, organization_id, index_key, title, description, \
     status, data_scope, config_json, created_at, updated_at, version, \
     deleted_at, deleted_by";

/// bind order: title, description, status, config_json, id, tenant_id
pub const INDEX_UPDATE: &str = "UPDATE search_index SET title = :title, \
     description = :description, status = :status, config_json = :config_json, \
     version = version + 1, updated_at = NOW() \
     WHERE id = :id AND tenant_id = :tenant_id AND deleted_at IS NULL \
     RETURNING id, uuid, tenant_id, organization_id, index_key, title, description, \
     status, data_scope, config_json, created_at, updated_at, version, \
     deleted_at, deleted_by";

/// bind order: deleted_by, id, tenant_id
pub const INDEX_DELETE: &str = "UPDATE search_index SET deleted_at = NOW(), \
     deleted_by = :deleted_by, version = version + 1, updated_at = NOW() \
     WHERE id = :id AND tenant_id = :tenant_id AND deleted_at IS NULL";

// ===========================================================================
// search_index_job
// ===========================================================================

/// bind order: id, uuid, tenant_id, organization_id, index_key, job_type, payload_json
///
/// 创建索引任务记录，`status` 默认 0（pending），`scheduled_at` 默认 NOW()。
/// 返回 uuid 供后续状态跟踪。
pub const JOB_CREATE: &str = "INSERT INTO search_index_job \
     (id, uuid, tenant_id, organization_id, index_key, job_type, status, \
      payload_json, scheduled_at) \
     VALUES (:id, :uuid, :tenant_id, :organization_id, :index_key, :job_type, \
             0, :payload_json, NOW()) \
     RETURNING id, uuid, tenant_id, organization_id, index_key, job_type, status, \
     payload_json, scheduled_at, started_at, finished_at, error_summary, retry_count, \
     created_at, updated_at, version";

/// bind order: tenant_id, uuid
///
/// 按任务 uuid 查询单条任务记录（带租户隔离）。
pub const JOB_GET: &str = "SELECT id, uuid, tenant_id, organization_id, index_key, \
     job_type, status, payload_json, scheduled_at, started_at, finished_at, error_summary, \
     retry_count, created_at, updated_at, version \
     FROM search_index_job \
     WHERE tenant_id = :tenant_id AND uuid = :uuid";

/// bind order: status, started_at, finished_at, error_summary, uuid, tenant_id
///
/// 更新任务状态；`started_at`/`finished_at`/`error_summary` 可为 NULL，由调用方按状态决定。
/// 通过 uuid + tenant_id 定位，未命中返回 0 行（适配层映射为 false）。
pub const JOB_UPDATE_STATUS: &str = "UPDATE search_index_job SET status = :status, \
     started_at = :started_at, finished_at = :finished_at, error_summary = :error_summary, \
     updated_at = NOW(), version = version + 1 \
     WHERE uuid = :uuid AND tenant_id = :tenant_id";

// ===========================================================================
// search_document
// ===========================================================================

/// bind order: tenant_id, organization_id, index_key, document_id
pub const DOCUMENT_GET: &str = document_select!(
    "FROM search_document \
     WHERE tenant_id = :tenant_id AND organization_id = :organization_id \
       AND index_key = :index_key AND document_id = :document_id \
       AND deleted_at IS NULL"
);

/// bind order: tenant_id, organization_id, index_key, limit, offset
pub const DOCUMENT_LIST: &str = document_select!(
    "FROM search_document \
     WHERE tenant_id = :tenant_id AND organization_id = :organization_id \
       AND index_key = :index_key AND deleted_at IS NULL \
     ORDER BY updated_at DESC LIMIT :limit OFFSET :offset"
);

/// bind order: id, uuid, tenant_id, organization_id, index_id, index_key,
/// document_id, capability, scope, group_key, group_title, source_ref, title,
/// body_text, keyword_text, payload_json, token_json, embedding_json, status,
/// data_scope
pub const DOCUMENT_UPSERT: &str = "INSERT INTO search_document \
     (id, uuid, tenant_id, organization_id, index_id, index_key, document_id, \
      capability, scope, group_key, group_title, source_ref, title, body_text, \
      keyword_text, payload_json, token_json, embedding_json, status, data_scope, \
      indexed_at) \
     VALUES (:id, :uuid, :tenant_id, :organization_id, :index_id, :index_key, \
             :document_id, :capability, :scope, :group_key, :group_title, \
             :source_ref, :title, :body_text, :keyword_text, :payload_json, \
             :token_json, :embedding_json, :status, :data_scope, NOW()) \
     ON CONFLICT (tenant_id, organization_id, index_key, document_id) \
     DO UPDATE SET index_id = EXCLUDED.index_id, capability = EXCLUDED.capability, \
       scope = EXCLUDED.scope, group_key = EXCLUDED.group_key, \
       group_title = EXCLUDED.group_title, source_ref = EXCLUDED.source_ref, \
       title = EXCLUDED.title, body_text = EXCLUDED.body_text, \
       keyword_text = EXCLUDED.keyword_text, payload_json = EXCLUDED.payload_json, \
       token_json = EXCLUDED.token_json, embedding_json = EXCLUDED.embedding_json, \
       status = EXCLUDED.status, data_scope = EXCLUDED.data_scope, \
       indexed_at = NOW(), updated_at = NOW(), \
       version = search_document.version + 1 \
     RETURNING id, uuid, tenant_id, organization_id, index_id, index_key, \
     document_id, capability, scope, group_key, group_title, source_ref, title, \
     body_text, keyword_text, payload_json, token_json, embedding_json, status, \
     data_scope, indexed_at, created_at, updated_at, version, deleted_at, deleted_by";

/// bind order: deleted_by, tenant_id, organization_id, index_key, document_id
pub const DOCUMENT_DELETE: &str = "UPDATE search_document SET deleted_at = NOW(), \
     deleted_by = :deleted_by, version = version + 1, updated_at = NOW() \
     WHERE tenant_id = :tenant_id AND organization_id = :organization_id \
       AND index_key = :index_key AND document_id = :document_id \
       AND deleted_at IS NULL";

/// bind order: query, tenant_id, organization_id, index_key, limit, offset
///
/// `:query` appears first in the `ts_rank` projection, then again in the
/// `@@ plainto_tsquery` filter; bind it once.
pub const DOCUMENT_FULLTEXT_SEARCH: &str = document_select!(
    ", ts_rank(search_vector, plainto_tsquery(:query)) AS score \
     FROM search_document \
     WHERE tenant_id = :tenant_id AND organization_id = :organization_id \
       AND index_key = :index_key \
       AND search_vector @@ plainto_tsquery(:query) \
       AND deleted_at IS NULL \
     ORDER BY score DESC LIMIT :limit OFFSET :offset"
);

/// bind order: query, tenant_id, organization_id, index_key, limit, offset
///
/// `:query` appears first in the `similarity` projection, then again in the
/// `similarity(...) > 0.1` filter; bind it once.
pub const DOCUMENT_FUZZY_SEARCH: &str = document_select!(
    ", similarity(title, :query) AS score \
     FROM search_document \
     WHERE tenant_id = :tenant_id AND organization_id = :organization_id \
       AND index_key = :index_key \
       AND similarity(title, :query) > 0.1 \
       AND deleted_at IS NULL \
     ORDER BY score DESC LIMIT :limit OFFSET :offset"
);

/// bind order: query_embedding, tenant_id, organization_id, index_key, limit, offset
///
/// Uses pgvector cosine distance (`<=>`) on the embedding extracted from
/// `embedding_json`. The caller binds `query_embedding` as a pgvector literal
/// string (for example `"[0.1,0.2,...]"`).
pub const DOCUMENT_SEMANTIC_SEARCH: &str = document_select!(
    ", 1 - ((embedding_json -> 'vector')::text)::vector <=> \
        :query_embedding::vector AS score \
     FROM search_document \
     WHERE tenant_id = :tenant_id AND organization_id = :organization_id \
       AND index_key = :index_key \
       AND embedding_json ? 'vector' \
       AND deleted_at IS NULL \
     ORDER BY score DESC LIMIT :limit OFFSET :offset"
);

/// bind order: tenant_id, organization_id, index_key
///
/// 统计索引内未软删除的文档数量。
pub const DOCUMENT_COUNT: &str = "SELECT COUNT(*) AS count FROM search_document \
     WHERE tenant_id = :tenant_id AND organization_id = :organization_id \
       AND index_key = :index_key AND deleted_at IS NULL";

/// bind order: tenant_id, organization_id, index_key
///
/// 单行聚合：文档总数、活跃文档数（status=1）、最后更新时间。
pub const DOCUMENT_STATS: &str = "SELECT COUNT(*) AS document_count, \
            COUNT(*) FILTER (WHERE status = 1) AS active_document_count, \
            MAX(updated_at) AS last_updated \
     FROM search_document \
     WHERE tenant_id = :tenant_id AND organization_id = :organization_id \
       AND index_key = :index_key AND deleted_at IS NULL";

// ===========================================================================
// search_query_suggestion
// ===========================================================================

/// bind order: tenant_id, organization_id, index_key, prefix, limit
///
/// `:prefix` appears first in the `similarity(...) > 0.1` filter, then again in
/// the `ORDER BY similarity(...)`; bind it once.
pub const SUGGESTION_LIST: &str = "SELECT id, uuid, tenant_id, organization_id, \
     index_key, suggestion_text, source, score, status, created_at, updated_at, \
     version FROM search_query_suggestion \
     WHERE tenant_id = :tenant_id AND organization_id = :organization_id \
       AND index_key = :index_key AND status = 1 \
       AND similarity(suggestion_text, :prefix) > 0.1 \
     ORDER BY similarity(suggestion_text, :prefix) DESC, score DESC \
     LIMIT :limit";

/// bind order: id, uuid, tenant_id, organization_id, index_key, suggestion_text
///
/// `source` is omitted so the column default (`'query'`) applies on insert.
pub const SUGGESTION_UPSERT: &str = "INSERT INTO search_query_suggestion \
     (id, uuid, tenant_id, organization_id, index_key, suggestion_text, \
      score, status, version) \
     VALUES (:id, :uuid, :tenant_id, :organization_id, :index_key, \
             :suggestion_text, 1, 1, 0) \
     ON CONFLICT (tenant_id, organization_id, index_key, suggestion_text) \
     DO UPDATE SET score = search_query_suggestion.score + 1, \
       updated_at = NOW(), version = version + 1 \
     RETURNING id, uuid, tenant_id, organization_id, index_key, suggestion_text, \
     source, score, status, created_at, updated_at, version";

/// bind order: id, tenant_id, organization_id
pub const SUGGESTION_INCREMENT_POPULARITY: &str = "UPDATE search_query_suggestion \
     SET score = score + 1, updated_at = NOW(), version = version + 1 \
     WHERE id = :id AND tenant_id = :tenant_id \
       AND organization_id = :organization_id AND status = 1";

// ===========================================================================
// search_user_event / search_recent_query
// ===========================================================================

/// bind order: id, uuid, tenant_id, organization_id, user_id, event_type, surface,
/// index_key, document_id, placement, q, result_position, request_id, metadata_json
pub const USER_EVENT_RECORD: &str = "INSERT INTO search_user_event \
     (id, uuid, tenant_id, organization_id, user_id, event_type, surface, \
      index_key, document_id, placement, q, result_position, request_id, \
      metadata_json, occurred_at) \
     VALUES (:id, :uuid, :tenant_id, :organization_id, :user_id, :event_type, \
             :surface, :index_key, :document_id, :placement, :q, \
             :result_position, :request_id, :metadata_json, NOW()) \
     RETURNING id, uuid, tenant_id, organization_id, user_id, event_type, surface, \
     index_key, document_id, placement, q, result_position, request_id, \
     metadata_json, occurred_at, created_at";

/// bind order: id, uuid, tenant_id, organization_id, user_id, index_key, q, result_count
pub const RECENT_QUERY_UPSERT: &str = "INSERT INTO search_recent_query \
     (id, uuid, tenant_id, organization_id, user_id, index_key, q, result_count, \
      last_used_at) \
     VALUES (:id, :uuid, :tenant_id, :organization_id, :user_id, :index_key, \
             :q, :result_count, NOW()) \
     ON CONFLICT (tenant_id, organization_id, user_id, index_key, q) \
     DO UPDATE SET result_count = EXCLUDED.result_count, last_used_at = NOW(), \
       updated_at = NOW() \
     RETURNING id, uuid, tenant_id, organization_id, user_id, index_key, q, \
     result_count, last_used_at, created_at, updated_at";

/// bind order: tenant_id, organization_id, user_id, limit
pub const RECENT_QUERY_LIST: &str = "SELECT id, uuid, tenant_id, organization_id, \
     user_id, index_key, q, result_count, last_used_at, created_at, updated_at \
     FROM search_recent_query \
     WHERE tenant_id = :tenant_id AND organization_id = :organization_id \
       AND user_id = :user_id \
     ORDER BY last_used_at DESC LIMIT :limit";

/// bind order: tenant_id, organization_id, user_id, event_type, limit
///
/// `:event_type` 在 `IS NULL` 判定与等值过滤中各出现一次；当绑定 `None` 时返回全部事件，
/// 否则按 `event_type` 过滤。具名参数只绑定一次。
pub const USER_EVENT_LIST_BY_USER: &str = "SELECT id, uuid, tenant_id, organization_id, \
     user_id, event_type, surface, index_key, document_id, placement, q, result_position, \
     request_id, metadata_json, occurred_at, created_at \
     FROM search_user_event \
     WHERE tenant_id = :tenant_id AND organization_id = :organization_id \
       AND user_id = :user_id \
       AND (:event_type IS NULL OR event_type = :event_type) \
     ORDER BY occurred_at DESC LIMIT :limit";

/// bind order: tenant_id, organization_id, index_key, limit
///
/// 聚合 `search_user_event` 中 `view`/`click` 事件，按 `document_id` 出现频率降序返回热门文档。
pub const USER_EVENT_TRENDING: &str = "SELECT document_id, COUNT(*) AS event_count \
     FROM search_user_event \
     WHERE tenant_id = :tenant_id AND organization_id = :organization_id \
       AND index_key = :index_key \
       AND event_type IN ('view', 'click') \
       AND document_id IS NOT NULL \
     GROUP BY document_id \
     ORDER BY event_count DESC \
     LIMIT :limit";

/// bind order: tenant_id, organization_id, index_key, document_ids, limit
///
/// item-based 协同过滤：查找与指定文档有共同用户行为的相似文档。
///
/// `:document_ids` 在 `!= ALL(:document_ids::text[])` 与子查询 `= ANY(:document_ids::text[])`
/// 中各出现一次；具名参数只绑定一次。排除用户已交互的文档，仅统计 `view`/`click` 事件，
/// 按共同用户数降序返回。租户隔离由 `tenant_id` + `organization_id` 过滤保证。
pub const USER_EVENT_SIMILAR_DOCUMENTS: &str = "SELECT document_id, \
     COUNT(DISTINCT user_id) AS similarity_score \
     FROM search_user_event \
     WHERE tenant_id = :tenant_id AND organization_id = :organization_id \
       AND index_key = :index_key \
       AND document_id != ALL(:document_ids::text[]) \
       AND user_id IN ( \
         SELECT DISTINCT user_id FROM search_user_event \
         WHERE document_id = ANY(:document_ids::text[]) \
       ) \
       AND document_id IS NOT NULL \
       AND event_type IN ('view', 'click') \
     GROUP BY document_id \
     ORDER BY similarity_score DESC \
     LIMIT :limit";

// ===========================================================================
// search_promotion
// ===========================================================================

/// bind order: tenant_id, organization_id, index_key, placement, now
///
/// 列出当前租户/组织下处于活跃状态（`status=1`、未软删除、有效期覆盖 `:now`）的推广。
pub const PROMOTION_LIST_ACTIVE: &str = "SELECT id, uuid, tenant_id, organization_id, \
     promotion_key, placement, index_key, document_id, priority, rule_json, status, \
     active_from, active_until, created_at, updated_at, version, deleted_at, deleted_by \
     FROM search_promotion \
     WHERE tenant_id = :tenant_id AND organization_id = :organization_id \
       AND index_key = :index_key AND placement = :placement \
       AND status = 1 AND deleted_at IS NULL \
       AND (active_from IS NULL OR active_from <= :now) \
       AND (active_until IS NULL OR active_until >= :now) \
     ORDER BY priority DESC, updated_at DESC";

/// bind order: id, uuid, tenant_id, organization_id, promotion_key, placement,
/// index_key, document_id, priority, rule_json
pub const PROMOTION_CREATE: &str = "INSERT INTO search_promotion \
     (id, uuid, tenant_id, organization_id, promotion_key, placement, index_key, \
      document_id, priority, rule_json, status, version) \
     VALUES (:id, :uuid, :tenant_id, :organization_id, :promotion_key, :placement, \
             :index_key, :document_id, :priority, :rule_json, 1, 0) \
     RETURNING id, uuid, tenant_id, organization_id, promotion_key, placement, index_key, \
     document_id, priority, rule_json, status, active_from, active_until, created_at, \
     updated_at, version, deleted_at, deleted_by";

/// bind order: id, uuid, tenant_id, organization_id, user_id, event_type, promotion_id, metadata_json
///
/// 将推广曝光/点击写入 `search_user_event`，`surface` 固定为 `'promotion'`，
/// `document_id` 复用为 `promotion_id` 以便后续聚合统计。
pub const PROMOTION_EVENT_INSERT: &str = "INSERT INTO search_user_event \
     (id, uuid, tenant_id, organization_id, user_id, event_type, surface, \
      document_id, metadata_json, occurred_at) \
     VALUES (:id, :uuid, :tenant_id, :organization_id, :user_id, :event_type, \
             'promotion', :promotion_id, :metadata_json, NOW())";

/// bind order: tenant_id, organization_id, promotion_id
///
/// 聚合指定推广的曝光（`promotion_delivery`）与点击（`promotion_click`）计数。
pub const PROMOTION_STATS: &str = "SELECT \
     COUNT(*) FILTER (WHERE event_type = 'promotion_delivery') AS delivery_count, \
     COUNT(*) FILTER (WHERE event_type = 'promotion_click') AS click_count \
     FROM search_user_event \
     WHERE tenant_id = :tenant_id AND organization_id = :organization_id \
       AND document_id = :promotion_id \
       AND event_type IN ('promotion_delivery', 'promotion_click')";

/// bind order: tenant_id, organization_id, index_key, limit, offset
///
/// 列出索引下全部推广（含未活跃），按优先级与更新时间排序。
pub const PROMOTION_LIST_ALL: &str = promotion_select!(
    "FROM search_promotion \
     WHERE tenant_id = :tenant_id AND organization_id = :organization_id \
       AND index_key = :index_key AND deleted_at IS NULL \
     ORDER BY priority DESC, updated_at DESC LIMIT :limit OFFSET :offset"
);

/// bind order: tenant_id, organization_id, promotion_key
///
/// 按 `promotion_key` 查询单个推广（未软删除）。
pub const PROMOTION_GET: &str = promotion_select!(
    "FROM search_promotion \
     WHERE tenant_id = :tenant_id AND organization_id = :organization_id \
       AND promotion_key = :promotion_key AND deleted_at IS NULL"
);

/// bind order: placement, document_id, priority, rule_json, status, active_from,
///             active_until, tenant_id, organization_id, promotion_key
///
/// 按字段补丁更新推广；`None` 字段通过 `COALESCE` 保留原值。
pub const PROMOTION_UPDATE: &str = "UPDATE search_promotion SET \
       placement = COALESCE(:placement, placement), \
       document_id = COALESCE(:document_id, document_id), \
       priority = COALESCE(:priority, priority), \
       rule_json = COALESCE(:rule_json, rule_json), \
       status = COALESCE(:status, status), \
       active_from = COALESCE(:active_from, active_from), \
       active_until = COALESCE(:active_until, active_until), \
       updated_at = NOW(), version = version + 1 \
     WHERE tenant_id = :tenant_id AND organization_id = :organization_id \
       AND promotion_key = :promotion_key AND deleted_at IS NULL \
     RETURNING id, uuid, tenant_id, organization_id, promotion_key, placement, \
     index_key, document_id, priority, rule_json, status, active_from, \
     active_until, created_at, updated_at, version, deleted_at, deleted_by";

/// bind order: deleted_by, tenant_id, organization_id, promotion_key
///
/// 软删除推广：设置 `deleted_at` 与 `deleted_by`。
pub const PROMOTION_DELETE: &str = "UPDATE search_promotion SET deleted_at = NOW(), \
     deleted_by = :deleted_by, version = version + 1, updated_at = NOW() \
     WHERE tenant_id = :tenant_id AND organization_id = :organization_id \
       AND promotion_key = :promotion_key AND deleted_at IS NULL";
