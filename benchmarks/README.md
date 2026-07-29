# DMLPact performance baseline

This directory defines and enforces DMLPact's reproducible PostgreSQL v1.0
performance and resource thresholds on pull requests and in the weekly
scheduled benchmark.

## Workloads

`setup.sql` creates a dedicated 100,000-row synthetic table with a primary key,
10,000 selected rows, and no triggers, rewrite rules, row-level security, or
referential actions. The harness measures fresh CLI processes for:

- plan, apply, and offline receipt verification over exactly 10,000 targets;
- plan, apply, and offline receipt verification over exactly 100,000 targets;
- a 250 ms lock-timeout refusal while another session holds an
  `ACCESS EXCLUSIVE` table lock;
- complete plan JSON Schema generation.

Committed runs must change exactly the fingerprinted target count, leave the
90,000 non-target rows unchanged in the 10k case, produce a complete
integrity-valid two-event receipt, and match the expected final database state.
The contention run must refuse without mutation and also produce a complete
integrity-valid receipt.

The raw result records GNU `time` wall time and peak resident memory, SQL, plan,
and receipt bytes, target-set and artifact digests, database schema and identity,
PostgreSQL and runner images, affected-row and post-state checks, lock timeout,
and the exact DMLPact commit. Database setup and the release build are excluded.

Apply timing includes both DMLPact's locked client-side revalidation and server
mutation execution. It is an end-to-end upper bound, not an isolated claim
about client time. Enforcing the stronger end-to-end limit nevertheless proves
that client overhead alone cannot exceed that same limit.

Each sample performs untimed build and database reset. The workflow discards
one warm-up and captures 20 samples against the same digest-pinned PostgreSQL
image.

## Enforced thresholds

The versioned policy in `thresholds.json` enforces:

- 10,000-target plan generation below 5 seconds p95;
- locked revalidation plus server mutation below 2 seconds p95;
- peak RSS no greater than 256 MiB in every bounded sample, including the
  100,000-target workflow.

Twenty samples make nearest-rank p95 the second-slowest observation. Once
`baseline-ubuntu24.json` is present, metrics must also remain within the
stricter of the absolute limit and the versioned noise allowance: 1.5 times
baseline or baseline plus 100 ms for time and 16 MiB for memory.

## Run

The supported environment is the `ubuntu-24.04` x86_64 runner and digest-pinned
`postgres:17-alpine` service selected by `.github/workflows/benchmark.yml`. For
safety, `run.sh` refuses to reset any database whose current name is not exactly
`dmlpact_benchmark`.

Run manually with the **Benchmark** workflow, or against an expendable local
database:

```bash
export DMLPACT_DATABASE_URL='postgresql://postgres:postgres@127.0.0.1:5432/dmlpact_benchmark'
benchmarks/run.sh benchmark-results.json
jq . benchmark-results.json
```

Run evaluator tests with:

```bash
python3 -m unittest benchmarks/test_evaluate.py
```

GNU `time`, GNU `stat`, `timeout`, `psql`, `jq`, Git, Cargo, and the locked Rust
dependency graph are required. Plans and receipts are temporary and are not
uploaded.

The workflow uploads all 20 raw samples and the aggregate evaluation for 90
days, including raw samples from a failed threshold evaluation. The checked-in
baseline is refreshed only from a successful evaluation of the exact commit on
the fixed runner class.
