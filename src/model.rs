use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const PLAN_SCHEMA_VERSION: &str = "dmlpact.plan.v1";
pub const RECEIPT_SCHEMA_VERSION: &str = "dmlpact.receipt-event.v1";
pub const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StatementKind {
    Insert,
    Update,
    Delete,
}

impl StatementKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Insert => "insert",
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TransportPolicy {
    TlsRequired,
    InsecureLocalhost,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub max_rows: u64,
    pub statement_timeout_ms: u64,
    pub lock_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DatabaseIdentity {
    pub database: String,
    pub role: String,
    pub search_path: String,
    pub server_version_num: i64,
    pub endpoint_sha256: String,
    pub server_address: Option<String>,
    pub server_port: Option<i32>,
    pub settings: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct ColumnEvidence {
    pub position: i16,
    pub name: String,
    pub data_type: String,
    pub not_null: bool,
    pub has_default: bool,
    pub default_expression: Option<String>,
    pub identity: String,
    pub generated: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct NamedDefinition {
    pub name: String,
    pub definition: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct TableEvidence {
    pub schema: String,
    pub table: String,
    pub oid: i64,
    pub relation_kind: String,
    pub persistence: String,
    pub row_security: bool,
    pub force_row_security: bool,
    pub columns: Vec<ColumnEvidence>,
    pub constraints: Vec<NamedDefinition>,
    pub indexes: Vec<NamedDefinition>,
    pub user_triggers: Vec<NamedDefinition>,
    pub rewrite_rules: Vec<NamedDefinition>,
    pub policies: Vec<NamedDefinition>,
    pub inheritance_relations: Vec<String>,
    pub referential_actions: Vec<NamedDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanPreconditions {
    pub database: DatabaseIdentity,
    pub canonical_table: String,
    pub table_schema_sha256: String,
    pub target_count: u64,
    pub target_set_sha256: String,
    pub relation_kind: String,
    pub user_trigger_count: usize,
    pub rewrite_rule_count: usize,
    pub row_security: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema_version: String,
    pub tool_version: String,
    pub plan_id: String,
    pub created_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub statement_kind: StatementKind,
    pub requested_table: String,
    pub sql_file_sha256: String,
    pub normalized_sql_sha256: String,
    pub count_query_sha256: Option<String>,
    pub target_query_sha256: Option<String>,
    pub explain_sha256: String,
    pub limits: Limits,
    pub transport_policy: TransportPolicy,
    pub allow_triggers: bool,
    pub allow_row_security: bool,
    pub preconditions: PlanPreconditions,
    pub plan_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct LintReport {
    pub schema_version: String,
    pub tool_version: String,
    pub statement_kind: StatementKind,
    pub requested_table: String,
    pub sql_file_sha256: String,
    pub normalized_sql_sha256: String,
    pub generated_target_count: u64,
    pub count_query_sha256: Option<String>,
    pub target_query_sha256: Option<String>,
    pub policy_checks: Vec<String>,
    pub executable_sql_emitted: bool,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct InspectReport {
    pub schema_version: String,
    pub tool_version: String,
    pub database: DatabaseIdentity,
    pub table_schema_sha256: String,
    pub table: TableEvidence,
    pub transport_policy: TransportPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptState {
    Prepared,
    Committed,
    RolledBack,
    Refused,
    Uncertain,
}

impl ReceiptState {
    pub fn is_final(self) -> bool {
        !matches!(self, Self::Prepared)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReceiptResult {
    pub affected_rows: Option<u64>,
    pub reason_code: String,
    pub sqlstate: Option<String>,
    pub database: Option<DatabaseIdentity>,
    pub table_schema_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReceiptEvent {
    pub schema_version: String,
    pub tool_version: String,
    pub receipt_id: String,
    pub sequence: u32,
    pub timestamp_unix_ms: u64,
    pub plan_sha256: String,
    pub sql_file_sha256: String,
    pub previous_event_sha256: Option<String>,
    pub state: ReceiptState,
    pub result: Option<ReceiptResult>,
    pub event_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ApplyResult {
    pub schema_version: String,
    pub receipt_id: String,
    pub state: ReceiptState,
    pub plan_sha256: String,
    pub affected_rows: Option<u64>,
    pub reason_code: String,
    pub receipt_path: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReceiptVerification {
    pub schema_version: String,
    pub receipt_id: String,
    pub integrity_valid: bool,
    pub complete: bool,
    pub event_count: usize,
    pub final_state: ReceiptState,
    pub plan_sha256: String,
    pub final_event_sha256: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Capabilities {
    pub schema_version: String,
    pub tool_version: String,
    pub database: String,
    pub statement_kinds: Vec<String>,
    pub transport_policies: Vec<String>,
    pub safety_defaults: BTreeMap<String, String>,
    pub limitations: Vec<String>,
    pub supported_platforms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ContractBrief {
    pub schema_version: String,
    pub tool_version: String,
    pub commands: Vec<String>,
    pub output_formats: Vec<String>,
    pub data_schemas: BTreeMap<String, String>,
    pub exit_codes: BTreeMap<String, String>,
    pub safety_defaults: BTreeMap<String, String>,
}
