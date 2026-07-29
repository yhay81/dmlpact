#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
result_path="${1:-${root_dir}/benchmark-results.json}"
binary="${root_dir}/target/release/dmlpact"
setup_sql="${root_dir}/benchmarks/setup.sql"
sql_10k="${root_dir}/benchmarks/update-10k.sql"
sql_100k="${root_dir}/benchmarks/update-100k.sql"
sql_lock="${root_dir}/benchmarks/update-lock.sql"

for dependency in cargo git jq psql seq stat timeout uname; do
  command -v "${dependency}" >/dev/null || {
    printf 'missing benchmark dependency: %s\n' "${dependency}" >&2
    exit 1
  }
done

if ! /usr/bin/time --version 2>&1 | grep -qi 'GNU time'; then
  printf 'benchmarks/run.sh requires GNU /usr/bin/time (the Ubuntu runner provides it)\n' >&2
  exit 1
fi

export DMLPACT_DATABASE_URL="${DMLPACT_DATABASE_URL:-postgresql://postgres:postgres@127.0.0.1:5432/dmlpact_benchmark}"
database_name="$(psql "${DMLPACT_DATABASE_URL}" -v ON_ERROR_STOP=1 -Atc 'SELECT current_database()')"
if test "${database_name}" != "dmlpact_benchmark"; then
  printf 'refusing to reset non-benchmark database: %s\n' "${database_name}" >&2
  exit 1
fi

temp_dir="$(mktemp -d)"
lock_holder_pid=""
cleanup() {
  if test -n "${lock_holder_pid}"; then
    kill "${lock_holder_pid}" 2>/dev/null || true
    wait "${lock_holder_pid}" 2>/dev/null || true
  fi
  rm -rf "${temp_dir}"
}
trap cleanup EXIT

plan_10k="${temp_dir}/update-10k.plan.json"
receipt_10k="${temp_dir}/update-10k.receipt.ndjson"
plan_100k="${temp_dir}/update-100k.plan.json"
receipt_100k="${temp_dir}/update-100k.receipt.ndjson"
plan_lock="${temp_dir}/update-lock.plan.json"
receipt_lock="${temp_dir}/update-lock.receipt.ndjson"

cd "${root_dir}"
cargo build --release --locked
psql "${DMLPACT_DATABASE_URL}" -v ON_ERROR_STOP=1 -f "${setup_sql}" >/dev/null

measure() {
  local metrics="$1"
  local output="$2"
  local diagnostics="$3"
  shift 3

  /usr/bin/time \
    -f '{"wall_seconds": %e, "max_rss_kib": %M, "exit_code": %x}' \
    -o "${metrics}" \
    timeout --signal=KILL 120s "$@" >"${output}" 2>"${diagnostics}"
  jq -e . "${metrics}" >/dev/null
  jq -e . "${output}" >/dev/null
}

schema_metrics="${temp_dir}/schema.metrics.json"
schema_output="${temp_dir}/schema.json"
schema_diagnostics="${temp_dir}/schema.stderr"
plan_10k_metrics="${temp_dir}/plan-10k.metrics.json"
plan_10k_output="${temp_dir}/plan-10k.json"
plan_10k_diagnostics="${temp_dir}/plan-10k.stderr"
apply_10k_metrics="${temp_dir}/apply-10k.metrics.json"
apply_10k_output="${temp_dir}/apply-10k.json"
apply_10k_diagnostics="${temp_dir}/apply-10k.stderr"
verify_10k_metrics="${temp_dir}/verify-10k.metrics.json"
verify_10k_output="${temp_dir}/verify-10k.json"
verify_10k_diagnostics="${temp_dir}/verify-10k.stderr"
plan_100k_metrics="${temp_dir}/plan-100k.metrics.json"
plan_100k_output="${temp_dir}/plan-100k.json"
plan_100k_diagnostics="${temp_dir}/plan-100k.stderr"
apply_100k_metrics="${temp_dir}/apply-100k.metrics.json"
apply_100k_output="${temp_dir}/apply-100k.json"
apply_100k_diagnostics="${temp_dir}/apply-100k.stderr"
verify_100k_metrics="${temp_dir}/verify-100k.metrics.json"
verify_100k_output="${temp_dir}/verify-100k.json"
verify_100k_diagnostics="${temp_dir}/verify-100k.stderr"
lock_apply_metrics="${temp_dir}/lock-apply.metrics.json"
lock_apply_output="${temp_dir}/lock-apply.json"
lock_apply_diagnostics="${temp_dir}/lock-apply.stderr"
lock_verify_output="${temp_dir}/lock-verify.json"

