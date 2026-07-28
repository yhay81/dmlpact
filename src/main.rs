use std::{collections::BTreeMap, path::PathBuf, process::ExitCode, time::Duration};

use clap::{error::ErrorKind, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use dmlpact::{
    engine::{
        apply_plan, create_plan, inspect_table, lint_sql, ApplyOptions, DatabaseOptions,
        PlanOptions,
    },
    error::{AppError, AppResult, ErrorClass, ErrorDocument},
    model::{
        ApplyResult, Capabilities, ContractBrief, Limits, Plan, ReceiptEvent, ReceiptState,
        ReceiptVerification, TOOL_VERSION,
    },
    receipt::verify_receipt,
};
use schemars::{schema_for, JsonSchema};
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(
    name = "dmlpact",
    version,
    about = "Plan, constrain, apply, and audit PostgreSQL data changes",
    long_about = None
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate a SQL file without connecting to PostgreSQL.
    Lint {
        #[arg(long, value_name = "FILE")]
        sql: PathBuf,
    },
    /// Inspect the resolved table and its safety-relevant schema evidence.
    Inspect {
        #[arg(long)]
        table: String,
        #[command(flatten)]
        database: DatabaseArgs,
    },
    /// Create a sealed, expiring execution plan without mutating data.
    Plan {
        #[arg(long, value_name = "FILE")]
        sql: PathBuf,
        #[arg(long, value_name = "NEW_FILE")]
        out: PathBuf,
        #[arg(long, default_value_t = 100)]
        max_rows: u64,
        #[arg(long, default_value = "20s", value_parser = parse_duration)]
        statement_timeout: Duration,
        #[arg(long, default_value = "2s", value_parser = parse_duration)]
        lock_timeout: Duration,
        #[arg(long, default_value = "15m", value_parser = parse_duration)]
        valid_for: Duration,
        #[arg(long)]
        allow_triggers: bool,
        #[arg(long)]
        allow_row_security: bool,
        #[command(flatten)]
        database: DatabaseArgs,
    },
    /// Apply the exact SQL bound to a sealed plan and write a mandatory receipt.
    Apply {
        #[arg(long, value_name = "FILE")]
        sql: PathBuf,
        #[arg(long, value_name = "FILE")]
        plan: PathBuf,
        #[arg(long, value_name = "NEW_FILE")]
        receipt: PathBuf,
        #[command(flatten)]
        database: DatabaseArgs,
    },
    /// Verify or inspect an append-only execution receipt.
    Receipt {
        #[command(subcommand)]
        command: ReceiptCommand,
    },
    /// Emit a JSON Schema for a stable machine contract.
    Schema {
        #[arg(value_enum)]
        document: SchemaDocument,
    },
    /// Describe machine-visible safety and feature capabilities.
    Capabilities,
    /// Emit the compact CLI contract.
    Contract,
    /// Generate shell completion source.
    Completions {
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[derive(Debug, Subcommand)]
enum ReceiptCommand {
    /// Verify event hashes, ordering, and chain linkage without a database.
    Verify {
        #[arg(long, value_name = "FILE")]
        receipt: PathBuf,
    },
}

#[derive(Debug, Clone, clap::Args)]
struct DatabaseArgs {
    /// Environment variable containing the PostgreSQL connection string.
    #[arg(long, default_value = "DMLPACT_DATABASE_URL")]
    database_env: String,
    /// Disable TLS only when every configured host is loopback or a Unix socket.
    #[arg(long)]
    allow_insecure_localhost: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SchemaDocument {
    Plan,
    ReceiptEvent,
    ReceiptVerification,
    ApplyResult,
    Error,
    Capabilities,
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) =>
        {
            print!("{error}");
            return ExitCode::SUCCESS;
        }
        Err(_) => {
            let error = AppError::new(
                ErrorClass::Usage,
                "cli_arguments_invalid",
                "invalid command-line arguments; run dmlpact --help",
            );
            emit_error(&error);
            return ExitCode::from(error.class.exit_code());
        }
    };

    match run(cli) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            emit_error(&error);
            ExitCode::from(error.class.exit_code())
        }
    }
}

