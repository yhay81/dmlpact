use std::{env, str::FromStr, time::Duration};

use postgres::{
    config::{Host, SslMode},
    Client, Config, GenericClient, NoTls,
};
use tokio_postgres_rustls::MakeRustlsConnect;

use crate::{
    error::{AppError, AppResult, ErrorClass},
    integrity::{sha256_bytes, sha256_json},
    model::{
        ColumnEvidence, DatabaseIdentity, Limits, NamedDefinition, StatementKind, TableEvidence,
        TransportPolicy,
    },
};

const MAX_CONNECTION_VALUE_BYTES: usize = 16_384;

pub struct ConnectedDatabase {
    pub client: Client,
    pub transport_policy: TransportPolicy,
    pub endpoint_sha256: String,
}

pub fn connect_database(
    environment_name: &str,
    allow_insecure_localhost: bool,
) -> AppResult<ConnectedDatabase> {
    validate_environment_name(environment_name)?;
    let connection_value = env::var(environment_name).map_err(|_| {
        AppError::new(
            ErrorClass::Usage,
            "database_env_missing",
            format!("database connection environment variable {environment_name} is not set"),
        )
    })?;
    if connection_value.len() > MAX_CONNECTION_VALUE_BYTES {
        return Err(AppError::new(
            ErrorClass::Usage,
            "database_env_too_large",
            "the database connection value exceeds the 16 KiB contract limit",
        ));
    }
    let mut config = Config::from_str(&connection_value).map_err(|_| {
        AppError::new(
            ErrorClass::Usage,
            "database_config_invalid",
            "the database connection value is invalid",
        )
    })?;
    config
        .application_name("dmlpact/0.1")
        .connect_timeout(Duration::from_secs(10));
    let endpoint_sha256 = endpoint_sha256(&config);

    if allow_insecure_localhost {
        if !all_hosts_are_local(&config) {
            return Err(AppError::new(
                ErrorClass::Policy,
                "insecure_remote_denied",
                "--allow-insecure-localhost is valid only for loopback or Unix-socket connections",
            ));
        }
        config.ssl_mode(SslMode::Disable);
        let client = config
            .connect(NoTls)
            .map_err(|_| database_error("connect_failed"))?;
        Ok(ConnectedDatabase {
            client,
            transport_policy: TransportPolicy::InsecureLocalhost,
            endpoint_sha256,
        })
    } else {
        config.ssl_mode(SslMode::Require);
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
        let (connector, _certificate_warnings) =
            MakeRustlsConnect::with_native_certs().map_err(|_| {
                AppError::new(
                    ErrorClass::Io,
                    "native_certificates_unavailable",
                    "no usable native TLS root certificates were found",
                )
            })?;
        let client = config
            .connect(connector)
            .map_err(|_| database_error("tls_connect_failed"))?;
        Ok(ConnectedDatabase {
            client,
            transport_policy: TransportPolicy::TlsRequired,
            endpoint_sha256,
        })
    }
}

pub fn database_identity(
    client: &mut impl GenericClient,
    endpoint_sha256: &str,
) -> AppResult<DatabaseIdentity> {
    let row = client
        .query_one(
            "SELECT current_database(), current_user, current_setting('search_path'), \
                    current_setting('server_version_num')::bigint, \
                    pg_catalog.inet_server_addr()::text, \
                    pg_catalog.inet_server_port(), \
                    current_setting('TimeZone'), current_setting('DateStyle'), \
                    current_setting('IntervalStyle'), \
                    current_setting('standard_conforming_strings'), \
                    current_setting('client_encoding'), current_setting('lc_numeric'), \
                    current_setting('lc_monetary'), current_setting('lc_time')",
            &[],
        )
        .map_err(|_| database_error("identity_query_failed"))?;
    Ok(DatabaseIdentity {
        database: row.get(0),
        role: row.get(1),
        search_path: row.get(2),
        server_version_num: row.get(3),
        endpoint_sha256: endpoint_sha256.to_owned(),
        server_address: row.get(4),
        server_port: row.get(5),
        settings: std::collections::BTreeMap::from([
            ("TimeZone".to_owned(), row.get(6)),
            ("DateStyle".to_owned(), row.get(7)),
            ("IntervalStyle".to_owned(), row.get(8)),
            ("standard_conforming_strings".to_owned(), row.get(9)),
            ("client_encoding".to_owned(), row.get(10)),
            ("lc_numeric".to_owned(), row.get(11)),
            ("lc_monetary".to_owned(), row.get(12)),
            ("lc_time".to_owned(), row.get(13)),
        ]),
    })
}

