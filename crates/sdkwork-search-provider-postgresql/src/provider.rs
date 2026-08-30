//! PostgreSQL search provider implementation.
//!
//! Uses PostgreSQL `tsvector` full-text search, `pg_trgm` for fuzzy matching and suggestions,
//! and `pgvector` for semantic/vector search. Operates on the `search_document` and related
//! tables owned by `sdkwork-search-indexing-repository-sqlx` migrations.
//!
//! ## Column mapping
//!
//! The `search_document` table uses these columns (see migration `0001_search_storage.sql`):
//!
//! | Logical concept | Physical column   | Type      |
//! |-----------------|-------------------|-----------|
//! | Document source | `payload_json`    | JSONB     |
//! | Body text       | `body_text`       | TEXT      |
//! | Keywords/tags    | `keyword_text`    | TEXT      |
//! | Embedding       | `embedding_json`  | JSONB     |
//! | Search vector   | `search_vector`   | TSVECTOR  |
//!
//! `embedding_json` stores `{"vector": [0.1, 0.2, ...]}` so that semantic search can
//! extract and cast it: `(embedding_json->'vector')::text::vector`.

use sdkwork_search_provider_spi::{
    context::SearchProviderContext,
    document::{
        DeleteDocument, IndexDocument, IndexDocumentBatch, IndexOperationError,
        IndexOperationResult, UpdateDocument,
    },
    error::{SearchProviderError, SearchProviderResult},
    provider::{
        SearchProvider, SearchProviderCapability, SearchProviderConfig, SearchProviderKind,
    },
    query::{
        FacetBucket, SearchHit, SearchQuery, SearchResponse, SearchSuggestion,
        SearchSuggestionQuery, SearchSuggestionResponse, SemanticSearchHit, SemanticSearchQuery,
        SemanticSearchResponse, SortOrder,
    },
    registry::SearchProviderFactory,
};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::sync::Arc;

const CAPABILITIES: &[SearchProviderCapability] = &[
    SearchProviderCapability::FullTextSearch,
    SearchProviderCapability::FuzzySearch,
    SearchProviderCapability::SemanticSearch,
    SearchProviderCapability::VectorSearch,
    SearchProviderCapability::FacetedSearch,
    SearchProviderCapability::Autocomplete,
    SearchProviderCapability::Highlighting,
    SearchProviderCapability::Ranking,
    SearchProviderCapability::Suggestions,
    SearchProviderCapability::Indexing,
    SearchProviderCapability::DocumentCrud,
];

pub struct PostgresqlSearchProvider {
    id: String,
    pool: PgPool,
}

impl PostgresqlSearchProvider {
    pub fn new(id: impl Into<String>, pool: PgPool) -> Self {
        Self {
            id: id.into(),
            pool,
        }
    }

    /// Ensure required extensions exist. Called lazily on health check.
    async fn ensure_extensions(&self) -> SearchProviderResult<()> {
        sqlx::query("CREATE EXTENSION IF NOT EXISTS pg_trgm")
            .execute(&self.pool)
            .await
            .map_err(|e| SearchProviderError::Backend {
                message: e.to_string(),
            })?;
        // vector extension is optional for deployments without semantic search; ignore error
        let _ = sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
            .execute(&self.pool)
            .await;
        Ok(())
    }
}