fn run(cli: Cli) -> AppResult<u8> {
    match cli.command {
        Command::Lint { sql } => {
            emit_json(&lint_sql(&sql)?)?;
        }
        Command::Inspect { table, database } => {
            emit_json(&inspect_table(&table, &database.into())?)?;
        }
        Command::Plan {
            sql,
            out,
            max_rows,
            statement_timeout,
            lock_timeout,
            valid_for,
            allow_triggers,
            allow_row_security,
            database,
        } => {
            let options = PlanOptions {
                sql_path: sql,
                output_path: out,
                database: database.into(),
                limits: Limits {
                    max_rows,
                    statement_timeout_ms: duration_ms(statement_timeout)?,
                    lock_timeout_ms: duration_ms(lock_timeout)?,
                },
                lifetime_ms: duration_ms(valid_for)?,
                allow_triggers,
                allow_row_security,
            };
            emit_json(&create_plan(&options)?)?;
        }
        Command::Apply {
            sql,
            plan,
            receipt,
            database,
        } => {
            let result = apply_plan(&ApplyOptions {
                sql_path: sql,
                plan_path: plan,
                receipt_path: receipt,
                database: database.into(),
            })?;
            let exit_code = if result.state == ReceiptState::Uncertain {
                ErrorClass::Contract.exit_code()
            } else {
                0
            };
            emit_json(&result)?;
            return Ok(exit_code);
        }
        Command::Receipt { command } => match command {
            ReceiptCommand::Verify { receipt } => emit_json(&verify_receipt(&receipt)?)?,
        },
        Command::Schema { document } => match document {
            SchemaDocument::Plan => emit_schema::<Plan>()?,
            SchemaDocument::ReceiptEvent => emit_schema::<ReceiptEvent>()?,
            SchemaDocument::ReceiptVerification => emit_schema::<ReceiptVerification>()?,
            SchemaDocument::ApplyResult => emit_schema::<ApplyResult>()?,
            SchemaDocument::Error => emit_schema::<ErrorDocument>()?,
            SchemaDocument::Capabilities => emit_schema::<Capabilities>()?,
        },
        Command::Capabilities => emit_json(&capabilities())?,
        Command::Contract => emit_json(&contract())?,
        Command::Completions { shell } => {
            let mut command = <Cli as clap::CommandFactory>::command();
            generate(shell, &mut command, "dmlpact", &mut std::io::stdout());
        }
    }
    Ok(0)
}

impl From<DatabaseArgs> for DatabaseOptions {
    fn from(value: DatabaseArgs) -> Self {
        Self {
            environment_name: value.database_env,
            allow_insecure_localhost: value.allow_insecure_localhost,
        }
    }
}