pub fn table_evidence(
    client: &mut impl GenericClient,
    requested_table: &str,
) -> AppResult<TableEvidence> {
    let row = client
        .query_opt(
            "SELECT c.oid::bigint, n.nspname, c.relname, c.relkind::text, \
                    c.relpersistence::text, c.relrowsecurity, c.relforcerowsecurity \
             FROM pg_catalog.pg_class AS c \
             JOIN pg_catalog.pg_namespace AS n ON n.oid = c.relnamespace \
             WHERE c.oid = pg_catalog.to_regclass($1)::oid",
            &[&requested_table],
        )
        .map_err(|_| database_error("table_lookup_failed"))?
        .ok_or_else(|| {
            AppError::new(
                ErrorClass::Policy,
                "table_not_found",
                "the requested table does not resolve in the current database and search path",
            )
        })?;

    let oid: i64 = row.get(0);
    let columns = client
        .query(
            "SELECT a.attnum::smallint, a.attname, \
                    pg_catalog.format_type(a.atttypid, a.atttypmod), \
                    a.attnotnull, a.atthasdef, \
                    pg_catalog.pg_get_expr(d.adbin, d.adrelid), \
                    a.attidentity::text, a.attgenerated::text \
             FROM pg_catalog.pg_attribute AS a \
             LEFT JOIN pg_catalog.pg_attrdef AS d \
               ON d.adrelid = a.attrelid AND d.adnum = a.attnum \
             WHERE a.attrelid = $1::bigint::oid AND a.attnum > 0 AND NOT a.attisdropped \
             ORDER BY a.attnum",
            &[&oid],
        )
        .map_err(|_| database_error("column_evidence_failed"))?
        .into_iter()
        .map(|column| ColumnEvidence {
            position: column.get(0),
            name: column.get(1),
            data_type: column.get(2),
            not_null: column.get(3),
            has_default: column.get(4),
            default_expression: column.get(5),
            identity: column.get(6),
            generated: column.get(7),
        })
        .collect();
    let constraints = named_definitions(
        client,
        "SELECT conname, pg_catalog.pg_get_constraintdef(oid, true) \
         FROM pg_catalog.pg_constraint WHERE conrelid = $1::bigint::oid ORDER BY conname",
        oid,
        "constraint_evidence_failed",
    )?;
    let indexes = named_definitions(
        client,
        "SELECT indexrelid::regclass::text, pg_catalog.pg_get_indexdef(indexrelid) \
         FROM pg_catalog.pg_index WHERE indrelid = $1::bigint::oid \
         ORDER BY indexrelid::regclass::text",
        oid,
        "index_evidence_failed",
    )?;
    let user_triggers = named_definitions(
        client,
        "SELECT tgname, concat(tgenabled::text, ' ', \
                pg_catalog.pg_get_triggerdef(oid, true)) \
         FROM pg_catalog.pg_trigger \
         WHERE tgrelid = $1::bigint::oid AND NOT tgisinternal ORDER BY tgname",
        oid,
        "trigger_evidence_failed",
    )?;
    let rewrite_rules = named_definitions(
        client,
        "SELECT rulename, pg_catalog.pg_get_ruledef(oid, true) \
         FROM pg_catalog.pg_rewrite \
         WHERE ev_class = $1::bigint::oid AND rulename <> '_RETURN' ORDER BY rulename",
        oid,
        "rule_evidence_failed",
    )?;
    let policies = named_definitions(
        client,
        "SELECT polname, concat_ws(' ', polcmd, polpermissive::text, \
                    polroles::text, \
                    pg_catalog.pg_get_expr(polqual, polrelid), \
                    pg_catalog.pg_get_expr(polwithcheck, polrelid)) \
         FROM pg_catalog.pg_policy WHERE polrelid = $1::bigint::oid ORDER BY polname",
        oid,
        "policy_evidence_failed",
    )?;
    let inheritance_relations = client
        .query(
            "SELECT relation FROM ( \
               SELECT concat('parent:', inhparent::regclass::text) AS relation \
               FROM pg_catalog.pg_inherits WHERE inhrelid = $1::bigint::oid \
               UNION ALL \
               SELECT concat('child:', inhrelid::regclass::text) AS relation \
               FROM pg_catalog.pg_inherits WHERE inhparent = $1::bigint::oid \
             ) AS links ORDER BY relation",
            &[&oid],
        )
        .map_err(|_| database_error("inheritance_evidence_failed"))?
        .into_iter()
        .map(|link| link.get(0))
        .collect();
    let referential_actions = named_definitions(
        client,
        "SELECT conname, pg_catalog.pg_get_constraintdef(oid, true) \
         FROM pg_catalog.pg_constraint \
         WHERE confrelid = $1::bigint::oid AND contype = 'f' \
           AND (confupdtype IN ('c', 'n', 'd') OR confdeltype IN ('c', 'n', 'd')) \
         ORDER BY conname",
        oid,
        "referential_action_evidence_failed",
    )?;

    Ok(TableEvidence {
        schema: row.get(1),
        table: row.get(2),
        oid,
        relation_kind: row.get(3),
        persistence: row.get(4),
        row_security: row.get(5),
        force_row_security: row.get(6),
        columns,
        constraints,
        indexes,
        user_triggers,
        rewrite_rules,
        policies,
        inheritance_relations,
        referential_actions,
    })
}

