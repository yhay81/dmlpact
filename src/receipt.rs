use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use crate::{
    error::{AppError, AppResult, ErrorClass},
    integrity::{sha256_bytes, sha256_json, unix_ms},
    model::{
        Plan, ReceiptEvent, ReceiptResult, ReceiptState, ReceiptVerification, PLAN_SCHEMA_VERSION,
        RECEIPT_SCHEMA_VERSION, TOOL_VERSION,
    },
};

const MAX_PLAN_BYTES: u64 = 1_048_576;
const MAX_RECEIPT_BYTES: u64 = 2_097_152;
const PREVIOUSLY_SUPPORTED_ARTIFACT_TOOL_VERSIONS: &[&str] = &["0.1.0", "0.2.0"];

pub struct ReceiptJournal {
    file: File,
    path: PathBuf,
    prepared: ReceiptEvent,
}

impl ReceiptJournal {
    pub fn create(path: &Path, plan: &Plan, sql_file_sha256: &str) -> AppResult<Self> {
        let plan_sha256 = verified_plan_sha256(plan)?;
        let timestamp = unix_ms()?;
        let receipt_id =
            sha256_bytes(format!("{plan_sha256}:{sql_file_sha256}:{timestamp}").as_bytes());
        let mut prepared = ReceiptEvent {
            schema_version: RECEIPT_SCHEMA_VERSION.to_owned(),
            tool_version: TOOL_VERSION.to_owned(),
            receipt_id,
            sequence: 1,
            timestamp_unix_ms: timestamp,
            plan_sha256,
            sql_file_sha256: sql_file_sha256.to_owned(),
            previous_event_sha256: None,
            state: ReceiptState::Prepared,
            result: None,
            event_sha256: None,
        };
        prepared.event_sha256 = Some(event_sha256(&prepared)?);

        let mut file = OpenOptions::new()
            .append(true)
            .create_new(true)
            .open(path)
            .map_err(|_| {
                AppError::new(
                    ErrorClass::Io,
                    "receipt_create_failed",
                    "the receipt path must be new, writable, and not a symlink",
                )
            })?;
        write_event(&mut file, &prepared)?;

        Ok(Self {
            file,
            path: path.to_path_buf(),
            prepared,
        })
    }

    pub fn finalize(
        mut self,
        state: ReceiptState,
        result: ReceiptResult,
    ) -> AppResult<ReceiptEvent> {
        if !state.is_final() {
            return Err(AppError::new(
                ErrorClass::Contract,
                "receipt_final_state_invalid",
                "a receipt must be finalized with a terminal state",
            ));
        }
        let mut event = ReceiptEvent {
            schema_version: RECEIPT_SCHEMA_VERSION.to_owned(),
            tool_version: TOOL_VERSION.to_owned(),
            receipt_id: self.prepared.receipt_id.clone(),
            sequence: 2,
            timestamp_unix_ms: unix_ms()?,
            plan_sha256: self.prepared.plan_sha256.clone(),
            sql_file_sha256: self.prepared.sql_file_sha256.clone(),
            previous_event_sha256: self.prepared.event_sha256.clone(),
            state,
            result: Some(result),
            event_sha256: None,
        };
        event.event_sha256 = Some(event_sha256(&event)?);
        write_event(&mut self.file, &event)?;
        Ok(event)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn receipt_id(&self) -> &str {
        &self.prepared.receipt_id
    }
}

pub fn read_plan(path: &Path) -> AppResult<Plan> {
    let metadata = fs::metadata(path).map_err(|_| {
        AppError::new(
            ErrorClass::Io,
            "plan_unreadable",
            "the plan file could not be read",
        )
    })?;
    if !metadata.is_file() || metadata.len() > MAX_PLAN_BYTES {
        return Err(AppError::new(
            ErrorClass::Contract,
            "plan_file_invalid",
            "the plan must be a regular JSON file no larger than 1 MiB",
        ));
    }
    let bytes = fs::read(path).map_err(|_| {
        AppError::new(
            ErrorClass::Io,
            "plan_unreadable",
            "the plan file could not be read",
        )
    })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_PLAN_BYTES {
        return Err(AppError::new(
            ErrorClass::Contract,
            "plan_file_invalid",
            "the plan must be a regular JSON file no larger than 1 MiB",
        ));
    }
    let plan: Plan = serde_json::from_slice(&bytes).map_err(|_| {
        AppError::new(
            ErrorClass::Contract,
            "plan_json_invalid",
            "the plan is not a valid strict dmlpact plan document",
        )
    })?;
    verified_plan_sha256(&plan)?;
    Ok(plan)
}

pub fn write_plan_new(path: &Path, plan: &Plan) -> AppResult<()> {
    verified_plan_sha256(plan)?;
    let mut bytes = serde_json::to_vec_pretty(plan).map_err(|_| {
        AppError::new(
            ErrorClass::Contract,
            "plan_serialization_failed",
            "the plan could not be serialized",
        )
    })?;
    bytes.push(b'\n');
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| {
            AppError::new(
                ErrorClass::Io,
                "plan_create_failed",
                "the plan output path must be new, writable, and not a symlink",
            )
        })?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| {
            AppError::new(
                ErrorClass::Io,
                "plan_write_failed",
                "the plan could not be durably written",
            )
        })
}

