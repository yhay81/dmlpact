use std::path::{Path, PathBuf};

use postgres::fallible_iterator::FallibleIterator;
use postgres::IsolationLevel;

use crate::{
    db::{
        apply_local_timeouts, canonical_table, connect_database, database_identity,
        enforce_table_policy, sqlstate, table_evidence, table_schema_sha256,
    },
    error::{AppError, AppResult, ErrorClass},
    integrity::{sha256_bytes, sha256_json, unix_ms},
    model::{
        ApplyResult, InspectReport, Limits, LintReport, Plan, PlanPreconditions, ReceiptResult,
        ReceiptState, PLAN_SCHEMA_VERSION, TOOL_VERSION,
    },
    receipt::{read_plan, seal_plan, write_plan_new, ReceiptJournal},
    sql::{read_and_analyze, AnalyzedSql},
};

const MIN_POSTGRES_VERSION_NUM: i64 = 130_000;
const MAX_PLAN_LIFETIME_MS: u64 = 86_400_000;
const MAX_TARGET_EVIDENCE_BYTES: usize = 67_108_864;

#[derive(Debug, Clone)]
pub struct DatabaseOptions {
    pub environment_name: String,
    pub allow_insecure_localhost: bool,
}

#[derive(Debug, Clone)]
pub struct PlanOptions {
    pub sql_path: PathBuf,
    pub output_path: PathBuf,
    pub database: DatabaseOptions,
    pub limits: Limits,
    pub lifetime_ms: u64,
    pub allow_triggers: bool,
    pub allow_row_security: bool,
}

#[derive(Debug, Clone)]
pub struct ApplyOptions {
    pub sql_path: PathBuf,
    pub plan_path: PathBuf,
    pub receipt_path: PathBuf,
    pub database: DatabaseOptions,
}

pub fn lint_sql(path: &Path) -> AppResult<LintReport> {
    let analyzed = read_and_analyze(path)?;
    let mut policy_checks = vec![
        "exactly_one_statement".to_owned(),
        "single_named_table".to_owned(),
        "no_functions_subqueries_or_placeholders".to_owned(),
        "no_returning_joins_or_auxiliary_tables".to_owned(),
    ];
    if !matches!(analyzed.kind, crate::model::StatementKind::Insert) {
        policy_checks.push("explicit_where_clause".to_owned());
    } else {
        policy_checks.push("explicit_values_rows".to_owned());
    }
    Ok(LintReport {
        schema_version: "dmlpact.lint.v1".to_owned(),
        tool_version: TOOL_VERSION.to_owned(),
        statement_kind: analyzed.kind,
        requested_table: analyzed.requested_table,
        sql_file_sha256: analyzed.sql_file_sha256,
        normalized_sql_sha256: analyzed.normalized_sql_sha256,
        generated_target_count: analyzed.inserted_rows,
        count_query_sha256: analyzed.count_query_sha256,
        target_query_sha256: analyzed.target_query_sha256,
        policy_checks,
        executable_sql_emitted: false,
    })
}

pub fn inspect_table(requested_table: &str, options: &DatabaseOptions) -> AppResult<InspectReport> {
    let mut database =
        connect_database(&options.environment_name, options.allow_insecure_localhost)?;
    let identity = database_identity(&mut database.client, &database.endpoint_sha256)?;
    enforce_postgres_version(&identity)?;
    let evidence = table_evidence(&mut database.client, requested_table)?;
    let schema_hash = table_schema_sha256(&evidence)?;
    Ok(InspectReport {
        schema_version: "dmlpact.inspect.v1".to_owned(),
        tool_version: TOOL_VERSION.to_owned(),
        database: identity,
        table_schema_sha256: schema_hash,
        table: evidence,
        transport_policy: database.transport_policy,
    })
}