pub fn table_schema_sha256(evidence: &TableEvidence) -> AppResult<String> {
    sha256_json(evidence)
}

pub fn canonical_table(evidence: &TableEvidence) -> String {
    format!(
        "{}.{}",
        quote_identifier(&evidence.schema),
        quote_identifier(&evidence.table)
    )
}

pub fn enforce_table_policy(
    evidence: &TableEvidence,
    statement_kind: StatementKind,
    allow_triggers: bool,
    allow_row_security: bool,
) -> AppResult<()> {
    if evidence.relation_kind != "r" || evidence.persistence != "p" {
        return Err(AppError::new(
            ErrorClass::Policy,
            "relation_kind_denied",
            "v0.1 supports only ordinary permanent PostgreSQL tables",
        ));
    }
    if !evidence.rewrite_rules.is_empty() {
        return Err(AppError::new(
            ErrorClass::Policy,
            "rewrite_rules_denied",
            "tables with user rewrite rules are not supported",
        ));
    }
    if !evidence.inheritance_relations.is_empty() {
        return Err(AppError::new(
            ErrorClass::Policy,
            "table_inheritance_denied",
            "tables participating in inheritance or partitioning are not supported",
        ));
    }
    if !evidence.referential_actions.is_empty() {
        return Err(AppError::new(
            ErrorClass::Policy,
            "referential_actions_denied",
            "tables referenced by cascading or SET NULL/DEFAULT foreign keys are not supported",
        ));
    }
    if !allow_triggers && !evidence.user_triggers.is_empty() {
        return Err(AppError::new(
            ErrorClass::Policy,
            "user_triggers_denied",
            "the table has user triggers; acknowledge them with --allow-triggers",
        ));
    }
    if !allow_row_security
        && (evidence.row_security || evidence.force_row_security || !evidence.policies.is_empty())
    {
        return Err(AppError::new(
            ErrorClass::Policy,
            "row_security_denied",
            "the table uses row-level security; acknowledge it with --allow-row-security",
        ));
    }
    if statement_kind == StatementKind::Insert
        && evidence.columns.iter().any(|column| {
            column.default_expression.is_some()
                || !column.identity.is_empty()
                || !column.generated.is_empty()
        })
    {
        return Err(AppError::new(
            ErrorClass::Policy,
            "insert_implicit_behavior_denied",
            "INSERT targets with defaults, identity columns, or generated columns are not supported",
        ));
    }
    Ok(())
}