measure "${schema_metrics}" "${schema_output}" "${schema_diagnostics}" \
  "${binary}" schema plan

measure "${plan_10k_metrics}" "${plan_10k_output}" "${plan_10k_diagnostics}" \
  "${binary}" plan \
  --sql "${sql_10k}" \
  --out "${plan_10k}" \
  --max-rows 10000 \
  --statement-timeout 30s \
  --lock-timeout 5s \
  --valid-for 15m \
  --allow-insecure-localhost
jq -e '
  .schema_version == "dmlpact.plan.v1"
  and .statement_kind == "update"
  and .preconditions.target_count == 10000
  and .limits.max_rows == 10000
  and (.plan_sha256 | type == "string" and length == 64)
' "${plan_10k}" >/dev/null

measure "${apply_10k_metrics}" "${apply_10k_output}" "${apply_10k_diagnostics}" \
  "${binary}" apply \
  --sql "${sql_10k}" \
  --plan "${plan_10k}" \
  --receipt "${receipt_10k}" \
  --allow-insecure-localhost
jq -e '
  .schema_version == "dmlpact.apply-result.v1"
  and .state == "committed"
  and .affected_rows == 10000
  and .reason_code == "applied"
' "${apply_10k_output}" >/dev/null

measure "${verify_10k_metrics}" "${verify_10k_output}" "${verify_10k_diagnostics}" \
  "${binary}" receipt verify --receipt "${receipt_10k}"
jq -e '
  .schema_version == "dmlpact.receipt-verification.v1"
  and .integrity_valid
  and .complete
  and .event_count == 2
  and .final_state == "committed"
' "${verify_10k_output}" >/dev/null

rows_after_10k="$(
  psql "${DMLPACT_DATABASE_URL}" -v ON_ERROR_STOP=1 -Atc \
    'SELECT count(*) FROM dmlpact_benchmark WHERE benchmark_value = 1'
)"
untouched_after_10k="$(
  psql "${DMLPACT_DATABASE_URL}" -v ON_ERROR_STOP=1 -Atc \
    'SELECT count(*) FROM dmlpact_benchmark WHERE benchmark_value = 0'
)"
test "${rows_after_10k}" = "10000"
test "${untouched_after_10k}" = "90000"

measure "${plan_100k_metrics}" "${plan_100k_output}" "${plan_100k_diagnostics}" \
  "${binary}" plan \
  --sql "${sql_100k}" \
  --out "${plan_100k}" \
  --max-rows 100000 \
  --statement-timeout 30s \
  --lock-timeout 5s \
  --valid-for 15m \
  --allow-insecure-localhost
jq -e '
  .schema_version == "dmlpact.plan.v1"
  and .statement_kind == "update"
  and .preconditions.target_count == 100000
  and .limits.max_rows == 100000
' "${plan_100k}" >/dev/null

measure "${apply_100k_metrics}" "${apply_100k_output}" "${apply_100k_diagnostics}" \
  "${binary}" apply \
  --sql "${sql_100k}" \
  --plan "${plan_100k}" \
  --receipt "${receipt_100k}" \
  --allow-insecure-localhost
jq -e '
  .schema_version == "dmlpact.apply-result.v1"
  and .state == "committed"
  and .affected_rows == 100000
  and .reason_code == "applied"
' "${apply_100k_output}" >/dev/null

measure "${verify_100k_metrics}" "${verify_100k_output}" "${verify_100k_diagnostics}" \
  "${binary}" receipt verify --receipt "${receipt_100k}"
jq -e '
  .schema_version == "dmlpact.receipt-verification.v1"
  and .integrity_valid
  and .complete
  and .event_count == 2
  and .final_state == "committed"
' "${verify_100k_output}" >/dev/null