pub fn seal_plan(plan: &mut Plan) -> AppResult<()> {
    plan.plan_sha256 = None;
    plan.plan_sha256 = Some(sha256_json(plan)?);
    Ok(())
}

pub fn verified_plan_sha256(plan: &Plan) -> AppResult<String> {
    if plan.schema_version != PLAN_SCHEMA_VERSION
        || !is_supported_artifact_tool_version(&plan.tool_version)
    {
        return Err(AppError::new(
            ErrorClass::Contract,
            "plan_version_unsupported",
            "the plan schema or tool version is not supported",
        ));
    }
    let claimed = plan.plan_sha256.as_ref().ok_or_else(|| {
        AppError::new(
            ErrorClass::Contract,
            "plan_hash_missing",
            "the plan does not contain its integrity hash",
        )
    })?;
    let mut unsigned = plan.clone();
    unsigned.plan_sha256 = None;
    let actual = sha256_json(&unsigned)?;
    if claimed != &actual {
        return Err(AppError::new(
            ErrorClass::Contract,
            "plan_hash_mismatch",
            "the plan integrity hash does not match its contents",
        ));
    }
    Ok(actual)
}

pub fn verify_receipt(path: &Path) -> AppResult<ReceiptVerification> {
    let metadata = fs::metadata(path).map_err(|_| {
        AppError::new(
            ErrorClass::Io,
            "receipt_unreadable",
            "the receipt file could not be read",
        )
    })?;
    if !metadata.is_file() || metadata.len() > MAX_RECEIPT_BYTES {
        return Err(AppError::new(
            ErrorClass::Contract,
            "receipt_file_invalid",
            "the receipt must be a regular NDJSON file no larger than 2 MiB",
        ));
    }
    let mut content = String::new();
    File::open(path)
        .and_then(|mut file| file.read_to_string(&mut content))
        .map_err(|_| {
            AppError::new(
                ErrorClass::Contract,
                "receipt_not_utf8",
                "the receipt must be valid UTF-8",
            )
        })?;
    if u64::try_from(content.len()).unwrap_or(u64::MAX) > MAX_RECEIPT_BYTES {
        return Err(AppError::new(
            ErrorClass::Contract,
            "receipt_file_invalid",
            "the receipt must be a regular NDJSON file no larger than 2 MiB",
        ));
    }
    let lines: Vec<&str> = content.lines().collect();
    if !(1..=2).contains(&lines.len()) || lines.iter().any(|line| line.trim().is_empty()) {
        return Err(contract(
            "receipt_event_count_invalid",
            "a receipt must contain one prepared event and at most one final event",
        ));
    }
    let events: Vec<ReceiptEvent> = lines
        .iter()
        .map(|line| {
            serde_json::from_str(line).map_err(|_| {
                contract(
                    "receipt_event_invalid",
                    "a receipt event is not valid strict dmlpact NDJSON",
                )
            })
        })
        .collect::<AppResult<_>>()?;
    validate_event(&events[0])?;
    let prepared = &events[0];
    if prepared.sequence != 1
        || prepared.state != ReceiptState::Prepared
        || prepared.previous_event_sha256.is_some()
        || prepared.result.is_some()
    {
        return Err(contract(
            "receipt_prepared_invalid",
            "the first receipt event is not a valid prepared event",
        ));
    }

    let final_event = if events.len() == 2 {
        let event = &events[1];
        validate_event(event)?;
        if event.sequence != 2
            || !event.state.is_final()
            || event.result.is_none()
            || event.receipt_id != prepared.receipt_id
            || event.plan_sha256 != prepared.plan_sha256
            || event.sql_file_sha256 != prepared.sql_file_sha256
            || event.previous_event_sha256 != prepared.event_sha256
            || event.timestamp_unix_ms < prepared.timestamp_unix_ms
        {
            return Err(contract(
                "receipt_chain_invalid",
                "the final receipt event does not continue the prepared event",
            ));
        }
        event
    } else {
        prepared
    };
    let final_hash = final_event.event_sha256.clone().ok_or_else(|| {
        contract(
            "receipt_event_hash_missing",
            "a receipt event is missing its integrity hash",
        )
    })?;
    Ok(ReceiptVerification {
        schema_version: "dmlpact.receipt-verification.v1".to_owned(),
        receipt_id: prepared.receipt_id.clone(),
        integrity_valid: true,
        complete: events.len() == 2,
        event_count: events.len(),
        final_state: final_event.state,
        plan_sha256: prepared.plan_sha256.clone(),
        final_event_sha256: final_hash,
    })
}