fn capabilities() -> Capabilities {
    Capabilities {
        schema_version: "dmlpact.capabilities.v1".to_owned(),
        tool_version: TOOL_VERSION.to_owned(),
        database: "PostgreSQL 13+".to_owned(),
        statement_kinds: vec![
            "insert_values".to_owned(),
            "update_with_where".to_owned(),
            "delete_with_where".to_owned(),
        ],
        transport_policies: vec![
            "tls_required".to_owned(),
            "insecure_localhost_explicit_opt_in".to_owned(),
        ],
        safety_defaults: BTreeMap::from([
            ("apply_receipt".to_owned(), "required_new_path".to_owned()),
            (
                "connection_secret".to_owned(),
                "environment_only".to_owned(),
            ),
            ("drift".to_owned(), "refuse".to_owned()),
            (
                "row_security".to_owned(),
                "refuse_unless_acknowledged".to_owned(),
            ),
            ("tls".to_owned(), "required".to_owned()),
            (
                "triggers".to_owned(),
                "refuse_unless_acknowledged".to_owned(),
            ),
            ("where".to_owned(), "required_for_update_delete".to_owned()),
        ]),
        limitations: vec![
            "single ordinary permanent table only".to_owned(),
            "no functions, subqueries, joins, RETURNING, ON CONFLICT, USING, or UPDATE FROM"
                .to_owned(),
            "no partitioned, foreign, temporary, or rewrite-rule targets".to_owned(),
            "no inheritance links or cascading/SET NULL/SET DEFAULT referential actions".to_owned(),
            "INSERT targets cannot have defaults, identity columns, or generated columns"
                .to_owned(),
            "plans expire within 24 hours".to_owned(),
        ],
        supported_platforms: vec![
            "linux-x86_64".to_owned(),
            "macos-x86_64".to_owned(),
            "macos-aarch64".to_owned(),
            "windows-x86_64".to_owned(),
        ],
    }
}

fn contract() -> ContractBrief {
    ContractBrief {
        schema_version: "dmlpact.contract.v1".to_owned(),
        tool_version: TOOL_VERSION.to_owned(),
        commands: vec![
            "lint".to_owned(),
            "inspect".to_owned(),
            "plan".to_owned(),
            "apply".to_owned(),
            "receipt verify".to_owned(),
            "schema".to_owned(),
            "capabilities".to_owned(),
            "contract".to_owned(),
            "completions".to_owned(),
        ],
        output_formats: vec!["json".to_owned(), "ndjson_receipt".to_owned()],
        data_schemas: BTreeMap::from([
            (
                "apply_result".to_owned(),
                "dmlpact.apply-result.v1".to_owned(),
            ),
            ("error".to_owned(), "dmlpact.error.v1".to_owned()),
            ("plan".to_owned(), "dmlpact.plan.v1".to_owned()),
            (
                "receipt_event".to_owned(),
                "dmlpact.receipt-event.v1".to_owned(),
            ),
        ]),
        exit_codes: BTreeMap::from([
            (
                "0".to_owned(),
                "success_or_receipted_safety_refusal".to_owned(),
            ),
            ("1".to_owned(), "io_or_database_transport".to_owned()),
            ("2".to_owned(), "usage".to_owned()),
            ("3".to_owned(), "policy".to_owned()),
            ("4".to_owned(), "budget".to_owned()),
            ("5".to_owned(), "contract_or_uncertain_commit".to_owned()),
        ]),
        safety_defaults: capabilities().safety_defaults,
    }
}

fn emit_schema<T: JsonSchema>() -> AppResult<()> {
    emit_json(&schema_for!(T))
}

fn emit_json<T: Serialize>(value: &T) -> AppResult<()> {
    use std::io::Write as _;

    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer_pretty(&mut output, value).map_err(|_| {
        AppError::new(
            ErrorClass::Io,
            "stdout_write_failed",
            "could not write the command result",
        )
    })?;
    output.write_all(b"\n").map_err(|_| {
        AppError::new(
            ErrorClass::Io,
            "stdout_write_failed",
            "could not write the command result",
        )
    })
}

fn emit_error(error: &AppError) {
    let document = ErrorDocument::from(error);
    if serde_json::to_writer(std::io::stderr(), &document).is_ok() {
        use std::io::Write as _;
        std::io::stderr().write_all(b"\n").ok();
    }
}

fn parse_duration(value: &str) -> Result<Duration, String> {
    humantime::parse_duration(value)
        .map_err(|_| "expected a duration such as 500ms, 20s, or 15m".to_owned())
}

fn duration_ms(duration: Duration) -> AppResult<u64> {
    u64::try_from(duration.as_millis()).map_err(|_| {
        AppError::new(
            ErrorClass::Usage,
            "duration_out_of_range",
            "the duration is outside the supported range",
        )
    })
}