rows_after_100k="$(
  psql "${DMLPACT_DATABASE_URL}" -v ON_ERROR_STOP=1 -Atc \
    'SELECT count(*) FROM dmlpact_benchmark WHERE benchmark_value = 2'
)"
test "${rows_after_100k}" = "100000"

"${binary}" plan \
  --sql "${sql_lock}" \
  --out "${plan_lock}" \
  --max-rows 1 \
  --statement-timeout 30s \
  --lock-timeout 250ms \
  --valid-for 15m \
  --allow-insecure-localhost >"${temp_dir}/lock-plan.json"

psql "${DMLPACT_DATABASE_URL}" -v ON_ERROR_STOP=1 \
  -c 'BEGIN; LOCK TABLE dmlpact_benchmark IN ACCESS EXCLUSIVE MODE; SELECT pg_sleep(3); COMMIT;' \
  >"${temp_dir}/lock-holder.log" 2>&1 &
lock_holder_pid="$!"

lock_acquired=false
for _attempt in $(seq 1 100); do
  held_locks="$(
    psql "${DMLPACT_DATABASE_URL}" -v ON_ERROR_STOP=1 -Atc "
      SELECT count(*)
      FROM pg_locks AS locks
      JOIN pg_class AS relation ON relation.oid = locks.relation
      WHERE relation.relname = 'dmlpact_benchmark'
        AND locks.mode = 'AccessExclusiveLock'
        AND locks.granted
    "
  )"
  if test "${held_locks}" = "1"; then
    lock_acquired=true
    break
  fi
  sleep 0.02
done
test "${lock_acquired}" = "true"

measure "${lock_apply_metrics}" "${lock_apply_output}" "${lock_apply_diagnostics}" \
  "${binary}" apply \
  --sql "${sql_lock}" \
  --plan "${plan_lock}" \
  --receipt "${receipt_lock}" \
  --allow-insecure-localhost
jq -e '
  .schema_version == "dmlpact.apply-result.v1"
  and .state == "refused"
  and .affected_rows == null
  and .reason_code == "table_lock_failed"
' "${lock_apply_output}" >/dev/null
"${binary}" receipt verify --receipt "${receipt_lock}" >"${lock_verify_output}"
jq -e '
  .integrity_valid
  and .complete
  and .event_count == 2
  and .final_state == "refused"
' "${lock_verify_output}" >/dev/null

wait "${lock_holder_pid}"
lock_holder_pid=""
lock_target_value="$(
  psql "${DMLPACT_DATABASE_URL}" -v ON_ERROR_STOP=1 -Atc \
    'SELECT benchmark_value FROM dmlpact_benchmark WHERE id = 1'
)"
test "${lock_target_value}" = "2"

