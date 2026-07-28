use std::{fs, ops::ControlFlow, path::Path};

use sqlparser::{
    ast::{
        visit_expressions, Delete, Expr, FromTable, Insert, ObjectName, SetExpr, Statement,
        TableFactor, TableObject, TableWithJoins, Update, Value,
    },
    dialect::PostgreSqlDialect,
    parser::Parser,
};

use crate::{
    error::{AppError, AppResult, ErrorClass},
    integrity::sha256_bytes,
    model::StatementKind,
};

const MAX_SQL_BYTES: u64 = 1_048_576;

#[derive(Debug, Clone)]
pub struct AnalyzedSql {
    pub normalized: String,
    pub kind: StatementKind,
    pub requested_table: String,
    pub sql_file_sha256: String,
    pub normalized_sql_sha256: String,
    pub count_sql: Option<String>,
    pub count_query_sha256: Option<String>,
    pub target_rows_sql: Option<String>,
    pub target_query_sha256: Option<String>,
    pub inserted_rows: u64,
}

struct StatementDetails {
    kind: StatementKind,
    table: String,
    count_sql: Option<String>,
    target_rows_sql: Option<String>,
    inserted_rows: u64,
}

pub fn read_and_analyze(path: &Path) -> AppResult<AnalyzedSql> {
    let metadata = fs::metadata(path).map_err(|_| {
        AppError::new(
            ErrorClass::Io,
            "sql_file_unreadable",
            "the SQL file could not be read",
        )
    })?;
    if !metadata.is_file() {
        return Err(AppError::new(
            ErrorClass::Usage,
            "sql_path_not_file",
            "the SQL path must refer to a regular file",
        ));
    }
    if metadata.len() > MAX_SQL_BYTES {
        return Err(AppError::new(
            ErrorClass::Usage,
            "sql_file_too_large",
            "the SQL file exceeds the 1 MiB contract limit",
        ));
    }

    let raw = fs::read_to_string(path).map_err(|_| {
        AppError::new(
            ErrorClass::Io,
            "sql_file_not_utf8",
            "the SQL file must be valid UTF-8",
        )
    })?;
    analyze_sql(&raw)
}

/// Parses and enforces DMLPact's single-statement SQL policy without file I/O.
///
/// # Errors
///
/// Returns an error when the SQL exceeds the input bound, cannot be parsed, or
/// uses a statement shape or expression denied by the safety contract.
pub fn analyze_sql(raw: &str) -> AppResult<AnalyzedSql> {
    if u64::try_from(raw.len()).unwrap_or(u64::MAX) > MAX_SQL_BYTES {
        return Err(AppError::new(
            ErrorClass::Usage,
            "sql_file_too_large",
            "the SQL file exceeds the 1 MiB contract limit",
        ));
    }
    if raw.trim().is_empty() {
        return Err(policy("empty_sql", "the SQL file is empty"));
    }

    let dialect = PostgreSqlDialect {};
    let statements = Parser::parse_sql(&dialect, raw).map_err(|_| {
        policy(
            "sql_parse_failed",
            "the SQL file is not valid supported PostgreSQL syntax",
        )
    })?;
    if statements.len() != 1 {
        return Err(policy(
            "statement_count_not_one",
            "exactly one SQL statement is required",
        ));
    }
    let statement = statements
        .first()
        .ok_or_else(|| policy("empty_sql", "the SQL file is empty"))?;
    reject_unsafe_expressions(statement)?;

    let details = match statement {
        Statement::Insert(insert) => analyze_insert(insert)?,
        Statement::Update(update) => analyze_update(update)?,
        Statement::Delete(delete) => analyze_delete(delete)?,
        _ => {
            return Err(policy(
                "statement_kind_denied",
                "only INSERT VALUES, UPDATE, and DELETE are supported",
            ));
        }
    };

    let normalized = statement.to_string();
    let count_query_sha256 = details
        .count_sql
        .as_ref()
        .map(|query| sha256_bytes(query.as_bytes()));
    let target_query_sha256 = details
        .target_rows_sql
        .as_ref()
        .map(|query| sha256_bytes(query.as_bytes()));

    Ok(AnalyzedSql {
        normalized_sql_sha256: sha256_bytes(normalized.as_bytes()),
        normalized,
        kind: details.kind,
        requested_table: details.table,
        sql_file_sha256: sha256_bytes(raw.as_bytes()),
        count_sql: details.count_sql,
        count_query_sha256,
        target_rows_sql: details.target_rows_sql,
        target_query_sha256,
        inserted_rows: details.inserted_rows,
    })
}