pub fn create_plan(options: &PlanOptions) -> AppResult<Plan> {
    validate_limits(&options.limits)?;
    if options.lifetime_ms == 0 || options.lifetime_ms > MAX_PLAN_LIFETIME_MS {
        return Err(AppError::new(
            ErrorClass::Usage,
            "plan_lifetime_invalid",
            "plan lifetime must be between 1 millisecond and 24 hours",
        ));
    }
    let analyzed = read_and_analyze(&options.sql_path)?;
    let mut database = connect_database(
        &options.database.environment_name,
        options.database.allow_insecure_localhost,
    )?;
    let mut transaction = database
        .client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(true)
        .start()
        .map_err(|_| database_operation_error("plan_transaction_failed"))?;
    apply_local_timeouts(&mut transaction, &options.limits)?;
    let identity = database_identity(&mut transaction, &database.endpoint_sha256)?;
    enforce_postgres_version(&identity)?;
    let evidence = table_evidence(&mut transaction, &analyzed.requested_table)?;
    enforce_table_policy(
        &evidence,
        analyzed.kind,
        options.allow_triggers,
        options.allow_row_security,
    )?;
    let schema_hash = table_schema_sha256(&evidence)?;
    let target_count = target_count(&mut transaction, &analyzed)?;
    if target_count > options.limits.max_rows {
        return Err(AppError::new(
            ErrorClass::Budget,
            "row_budget_exceeded",
            format!(
                "the planned target count {target_count} exceeds max_rows {}",
                options.limits.max_rows
            ),
        ));
    }
    let target_set_sha256 = target_set_sha256(&mut transaction, &analyzed, target_count)?;
    let explain_query = format!(
        "EXPLAIN (FORMAT JSON, COSTS TRUE, VERBOSE TRUE) {}",
        analyzed.normalized
    );
    let explain_row = transaction
        .query_one(&explain_query, &[])
        .map_err(|_| database_operation_error("explain_failed"))?;
    let explain: serde_json::Value = explain_row.get(0);
    let explain_sha256 = sha256_json(&explain)?;
    transaction
        .commit()
        .map_err(|_| database_operation_error("plan_transaction_commit_failed"))?;

    let created_at = unix_ms()?;
    let expires_at = created_at.checked_add(options.lifetime_ms).ok_or_else(|| {
        AppError::new(
            ErrorClass::Usage,
            "plan_expiry_overflow",
            "plan expiry overflow",
        )
    })?;
    let canonical = canonical_table(&evidence);
    let plan_id = sha256_bytes(
        format!(
            "{}:{}:{}:{}",
            analyzed.normalized_sql_sha256, schema_hash, target_count, created_at
        )
        .as_bytes(),
    );
    let mut plan = Plan {
        schema_version: PLAN_SCHEMA_VERSION.to_owned(),
        tool_version: TOOL_VERSION.to_owned(),
        plan_id,
        created_at_unix_ms: created_at,
        expires_at_unix_ms: expires_at,
        statement_kind: analyzed.kind,
        requested_table: analyzed.requested_table,
        sql_file_sha256: analyzed.sql_file_sha256,
        normalized_sql_sha256: analyzed.normalized_sql_sha256,
        count_query_sha256: analyzed.count_query_sha256,
        target_query_sha256: analyzed.target_query_sha256,
        explain_sha256,
        limits: options.limits.clone(),
        transport_policy: database.transport_policy,
        allow_triggers: options.allow_triggers,
        allow_row_security: options.allow_row_security,
        preconditions: PlanPreconditions {
            database: identity,
            canonical_table: canonical,
            table_schema_sha256: schema_hash,
            target_count,
            target_set_sha256,
            relation_kind: evidence.relation_kind.clone(),
            user_trigger_count: evidence.user_triggers.len(),
            rewrite_rule_count: evidence.rewrite_rules.len(),
            row_security: evidence.row_security || evidence.force_row_security,
        },
        plan_sha256: None,
    };
    seal_plan(&mut plan)?;
    write_plan_new(&options.output_path, &plan)?;
    Ok(plan)
}

