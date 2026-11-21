-- sdkwork:migration
-- id: 0001_organization_id_not_null
-- engine: postgres
-- module: sdkwork-search
-- purpose: Enforce organization_id NOT NULL DEFAULT on all tables in the
--   consolidated baseline. NULL rows (pre-standard data anomalies) are
--   backfilled with the platform sentinel before NOT NULL is set, and
--   NOT NULL columns without an explicit default receive the sentinel
--   default, keeping existing deployments consistent with fresh baseline
--   installs.
-- reversible: false
-- rollback: forward-fix (sentinel backfill is the canonical fix; NULL
--   organization rows are data anomalies)
-- transactional: true
-- lock: lightweight
-- lock_timeout: 2s
-- statement_timeout: 30s

BEGIN;

ALTER TABLE search_index ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE search_index SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE search_index ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE search_index ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE search_document ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE search_document SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE search_document ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE search_document ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE search_synonym_set ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE search_synonym_set SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE search_synonym_set ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE search_synonym_set ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE search_synonym_entry ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE search_synonym_entry SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE search_synonym_entry ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE search_synonym_entry ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE search_ranking_profile ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE search_ranking_profile SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE search_ranking_profile ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE search_ranking_profile ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE search_recommendation_strategy ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE search_recommendation_strategy SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE search_recommendation_strategy ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE search_recommendation_strategy ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE search_promotion ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE search_promotion SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE search_promotion ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE search_promotion ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE search_user_event ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE search_user_event SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE search_user_event ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE search_user_event ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE search_recent_query ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE search_recent_query SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE search_recent_query ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE search_recent_query ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE search_query_suggestion ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE search_query_suggestion SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE search_query_suggestion ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE search_query_suggestion ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE search_embedding_job ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE search_embedding_job SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE search_embedding_job ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE search_embedding_job ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE search_provider_config ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE search_provider_config SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE search_provider_config ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE search_provider_config ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE search_provider_health_check ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE search_provider_health_check SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE search_provider_health_check ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE search_provider_health_check ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE search_ab_experiment ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE search_ab_experiment SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE search_ab_experiment ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE search_ab_experiment ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE search_ab_assignment ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE search_ab_assignment SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE search_ab_assignment ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE search_ab_assignment ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE search_document_projection ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE search_document_projection SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE search_document_projection ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE search_document_projection ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE search_query_audit ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE search_query_audit SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE search_query_audit ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE search_query_audit ALTER COLUMN organization_id SET NOT NULL;

ALTER TABLE search_index_job ADD COLUMN IF NOT EXISTS organization_id BIGINT NOT NULL DEFAULT 0;
UPDATE search_index_job SET organization_id = 0 WHERE organization_id IS NULL;
ALTER TABLE search_index_job ALTER COLUMN organization_id SET DEFAULT 0;
ALTER TABLE search_index_job ALTER COLUMN organization_id SET NOT NULL;

COMMIT;