fn analyze_insert(insert: &Insert) -> AppResult<StatementDetails> {
    if !insert.optimizer_hints.is_empty()
        || insert.or.is_some()
        || insert.ignore
        || insert.table_alias.is_some()
        || insert.overwrite
        || !insert.assignments.is_empty()
        || insert.partitioned.is_some()
        || !insert.after_columns.is_empty()
        || insert.has_table_keyword
        || insert.on.is_some()
        || insert.returning.is_some()
        || insert.output.is_some()
        || insert.replace_into
        || insert.priority.is_some()
        || insert.insert_alias.is_some()
        || insert.settings.is_some()
        || insert.format_clause.is_some()
        || insert.multi_table_insert_type.is_some()
        || !insert.multi_table_into_clauses.is_empty()
        || !insert.multi_table_when_clauses.is_empty()
        || insert.multi_table_else_clause.is_some()
    {
        return Err(policy(
            "insert_form_denied",
            "only a plain single-table INSERT ... VALUES statement is supported",
        ));
    }

    let name = match &insert.table {
        TableObject::TableName(name) => validate_object_name(name)?,
        _ => {
            return Err(policy(
                "insert_target_denied",
                "the INSERT target must be a named table",
            ));
        }
    };
    let source = insert.source.as_ref().ok_or_else(|| {
        policy(
            "insert_values_required",
            "INSERT must contain an explicit VALUES clause",
        )
    })?;
    if source.with.is_some()
        || source.order_by.is_some()
        || source.limit_clause.is_some()
        || source.fetch.is_some()
        || !source.locks.is_empty()
        || source.for_clause.is_some()
        || source.settings.is_some()
        || source.format_clause.is_some()
        || !source.pipe_operators.is_empty()
    {
        return Err(policy(
            "insert_query_denied",
            "INSERT query modifiers are not supported",
        ));
    }
    let values = match source.body.as_ref() {
        SetExpr::Values(values) => values,
        _ => {
            return Err(policy(
                "insert_select_denied",
                "INSERT ... SELECT is not supported; use explicit VALUES",
            ));
        }
    };
    if values.explicit_row || values.value_keyword || values.rows.is_empty() {
        return Err(policy(
            "insert_values_form_denied",
            "INSERT requires one or more standard VALUES rows",
        ));
    }
    let width = values
        .rows
        .first()
        .map(|row| row.len())
        .ok_or_else(|| policy("insert_values_empty", "INSERT VALUES must not be empty"))?;
    if width == 0 || values.rows.iter().any(|row| row.len() != width) {
        return Err(policy(
            "insert_values_width_mismatch",
            "all INSERT VALUES rows must have the same non-zero width",
        ));
    }
    if !insert.columns.is_empty() && insert.columns.len() != width {
        return Err(policy(
            "insert_column_count_mismatch",
            "the INSERT column count must match every VALUES row",
        ));
    }
    let row_count = u64::try_from(values.rows.len()).map_err(|_| {
        policy(
            "insert_row_count_out_of_range",
            "the INSERT row count is outside the supported range",
        )
    })?;
    Ok(StatementDetails {
        kind: StatementKind::Insert,
        table: name,
        count_sql: None,
        target_rows_sql: None,
        inserted_rows: row_count,
    })
}