pub fn apply_plan(options: &ApplyOptions) -> AppResult<ApplyResult> {
    let plan = read_plan(&options.plan_path)?;
    validate_limits(&plan.limits)?;
    let analyzed = read_and_analyze(&options.sql_path)?;
    validate_plan_binding(&plan, &analyzed)?;
    if unix_ms()? > plan.expires_at_unix_ms {
        return Err(AppError::new(
            ErrorClass::Contract,
            "plan_expired",
            "the plan has expired; create a fresh plan",
        ));
    }

    let journal = ReceiptJournal::create(&options.receipt_path, &plan, &analyzed.sql_file_sha256)?;
    apply_after_receipt(journal, &plan, &analyzed, &options.database)
}

fn apply_after_receipt(
    journal: ReceiptJournal,
    plan: &Plan,
    analyzed: &AnalyzedSql,
    database_options: &DatabaseOptions,
) -> AppResult<ApplyResult> {
    let requested_transport = if database_options.allow_insecure_localhost {
        crate::model::TransportPolicy::InsecureLocalhost
    } else {
        crate::model::TransportPolicy::TlsRequired
    };
    if requested_transport != plan.transport_policy {
        return finish_refused(journal, "transport_policy_drift", None, None);
    }
    let mut database = match connect_database(
        &database_options.environment_name,
        database_options.allow_insecure_localhost,
    ) {
        Ok(database) => database,
        Err(error) => {
            return finish(
                journal,
                ReceiptState::Refused,
                ReceiptResult {
                    affected_rows: None,
                    reason_code: error.code,
                    sqlstate: None,
                    database: None,
                    table_schema_sha256: None,
                },
            );
        }
    };
    if database.transport_policy != plan.transport_policy {
        return finish_refused(journal, "transport_policy_drift", None, None);
    }

    let mut transaction = match database
        .client
        .build_transaction()
        .isolation_level(IsolationLevel::RepeatableRead)
        .read_only(false)
        .start()
    {
        Ok(transaction) => transaction,
        Err(_) => return finish_refused(journal, "apply_transaction_failed", None, None),
    };
    if apply_local_timeouts(&mut transaction, &plan.limits).is_err() {
        return finish_refused(journal, "timeout_configuration_failed", None, None);
    }
    let lock_query = format!(
        "LOCK TABLE {} IN SHARE ROW EXCLUSIVE MODE",
        plan.preconditions.canonical_table
    );
    if transaction.batch_execute(&lock_query).is_err() {
        return finish_refused(journal, "table_lock_failed", None, None);
    }

    let identity = match database_identity(&mut transaction, &database.endpoint_sha256) {
        Ok(identity) => identity,
        Err(_) => return finish_refused(journal, "identity_query_failed", None, None),
    };
    if identity != plan.preconditions.database {
        return finish_refused(journal, "database_identity_drift", Some(identity), None);
    }
    let evidence = match table_evidence(&mut transaction, &analyzed.requested_table) {
        Ok(evidence) => evidence,
        Err(error) => {
            return finish_refused(journal, &error.code, Some(identity), None);
        }
    };
    let schema_hash = match table_schema_sha256(&evidence) {
        Ok(hash) => hash,
        Err(_) => {
            return finish_refused(journal, "table_schema_hash_failed", Some(identity), None);
        }
    };
    if canonical_table(&evidence) != plan.preconditions.canonical_table
        || schema_hash != plan.preconditions.table_schema_sha256
    {
        return finish_refused(
            journal,
            "table_schema_drift",
            Some(identity),
            Some(schema_hash),
        );
    }
    if let Err(error) = enforce_table_policy(
        &evidence,
        analyzed.kind,
        plan.allow_triggers,
        plan.allow_row_security,
    ) {
        return finish_refused(journal, &error.code, Some(identity), Some(schema_hash));
    }
    let count = match target_count(&mut transaction, analyzed) {
        Ok(count) => count,
        Err(_) => {
            return finish_refused(
                journal,
                "target_count_failed",
                Some(identity),
                Some(schema_hash),
            );
        }
    };
    if count != plan.preconditions.target_count {
        return finish_refused(
            journal,
            "target_count_drift",
            Some(identity),
            Some(schema_hash),
        );
    }
    if count > plan.limits.max_rows {
        return finish_refused(
            journal,
            "row_budget_exceeded",
            Some(identity),
            Some(schema_hash),
        );
    }
    let target_set_hash = match target_set_sha256(&mut transaction, analyzed, count) {
        Ok(hash) => hash,
        Err(error) => {
            return finish_refused(journal, &error.code, Some(identity), Some(schema_hash));
        }
    };
    if target_set_hash != plan.preconditions.target_set_sha256 {
        return finish_refused(
            journal,
            "target_set_drift",
            Some(identity),
            Some(schema_hash),
        );
    }

    let affected = match transaction.execute(&analyzed.normalized, &[]) {
        Ok(affected) => affected,
        Err(error) => {
            let state_code = sqlstate(&error);
            transaction.rollback().ok();
            return finish(
                journal,
                ReceiptState::RolledBack,
                ReceiptResult {
                    affected_rows: None,
                    reason_code: "statement_execution_failed".to_owned(),
                    sqlstate: state_code,
                    database: Some(identity),
                    table_schema_sha256: Some(schema_hash),
                },
            );
        }
    };
    if affected != count {
        transaction.rollback().ok();
        return finish(
            journal,
            ReceiptState::RolledBack,
            ReceiptResult {
                affected_rows: Some(affected),
                reason_code: "affected_rows_mismatch".to_owned(),
                sqlstate: None,
                database: Some(identity),
                table_schema_sha256: Some(schema_hash),
            },
        );
    }
    if transaction.commit().is_err() {
        return finish(
            journal,
            ReceiptState::Uncertain,
            ReceiptResult {
                affected_rows: Some(affected),
                reason_code: "commit_outcome_uncertain".to_owned(),
                sqlstate: None,
                database: Some(identity),
                table_schema_sha256: Some(schema_hash),
            },
        );
    }
    finish(
        journal,
        ReceiptState::Committed,
        ReceiptResult {
            affected_rows: Some(affected),
            reason_code: "applied".to_owned(),
            sqlstate: None,
            database: Some(identity),
            table_schema_sha256: Some(schema_hash),
        },
    )
}