pub fn apply_local_timeouts(client: &mut impl GenericClient, limits: &Limits) -> AppResult<()> {
    let idle = limits.statement_timeout_ms.saturating_add(5_000);
    let settings = format!(
        "SET LOCAL statement_timeout = '{}ms'; \
         SET LOCAL lock_timeout = '{}ms'; \
         SET LOCAL idle_in_transaction_session_timeout = '{idle}ms'",
        limits.statement_timeout_ms, limits.lock_timeout_ms
    );
    client
        .batch_execute(&settings)
        .map_err(|_| database_error("timeout_configuration_failed"))?;
    Ok(())
}

pub fn sqlstate(error: &postgres::Error) -> Option<String> {
    error.code().map(|code| code.code().to_owned())
}

fn named_definitions(
    client: &mut impl GenericClient,
    query: &str,
    oid: i64,
    error_code: &'static str,
) -> AppResult<Vec<NamedDefinition>> {
    client
        .query(query, &[&oid])
        .map_err(|_| database_error(error_code))
        .map(|rows| {
            rows.into_iter()
                .map(|definition| NamedDefinition {
                    name: definition.get(0),
                    definition: definition.get(1),
                })
                .collect()
        })
}

fn validate_environment_name(name: &str) -> AppResult<()> {
    let mut characters = name.chars();
    let first_valid = characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic());
    if !first_valid
        || !characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
    {
        return Err(AppError::new(
            ErrorClass::Usage,
            "database_env_name_invalid",
            "the database environment variable name is invalid",
        ));
    }
    Ok(())
}

fn all_hosts_are_local(config: &Config) -> bool {
    let hosts_local = config.get_hosts().iter().all(|host| match host {
        Host::Tcp(hostname) => {
            matches!(
                hostname.to_ascii_lowercase().as_str(),
                "localhost" | "127.0.0.1" | "::1"
            )
        }
        #[cfg(unix)]
        Host::Unix(_) => true,
    });
    let addresses_local = config
        .get_hostaddrs()
        .iter()
        .all(std::net::IpAddr::is_loopback);
    hosts_local && addresses_local
}

fn endpoint_sha256(config: &Config) -> String {
    let hosts = config
        .get_hosts()
        .iter()
        .map(|host| match host {
            Host::Tcp(hostname) => format!("tcp:{hostname}"),
            #[cfg(unix)]
            Host::Unix(path) => format!("unix:{}", path.to_string_lossy()),
        })
        .collect::<Vec<_>>()
        .join("\n");
    let addresses = config
        .get_hostaddrs()
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    let ports = config
        .get_ports()
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let identity = format!(
        "hosts={hosts}\naddresses={addresses}\nports={ports}\nuser={}\ndatabase={}",
        config.get_user().unwrap_or(""),
        config.get_dbname().unwrap_or("")
    );
    sha256_bytes(identity.as_bytes())
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn database_error(code: &'static str) -> AppError {
    AppError::new(
        ErrorClass::Io,
        code,
        "the PostgreSQL operation failed; connection details were not emitted",
    )
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use postgres::Config;

    use super::{all_hosts_are_local, endpoint_sha256, quote_identifier};

    #[test]
    fn identifies_only_explicit_local_hosts() {
        let local: Config = "host=127.0.0.1 user=test"
            .parse()
            .expect("valid local config");
        let remote: Config = "host=db.example.com user=test"
            .parse()
            .expect("valid remote config");
        assert!(all_hosts_are_local(&local));
        assert!(!all_hosts_are_local(&remote));
    }

    #[test]
    fn quotes_postgres_identifiers() {
        assert_eq!(quote_identifier("odd\"name"), "\"odd\"\"name\"");
    }

    #[test]
    fn endpoint_identity_excludes_password_but_binds_host() {
        let first: Config = "host=db-a.example user=app password=first dbname=app"
            .parse()
            .expect("valid first config");
        let rotated: Config = "host=db-a.example user=app password=second dbname=app"
            .parse()
            .expect("valid rotated config");
        let different_host: Config = "host=db-b.example user=app password=first dbname=app"
            .parse()
            .expect("valid alternate config");
        assert_eq!(endpoint_sha256(&first), endpoint_sha256(&rotated));
        assert_ne!(endpoint_sha256(&first), endpoint_sha256(&different_host));
    }
}