fn validate_event(event: &ReceiptEvent) -> AppResult<()> {
    if event.schema_version != RECEIPT_SCHEMA_VERSION
        || !is_supported_artifact_tool_version(&event.tool_version)
    {
        return Err(contract(
            "receipt_version_unsupported",
            "the receipt schema or tool version is not supported",
        ));
    }
    let claimed = event.event_sha256.as_ref().ok_or_else(|| {
        contract(
            "receipt_event_hash_missing",
            "a receipt event is missing its integrity hash",
        )
    })?;
    let actual = event_sha256(event)?;
    if claimed != &actual {
        return Err(contract(
            "receipt_event_hash_mismatch",
            "a receipt event integrity hash does not match its contents",
        ));
    }
    Ok(())
}

fn is_supported_artifact_tool_version(tool_version: &str) -> bool {
    tool_version == TOOL_VERSION
        || PREVIOUSLY_SUPPORTED_ARTIFACT_TOOL_VERSIONS.contains(&tool_version)
}

fn event_sha256(event: &ReceiptEvent) -> AppResult<String> {
    let mut unsigned = event.clone();
    unsigned.event_sha256 = None;
    sha256_json(&unsigned)
}

fn write_event(file: &mut File, event: &ReceiptEvent) -> AppResult<()> {
    let mut bytes = serde_json::to_vec(event).map_err(|_| {
        contract(
            "receipt_serialization_failed",
            "a receipt event could not be serialized",
        )
    })?;
    bytes.push(b'\n');
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| {
            AppError::new(
                ErrorClass::Io,
                "receipt_write_failed",
                "the receipt event could not be durably written",
            )
        })
}