mkdir -p "$(dirname "${result_path}")"
jq -n \
  --arg generated_at "$(date -u +'%Y-%m-%dT%H:%M:%SZ')" \
  --arg git_sha "$(git rev-parse HEAD)" \
  --arg runner_os "${RUNNER_OS:-Linux}" \
  --arg runner_arch "$(uname -m)" \
  --arg runner_image "${ImageOS:-unknown}" \
  --arg runner_image_version "${ImageVersion:-unknown}" \
  --arg postgres_image "${DMLPACT_POSTGRES_IMAGE:-unknown}" \
  --argjson rows_after_10k "${rows_after_10k}" \
  --argjson untouched_after_10k "${untouched_after_10k}" \
  --argjson rows_after_100k "${rows_after_100k}" \
  --argjson lock_target_value "${lock_target_value}" \
  --argjson sql_10k_bytes "$(stat -c '%s' "${sql_10k}")" \
  --argjson sql_100k_bytes "$(stat -c '%s' "${sql_100k}")" \
  --argjson schema_output_bytes "$(stat -c '%s' "${schema_output}")" \
  --argjson plan_10k_bytes "$(stat -c '%s' "${plan_10k}")" \
  --argjson receipt_10k_bytes "$(stat -c '%s' "${receipt_10k}")" \
  --argjson plan_100k_bytes "$(stat -c '%s' "${plan_100k}")" \
  --argjson receipt_100k_bytes "$(stat -c '%s' "${receipt_100k}")" \
  --argjson receipt_lock_bytes "$(stat -c '%s' "${receipt_lock}")" \
  --slurpfile schema_metrics "${schema_metrics}" \
  --slurpfile schema "${schema_output}" \
  --slurpfile plan_10k_metrics "${plan_10k_metrics}" \
  --slurpfile plan_10k "${plan_10k}" \
  --slurpfile apply_10k_metrics "${apply_10k_metrics}" \
  --slurpfile apply_10k "${apply_10k_output}" \
  --slurpfile verify_10k_metrics "${verify_10k_metrics}" \
  --slurpfile verify_10k "${verify_10k_output}" \
  --slurpfile plan_100k_metrics "${plan_100k_metrics}" \
  --slurpfile plan_100k "${plan_100k}" \
  --slurpfile apply_100k_metrics "${apply_100k_metrics}" \
  --slurpfile apply_100k "${apply_100k_output}" \
  --slurpfile verify_100k_metrics "${verify_100k_metrics}" \
  --slurpfile verify_100k "${verify_100k_output}" \
  --slurpfile lock_apply_metrics "${lock_apply_metrics}" \
  --slurpfile lock_apply "${lock_apply_output}" \
  --slurpfile lock_verify "${lock_verify_output}" \
  '{
    schema_version: "dmlpact.benchmark.v1",
    generated_at: $generated_at,
    git_sha: $git_sha,
    runner: {
      os: $runner_os,
      arch: $runner_arch,
      image: $runner_image,
      image_version: $runner_image_version
    },
    database_fixture: {
      image: $postgres_image,
      identity: $plan_10k[0].preconditions.database,
      transport_policy: $plan_10k[0].transport_policy,
      canonical_table: $plan_10k[0].preconditions.canonical_table,
      schema_sha256: $plan_10k[0].preconditions.table_schema_sha256,
      total_rows: 100000,
      selected_rows: 10000
    },
    measurements: [
      {
        id: "contract_plan_schema",
        class: "offline",
        process: $schema_metrics[0],
        output_bytes: $schema_output_bytes,
        result: {
          draft: $schema[0]."$schema",
          title: $schema[0].title
        }
      },
      {
        id: "plan_10k_targets",
        class: "live_read_only",
        process: $plan_10k_metrics[0],
        sql_bytes: $sql_10k_bytes,
        plan_bytes: $plan_10k_bytes,
        result: {
          schema_version: $plan_10k[0].schema_version,
          target_count: $plan_10k[0].preconditions.target_count,
          target_set_sha256: $plan_10k[0].preconditions.target_set_sha256,
          max_rows: $plan_10k[0].limits.max_rows,
          plan_sha256: $plan_10k[0].plan_sha256
        }
      },
      {
        id: "apply_10k_targets",
        class: "live_mutation",
        scope: "locked revalidation plus server mutation execution",
        process: $apply_10k_metrics[0],
        receipt_bytes: $receipt_10k_bytes,
        result: {
          schema_version: $apply_10k[0].schema_version,
          state: $apply_10k[0].state,
          affected_rows: $apply_10k[0].affected_rows,
          reason_code: $apply_10k[0].reason_code,
          rows_changed: $rows_after_10k,
          rows_outside_target_unchanged: $untouched_after_10k
        }
      },
      {
        id: "verify_10k_receipt",
        class: "offline",
        process: $verify_10k_metrics[0],
        result: {
          schema_version: $verify_10k[0].schema_version,
          integrity_valid: $verify_10k[0].integrity_valid,
          complete: $verify_10k[0].complete,
          event_count: $verify_10k[0].event_count,
          final_state: $verify_10k[0].final_state
        }
      },
      {
        id: "plan_100k_targets",
        class: "live_read_only",
        process: $plan_100k_metrics[0],
        sql_bytes: $sql_100k_bytes,
        plan_bytes: $plan_100k_bytes,
        result: {
          schema_version: $plan_100k[0].schema_version,
          target_count: $plan_100k[0].preconditions.target_count,
          target_set_sha256: $plan_100k[0].preconditions.target_set_sha256,
          max_rows: $plan_100k[0].limits.max_rows,
          plan_sha256: $plan_100k[0].plan_sha256
        }
      },
      {
        id: "apply_100k_targets",
        class: "live_mutation",
        scope: "locked revalidation plus server mutation execution",
        process: $apply_100k_metrics[0],
        receipt_bytes: $receipt_100k_bytes,
        result: {
          schema_version: $apply_100k[0].schema_version,
          state: $apply_100k[0].state,
          affected_rows: $apply_100k[0].affected_rows,
          reason_code: $apply_100k[0].reason_code,
          rows_changed: $rows_after_100k
        }
      },
      {
        id: "verify_100k_receipt",
        class: "offline",
        process: $verify_100k_metrics[0],
        result: {
          schema_version: $verify_100k[0].schema_version,
          integrity_valid: $verify_100k[0].integrity_valid,
          complete: $verify_100k[0].complete,
          event_count: $verify_100k[0].event_count,
          final_state: $verify_100k[0].final_state
        }
      },
      {
        id: "apply_lock_contention_refusal",
        class: "live_refusal",
        process: $lock_apply_metrics[0],
        receipt_bytes: $receipt_lock_bytes,
        configured_lock_timeout_ms: 250,
        result: {
          schema_version: $lock_apply[0].schema_version,
          state: $lock_apply[0].state,
          reason_code: $lock_apply[0].reason_code,
          receipt_integrity_valid: $lock_verify[0].integrity_valid,
          receipt_complete: $lock_verify[0].complete,
          target_value_after_refusal: $lock_target_value
        }
      }
    ],
    derived: {
      max_peak_rss_mib:
        ([
          $schema_metrics[0].max_rss_kib,
          $plan_10k_metrics[0].max_rss_kib,
          $apply_10k_metrics[0].max_rss_kib,
          $verify_10k_metrics[0].max_rss_kib,
          $plan_100k_metrics[0].max_rss_kib,
          $apply_100k_metrics[0].max_rss_kib,
          $verify_100k_metrics[0].max_rss_kib,
          $lock_apply_metrics[0].max_rss_kib
        ] | max | . / 1024),
      workflow_10k_wall_seconds:
        ($plan_10k_metrics[0].wall_seconds + $apply_10k_metrics[0].wall_seconds),
      workflow_100k_wall_seconds:
        ($plan_100k_metrics[0].wall_seconds + $apply_100k_metrics[0].wall_seconds)
    },
    threshold_status: "raw_sample"
  }' >"${result_path}"