fn analyze_update(update: &Update) -> AppResult<StatementDetails> {
    if !update.optimizer_hints.is_empty()
        || update.from.is_some()
        || update.returning.is_some()
        || update.output.is_some()
        || update.or.is_some()
        || !update.order_by.is_empty()
        || update.limit.is_some()
        || update.assignments.is_empty()
    {
        return Err(policy(
            "update_form_denied",
            "UPDATE joins, FROM, RETURNING, ORDER BY, LIMIT, and extensions are not supported",
        ));
    }
    let selection = update
        .selection
        .as_ref()
        .ok_or_else(|| policy("where_required", "UPDATE requires an explicit WHERE clause"))?;
    let (name, row_reference) = validate_table_with_joins(&update.table)?;
    let count_sql = format!(
        "SELECT COUNT(*)::bigint FROM {} WHERE {selection}",
        update.table
    );
    let target_rows_sql = format!(
        "SELECT pg_catalog.to_jsonb({row_reference})::text FROM {} WHERE {selection} \
         ORDER BY pg_catalog.to_jsonb({row_reference})::text",
        update.table
    );
    Ok(StatementDetails {
        kind: StatementKind::Update,
        table: name,
        count_sql: Some(count_sql),
        target_rows_sql: Some(target_rows_sql),
        inserted_rows: 0,
    })
}

fn analyze_delete(delete: &Delete) -> AppResult<StatementDetails> {
    if !delete.optimizer_hints.is_empty()
        || !delete.tables.is_empty()
        || delete.using.is_some()
        || delete.returning.is_some()
        || delete.output.is_some()
        || !delete.order_by.is_empty()
        || delete.limit.is_some()
    {
        return Err(policy(
            "delete_form_denied",
            "DELETE USING, RETURNING, ORDER BY, LIMIT, and extensions are not supported",
        ));
    }
    let selection = delete
        .selection
        .as_ref()
        .ok_or_else(|| policy("where_required", "DELETE requires an explicit WHERE clause"))?;
    let tables = match &delete.from {
        FromTable::WithFromKeyword(tables) => tables,
        FromTable::WithoutKeyword(_) => {
            return Err(policy(
                "delete_from_required",
                "DELETE must use the standard FROM form",
            ));
        }
    };
    if tables.len() != 1 {
        return Err(policy(
            "single_table_required",
            "exactly one target table is required",
        ));
    }
    let table = tables.first().ok_or_else(|| {
        policy(
            "single_table_required",
            "exactly one target table is required",
        )
    })?;
    let (name, row_reference) = validate_table_with_joins(table)?;
    let count_sql = format!("SELECT COUNT(*)::bigint FROM {table} WHERE {selection}");
    let target_rows_sql = format!(
        "SELECT pg_catalog.to_jsonb({row_reference})::text FROM {table} WHERE {selection} \
         ORDER BY pg_catalog.to_jsonb({row_reference})::text"
    );
    Ok(StatementDetails {
        kind: StatementKind::Delete,
        table: name,
        count_sql: Some(count_sql),
        target_rows_sql: Some(target_rows_sql),
        inserted_rows: 0,
    })
}

fn validate_table_with_joins(table: &TableWithJoins) -> AppResult<(String, String)> {
    if !table.joins.is_empty() {
        return Err(policy(
            "join_denied",
            "joins are not supported for mutation targets",
        ));
    }
    match &table.relation {
        TableFactor::Table {
            name,
            alias,
            args,
            with_hints,
            version,
            with_ordinality,
            partitions,
            json_path,
            sample,
            index_hints,
        } if args.is_none()
            && with_hints.is_empty()
            && version.is_none()
            && !with_ordinality
            && partitions.is_empty()
            && json_path.is_none()
            && sample.is_none()
            && index_hints.is_empty()
            && alias.as_ref().is_none_or(|value| value.columns.is_empty()) =>
        {
            let validated_name = validate_object_name(name)?;
            let row_reference = if let Some(alias) = alias {
                alias.name.to_string()
            } else {
                name.0
                    .last()
                    .and_then(|part| part.as_ident())
                    .ok_or_else(|| {
                        policy(
                            "table_name_denied",
                            "the target table must have an identifier row reference",
                        )
                    })?
                    .to_string()
            };
            Ok((validated_name, row_reference))
        }
        _ => Err(policy(
            "table_factor_denied",
            "the mutation target must be a plain named table",
        )),
    }
}