fn contract(code: impl Into<String>, message: impl Into<String>) -> AppError {
    AppError::new(ErrorClass::Contract, code, message)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::{
        model::{
            DatabaseIdentity, Limits, Plan, PlanPreconditions, StatementKind, TransportPolicy,
            PLAN_SCHEMA_VERSION, TOOL_VERSION,
        },
        receipt::{
            seal_plan, verify_receipt, write_plan_new, ReceiptJournal, ReceiptResult, ReceiptState,
        },
    };

    fn test_plan() -> Plan {
        let mut plan = Plan {
            schema_version: PLAN_SCHEMA_VERSION.to_owned(),
            tool_version: TOOL_VERSION.to_owned(),
            plan_id: "plan-1".to_owned(),
            created_at_unix_ms: 1,
            expires_at_unix_ms: 2,
            statement_kind: StatementKind::Delete,
            requested_table: "public.items".to_owned(),
            sql_file_sha256: "a".repeat(64),
            normalized_sql_sha256: "b".repeat(64),
            count_query_sha256: Some("c".repeat(64)),
            target_query_sha256: Some("f".repeat(64)),
            explain_sha256: "d".repeat(64),
            limits: Limits {
                max_rows: 1,
                statement_timeout_ms: 1_000,
                lock_timeout_ms: 100,
            },
            transport_policy: TransportPolicy::InsecureLocalhost,
            allow_triggers: false,
            allow_row_security: false,
            preconditions: PlanPreconditions {
                database: DatabaseIdentity {
                    database: "test".to_owned(),
                    role: "test".to_owned(),
                    search_path: "public".to_owned(),
                    server_version_num: 150_000,
                    endpoint_sha256: "1".repeat(64),
                    server_address: Some("127.0.0.1".to_owned()),
                    server_port: Some(5432),
                    settings: std::collections::BTreeMap::new(),
                },
                canonical_table: "\"public\".\"items\"".to_owned(),
                table_schema_sha256: "e".repeat(64),
                target_count: 1,
                target_set_sha256: "0".repeat(64),
                relation_kind: "r".to_owned(),
                user_trigger_count: 0,
                rewrite_rule_count: 0,
                row_security: false,
            },
            plan_sha256: None,
        };
        seal_plan(&mut plan).expect("seal plan");
        plan
    }

    #[test]
    fn plan_round_trip_and_hash_tamper_detection() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("plan.json");
        let plan = test_plan();
        write_plan_new(&path, &plan).expect("write plan");
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).expect("read plan")).expect("parse plan");
        value["limits"]["max_rows"] = serde_json::json!(2);
        fs::write(&path, serde_json::to_vec(&value).expect("serialize")).expect("tamper");
        assert_eq!(
            super::read_plan(&path)
                .expect_err("tamper should fail")
                .code,
            "plan_hash_mismatch"
        );
    }

    #[test]
    fn plan_rejects_unknown_nested_fields() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("plan.json");
        let plan = test_plan();
        let mut value = serde_json::to_value(&plan).expect("serialize plan");
        value["limits"]["unexpected"] = serde_json::json!(true);
        fs::write(&path, serde_json::to_vec(&value).expect("serialize")).expect("write");
        assert_eq!(
            super::read_plan(&path)
                .expect_err("unknown nested fields should fail")
                .code,
            "plan_json_invalid"
        );
    }

    #[test]
    fn receipt_round_trip_and_chain_verification() {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("receipt.ndjson");
        let plan = test_plan();
        let journal =
            ReceiptJournal::create(&path, &plan, &plan.sql_file_sha256).expect("create receipt");
        journal
            .finalize(
                ReceiptState::Committed,
                ReceiptResult {
                    affected_rows: Some(1),
                    reason_code: "applied".to_owned(),
                    sqlstate: None,
                    database: Some(plan.preconditions.database.clone()),
                    table_schema_sha256: Some(plan.preconditions.table_schema_sha256.clone()),
                },
            )
            .expect("finalize");
        let verification = verify_receipt(&path).expect("verify receipt");
        assert!(verification.complete);
        assert_eq!(verification.final_state, ReceiptState::Committed);
    }
}