jq -e '
  .schema_version == "dmlpact.benchmark.v1"
  and .database_fixture.total_rows == 100000
  and .database_fixture.selected_rows == 10000
  and all(
    .measurements[];
    .process.exit_code == 0
      and .process.wall_seconds >= 0
      and .process.max_rss_kib > 0
  )
  and any(
    .measurements[];
    .id == "plan_10k_targets"
      and .result.target_count == 10000
      and .result.max_rows == 10000
  )
  and any(
    .measurements[];
    .id == "apply_10k_targets"
      and .result.state == "committed"
      and .result.affected_rows == 10000
      and .result.rows_changed == 10000
      and .result.rows_outside_target_unchanged == 90000
  )
  and any(
    .measurements[];
    .id == "plan_100k_targets"
      and .result.target_count == 100000
      and .result.max_rows == 100000
  )
  and any(
    .measurements[];
    .id == "apply_100k_targets"
      and .result.state == "committed"
      and .result.affected_rows == 100000
      and .result.rows_changed == 100000
  )
  and all(
    .measurements[]
      | select(.id == "verify_10k_receipt" or .id == "verify_100k_receipt");
    .result.integrity_valid
      and .result.complete
      and .result.event_count == 2
      and .result.final_state == "committed"
  )
  and any(
    .measurements[];
    .id == "apply_lock_contention_refusal"
      and .configured_lock_timeout_ms == 250
      and .result.state == "refused"
      and .result.reason_code == "table_lock_failed"
      and .result.receipt_integrity_valid
      and .result.receipt_complete
      and .result.target_value_after_refusal == 2
  )
' "${result_path}" >/dev/null

printf 'wrote %s\n' "${result_path}"