fn finish_refused(
    journal: ReceiptJournal,
    reason: &str,
    database: Option<crate::model::DatabaseIdentity>,
    table_schema_sha256: Option<String>,
) -> AppResult<ApplyResult> {
    finish(
        journal,
        ReceiptState::Refused,
        ReceiptResult {
            affected_rows: None,
            reason_code: reason.to_owned(),
            sqlstate: None,
            database,
            table_schema_sha256,
        },
    )
}

fn finish(
    journal: ReceiptJournal,
    state: ReceiptState,
    result: ReceiptResult,
) -> AppResult<ApplyResult> {
    let path = journal.path().to_string_lossy().into_owned();
    let receipt_id = journal.receipt_id().to_owned();
    let reason_code = result.reason_code.clone();
    let affected_rows = result.affected_rows;
    let event = journal.finalize(state, result)?;
    Ok(ApplyResult {
        schema_version: "dmlpact.apply-result.v1".to_owned(),
        receipt_id,
        state,
        plan_sha256: event.plan_sha256,
        affected_rows,
        reason_code,
        receipt_path: path,
    })
}

fn target_count(
    client: &mut impl postgres::GenericClient,
    analyzed: &AnalyzedSql,
) -> AppResult<u64> {
    if let Some(query) = &analyzed.count_sql {
        let count: i64 = client
            .query_one(query, &[])
            .map_err(|_| database_operation_error("target_count_failed"))?
            .get(0);
        u64::try_from(count).map_err(|_| {
            AppError::new(
                ErrorClass::Contract,
                "target_count_invalid",
                "PostgreSQL returned an invalid target count",
            )
        })
    } else {
        Ok(analyzed.inserted_rows)
    }
}