fn validate_object_name(name: &ObjectName) -> AppResult<String> {
    if !(1..=2).contains(&name.0.len()) || name.0.iter().any(|part| part.as_ident().is_none()) {
        return Err(policy(
            "table_name_denied",
            "table names must be unqualified or schema-qualified identifiers",
        ));
    }
    Ok(name.to_string())
}

fn reject_unsafe_expressions(statement: &Statement) -> AppResult<()> {
    let result = visit_expressions(statement, |expression| match expression {
        Expr::Function(_) => ControlFlow::Break("function"),
        Expr::Subquery(_) | Expr::Exists { .. } | Expr::InSubquery { .. } => {
            ControlFlow::Break("subquery")
        }
        Expr::Value(value) if matches!(value.value, Value::Placeholder(_)) => {
            ControlFlow::Break("placeholder")
        }
        Expr::Identifier(identifier)
            if identifier.quote_style.is_none()
                && identifier.value.eq_ignore_ascii_case("default") =>
        {
            ControlFlow::Break("default")
        }
        _ => ControlFlow::Continue(()),
    });
    match result {
        ControlFlow::Continue(()) => Ok(()),
        ControlFlow::Break(kind) => Err(policy(
            "dynamic_expression_denied",
            format!("{kind} expressions are not supported in v0.1 safety contracts"),
        )),
    }
}

fn policy(code: impl Into<String>, message: impl Into<String>) -> AppError {
    AppError::new(ErrorClass::Policy, code, message)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::read_and_analyze;

    fn analyze(sql: &str) -> crate::error::AppResult<super::AnalyzedSql> {
        let directory = tempdir().expect("temporary directory");
        let path = directory.path().join("change.sql");
        fs::write(&path, sql).expect("write SQL fixture");
        read_and_analyze(&path)
    }

    #[test]
    fn accepts_bounded_update() {
        let analyzed = analyze("UPDATE accounts SET active = false WHERE id = 7")
            .expect("bounded update should parse");
        assert_eq!(analyzed.requested_table, "accounts");
        assert!(analyzed.count_sql.is_some());
    }

    #[test]
    fn accepts_explicit_insert_rows() {
        let analyzed = analyze("INSERT INTO events (id, name) VALUES (1, 'a'), (2, 'b')")
            .expect("values insert should parse");
        assert_eq!(analyzed.inserted_rows, 2);
    }

    #[test]
    fn rejects_missing_where() {
        let error = analyze("DELETE FROM accounts").expect_err("DELETE must be bounded");
        assert_eq!(error.code, "where_required");
    }

    #[test]
    fn rejects_multiple_statements() {
        let error = analyze("DELETE FROM a WHERE id = 1; DELETE FROM b WHERE id = 2")
            .expect_err("multiple statements must be rejected");
        assert_eq!(error.code, "statement_count_not_one");
    }

    #[test]
    fn rejects_functions_and_subqueries() {
        assert_eq!(
            analyze("UPDATE accounts SET seen_at = now() WHERE id = 1")
                .expect_err("functions must be rejected")
                .code,
            "dynamic_expression_denied"
        );
        assert_eq!(
            analyze("DELETE FROM accounts WHERE id IN (SELECT id FROM stale)")
                .expect_err("subqueries must be rejected")
                .code,
            "dynamic_expression_denied"
        );
        assert_eq!(
            analyze("UPDATE accounts SET status = DEFAULT WHERE id = 1")
                .expect_err("DEFAULT must be rejected")
                .code,
            "dynamic_expression_denied"
        );
    }
}