/// Validates that a string is safe to embed directly in SQL (alphanumeric + underscore only).
/// Used for filter keys, facet field names, and sort field names which cannot be parameterized.
fn is_safe_identifier(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Renders a float slice as a pgvector literal, e.g. `"[0.1,0.2,0.3]"`.
fn format_vector_literal(embedding: &[f32]) -> String {
    let mut body = String::from("[");
    for (i, value) in embedding.iter().enumerate() {
        if i > 0 {
            body.push(',');
        }
        body.push_str(&value.to_string());
    }
    body.push(']');
    body
}

/// Stores embedding as JSONB `{"vector": [0.1, 0.2, ...]}`.
fn embedding_to_json(embedding: &[f32]) -> serde_json::Value {
    serde_json::json!({ "vector": embedding })
}

#[async_trait::async_trait]
impl SearchProvider for PostgresqlSearchProvider {
    fn kind(&self) -> SearchProviderKind {
        SearchProviderKind::Postgresql
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> &[SearchProviderCapability] {
        CAPABILITIES
    }

    async fn health(&self) -> SearchProviderResult<bool> {
        // Best-effort extension provisioning; failure does not fail health check,
        // because some deployments may not have superuser privileges.
        let _ = self.ensure_extensions().await;
        let row: (i64,) = sqlx::query_as("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| SearchProviderError::Backend {
                message: e.to_string(),
            })?;
        Ok(row.0 == 1)
    }

    async fn search(
        &self,
        ctx: &SearchProviderContext,
        query: &SearchQuery,
    ) -> SearchProviderResult<SearchResponse> {
        let start = std::time::Instant::now();
        let offset = ((query.page.max(1) - 1) * query.page_size.max(1)) as i64;
        let limit = query.page_size.max(1) as i64;

        // --- Build dynamic WHERE clause ---
        let mut where_parts: Vec<String> = vec![
            "tenant_id = $1".into(),
            "organization_id = $2".into(),
            "index_key = $3".into(),
            "deleted_at IS NULL".into(),
        ];
        let mut next_param: usize = 4;

        let has_query = !query.query_text.is_empty();

        // Full-text + fuzzy search condition
        if has_query {
            where_parts.push(format!(
                "(search_vector @@ websearch_to_tsquery('pg_catalog.english', ${next_param}) \
                 OR title % ${next_param} \
                 OR body_text % ${next_param})"
            ));
            next_param += 1;
        }

        // Structured filters: payload_json->>'key' IN ($v1, $v2, ...)
        for (key, values) in &query.filters {
            if !is_safe_identifier(key) || values.is_empty() {
                continue;
            }
            let placeholders: Vec<String> = values
                .iter()
                .map(|_| {
                    let p = format!("${next_param}");
                    next_param += 1;
                    p
                })
                .collect();
            where_parts.push(format!(
                "payload_json->>'{}' IN ({})",
                key,
                placeholders.join(", ")
            ));
        }

        let where_clause = where_parts.join(" AND ");

        // --- Build ORDER BY ---
        let order_clause = if query.sort.is_empty() {
            if has_query {
                "ORDER BY rank DESC".to_string()
            } else {
                "ORDER BY updated_at DESC".to_string()
            }
        } else {
            let sort_parts: Vec<String> = query
                .sort
                .iter()
                .filter(|s| is_safe_identifier(&s.field))
                .map(|s| {
                    let direction = if s.order == SortOrder::Desc {
                        "DESC"
                    } else {
                        "ASC"
                    };
                    format!("payload_json->>'{}' {}", s.field, direction)
                })
                .collect();
            if sort_parts.is_empty() {
                "ORDER BY updated_at DESC".to_string()
            } else {
                format!("ORDER BY {}", sort_parts.join(", "))
            }
        };

        // --- Build SELECT with highlight + total count ---
        let highlight_select = if let Some(hl) = &query.highlight {
            let fields: Vec<&str> = hl.fields.iter().map(|s| s.as_str()).collect();
            let headline_exprs: Vec<String> = fields
                .iter()
                .filter(|f| is_safe_identifier(f))
                .map(|field| {
                    format!(
                        "ts_headline('pg_catalog.english', {field}, \
                         websearch_to_tsquery('pg_catalog.english', $4), \
                         'StartSel={pre}, StopSel={post}, MaxWords={max_words}') AS hl_{field}",
                        field = field,
                        pre = hl.pre_tag.as_deref().unwrap_or("<em>"),
                        post = hl.post_tag.as_deref().unwrap_or("</em>"),
                        max_words = hl.fragment_size.unwrap_or(100)
                    )
                })
                .collect();
            if headline_exprs.is_empty() {
                String::new()
            } else {
                format!(", {}", headline_exprs.join(", "))
            }
        } else {
            String::new()
        };

        let rank_expr = if has_query {
            "ts_rank(search_vector, websearch_to_tsquery('pg_catalog.english', $4)) AS rank"
        } else {
            "0.0::float8 AS rank"
        };

        let sql = format!(
            r#"SELECT document_id, title, payload_json,
                      {rank_expr},
                      COUNT(*) OVER() AS total_count
                      {highlight_select}
               FROM search_document
               WHERE {where_clause}
               {order_clause}
               LIMIT ${limit_param} OFFSET ${offset_param}"#,
            rank_expr = rank_expr,
            highlight_select = highlight_select,
            where_clause = where_clause,
            order_clause = order_clause,
            limit_param = next_param,
            offset_param = next_param + 1,
        );

        // --- Bind parameters ---
        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(ctx.tenant_id)
            .bind(ctx.organization_id)
            .bind(&query.index_key);

        if has_query {
            q = q.bind(&query.query_text);
        }

        for values in query.filters.values() {
            for value in values {
                q = q.bind(value);
            }
        }

        q = q.bind(limit).bind(offset);

        let rows = q
            .fetch_all(&self.pool)
            .await
            .map_err(|e| SearchProviderError::Backend {
                message: e.to_string(),
            })?;

        // --- Decode results ---
        let mut total: u64 = 0;
        let mut hits: Vec<SearchHit> = Vec::with_capacity(rows.len());

        for row in &rows {
            let document_id: String = row.get("document_id");
            let title: Option<String> = row.get("title");
            let payload: serde_json::Value = row.get("payload_json");
            let rank: f64 = row.get("rank");
            let total_count: i64 = row.get("total_count");
            total = total_count.max(0) as u64;

            // Merge title into payload for client convenience
            let mut source = payload;
            if let Some(title) = title {
                if let Some(obj) = source.as_object_mut() {
                    obj.entry("title".to_string())
                        .or_insert(serde_json::Value::String(title));
                }
            }

            // Build highlight map
            let mut highlight_map: HashMap<String, Vec<String>> = HashMap::new();
            if let Some(hl) = &query.highlight {
                for field in &hl.fields {
                    if !is_safe_identifier(field) {
                        continue;
                    }
                    let column = format!("hl_{}", field);
                    if let Ok(Some(text)) = row.try_get::<Option<String>, _>(column.as_str()) {
                        highlight_map.insert(field.clone(), vec![text]);
                    }
                }
            }

            hits.push(SearchHit {
                document_id,
                score: rank,
                source,
                highlight: highlight_map,
                index_key: Some(query.index_key.clone()),
            });
        }

        // Apply min_score filter (post-fetch, since rank is computed)
        if let Some(min_score) = query.min_score {
            hits.retain(|h| h.score >= min_score);
        }

        // --- Run facet queries ---
        let facets: HashMap<String, Vec<FacetBucket>> = if !query.facets.is_empty() {
            self.compute_facets(ctx, query, &where_clause).await?
        } else {
            HashMap::new()
        };

        let max_score = hits.first().map(|h| h.score);

        Ok(SearchResponse {
            total,
            hits,
            facets,
            took_ms: start.elapsed().as_millis() as u64,
            max_score,
            request_id: ctx.request_id.clone(),
        })
    }

    async fn suggest(
        &self,
        ctx: &SearchProviderContext,
        query: &SearchSuggestionQuery,
    ) -> SearchProviderResult<SearchSuggestionResponse> {
        let start = std::time::Instant::now();
        let limit = query.page_size.max(1) as i64;

        let rows: Vec<(String, f32)> = sqlx::query_as(
            r#"
            SELECT suggestion_text, similarity(suggestion_text, $4) AS sim
            FROM search_query_suggestion
            WHERE tenant_id = $1
              AND organization_id = $2
              AND index_key = $3
              AND suggestion_text % $4
            ORDER BY sim DESC
            LIMIT $5
            "#,
        )
        .bind(ctx.tenant_id)
        .bind(ctx.organization_id)
        .bind(&query.index_key)
        .bind(&query.prefix)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SearchProviderError::Backend {
            message: e.to_string(),
        })?;

        let suggestions: Vec<SearchSuggestion> = rows
            .into_iter()
            .map(|(text, sim)| SearchSuggestion {
                text,
                score: sim as f64,
                payload: None,
            })
            .collect();

        Ok(SearchSuggestionResponse {
            suggestions,
            took_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn semantic_search(
        &self,
        ctx: &SearchProviderContext,
        query: &SemanticSearchQuery,
    ) -> SearchProviderResult<SemanticSearchResponse> {
        let start = std::time::Instant::now();
        let top_k = query.top_k.max(1) as i64;

        if query.query_embedding.is_empty() {
            return Ok(SemanticSearchResponse {
                hits: Vec::new(),
                took_ms: start.elapsed().as_millis() as u64,
            });
        }

        let embedding_str = format_vector_literal(&query.query_embedding);

        // Extract vector from embedding_json JSONB column and use cosine distance.
        let rows: Vec<(String, f32, Option<serde_json::Value>, Option<String>)> = sqlx::query_as(
            r#"
            SELECT document_id,
                   1 - ((embedding_json->'vector')::text)::vector <=> $4::vector AS score,
                   payload_json,
                   title
            FROM search_document
            WHERE tenant_id = $1
              AND organization_id = $2
              AND index_key = $3
              AND deleted_at IS NULL
              AND embedding_json ? 'vector'
            ORDER BY (embedding_json->'vector')::text::vector <=> $4::vector
            LIMIT $5
            "#,
        )
        .bind(ctx.tenant_id)
        .bind(ctx.organization_id)
        .bind(&query.index_key)
        .bind(&embedding_str)
        .bind(top_k)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SearchProviderError::Backend {
            message: e.to_string(),
        })?;

        let mut hits: Vec<SemanticSearchHit> = rows
            .into_iter()
            .map(|(doc_id, score, payload, title)| {
                let mut source = payload.unwrap_or(serde_json::Value::Null);
                if let Some(title) = title {
                    if let Some(obj) = source.as_object_mut() {
                        obj.entry("title".to_string())
                            .or_insert(serde_json::Value::String(title));
                    }
                }
                SemanticSearchHit {
                    document_id: doc_id,
                    score,
                    source,
                }
            })
            .collect();

        // Apply min_score filter if configured
        if let Some(min_score) = query.min_score {
            hits.retain(|h| h.score >= min_score);
        }

        Ok(SemanticSearchResponse {
            hits,
            took_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn index_document(
        &self,
        ctx: &SearchProviderContext,
        doc: &IndexDocument,
    ) -> SearchProviderResult<()> {
        let title = doc.title.as_deref().unwrap_or("");
        let body_text = doc.content.as_deref().unwrap_or("");
        // Join tags into keyword_text (pipe-separated for multi-value search)
        let keyword_text = if doc.tags.is_empty() {
            None
        } else {
            Some(doc.tags.join(" | "))
        };

        // Merge drive references into payload_json
        let mut payload = doc.source.clone();
        if let Some(obj) = payload.as_object_mut() {
            if let Some(space_id) = &doc.drive_space_id {
                obj.entry("drive_space_id")
                    .or_insert(serde_json::Value::String(space_id.clone()));
            }
            if let Some(node_id) = &doc.drive_node_id {
                obj.entry("drive_node_id")
                    .or_insert(serde_json::Value::String(node_id.clone()));
            }
        }

        let embedding_json = embedding_to_json(&doc.embedding);

        // search_vector is auto-populated by the trigger (migration 0002),
        // so we do not set it explicitly here.
        sqlx::query(
            r#"
            INSERT INTO search_document
                (uuid, tenant_id, organization_id, index_id, index_key, document_id,
                 title, body_text, keyword_text, payload_json, token_json,
                 embedding_json, status, data_scope, indexed_at)
            VALUES ($1, $2, $3, 0, $4, $5, $6, $7, $8, $9, '[]'::jsonb,
                    $10, 1, 0, CURRENT_TIMESTAMP)
            ON CONFLICT (tenant_id, organization_id, index_key, document_id)
            DO UPDATE SET
                title = EXCLUDED.title,
                body_text = EXCLUDED.body_text,
                keyword_text = EXCLUDED.keyword_text,
                payload_json = EXCLUDED.payload_json,
                embedding_json = EXCLUDED.embedding_json,
                indexed_at = CURRENT_TIMESTAMP,
                updated_at = CURRENT_TIMESTAMP,
                version = search_document.version + 1
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(ctx.tenant_id)
        .bind(ctx.organization_id)
        .bind(&doc.index_key)
        .bind(&doc.document_id)
        .bind(title)
        .bind(body_text)
        .bind(keyword_text)
        .bind(&payload)
        .bind(&embedding_json)
        .execute(&self.pool)
        .await
        .map_err(|e| SearchProviderError::Indexing {
            message: e.to_string(),
        })?;

        Ok(())
    }

    async fn index_batch(
        &self,
        ctx: &SearchProviderContext,
        batch: &IndexDocumentBatch,
    ) -> SearchProviderResult<IndexOperationResult> {
        let mut success = 0u32;
        let mut failures = 0u32;
        let mut errors = Vec::new();
        for doc in &batch.documents {
            match self.index_document(ctx, doc).await {
                Ok(()) => success += 1,
                Err(e) => {
                    failures += 1;
                    errors.push(IndexOperationError {
                        document_id: doc.document_id.clone(),
                        message: e.to_string(),
                    });
                }
            }
        }
        Ok(IndexOperationResult {
            success_count: success,
            failure_count: failures,
            errors,
        })
    }

    async fn update_document(
        &self,
        ctx: &SearchProviderContext,
        doc: &UpdateDocument,
    ) -> SearchProviderResult<()> {
        if doc.partial {
            // Partial update: merge payload_json fields
            sqlx::query(
                r#"
                UPDATE search_document
                SET payload_json = payload_json || $4,
                    updated_at = CURRENT_TIMESTAMP,
                    version = version + 1
                WHERE tenant_id = $1 AND organization_id = $2
                  AND index_key = $3 AND document_id = $5
                  AND deleted_at IS NULL
                "#,
            )
            .bind(ctx.tenant_id)
            .bind(ctx.organization_id)
            .bind(&doc.index_key)
            .bind(&doc.source)
            .bind(&doc.document_id)
            .execute(&self.pool)
            .await
            .map_err(|e| SearchProviderError::Indexing {
                message: e.to_string(),
            })?;
        } else {
            // Full replace of payload_json
            sqlx::query(
                r#"
                UPDATE search_document
                SET payload_json = $4,
                    updated_at = CURRENT_TIMESTAMP,
                    version = version + 1
                WHERE tenant_id = $1 AND organization_id = $2
                  AND index_key = $3 AND document_id = $5
                  AND deleted_at IS NULL
                "#,
            )
            .bind(ctx.tenant_id)
            .bind(ctx.organization_id)
            .bind(&doc.index_key)
            .bind(&doc.source)
            .bind(&doc.document_id)
            .execute(&self.pool)
            .await
            .map_err(|e| SearchProviderError::Indexing {
                message: e.to_string(),
            })?;
        }
        Ok(())
    }

    async fn delete_document(
        &self,
        ctx: &SearchProviderContext,
        doc: &DeleteDocument,
    ) -> SearchProviderResult<()> {
        // Soft delete per DATABASE_SPEC.md conventions
        sqlx::query(
            r#"
            UPDATE search_document
            SET deleted_at = CURRENT_TIMESTAMP,
                status = 0,
                version = version + 1
            WHERE tenant_id = $1 AND organization_id = $2
              AND index_key = $3 AND document_id = $4
              AND deleted_at IS NULL
            "#,
        )
        .bind(ctx.tenant_id)
        .bind(ctx.organization_id)
        .bind(&doc.index_key)
        .bind(&doc.document_id)
        .execute(&self.pool)
        .await
        .map_err(|e| SearchProviderError::Indexing {
            message: e.to_string(),
        })?;
        Ok(())
    }

    async fn create_index(
        &self,
        ctx: &SearchProviderContext,
        index_key: &str,
        schema: &serde_json::Value,
    ) -> SearchProviderResult<()> {
        sqlx::query(
            r#"
            INSERT INTO search_index
                (uuid, tenant_id, organization_id, index_key, title, description,
                 status, data_scope, config_json)
            VALUES ($1, $2, $3, $4, $4, NULL, 1, 0, $5)
            ON CONFLICT (tenant_id, organization_id, index_key)
            DO UPDATE SET config_json = EXCLUDED.config_json, updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(ctx.tenant_id)
        .bind(ctx.organization_id)
        .bind(index_key)
        .bind(schema)
        .execute(&self.pool)
        .await
        .map_err(|e| SearchProviderError::Indexing {
            message: e.to_string(),
        })?;
        Ok(())
    }

    async fn drop_index(
        &self,
        ctx: &SearchProviderContext,
        index_key: &str,
    ) -> SearchProviderResult<()> {
        sqlx::query(
            r#"
            UPDATE search_index
            SET deleted_at = CURRENT_TIMESTAMP, status = 0, updated_at = CURRENT_TIMESTAMP
            WHERE tenant_id = $1 AND organization_id = $2 AND index_key = $3
            "#,
        )
        .bind(ctx.tenant_id)
        .bind(ctx.organization_id)
        .bind(index_key)
        .execute(&self.pool)
        .await
        .map_err(|e| SearchProviderError::Indexing {
            message: e.to_string(),
        })?;
        Ok(())
    }
}

impl PostgresqlSearchProvider {
    /// Executes facet aggregation queries for the requested facet fields.
    ///
    /// Each facet field produces a `Vec<FacetBucket>` with value → count.
    async fn compute_facets(
        &self,
        ctx: &SearchProviderContext,
        query: &SearchQuery,
        base_where: &str,
    ) -> SearchProviderResult<HashMap<String, Vec<FacetBucket>>> {
        let mut facets: HashMap<String, Vec<FacetBucket>> = HashMap::new();

        for field in &query.facets {
            if !is_safe_identifier(field) {
                continue;
            }

            // Build facet SQL using the same WHERE clause (minus the full-text condition
            // which references $4). We rebuild a simplified WHERE without the query_text
            // binding — facets should count all matching documents regardless of query text.
            let facet_where = base_where
                .replace(
                    "search_vector @@ websearch_to_tsquery('pg_catalog.english', $4)",
                    "true",
                )
                .replace("title % $4", "true")
                .replace("body_text % $4", "true");

            let sql = format!(
                r#"SELECT payload_json->>'{}' AS value, COUNT(*) AS count
                   FROM search_document
                   WHERE {}
                   AND payload_json ? '{}'
                   AND payload_json->>'{}' IS NOT NULL
                   AND payload_json->>'{}' != ''
                   GROUP BY payload_json->>'{}'
                   ORDER BY count DESC
                   LIMIT 50"#,
                field, facet_where, field, field, field, field
            );

            // Rebuild bindings for facet query (only tenant/org/index)
            let rows: Vec<(Option<String>, i64)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
                .bind(ctx.tenant_id)
                .bind(ctx.organization_id)
                .bind(&query.index_key)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| SearchProviderError::Backend {
                    message: e.to_string(),
                })?;

            let buckets: Vec<FacetBucket> = rows
                .into_iter()
                .filter_map(|(value, count)| {
                    value.map(|v| FacetBucket {
                        value: v,
                        count: count as u64,
                    })
                })
                .collect();

            facets.insert(field.clone(), buckets);
        }

        Ok(facets)
    }
}

/// Factory function for registry registration.
/// The `connection` field of `SearchProviderConfig` must contain `{"url": "postgres://..."}`.
pub fn factory() -> SearchProviderFactory {
    Arc::new(|cfg: &SearchProviderConfig| {
        let url = cfg
            .connection
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SearchProviderError::Configuration {
                message: "missing connection.url for postgresql provider".into(),
            })?;
        let pool =
            sqlx::PgPool::connect_lazy(url).map_err(|e| SearchProviderError::Configuration {
                message: format!("failed to create pg pool: {e}"),
            })?;
        Ok(Arc::new(PostgresqlSearchProvider::new(&cfg.id, pool)) as Arc<dyn SearchProvider>)
    })
}