fn target_set_sha256(
    client: &mut impl postgres::GenericClient,
    analyzed: &AnalyzedSql,
    expected_count: u64,
) -> AppResult<String> {
    let Some(query) = &analyzed.target_rows_sql else {
        return Ok(sha256_bytes(analyzed.normalized.as_bytes()));
    };
    let mut rows = client
        .query_raw(query, std::iter::empty::<&str>())
        .map_err(|_| database_operation_error("target_evidence_query_failed"))?;
    let mut evidence = Vec::new();
    let mut observed_count = 0_u64;
    while let Some(row) = rows
        .next()
        .map_err(|_| database_operation_error("target_evidence_query_failed"))?
    {
        let value: String = row.get(0);
        let length = u64::try_from(value.len()).map_err(|_| {
            AppError::new(
                ErrorClass::Budget,
                "target_evidence_bytes_exceeded",
                "target evidence exceeded the supported size",
            )
        })?;
        let required = evidence
            .len()
            .saturating_add(std::mem::size_of::<u64>())
            .saturating_add(value.len());
        if required > MAX_TARGET_EVIDENCE_BYTES {
            return Err(AppError::new(
                ErrorClass::Budget,
                "target_evidence_bytes_exceeded",
                "target evidence exceeds the fixed 64 MiB safety limit",
            ));
        }
        evidence.extend_from_slice(&length.to_be_bytes());
        evidence.extend_from_slice(value.as_bytes());
        observed_count = observed_count.saturating_add(1);
    }
    if observed_count != expected_count {
        return Err(AppError::new(
            ErrorClass::Contract,
            "target_evidence_count_mismatch",
            "target evidence count does not match the pre-count in the same transaction",
        ));
    }
    Ok(sha256_bytes(&evidence))
}

fn validate_plan_binding(plan: &Plan, analyzed: &AnalyzedSql) -> AppResult<()> {
    if plan.statement_kind != analyzed.kind
        || plan.requested_table != analyzed.requested_table
        || plan.sql_file_sha256 != analyzed.sql_file_sha256
        || plan.normalized_sql_sha256 != analyzed.normalized_sql_sha256
        || plan.count_query_sha256 != analyzed.count_query_sha256
        || plan.target_query_sha256 != analyzed.target_query_sha256
    {
        return Err(AppError::new(
            ErrorClass::Contract,
            "plan_sql_mismatch",
            "the SQL file does not exactly match the approved plan",
        ));
    }
    if plan.preconditions.target_count > plan.limits.max_rows {
        return Err(AppError::new(
            ErrorClass::Contract,
            "plan_budget_invalid",
            "the plan target count exceeds its own row budget",
        ));
    }
    Ok(())
}

fn validate_limits(limits: &Limits) -> AppResult<()> {
    if limits.max_rows == 0 || limits.max_rows > 1_000_000 {
        return Err(AppError::new(
            ErrorClass::Usage,
            "max_rows_invalid",
            "max_rows must be between 1 and 1,000,000",
        ));
    }
    if !(100..=600_000).contains(&limits.statement_timeout_ms)
        || !(1..=60_000).contains(&limits.lock_timeout_ms)
        || limits.lock_timeout_ms > limits.statement_timeout_ms
    {
        return Err(AppError::new(
            ErrorClass::Usage,
            "timeout_invalid",
            "statement timeout must be 100ms..10m and lock timeout 1ms..1m, not exceeding it",
        ));
    }
    Ok(())
}

fn enforce_postgres_version(identity: &crate::model::DatabaseIdentity) -> AppResult<()> {
    if identity.server_version_num < MIN_POSTGRES_VERSION_NUM {
        return Err(AppError::new(
            ErrorClass::Policy,
            "postgres_version_unsupported",
            "PostgreSQL 13 or newer is required",
        ));
    }
    Ok(())
}

fn database_operation_error(code: &'static str) -> AppError {
    AppError::new(
        ErrorClass::Io,
        code,
        "the PostgreSQL operation failed; connection details were not emitted",
    )
}
