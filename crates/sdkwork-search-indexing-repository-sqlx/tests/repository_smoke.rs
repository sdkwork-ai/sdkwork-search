//! Repository smoke tests.
//!
//! These tests verify the crate assembles correctly without requiring a live
//! PostgreSQL connection: schema constants are well-formed, the migration SQL is
//! embedded, params structs can be constructed, and error conversion works.
// WORKSPACE-PATH:allow-fixture: fixtures name a foreign checkout root, drive, or home directory to exercise path handling, so the literal is the value under assertion rather than a binding this build resolves

use sdkwork_search_indexing_repository_sqlx::db::schema::{
    SEARCH_DOCUMENT_TABLE, SEARCH_INDEX_TABLE, SEARCH_QUERY_SUGGESTION_TABLE,
    SEARCH_RECENT_QUERY_TABLE, SEARCH_USER_EVENT_TABLE,
};
use sdkwork_search_indexing_repository_sqlx::repository::{
    CreateIndexParams, RecordUserEventParams, UpdateIndexParams, UpsertDocumentParams,
};
use sdkwork_search_indexing_repository_sqlx::{
    CreatePromotionParams, RepositoryError, SearchDocumentRepository, SearchIndexRepository,
    SearchPromotionRepository, SearchRepositoryAdapter, SearchSuggestionRepository,
    SearchUserEventRepository, UpsertRecentQueryParams, SEARCH_INDEXING_MIGRATION_SQL,
};

#[test]
fn embeds_search_migration_sql() {
    assert!(!SEARCH_INDEXING_MIGRATION_SQL.is_empty());
    assert!(
        SEARCH_INDEXING_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS search_index"),
        "migration must create search_index",
    );
    assert!(
        SEARCH_INDEXING_MIGRATION_SQL.contains("CREATE TABLE IF NOT EXISTS search_document"),
        "migration must create search_document",
    );
    assert!(
        SEARCH_INDEXING_MIGRATION_SQL.contains("CREATE EXTENSION IF NOT EXISTS pg_trgm"),
        "migration must enable pg_trgm",
    );
}

#[test]
fn exposes_search_prefixed_table_constants() {
    for table in [
        SEARCH_INDEX_TABLE,
        SEARCH_DOCUMENT_TABLE,
        SEARCH_QUERY_SUGGESTION_TABLE,
        SEARCH_USER_EVENT_TABLE,
        SEARCH_RECENT_QUERY_TABLE,
    ] {
        assert!(
            table.starts_with("search_"),
            "table constant must be search-prefixed: {table}",
        );
    }
}

#[test]
fn repository_error_converts_from_sqlx_row_not_found() {
    let error = RepositoryError::from(sqlx::Error::RowNotFound);
    match error {
        RepositoryError::NotFound(_) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[test]
fn repository_error_displays() {
    let error = RepositoryError::Conflict("duplicate index_key".into());
    let message = format!("{error}");
    assert!(message.contains("repository conflict"));
    assert!(message.contains("duplicate index_key"));
}

#[test]
fn can_construct_index_params() {
    let create = CreateIndexParams {
        id: 1,
        tenant_id: 100_001,
        organization_id: 0,
        index_key: "knowledge_base".into(),
        title: "Knowledge Base".into(),
        description: Some("docs index".into()),
        config_json: serde_json::json!({"analyzer": "default"}),
    };
    assert_eq!(create.index_key, "knowledge_base");

    let update = UpdateIndexParams {
        id: create.id,
        tenant_id: create.tenant_id,
        title: create.title.clone(),
        description: None,
        status: 0,
        config_json: serde_json::json!({}),
    };
    assert_eq!(update.status, 0);
}

#[test]
fn can_construct_document_params() {
    let params = UpsertDocumentParams {
        id: 42,
        tenant_id: 100_001,
        organization_id: 0,
        index_id: 1,
        index_key: "knowledge_base".into(),
        document_id: "doc-1".into(),
        capability: Some("wiki".into()),
        scope: "global".into(),
        group_key: None,
        group_title: None,
        source_ref: None,
        title: "Title".into(),
        body_text: Some("body".into()),
        keyword_text: None,
        payload_json: serde_json::json!({"path": "/a/b"}),
        token_json: serde_json::json!([]),
        embedding_json: serde_json::json!({"vector": [0.1, 0.2]}),
        status: 1,
        data_scope: 0,
    };
    assert_eq!(params.document_id, "doc-1");
}

#[test]
fn can_construct_user_event_params() {
    let params = RecordUserEventParams {
        id: 7,
        tenant_id: 100_001,
        organization_id: 0,
        user_id: 1,
        event_type: "search".into(),
        surface: "app".into(),
        index_key: Some("knowledge_base".into()),
        document_id: None,
        placement: None,
        q: Some("hello".into()),
        result_position: Some(1),
        request_id: Some("req-1".into()),
        metadata_json: serde_json::json!({}),
    };
    assert_eq!(params.event_type, "search");
}

#[test]
fn repositories_are_nameable_without_pool() {
    // The repository structs only construct from a PgPool, which we cannot
    // create without a database. This test confirms the types are reachable and
    // nameable from the crate root, exercising the public surface.
    fn _assert_types() {
        let _ = std::marker::PhantomData::<SearchIndexRepository>;
        let _ = std::marker::PhantomData::<SearchDocumentRepository>;
        let _ = std::marker::PhantomData::<SearchSuggestionRepository>;
        let _ = std::marker::PhantomData::<SearchUserEventRepository>;
        let _ = std::marker::PhantomData::<SearchPromotionRepository>;
        let _ = std::marker::PhantomData::<SearchRepositoryAdapter>;
    }
}

#[test]
fn can_construct_promotion_params() {
    let params = CreatePromotionParams {
        id: 100,
        tenant_id: 100_001,
        organization_id: 0,
        promotion_key: "summer_docs".into(),
        placement: "top".into(),
        index_key: "knowledge_base".into(),
        document_id: "doc-1".into(),
        priority: 10,
        rule_json: serde_json::json!([]),
    };
    assert_eq!(params.promotion_key, "summer_docs");
    assert_eq!(params.placement, "top");
}

#[test]
fn can_construct_upsert_recent_query_params() {
    let params = UpsertRecentQueryParams {
        id: 9,
        tenant_id: 100_001,
        organization_id: 0,
        user_id: 1,
        index_key: "knowledge_base",
        q: "hello",
        result_count: 5,
    };
    assert_eq!(params.q, "hello");
    assert_eq!(params.result_count, 5);
}
