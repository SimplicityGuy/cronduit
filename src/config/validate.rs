use super::{Config, ConfigError, JobConfig};
use croner::Cron;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;

static NETWORK_RE: Lazy<Regex> = Lazy::new(|| {
    // bridge | host | none | container:<name> | <named>
    Regex::new(r"^(bridge|host|none|container:[a-zA-Z0-9_.-]+|[a-zA-Z0-9_.-]+)$").unwrap()
});

static LABEL_KEY_RE: Lazy<Regex> = Lazy::new(|| {
    // Strict ASCII: leading char alphanumeric or underscore; subsequent chars
    // alphanumeric, dot, hyphen, or underscore. Per CONTEXT D-02; mirrors the
    // once_cell idiom at validate.rs:10-13 and interpolate.rs:23-24.
    Regex::new(r"^[a-zA-Z0-9_][a-zA-Z0-9._-]*$").unwrap()
});

/// Phase 22 TAG-04: tag charset regex. Anchored leading alphanumeric +
/// 0-30 body chars from [a-z0-9_-]; total length 1-31. Pattern is
/// checked against the post-normalization (lowercase + trim) form per
/// D-04 step 2 — uppercase input is normalized first and would only
/// fail charset if it contained chars outside [a-z0-9_-] post-lowercase.
static TAG_CHARSET_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-z0-9][a-z0-9_-]{0,30}$").unwrap());

/// Phase 22 TAG-04: reserved tag names. Finite list (NOT a prefix —
/// operators can use `cronduit2` or `cronduit-foo` freely). Expansion
/// is a single-line edit if needed in v1.3.
const RESERVED_TAGS: &[&str] = &["cronduit", "system", "internal"];

/// Phase 22 D-08: hard cap on tag count per job (post-dedup).
/// Rationale: (a) Phase 23 chip UI on a single dashboard row stays
/// readable — 16 chips fits next to a job name; (b) operators rarely
/// need >16 organizational dimensions on a single job; (c) the cap can
/// be lifted later without a migration if it bites.
const MAX_TAGS_PER_JOB: usize = 16;

/// Canonical RunFinalized status values per src/scheduler/run.rs:315-322.
/// Used by `check_webhook_block_completeness` for the operator's `webhook.states` filter.
const VALID_WEBHOOK_STATES: &[&str] = &[
    "success",
    "failed",
    "timeout",
    "stopped",
    "cancelled",
    "error",
];

/// Maximum byte length for an individual label value (LBL-06).
/// Aligns with Docker's documented label-value convention.
const MAX_LABEL_VALUE_BYTES: usize = 4 * 1024; // 4 KB

/// Maximum total byte length of all keys + values for a single job's labels (LBL-06).
/// Cronduit-side; well below dockerd's informal ~250 KB limit so operators see a
/// clear cronduit error at config-load instead of a confusing dockerd 400 at create.
const MAX_LABEL_SET_BYTES: usize = 32 * 1024; // 32 KB

/// Run every post-parse check; push errors into `errors`. Never fail-fast.
pub fn run_all_checks(cfg: &Config, path: &Path, raw: &str, errors: &mut Vec<ConfigError>) {
    check_timezone(&cfg.server.timezone, path, errors);
    check_bind(&cfg.server.bind, path, errors);
    check_duplicate_job_names(&cfg.jobs, path, raw, errors);
    for job in &cfg.jobs {
        check_one_of_job_type(job, path, errors);
        check_cmd_only_on_docker_jobs(job, path, errors);
        check_network_mode(job, path, errors);
        check_schedule(job, path, errors);
        // Phase 17 / SEED-001 — operator labels (LBL-03, LBL-04, LBL-06, D-02)
        check_label_reserved_namespace(job, path, errors);
        check_labels_only_on_docker_jobs(
            job,
            cfg.defaults.as_ref().and_then(|d| d.labels.as_ref()),
            path,
            errors,
        );
        check_label_size_limits(job, path, errors);
        check_label_key_chars(job, path, errors);
        // Phase 18 / WH-01 — webhook validators.
        check_webhook_url(job, path, errors);
        check_webhook_block_completeness(job, path, errors);
        // Phase 22 / TAG-* — D-04 order: normalize → reject (charset + reserved
        // + empty) → dedup with WARN (folded into check_tag_charset_and_reserved).
        check_tag_charset_and_reserved(job, path, errors);
        check_tag_count_per_job(job, path, errors); // Phase 22 / D-08
    }
    // Phase 22 / TAG-05 — fleet-level substring-collision pass (D-03).
    // MUST run AFTER the per-job loop so the tag set has already been
    // normalized + dedup'd per-job (D-04 lock).
    check_tag_substring_collision(&cfg.jobs, path, errors);
}

fn check_timezone(tz: &str, path: &Path, errors: &mut Vec<ConfigError>) {
    if tz.parse::<chrono_tz::Tz>().is_err() {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!("not a valid IANA timezone: `{tz}` (see [server].timezone)"),
        });
    }
}

fn check_bind(bind: &str, path: &Path, errors: &mut Vec<ConfigError>) {
    if SocketAddr::from_str(bind).is_err() {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!("[server].bind is not a valid socket address: `{bind}`"),
        });
    }
}

fn check_one_of_job_type(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    let count =
        job.command.is_some() as u8 + job.script.is_some() as u8 + job.image.is_some() as u8;
    if count != 1 {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}` must declare exactly one of `command`, `script`, or `image` (found {count}). Note: `image` may also come from `[defaults].image` unless the job sets `use_defaults = false`.",
                job.name
            ),
        });
    }
}

fn check_network_mode(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    if let Some(net) = &job.network
        && !NETWORK_RE.is_match(net)
    {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: invalid network mode `{net}` (expected bridge|host|none|container:<name>|<named-network>)",
                job.name
            ),
        });
    }
}

/// Reject `cmd` on command/script jobs. `cmd` is a Docker container CMD override
/// with no meaningful analog for command or script jobs (no container to receive
/// it). Runs AFTER `apply_defaults`, so the merged view of the job is what we
/// inspect — for command/script jobs the docker-only fields (`image`/`network`/
/// `volumes`/`delete`) are intentionally not merged by `apply_defaults`, so an
/// `image.is_none()` test reliably distinguishes "this is a non-docker job" from
/// "this is a docker job inheriting its image from `[defaults]`".
fn check_cmd_only_on_docker_jobs(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    if job.cmd.is_some() && job.image.is_none() {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: `cmd` is only valid on docker jobs (job with `image = \"...\"` set either directly or via `[defaults].image`); command and script jobs cannot set `cmd` because there is no container to receive it. Remove the `cmd` line, or switch the job to a docker job by setting `image`.",
                job.name
            ),
        });
    }
}

fn check_schedule(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    use crate::scheduler::random::is_random_schedule;

    // Schedules containing @random tokens are resolved at sync time, not here.
    // Validate only the non-@random fields by substituting @random with valid
    // stand-in values per field position (minute=0, hour=0, dom=1, month=1, dow=0).
    const RANDOM_FALLBACKS: [&str; 5] = ["0", "0", "1", "1", "0"];
    let schedule_to_validate = if is_random_schedule(&job.schedule) {
        job.schedule
            .split_whitespace()
            .enumerate()
            .map(|(i, f)| {
                if f == "@random" {
                    RANDOM_FALLBACKS.get(i).copied().unwrap_or("0")
                } else {
                    f
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        job.schedule.clone()
    };

    if let Err(e) = schedule_to_validate.parse::<Cron>() {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: invalid cron expression `{}`: {}",
                job.name, job.schedule, e
            ),
        });
    }
}

/// LBL-03: reject operator labels under the reserved `cronduit.*` namespace.
/// The cronduit.* prefix is reserved for cronduit-internal labels (currently
/// cronduit.run_id, cronduit.job_name; consumed by docker_orphan reconciliation
/// at src/scheduler/docker_orphan.rs:31). Sorting the offending-key list is
/// CRITICAL — HashMap iteration is non-deterministic (see RESEARCH Pitfall 2).
fn check_label_reserved_namespace(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    let Some(labels) = &job.labels else { return };
    let mut offending: Vec<&str> = labels
        .keys()
        .filter(|k| k.starts_with("cronduit."))
        .map(String::as_str)
        .collect();
    if offending.is_empty() {
        return;
    }
    offending.sort(); // determinism — HashMap iter order is random
    errors.push(ConfigError {
        file: path.into(),
        line: 0,
        col: 0,
        message: format!(
            "[[jobs]] `{}`: labels under reserved namespace `cronduit.*` are not allowed: {}. Remove these keys; the cronduit.* prefix is reserved for cronduit-internal labels.",
            job.name,
            offending.join(", ")
        ),
    });
}

/// LBL-04: reject `labels = ...` on non-docker (command/script) jobs.
/// Mirrors check_cmd_only_on_docker_jobs (validate.rs:89).
///
/// IMPORTANT: this function runs AFTER `apply_defaults`, so `job.labels`
/// already contains the merged set (defaults + per-job). To distinguish
/// the operator-set case (operator wrote `labels = {...}` on a command
/// job) from the defaults-only case (operator added `[defaults].labels`
/// and forgot `use_defaults = false` on a command job), this function
/// set-diffs against `defaults_labels` (the original `[defaults].labels`
/// map, which `apply_defaults` clones from but does not consume). The
/// pin contract is `lbl_04_error_does_not_leak_defaults_keys_for_non_docker_jobs`
/// at src/config/defaults.rs:447-509.
///
/// Two distinct error messages emerge:
///   * operator_only_keys non-empty → legacy message ("Remove the
///     `labels` block...") — backwards-compatible with existing CI grep
///     and operator scripts.
///   * operator_only_keys empty + defaults_labels non-empty → new
///     message ("set `use_defaults = false` on this job to opt out...")
///     — names the actual fix the example file's comments teach.
fn check_labels_only_on_docker_jobs(
    job: &JobConfig,
    defaults_labels: Option<&HashMap<String, String>>,
    path: &Path,
    errors: &mut Vec<ConfigError>,
) {
    // Fast paths: no labels on this job, or this is a docker job (image set).
    let Some(job_labels) = &job.labels else {
        return;
    };
    if job.image.is_some() {
        return;
    }

    // Set-diff: operator-only keys = job.labels.keys() - defaults_labels.keys()
    // BTreeSet for deterministic ordering (RESEARCH Pitfall 2 — HashMap iter
    // order is random; even though we don't currently emit these keys in the
    // error message, keeping the structure deterministic future-proofs any
    // downstream change that does want to enumerate them).
    let operator_only_keys: std::collections::BTreeSet<&str> = match defaults_labels {
        Some(d) => job_labels
            .keys()
            .filter(|k| !d.contains_key(k.as_str()))
            .map(String::as_str)
            .collect(),
        None => job_labels.keys().map(String::as_str).collect(),
    };

    if !operator_only_keys.is_empty() {
        // Branch A — operator-set case. Preserve the legacy message text
        // verbatim for backwards compat (existing tests/scripts grep for
        // "Remove the `labels` block").
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: `labels` is only valid on docker jobs (job with `image = \"...\"` set either directly or via `[defaults].image`); command and script jobs cannot set `labels` because there is no container to attach them to. Remove the `labels` block, or switch the job to a docker job by setting `image`.",
                job.name
            ),
        });
    } else if defaults_labels.is_some_and(|d| !d.is_empty()) {
        // Branch B — defaults-only case. Operator did not write
        // `labels = {...}` on this job; the merged labels came from
        // `[defaults].labels` via apply_defaults. Tell the operator the
        // actual fix is `use_defaults = false`.
        let job_type = if job.command.is_some() {
            "command"
        } else if job.script.is_some() {
            "script"
        } else {
            "command/script"
        };
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: this is a {} job; labels are docker-only. `[defaults].labels` is set and would be merged into this job by `apply_defaults` — set `use_defaults = false` on this job to opt out, OR remove `[defaults].labels`.",
                job.name, job_type
            ),
        });
    }
    // No-op when both operator_only_keys is empty AND defaults_labels is None
    // (or empty) — that is structurally impossible given the fast paths above
    // (job.labels is Some, so either operator set keys or defaults supplied them).
}

/// LBL-06: enforce per-value (4 KB) and per-set (32 KB) byte-length limits.
/// Two independent checks may both fire for one job (per D-01 aggregation).
fn check_label_size_limits(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    let Some(labels) = &job.labels else { return };

    // Per-value check
    let mut oversized_keys: Vec<&str> = labels
        .iter()
        .filter(|(_, v)| v.len() > MAX_LABEL_VALUE_BYTES)
        .map(|(k, _)| k.as_str())
        .collect();
    if !oversized_keys.is_empty() {
        oversized_keys.sort();
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: label values exceed 4 KB limit: {}. Each label value must be ≤ {} bytes.",
                job.name,
                oversized_keys.join(", "),
                MAX_LABEL_VALUE_BYTES
            ),
        });
    }

    // Per-job total check
    let total_bytes: usize = labels.iter().map(|(k, v)| k.len() + v.len()).sum();
    if total_bytes > MAX_LABEL_SET_BYTES {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: total label-set size {} bytes exceeds 32 KB limit. Sum of all key+value byte lengths must be ≤ {} bytes.",
                job.name, total_bytes, MAX_LABEL_SET_BYTES
            ),
        });
    }
}

/// D-02: enforce strict ASCII char regex on label keys at config-LOAD.
///
/// Rejects label keys whose characters fall outside `^[a-zA-Z0-9_][a-zA-Z0-9._-]*$`
/// (alphanumeric or underscore start; alphanumeric, dot, hyphen, or underscore body).
/// This runs AFTER the whole-file textual interpolation pass in
/// `src/config/interpolate.rs::interpolate`, so the keys this function sees are
/// already post-interpolation. The interpolation pass does NOT distinguish key
/// positions from value positions — it operates on raw TOML text — so this
/// validator effectively enforces the D-02 character convention on the resolved
/// key string. Two cases:
///
///   * env var SET in key position (e.g. `labels = { "${TEAM}" = "v" }` with
///     `TEAM=ops`): the key resolves to `ops` BEFORE this function runs; this
///     function sees `ops`, which passes — by design, per D-02. Operators who
///     want stable label keys should write literal strings, not `${VAR}`
///     references; see README § Configuration > Labels > Env-var interpolation.
///   * env var UNSET in key position: the literal `${TEAM}` survives the
///     interpolation pass and is rejected here by the strict char regex (the
///     `$`, `{`, `}` characters are not in the allowed set).
///
/// Sort the offending-key list before format (RESEARCH Pitfall 2 — HashMap
/// iteration is non-deterministic).
fn check_label_key_chars(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    let Some(labels) = &job.labels else { return };
    let mut invalid: Vec<&str> = labels
        .keys()
        .filter(|k| !LABEL_KEY_RE.is_match(k))
        .map(String::as_str)
        .collect();
    if invalid.is_empty() {
        return;
    }
    invalid.sort();
    errors.push(ConfigError {
        file: path.into(),
        line: 0,
        col: 0,
        message: format!(
            "[[jobs]] `{}`: invalid label keys: {}. Keys must match the pattern `^[a-zA-Z0-9_][a-zA-Z0-9._-]*$` (alphanumeric or underscore start; alphanumeric, dot, hyphen, or underscore body). Note: env-var ${{...}} interpolation runs only on label VALUES, not keys.",
            job.name,
            invalid.join(", ")
        ),
    });
}

/// Phase 22 TAG-03 + TAG-04: per-job tag validation.
///
/// D-04 order:
/// 1. Normalize: `tags.iter().map(|t| t.trim().to_lowercase())`.
/// 2. Reject empty/whitespace-only inputs (Pitfall 6 — never silently drop).
/// 3. Reject charset violations (`^[a-z0-9][a-z0-9_-]{0,30}$`) on the
///    post-normalization form. Capital input lowercases and passes.
/// 4. Reject reserved names (`cronduit`, `system`, `internal`) on the
///    post-normalization form (so `Cronduit` is also rejected).
/// 5. Emit `tracing::warn!` line if normalization caused a dedup collapse
///    (TAG-03 WARN flags it; never blocks). Operator sees:
///    `WARN job '<name>': tags ["Backup", "backup ", "BACKUP"] collapsed to ["backup"] (case + whitespace normalization)`.
///
/// HashMap/iteration determinism: every offending list is `.sort()`ed
/// before `.join(", ")` per Pitfall 3 (`validate.rs:184` precedent).
fn check_tag_charset_and_reserved(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    if job.tags.is_empty() {
        return;
    }

    // Step 1: normalize (trim + lowercase). Preserve raw alongside for the
    // empty-after-trim diagnostic and the WARN line.
    let normalized: Vec<String> = job.tags.iter().map(|t| t.trim().to_lowercase()).collect();

    // Step 2: empty/whitespace-only (Pitfall 6) — uses raw inputs to keep
    // the message honest about what the operator wrote.
    let mut empty_after_trim: Vec<String> = job
        .tags
        .iter()
        .filter(|t| t.trim().is_empty())
        .cloned()
        .collect();
    empty_after_trim.sort();
    empty_after_trim.dedup();
    if !empty_after_trim.is_empty() {
        let display: Vec<String> = empty_after_trim.iter().map(|t| format!("{:?}", t)).collect();
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: empty or whitespace-only tags are not allowed: {}. Remove the empty entries.",
                job.name,
                display.join(", ")
            ),
        });
    }

    // Step 3: charset (post-normalization). Skip already-empty entries —
    // they are reported by Step 2.
    let mut bad_charset: Vec<String> = normalized
        .iter()
        .filter(|t| !t.is_empty() && !TAG_CHARSET_RE.is_match(t))
        .cloned()
        .collect();
    bad_charset.sort();
    bad_charset.dedup();
    if !bad_charset.is_empty() {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: tags fail charset `^[a-z0-9][a-z0-9_-]{{0,30}}$` (lowercase ASCII alphanumeric + underscore + dash; 1-31 chars; must start with [a-z0-9]): {}. Rename or remove these tags.",
                job.name,
                bad_charset
                    .iter()
                    .map(|t| format!("`{}`", t))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        });
    }

    // Step 4: reserved names (post-normalization).
    let mut reserved_hits: Vec<String> = normalized
        .iter()
        .filter(|t| RESERVED_TAGS.contains(&t.as_str()))
        .cloned()
        .collect();
    reserved_hits.sort();
    reserved_hits.dedup();
    if !reserved_hits.is_empty() {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: reserved tag names not allowed: {}. Reserved list is {:?}; pick a different tag.",
                job.name,
                reserved_hits
                    .iter()
                    .map(|t| format!("`{}`", t))
                    .collect::<Vec<_>>()
                    .join(", "),
                RESERVED_TAGS
            ),
        });
    }

    // Step 5: dedup-collapse WARN (TAG-03). Group raw inputs by canonical form;
    // any group with len > 1 means the operator wrote N inputs that collapsed
    // to 1. Skip groups whose canonical is empty (those are already errors).
    let mut groups: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for (raw_t, norm_t) in job.tags.iter().zip(normalized.iter()) {
        if norm_t.is_empty() {
            continue;
        }
        groups
            .entry(norm_t.clone())
            .or_default()
            .push(raw_t.clone());
    }
    let mut collapses: Vec<(String, Vec<String>)> = groups
        .into_iter()
        .filter(|(_, raws)| raws.len() > 1)
        .collect();
    collapses.sort_by(|a, b| a.0.cmp(&b.0));
    if !collapses.is_empty() {
        for (canon, raws) in &collapses {
            tracing::warn!(
                job = %job.name,
                inputs = ?raws,
                canonical = %canon,
                "job '{}': tags {:?} collapsed to [{:?}] (case + whitespace normalization)",
                job.name,
                raws,
                canon
            );
        }
    }
}

/// Phase 22 D-08: per-job count cap of 16 enforced on the POST-dedup
/// tag count. D-04 step 4 lock: operates on what the operator
/// effectively meant after normalization, not the raw input length.
fn check_tag_count_per_job(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    if job.tags.is_empty() {
        return;
    }
    // Same normalization as check_tag_charset_and_reserved; could be
    // factored into a shared `normalize_tags` helper but inlining keeps
    // each validator self-contained and the cost is microseconds.
    let mut normalized: Vec<String> = job
        .tags
        .iter()
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty())
        .collect();
    normalized.sort();
    normalized.dedup();

    if normalized.len() > MAX_TAGS_PER_JOB {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: has {} tags; max is {}. Remove tags or split into multiple jobs.",
                job.name,
                normalized.len(),
                MAX_TAGS_PER_JOB
            ),
        });
    }
}

/// Phase 22 D-03: format up to 3 representative job names for the
/// substring-collision error message body. Beyond 3, append `(+N more)`
/// to keep the error scannable when many jobs share an offending tag.
fn preview_jobs(jobs: &[String]) -> String {
    if jobs.len() <= 3 {
        format!("'{}'", jobs.join("', '"))
    } else {
        format!(
            "'{}', '{}', '{}' (+{} more)",
            jobs[0],
            jobs[1],
            jobs[2],
            jobs.len() - 3
        )
    }
}

/// Phase 22 TAG-05 / D-03: fleet-level substring-collision check.
/// Operates on the post-normalization, post-dedup tag set across the
/// WHOLE fleet (after the per-job loop completes). For each pair of
/// distinct tags where one is a substring of the other (`s1.contains(s2)`
/// where `s1 != s2`), emit ONE `ConfigError` naming both tags + the
/// jobs that use each.
///
/// Plain `str::contains` (NOT regex) per CONTEXT specifics § L530-537.
/// Identical tags across jobs (job A and B both have `backup`) are NOT
/// collisions — `BTreeSet` uniqueness + `i < j` iteration skips equal
/// pairs.
///
/// Complexity: O(T^2) where T = unique tag count across fleet. At
/// homelab scale (≤16 jobs × ≤16 tags = 256 max), microseconds.
fn check_tag_substring_collision(
    jobs: &[JobConfig],
    path: &Path,
    errors: &mut Vec<ConfigError>,
) {
    use std::collections::{BTreeSet, HashMap};

    // Step 1: build {tag → [jobs_using_it]} from the POST-normalization
    // view. BTreeSet<String> per job for inner uniqueness.
    let mut tag_to_jobs: HashMap<String, Vec<String>> = HashMap::new();
    for job in jobs {
        let normalized: BTreeSet<String> = job
            .tags
            .iter()
            .map(|t| t.trim().to_lowercase())
            .filter(|t| !t.is_empty()) // already rejected by Step 2 of charset validator
            .collect();
        for tag in normalized {
            tag_to_jobs.entry(tag).or_default().push(job.name.clone());
        }
    }
    // Stable sort of jobs per tag (for the `(+N more)` listing determinism).
    for v in tag_to_jobs.values_mut() {
        v.sort();
        v.dedup();
    }

    // Step 2: collect sorted unique tags for deterministic pair iteration.
    let tags: Vec<&String> = {
        let mut v: Vec<&String> = tag_to_jobs.keys().collect();
        v.sort();
        v
    };

    // Step 3: O(n^2) pair check with i < j (skips equal pairs and avoids
    // double-counting symmetric collisions).
    for i in 0..tags.len() {
        for j in (i + 1)..tags.len() {
            let a = tags[i];
            let b = tags[j];
            // Equal tags impossible here (BTreeSet uniqueness + i < j).
            if a.contains(b.as_str()) || b.contains(a.as_str()) {
                let jobs_a = &tag_to_jobs[a];
                let jobs_b = &tag_to_jobs[b];
                let preview_a = preview_jobs(jobs_a);
                let preview_b = preview_jobs(jobs_b);
                // Order the message so the SHORTER tag appears first in the
                // "tag '...' is a substring of '...'" phrasing, which reads
                // most naturally to the operator. When lengths are equal but
                // strings differ (impossible since equal-length distinct
                // strings can't be substrings of each other), default to (a, b).
                let (short_t, short_jobs, long_t, long_jobs) = if a.len() <= b.len() {
                    (a, &preview_a, b, &preview_b)
                } else {
                    (b, &preview_b, a, &preview_a)
                };
                errors.push(ConfigError {
                    file: path.into(),
                    line: 0,
                    col: 0,
                    message: format!(
                        "tag '{}' (used by {}) is a substring of '{}' (used by {}); rename or remove one to avoid SQL substring false-positives at filter time.",
                        short_t, short_jobs, long_t, long_jobs
                    ),
                });
            }
        }
    }
}

/// Phase 20 / WH-07 (D-19): Classify an HTTP webhook URL's host against the
/// locked loopback + RFC1918 + ULA allowlist. Returns `Ok(class_str)` for
/// allowed destinations (used for the boot-time INFO log) or `Err(())` for
/// rejected. NO DNS resolution at LOAD time (D-20) — hostnames that are not
/// literal `localhost` and not parseable as `IpAddr` are rejected.
///
/// Pitfall 4 (RESEARCH §8): we use `url::Url::host()` returning `Host<&str>`
/// so IPv6 literals are matched as `Host::Ipv6(_)` directly — NOT
/// `host_str().parse::<IpAddr>()`, which round-trips through brackets.
fn classify_http_destination(url: &url::Url) -> Result<&'static str, ()> {
    match url.host() {
        Some(url::Host::Ipv4(v4)) => {
            if v4.is_loopback() {
                Ok("loopback")
            } else if v4.is_private() {
                Ok("RFC1918")
            } else {
                Err(())
            }
        }
        Some(url::Host::Ipv6(v6)) => {
            if v6.is_loopback() {
                Ok("loopback")
            } else if v6.is_unique_local() {
                // RESEARCH §4.1: stdlib `is_unique_local` covers RFC 4193
                // `fc00::/7` (broader than the success-criterion-literal
                // `fd00::/8`). The broader range NEVER rejects a spec-allowed
                // address; we accept the broader range and keep the operator
                // error message citing `fd00::/8` for clarity.
                Ok("ULA")
            } else {
                Err(())
            }
        }
        Some(url::Host::Domain(name)) => {
            // No DNS resolution (D-20). Only the literal hostname `localhost`
            // is accepted via this arm; everything else is rejected at LOAD.
            if name.eq_ignore_ascii_case("localhost") {
                Ok("localhost")
            } else {
                Err(())
            }
        }
        None => Err(()), // unreachable for parsed http(s) URLs
    }
}

/// WH-01 / D-04 — verify `webhook.url` parses and uses an allowed scheme.
/// Phase 20 / WH-07 (D-19): also enforces HTTPS-for-non-loopback narrowing.
/// HTTP is permitted only for `127/8`, `::1`, `10/8`, `172.16/12`,
/// `192.168/16`, and ULA (`fd00::/8` per spec; `fc00::/7` per stdlib helper —
/// see RESEARCH §4.1) plus the literal hostname `localhost`. HTTPS is always
/// accepted (silent, no log). HTTP-allowed paths emit a boot-time INFO log
/// naming the URL + classified-net so operators see the validator's call.
fn check_webhook_url(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    let Some(wh) = &job.webhook else {
        return;
    };
    match url::Url::parse(&wh.url) {
        Err(e) => {
            errors.push(ConfigError {
                file: path.into(),
                line: 0,
                col: 0,
                message: format!(
                    "[[jobs]] `{}`: webhook.url `{}` is not a valid URL: {}. \
                     Provide a fully-qualified URL like `https://hook.example.com/path`.",
                    job.name, wh.url, e
                ),
            });
        }
        Ok(parsed) => {
            let scheme = parsed.scheme();
            if scheme != "http" && scheme != "https" {
                errors.push(ConfigError {
                    file: path.into(),
                    line: 0,
                    col: 0,
                    message: format!(
                        "[[jobs]] `{}`: webhook.url scheme `{}` is not supported \
                         (allowed: `http`, `https`).",
                        job.name, scheme
                    ),
                });
                return;
            }

            // Phase 20 / WH-07 (D-19): HTTPS required for non-loopback /
            // non-RFC1918 destinations. HTTPS path: silent acceptance, no log.
            if scheme == "http" {
                match classify_http_destination(&parsed) {
                    Ok(class) => {
                        // D-19 + RESEARCH §13.2: emit INFO log so operators
                        // see classification at LOAD time. Targeted to
                        // `cronduit.config` so operators can grep / filter.
                        tracing::info!(
                            target: "cronduit.config",
                            job = %job.name,
                            url = %wh.url,
                            classified_net = %class,
                            "webhook URL accepted on local net"
                        );
                    }
                    Err(()) => {
                        // D-21 verbatim error message:
                        errors.push(ConfigError {
                            file: path.into(),
                            line: 0,
                            col: 0,
                            message: format!(
                                "[[jobs]] `{}`: webhook.url `{}` requires HTTPS for non-loopback / non-RFC1918 destinations. \
                                 Use `https://` or one of the allowed local nets: 127/8, ::1, 10/8, 172.16/12, 192.168/16, fd00::/8.",
                                job.name, wh.url
                            ),
                        });
                    }
                }
            }
        }
    }
}

/// WH-01 / D-04 — verify the `webhook` block is internally consistent.
/// Combines five independent assertions (Phase 17 LBL precedent — D-01
/// aggregation in a single check function): non-empty + valid states,
/// `secret` xor `unsigned`, non-negative `fire_every`, non-empty resolved
/// secret (Pitfall H).
fn check_webhook_block_completeness(job: &JobConfig, path: &Path, errors: &mut Vec<ConfigError>) {
    use secrecy::ExposeSecret;
    let Some(wh) = &job.webhook else {
        return;
    };

    // 1. states non-empty.
    if wh.states.is_empty() {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: webhook.states is empty. Use absence of the \
                 `webhook` block to disable webhooks; `states = []` is meaningless.",
                job.name
            ),
        });
    }

    // 2. every state ∈ VALID_WEBHOOK_STATES (sorted offending list per Pitfall G).
    let mut invalid: Vec<&str> = wh
        .states
        .iter()
        .map(String::as_str)
        .filter(|s| !VALID_WEBHOOK_STATES.contains(s))
        .collect();
    if !invalid.is_empty() {
        invalid.sort(); // determinism — Phase 17 D-01 / Pitfall G
        invalid.dedup();
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: webhook.states contains unknown values: {}. \
                 Valid values: {}.",
                job.name,
                invalid.join(", "),
                VALID_WEBHOOK_STATES.join(", ")
            ),
        });
    }

    // 3. secret xor unsigned.
    let has_secret = wh.secret.is_some();
    if has_secret == wh.unsigned {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: if has_secret {
                format!(
                    "[[jobs]] `{}`: webhook block has both `secret` AND `unsigned = true`. \
                     Set `unsigned = true` to skip signing (omit `secret`), OR set `secret` \
                     to sign deliveries (omit `unsigned`).",
                    job.name
                )
            } else {
                format!(
                    "[[jobs]] `{}`: webhook block needs either `secret = \"${{ENV_VAR}}\"` \
                     (signed deliveries) OR `unsigned = true` (opt-in unsigned for receivers \
                     like Slack/Discord). Currently neither is set.",
                    job.name
                )
            },
        });
    }

    // 4. fire_every non-negative.
    if wh.fire_every < 0 {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: webhook.fire_every = {} is negative. \
                 Use 0 (always fire), 1 (first-of-stream — default), or N>1 (every Nth match).",
                job.name, wh.fire_every
            ),
        });
    }

    // 5. Pitfall H — empty resolved secret. The interpolate.rs pass produces
    //    MissingVar for unset env vars, but `${WEBHOOK_SECRET}=""` (set-but-empty)
    //    would silently sign HMACs with an empty key. Reject at LOAD time here.
    if let Some(secret) = &wh.secret
        && secret.expose_secret().is_empty()
    {
        errors.push(ConfigError {
            file: path.into(),
            line: 0,
            col: 0,
            message: format!(
                "[[jobs]] `{}`: webhook.secret resolved to an empty string. \
                 Set the source env var to a non-empty value (signing with an \
                 empty key produces signatures receivers will reject).",
                job.name
            ),
        });
    }
}

fn check_duplicate_job_names(
    jobs: &[JobConfig],
    path: &Path,
    raw: &str,
    errors: &mut Vec<ConfigError>,
) {
    // Find line numbers by scanning raw source for `name = "..."` matches in order.
    let mut first_seen: HashMap<&str, usize> = HashMap::new();
    let lines: Vec<&str> = raw.lines().collect();

    // Pre-compute (job_name, line_number) pairs from the raw text.
    let name_re = Regex::new(r#"^\s*name\s*=\s*"([^"]+)""#).unwrap();
    let mut occurrences: Vec<(String, usize)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if let Some(caps) = name_re.captures(line) {
            occurrences.push((caps[1].to_string(), i + 1));
        }
    }

    for job in jobs {
        let hits: Vec<usize> = occurrences
            .iter()
            .filter(|(n, _)| n == &job.name)
            .map(|(_, ln)| *ln)
            .collect();
        if hits.len() > 1 && !first_seen.contains_key(job.name.as_str()) {
            first_seen.insert(&job.name, hits[0]);
            for &dup_line in hits.iter().skip(1) {
                errors.push(ConfigError {
                    file: path.into(),
                    line: dup_line,
                    col: 1,
                    message: format!(
                        "duplicate job name `{}` (first declared at {}:{})",
                        job.name,
                        path.display(),
                        hits[0]
                    ),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iana_tz_accepted() {
        let mut e = Vec::new();
        check_timezone("America/Los_Angeles", Path::new("x"), &mut e);
        assert!(e.is_empty());
    }

    #[test]
    fn iana_tz_rejected() {
        let mut e = Vec::new();
        check_timezone("America/Los_Angles", Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
        assert!(e[0].message.contains("Los_Angles"));
    }

    #[test]
    fn network_mode_container_accepted() {
        assert!(NETWORK_RE.is_match("container:vpn"));
        assert!(NETWORK_RE.is_match("bridge"));
        assert!(NETWORK_RE.is_match("host"));
        assert!(NETWORK_RE.is_match("none"));
        assert!(NETWORK_RE.is_match("my_net"));
    }

    #[test]
    fn network_mode_whitespace_rejected() {
        assert!(!NETWORK_RE.is_match("container: vpn"));
        assert!(!NETWORK_RE.is_match(""));
    }

    fn stub_job(schedule: &str) -> JobConfig {
        JobConfig {
            name: "test-job".into(),
            schedule: schedule.into(),
            command: Some("echo hi".into()),
            script: None,
            image: None,
            use_defaults: None,
            env: Default::default(),
            volumes: None,
            labels: None,
            network: None,
            container_name: None,
            timeout: None,
            delete: None,
            cmd: None,
            tags: Vec::new(),
            webhook: None,
        }
    }

    #[test]
    fn schedule_valid_5field_accepted() {
        let mut e = Vec::new();
        check_schedule(&stub_job("*/5 * * * *"), Path::new("x"), &mut e);
        assert!(e.is_empty());
    }

    #[test]
    fn schedule_invalid_rejected() {
        let mut e = Vec::new();
        check_schedule(&stub_job("foo bar baz"), Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
        assert!(e[0].message.contains("invalid cron"));
        assert!(e[0].message.contains("test-job"));
    }

    #[test]
    fn schedule_l_modifier_accepted() {
        let mut e = Vec::new();
        check_schedule(&stub_job("0 3 L * *"), Path::new("x"), &mut e);
        assert!(e.is_empty());
    }

    #[test]
    fn schedule_empty_rejected() {
        let mut e = Vec::new();
        check_schedule(&stub_job(""), Path::new("x"), &mut e);
        assert!(!e.is_empty());
    }

    #[test]
    fn check_one_of_job_type_error_mentions_defaults() {
        // Issue #20: when a user relies on [defaults].image but typos the
        // defaults section away, the error must tell them `image` can come
        // from [defaults] so they know where else to look.
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.script = None;
        job.image = None;
        let mut e = Vec::new();
        check_one_of_job_type(&job, Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
        assert!(
            e[0].message.contains("[defaults]"),
            "error must point at [defaults] as a valid source of `image`: {}",
            e[0].message
        );
        assert!(
            e[0].message.contains("use_defaults"),
            "error must mention `use_defaults` as the opt-out knob: {}",
            e[0].message
        );
    }

    #[test]
    fn check_cmd_only_on_docker_jobs_rejects_on_command_job() {
        // A command job with `cmd = [...]` is nonsense — there's no container
        // to pass the args to. Pre-fix this was silently accepted and the
        // `cmd` field was dropped from serialization because command jobs
        // never read `config_json` back through DockerJobConfig. Reject
        // loudly so the operator fixes the config intent.
        let mut job = stub_job("*/5 * * * *");
        // stub_job defaults: command = Some, image = None — i.e. a command job.
        job.cmd = Some(vec!["echo".to_string(), "hi".to_string()]);
        let mut e = Vec::new();
        check_cmd_only_on_docker_jobs(&job, Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
        assert!(
            e[0].message.contains("test-job"),
            "error must name the job: {}",
            e[0].message
        );
        assert!(
            e[0].message.contains("cmd"),
            "error must name the offending field: {}",
            e[0].message
        );
        assert!(
            e[0].message.contains("docker jobs"),
            "error must explain cmd is docker-only: {}",
            e[0].message
        );
    }

    #[test]
    fn check_cmd_only_on_docker_jobs_rejects_on_script_job() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.script = Some("#!/bin/sh\necho hi\n".to_string());
        job.cmd = Some(vec!["ignored".to_string()]);
        let mut e = Vec::new();
        check_cmd_only_on_docker_jobs(&job, Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
        assert!(e[0].message.contains("test-job"));
        assert!(e[0].message.contains("docker jobs"));
    }

    #[test]
    fn check_cmd_only_on_docker_jobs_accepts_docker_job_with_cmd() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.image = Some("alpine:latest".to_string());
        job.cmd = Some(vec!["echo".to_string(), "hi".to_string()]);
        let mut e = Vec::new();
        check_cmd_only_on_docker_jobs(&job, Path::new("x"), &mut e);
        assert!(e.is_empty(), "docker job with cmd must pass: got {e:?}");
    }

    #[test]
    fn check_cmd_only_on_docker_jobs_accepts_when_cmd_is_none() {
        // Default case — no cmd set on any job type, validator is a no-op.
        let job = stub_job("*/5 * * * *"); // command job, cmd = None
        let mut e = Vec::new();
        check_cmd_only_on_docker_jobs(&job, Path::new("x"), &mut e);
        assert!(e.is_empty());
    }

    // ---- LBL-03 reserved-namespace ----

    #[test]
    fn check_label_reserved_namespace_rejects_cronduit_prefix() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.image = Some("alpine:latest".into());
        let mut labels = std::collections::HashMap::new();
        labels.insert("cronduit.foo".to_string(), "v".to_string());
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_label_reserved_namespace(&job, Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
        assert!(e[0].message.contains("cronduit.foo"));
        assert!(e[0].message.contains("reserved namespace"));
    }

    #[test]
    fn check_label_reserved_namespace_lists_multiple_keys_sorted() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.image = Some("alpine:latest".into());
        let mut labels = std::collections::HashMap::new();
        labels.insert("cronduit.zeta".to_string(), "v".to_string());
        labels.insert("cronduit.alpha".to_string(), "v".to_string());
        labels.insert("cronduit.mid".to_string(), "v".to_string());
        labels.insert("traefik.enable".to_string(), "true".to_string()); // not reserved
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_label_reserved_namespace(&job, Path::new("x"), &mut e);
        assert_eq!(
            e.len(),
            1,
            "D-01: one ConfigError per job per violation type"
        );
        // Determinism: alphabetical ordering. Per RESEARCH Pitfall 2.
        let pos_alpha = e[0].message.find("cronduit.alpha").expect("contains alpha");
        let pos_mid = e[0].message.find("cronduit.mid").expect("contains mid");
        let pos_zeta = e[0].message.find("cronduit.zeta").expect("contains zeta");
        assert!(pos_alpha < pos_mid, "alphabetical: alpha before mid");
        assert!(pos_mid < pos_zeta, "alphabetical: mid before zeta");
        assert!(
            !e[0].message.contains("traefik.enable"),
            "non-reserved key not listed"
        );
    }

    #[test]
    fn check_label_reserved_namespace_accepts_cronduit_underscore_keys() {
        // Per RESEARCH Edge Case 8.5 — `cronduit_foo` does NOT start with `cronduit.`
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.image = Some("alpine:latest".into());
        let mut labels = std::collections::HashMap::new();
        labels.insert("cronduit_foo".to_string(), "v".to_string());
        labels.insert("cronduitfoo".to_string(), "v".to_string());
        labels.insert("cronduit-foo".to_string(), "v".to_string());
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_label_reserved_namespace(&job, Path::new("x"), &mut e);
        assert!(
            e.is_empty(),
            "underscore/no-separator forms must pass: got {e:?}"
        );
    }

    #[test]
    fn check_label_reserved_namespace_accepts_none() {
        let job = stub_job("*/5 * * * *"); // labels: None
        let mut e = Vec::new();
        check_label_reserved_namespace(&job, Path::new("x"), &mut e);
        assert!(e.is_empty());
    }

    // ---- LBL-04 type-gate ----

    #[test]
    fn check_labels_only_on_docker_jobs_rejects_on_command_job() {
        let mut job = stub_job("*/5 * * * *");
        // stub_job defaults command = Some("echo hi") and image = None — command job.
        let mut labels = std::collections::HashMap::new();
        labels.insert("a".to_string(), "v".to_string());
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_labels_only_on_docker_jobs(&job, None, Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
        assert!(e[0].message.contains("test-job"));
        assert!(e[0].message.contains("docker jobs"));
        assert!(e[0].message.contains("labels"));
    }

    #[test]
    fn check_labels_only_on_docker_jobs_rejects_on_script_job() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.script = Some("echo hi".into());
        // image still None
        let mut labels = std::collections::HashMap::new();
        labels.insert("a".to_string(), "v".to_string());
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_labels_only_on_docker_jobs(&job, None, Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
    }

    #[test]
    fn check_labels_only_on_docker_jobs_accepts_docker_job() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.image = Some("alpine:latest".into());
        let mut labels = std::collections::HashMap::new();
        labels.insert("a".to_string(), "v".to_string());
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_labels_only_on_docker_jobs(&job, None, Path::new("x"), &mut e);
        assert!(e.is_empty(), "docker job with labels must pass: got {e:?}");
    }

    // ---- LBL-04 set-diff branches (CR-02 gap closure, plan 17-08) ----

    #[test]
    fn lbl_04_command_job_with_operator_set_labels_emits_legacy_message() {
        // CR-02 regression test (gap closure plan 17-08, Branch A).
        // Operator wrote `labels = {...}` on a command job, no defaults.labels.
        // Legacy message text preserved for backwards compat.
        let mut job = stub_job("*/5 * * * *");
        // stub_job defaults command = Some, image = None — command job.
        let mut labels = std::collections::HashMap::new();
        labels.insert("operator.key".to_string(), "v".to_string());
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_labels_only_on_docker_jobs(&job, None, Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
        assert!(
            e[0].message.contains("Remove the `labels` block"),
            "Branch A must preserve legacy message text; got: {}",
            e[0].message
        );
        // Negative — must NOT contain Branch B's remediation phrase.
        assert!(
            !e[0].message.contains("use_defaults = false"),
            "Branch A must not surface the use_defaults remediation; got: {}",
            e[0].message
        );
    }

    #[test]
    fn lbl_04_command_job_with_defaults_only_emits_distinct_use_defaults_false_message() {
        // CR-02 regression test (gap closure plan 17-08, Branch B).
        // Operator did NOT write a labels block on the command job; the
        // merged labels came from `[defaults].labels` via apply_defaults.
        // The error must name the actual fix: use_defaults = false.
        let mut job = stub_job("*/5 * * * *");
        // Simulate post-apply_defaults state: job.labels contains ONLY the
        // defaults keys (no operator-supplied keys).
        let mut merged_labels = std::collections::HashMap::new();
        merged_labels.insert("inherited.from.defaults".to_string(), "v".to_string());
        job.labels = Some(merged_labels);

        // The defaults map the validator will set-diff against — same keys
        // as the merged labels (this is the defaults-only case).
        let mut defaults_labels = std::collections::HashMap::new();
        defaults_labels.insert("inherited.from.defaults".to_string(), "v".to_string());

        let mut e = Vec::new();
        check_labels_only_on_docker_jobs(&job, Some(&defaults_labels), Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
        assert!(
            e[0].message.contains("set `use_defaults = false`"),
            "Branch B must surface the use_defaults remediation; got: {}",
            e[0].message
        );
        // Negative — must NOT contain Branch A's "Remove the `labels` block"
        // phrase (operator never wrote one).
        assert!(
            !e[0].message.contains("Remove the `labels` block"),
            "Branch B must not blame a labels block the operator never wrote; got: {}",
            e[0].message
        );
        // The defaults key must NOT appear in the error message (no leak).
        assert!(
            !e[0].message.contains("inherited.from.defaults"),
            "Branch B must not leak inherited keys into the error message; got: {}",
            e[0].message
        );
        // Job type discriminator — stub_job is a command job.
        assert!(
            e[0].message.contains("command job"),
            "Branch B must name the job type (command); got: {}",
            e[0].message
        );
    }

    #[test]
    fn lbl_04_command_job_with_mixed_operator_and_defaults_emits_legacy_message_only_for_operator_keys()
     {
        // CR-02 regression test (gap closure plan 17-08, mixed case).
        // Operator wrote `labels = {operator.key = ...}` AND inherited
        // `inherited.from.defaults` via apply_defaults merge. Branch A wins
        // (operator_only_keys is non-empty) and the legacy message fires.
        // The defaults-key MUST NOT appear in the error (set-diff hides it).
        let mut job = stub_job("*/5 * * * *");
        // Simulate post-apply_defaults state: job.labels = operator + defaults.
        let mut merged_labels = std::collections::HashMap::new();
        merged_labels.insert("operator.key".to_string(), "v".to_string());
        merged_labels.insert("inherited.from.defaults".to_string(), "v".to_string());
        job.labels = Some(merged_labels);

        let mut defaults_labels = std::collections::HashMap::new();
        defaults_labels.insert("inherited.from.defaults".to_string(), "v".to_string());

        let mut e = Vec::new();
        check_labels_only_on_docker_jobs(&job, Some(&defaults_labels), Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
        // Branch A wins → legacy message.
        assert!(
            e[0].message.contains("Remove the `labels` block"),
            "Mixed case must take Branch A (operator_only non-empty); got: {}",
            e[0].message
        );
        // The defaults-key must NOT be in the error (set-diff excluded it).
        assert!(
            !e[0].message.contains("inherited.from.defaults"),
            "Mixed case must not leak inherited keys; got: {}",
            e[0].message
        );
    }

    // ---- LBL-06 size limits ----

    #[test]
    fn check_label_size_limits_rejects_per_value_over_4kb() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.image = Some("alpine:latest".into());
        let mut labels = std::collections::HashMap::new();
        labels.insert("k".to_string(), "x".repeat(4097)); // 4097 bytes — over 4 KB
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_label_size_limits(&job, Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
        assert!(e[0].message.contains("4 KB") || e[0].message.contains("4096"));
        assert!(e[0].message.contains("k"));
    }

    #[test]
    fn check_label_size_limits_accepts_value_at_4kb_boundary() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.image = Some("alpine:latest".into());
        let mut labels = std::collections::HashMap::new();
        labels.insert("k".to_string(), "x".repeat(4096)); // exactly 4096 — must pass
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_label_size_limits(&job, Path::new("x"), &mut e);
        assert!(e.is_empty(), "4096-byte value must pass: got {e:?}");
    }

    #[test]
    fn check_label_size_limits_rejects_per_set_over_32kb() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.image = Some("alpine:latest".into());
        let mut labels = std::collections::HashMap::new();
        // Build a label set whose total bytes (keys + values) exceeds 32 KB but
        // keeps each value ≤ 4 KB so only the per-set check fires.
        for i in 0..10 {
            labels.insert(format!("key{:02}", i), "x".repeat(3500)); // ~3500 bytes each — 10 entries → ~35 KB
        }
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_label_size_limits(&job, Path::new("x"), &mut e);
        assert!(!e.is_empty(), "expected at least one error");
        assert!(
            e.iter().any(|err| err.message.contains("32 KB")),
            "expected a 32 KB error: got {e:?}"
        );
    }

    // ---- D-02 key chars ----

    #[test]
    fn check_label_key_chars_rejects_space_slash_empty() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.image = Some("alpine:latest".into());
        let mut labels = std::collections::HashMap::new();
        labels.insert("my key".to_string(), "v".to_string()); // space
        labels.insert("foo/bar".to_string(), "v".to_string()); // slash
        labels.insert("".to_string(), "v".to_string()); // empty
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_label_key_chars(&job, Path::new("x"), &mut e);
        assert_eq!(e.len(), 1, "D-01: one ConfigError per violation type");
        assert!(e[0].message.contains("my key"));
        assert!(e[0].message.contains("foo/bar"));
    }

    #[test]
    fn check_label_key_chars_rejects_leading_dot() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.image = Some("alpine:latest".into());
        let mut labels = std::collections::HashMap::new();
        labels.insert(".foo".to_string(), "v".to_string()); // leading char must be alphanumeric/underscore
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_label_key_chars(&job, Path::new("x"), &mut e);
        assert_eq!(e.len(), 1);
    }

    #[test]
    fn check_label_key_chars_accepts_dotted_and_underscore_keys() {
        let mut job = stub_job("*/5 * * * *");
        job.command = None;
        job.image = Some("alpine:latest".into());
        let mut labels = std::collections::HashMap::new();
        labels.insert(
            "com.centurylinklabs.watchtower.enable".to_string(),
            "false".to_string(),
        );
        labels.insert("traefik.http.routers.x.rule".to_string(), "v".to_string());
        labels.insert("_internal".to_string(), "v".to_string());
        labels.insert("a-b-c".to_string(), "v".to_string());
        labels.insert("0starts_digit".to_string(), "v".to_string());
        job.labels = Some(labels);
        let mut e = Vec::new();
        check_label_key_chars(&job, Path::new("x"), &mut e);
        assert!(e.is_empty(), "valid keys must all pass: got {e:?}");
    }

    // ---- Phase 18 / WH-01 webhook validators ----

    fn make_webhook_job(name: &str, wh: Option<crate::config::WebhookConfig>) -> JobConfig {
        use std::collections::BTreeMap;
        JobConfig {
            name: name.into(),
            schedule: "* * * * *".into(),
            command: Some("true".into()),
            script: None,
            image: None,
            use_defaults: None,
            env: BTreeMap::new(),
            volumes: None,
            labels: None,
            network: None,
            container_name: None,
            timeout: None,
            delete: None,
            cmd: None,
            tags: Vec::new(),
            webhook: wh,
        }
    }

    #[test]
    fn check_webhook_url_rejects_garbage() {
        use crate::config::WebhookConfig;
        use secrecy::SecretString;
        let job = make_webhook_job(
            "j",
            Some(WebhookConfig {
                url: "not a url".into(),
                states: vec!["failed".into()],
                secret: Some(SecretString::from("s")),
                unsigned: false,
                fire_every: 1,
            }),
        );
        let mut errors = Vec::new();
        check_webhook_url(&job, Path::new("test.toml"), &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("is not a valid URL"));
    }

    #[test]
    fn check_webhook_url_rejects_non_http_scheme() {
        use crate::config::WebhookConfig;
        use secrecy::SecretString;
        let job = make_webhook_job(
            "j",
            Some(WebhookConfig {
                url: "ftp://x.y/".into(),
                states: vec!["failed".into()],
                secret: Some(SecretString::from("s")),
                unsigned: false,
                fire_every: 1,
            }),
        );
        let mut errors = Vec::new();
        check_webhook_url(&job, Path::new("test.toml"), &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("scheme"));
        assert!(errors[0].message.contains("`ftp`"));
        assert!(errors[0].message.contains("`http`, `https`"));
    }

    #[test]
    fn check_webhook_url_accepts_https_and_http() {
        use crate::config::WebhookConfig;
        use secrecy::SecretString;
        for url in ["https://x.y/", "http://127.0.0.1:8080/path"] {
            let job = make_webhook_job(
                "j",
                Some(WebhookConfig {
                    url: url.into(),
                    states: vec!["failed".into()],
                    secret: Some(SecretString::from("s")),
                    unsigned: false,
                    fire_every: 1,
                }),
            );
            let mut errors = Vec::new();
            check_webhook_url(&job, Path::new("test.toml"), &mut errors);
            assert!(errors.is_empty(), "url {url} should pass; got: {errors:?}");
        }
    }

    #[test]
    fn check_webhook_block_rejects_empty_states() {
        use crate::config::WebhookConfig;
        use secrecy::SecretString;
        let job = make_webhook_job(
            "j",
            Some(WebhookConfig {
                url: "https://x/".into(),
                states: vec![],
                secret: Some(SecretString::from("s")),
                unsigned: false,
                fire_every: 1,
            }),
        );
        let mut errors = Vec::new();
        check_webhook_block_completeness(&job, Path::new("test.toml"), &mut errors);
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("webhook.states is empty"))
        );
    }

    #[test]
    fn check_webhook_block_rejects_unknown_state() {
        use crate::config::WebhookConfig;
        use secrecy::SecretString;
        let job = make_webhook_job(
            "j",
            Some(WebhookConfig {
                url: "https://x/".into(),
                states: vec!["fialed".into(), "boom".into()],
                secret: Some(SecretString::from("s")),
                unsigned: false,
                fire_every: 1,
            }),
        );
        let mut errors = Vec::new();
        check_webhook_block_completeness(&job, Path::new("test.toml"), &mut errors);
        let msg = errors
            .iter()
            .map(|e| e.message.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(msg.contains("unknown values"));
        // Sorted offending list (Pitfall G): boom before fialed alphabetically.
        let pos_boom = msg.find("boom").expect("boom present");
        let pos_fialed = msg.find("fialed").expect("fialed present");
        assert!(
            pos_boom < pos_fialed,
            "Pitfall G — offending values must be sorted"
        );
        // Valid list named.
        for v in [
            "success",
            "failed",
            "timeout",
            "stopped",
            "cancelled",
            "error",
        ] {
            assert!(msg.contains(v), "valid value `{v}` must appear in error");
        }
    }

    #[test]
    fn check_webhook_block_rejects_both_secret_and_unsigned() {
        use crate::config::WebhookConfig;
        use secrecy::SecretString;
        let job = make_webhook_job(
            "j",
            Some(WebhookConfig {
                url: "https://x/".into(),
                states: vec!["failed".into()],
                secret: Some(SecretString::from("s")),
                unsigned: true, // <-- both
                fire_every: 1,
            }),
        );
        let mut errors = Vec::new();
        check_webhook_block_completeness(&job, Path::new("test.toml"), &mut errors);
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("both `secret` AND `unsigned = true`"))
        );
    }

    #[test]
    fn check_webhook_block_rejects_neither_secret_nor_unsigned() {
        use crate::config::WebhookConfig;
        let job = make_webhook_job(
            "j",
            Some(WebhookConfig {
                url: "https://x/".into(),
                states: vec!["failed".into()],
                secret: None,
                unsigned: false, // <-- neither
                fire_every: 1,
            }),
        );
        let mut errors = Vec::new();
        check_webhook_block_completeness(&job, Path::new("test.toml"), &mut errors);
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("needs either `secret"))
        );
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("OR `unsigned = true`"))
        );
    }

    #[test]
    fn check_webhook_block_rejects_negative_fire_every() {
        use crate::config::WebhookConfig;
        use secrecy::SecretString;
        let job = make_webhook_job(
            "j",
            Some(WebhookConfig {
                url: "https://x/".into(),
                states: vec!["failed".into()],
                secret: Some(SecretString::from("s")),
                unsigned: false,
                fire_every: -1,
            }),
        );
        let mut errors = Vec::new();
        check_webhook_block_completeness(&job, Path::new("test.toml"), &mut errors);
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("webhook.fire_every = -1 is negative"))
        );
    }

    #[test]
    fn check_webhook_block_rejects_empty_secret() {
        // Pitfall H — secret resolved to empty string after interpolation.
        use crate::config::WebhookConfig;
        use secrecy::SecretString;
        let job = make_webhook_job(
            "j",
            Some(WebhookConfig {
                url: "https://x/".into(),
                states: vec!["failed".into()],
                secret: Some(SecretString::from("")), // <-- empty string
                unsigned: false,
                fire_every: 1,
            }),
        );
        let mut errors = Vec::new();
        check_webhook_block_completeness(&job, Path::new("test.toml"), &mut errors);
        assert!(errors.iter().any(|e| {
            e.message
                .contains("webhook.secret resolved to an empty string")
        }));
    }

    #[test]
    fn check_webhook_block_accepts_valid_signed() {
        use crate::config::WebhookConfig;
        use secrecy::SecretString;
        let job = make_webhook_job(
            "j",
            Some(WebhookConfig {
                url: "https://x/".into(),
                states: vec!["failed".into()],
                secret: Some(SecretString::from("s")),
                unsigned: false,
                fire_every: 0,
            }),
        );
        let mut errors = Vec::new();
        check_webhook_block_completeness(&job, Path::new("test.toml"), &mut errors);
        assert!(
            errors.is_empty(),
            "valid signed config should pass: {errors:?}"
        );
    }

    #[test]
    fn check_webhook_block_accepts_valid_unsigned() {
        use crate::config::WebhookConfig;
        let job = make_webhook_job(
            "j",
            Some(WebhookConfig {
                url: "https://x/".into(),
                states: vec!["timeout".into()],
                secret: None,
                unsigned: true,
                fire_every: 1,
            }),
        );
        let mut errors = Vec::new();
        check_webhook_block_completeness(&job, Path::new("test.toml"), &mut errors);
        assert!(
            errors.is_empty(),
            "valid unsigned config should pass: {errors:?}"
        );
    }

    // ---- Phase 20 / WH-07 webhook HTTPS-required validator (D-19/D-20/D-21) ----

    /// Helper that builds a job with the given webhook URL and runs the URL
    /// validator. Returns the collected errors so tests can pattern-match.
    fn run_webhook_url_check(url: &str) -> Vec<ConfigError> {
        use crate::config::WebhookConfig;
        use secrecy::SecretString;
        let job = make_webhook_job(
            "j",
            Some(WebhookConfig {
                url: url.into(),
                states: vec!["failed".into()],
                secret: Some(SecretString::from("s")),
                unsigned: false,
                fire_every: 1,
            }),
        );
        let mut errors = Vec::new();
        check_webhook_url(&job, Path::new("test.toml"), &mut errors);
        errors
    }

    #[test]
    fn https_anywhere_accepted_silent() {
        // HTTPS URLs are accepted regardless of host (no classification, no log).
        let errors = run_webhook_url_check("https://example.com/hook");
        assert!(
            errors.is_empty(),
            "https://example.com should pass: {errors:?}"
        );
    }

    #[test]
    fn http_public_rejected() {
        let errors = run_webhook_url_check("http://example.com/hook");
        assert_eq!(errors.len(), 1, "expected 1 error: {errors:?}");
        assert!(
            errors[0]
                .message
                .contains("requires HTTPS for non-loopback"),
            "D-21 wording missing: {}",
            errors[0].message
        );
    }

    #[test]
    fn http_localhost_accepted() {
        let errors = run_webhook_url_check("http://localhost/hook");
        assert!(
            errors.is_empty(),
            "http://localhost should pass: {errors:?}"
        );
    }

    #[test]
    fn http_localhost_uppercase_accepted() {
        // eq_ignore_ascii_case — URL host is normalized to lowercase by `url`
        // crate, but the validator's match must still be case-insensitive
        // for safety.
        let errors = run_webhook_url_check("http://LOCALHOST/hook");
        assert!(
            errors.is_empty(),
            "http://LOCALHOST should pass: {errors:?}"
        );
    }

    #[test]
    fn http_localhost_with_port_accepted() {
        let errors = run_webhook_url_check("http://localhost:8080/hook");
        assert!(
            errors.is_empty(),
            "http://localhost:8080 should pass: {errors:?}"
        );
    }

    #[test]
    fn http_loopback_v4_accepted() {
        for url in ["http://127.0.0.1/hook", "http://127.0.0.1:9000/path"] {
            let errors = run_webhook_url_check(url);
            assert!(errors.is_empty(), "{url} should pass: {errors:?}");
        }
    }

    #[test]
    fn http_rfc1918_v4_accepted() {
        for url in [
            "http://10.5.5.5/hook",
            "http://10.0.0.1/hook",
            "http://172.16.0.1/hook",
            "http://172.31.255.255/hook",
            "http://192.168.1.1/hook",
        ] {
            let errors = run_webhook_url_check(url);
            assert!(errors.is_empty(), "{url} should pass: {errors:?}");
        }
    }

    #[test]
    fn http_loopback_v6_accepted() {
        let errors = run_webhook_url_check("http://[::1]/hook");
        assert!(errors.is_empty(), "http://[::1] should pass: {errors:?}");
    }

    #[test]
    fn http_ula_v6_fd_accepted() {
        // Success-criterion-literal: fd00::/8
        let errors = run_webhook_url_check("http://[fd00::1]/hook");
        assert!(
            errors.is_empty(),
            "http://[fd00::1] should pass: {errors:?}"
        );
    }

    #[test]
    fn http_ula_v6_fc_accepted_broader_than_spec() {
        // RESEARCH §4.1: is_unique_local covers the broader fc00::/7
        // (NEVER rejects spec-allowed). Regression-lock the broader behavior.
        let errors = run_webhook_url_check("http://[fc00::1]/hook");
        assert!(
            errors.is_empty(),
            "http://[fc00::1] should pass: {errors:?}"
        );
    }

    #[test]
    fn http_public_v4_rejected() {
        // 198.51.100.0/24 = TEST-NET-2 (RFC 5737)
        let errors = run_webhook_url_check("http://198.51.100.1/hook");
        assert_eq!(errors.len(), 1, "expected 1 error: {errors:?}");
        assert!(
            errors[0]
                .message
                .contains("requires HTTPS for non-loopback")
        );
    }

    #[test]
    fn http_link_local_v4_rejected() {
        // Link-local 169.254/16 is explicitly NOT in the allowlist (RESEARCH §5).
        let errors = run_webhook_url_check("http://169.254.0.1/hook");
        assert_eq!(errors.len(), 1, "link-local must be rejected: {errors:?}");
    }

    #[test]
    fn http_public_dns_rejected() {
        // Hostnames that are not literal `localhost` and not an IpAddr → rejected.
        let errors = run_webhook_url_check("http://example.org/hook");
        assert_eq!(errors.len(), 1, "expected 1 error: {errors:?}");
        assert!(
            errors[0]
                .message
                .contains("requires HTTPS for non-loopback")
        );
    }

    #[test]
    fn http_public_v6_rejected() {
        // 2001:db8::/32 = documentation prefix; not in allowlist.
        let errors = run_webhook_url_check("http://[2001:db8::1]/hook");
        assert_eq!(errors.len(), 1, "expected 1 error: {errors:?}");
    }

    #[test]
    fn d21_error_message_lists_allowed_local_nets_verbatim() {
        // The error message must list the allowed local nets verbatim per D-21.
        let errors = run_webhook_url_check("http://example.com/hook");
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0]
                .message
                .contains("127/8, ::1, 10/8, 172.16/12, 192.168/16, fd00::/8"),
            "D-21 verbatim allowed-nets list missing: {}",
            errors[0].message
        );
    }

    // ---- Phase 22 / TAG-* tag validators (TAG-03, TAG-04, TAG-05, D-08) ----

    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use tracing::subscriber::DefaultGuard;
    use tracing_subscriber::fmt::MakeWriter;

    /// MakeWriter test fixture for capturing tracing output. Used by the
    /// TAG-03 dedup-collapse WARN test (`tracing-test` crate is NOT a dep,
    /// per D-17 — verified 2026-05-04).
    #[derive(Clone, Default)]
    struct CapturedWriter {
        buf: Arc<Mutex<Vec<u8>>>,
    }

    impl CapturedWriter {
        fn new() -> Self {
            Self::default()
        }
        fn captured(&self) -> String {
            let v = self.buf.lock().unwrap();
            String::from_utf8_lossy(&v).into_owned()
        }
    }

    impl Write for CapturedWriter {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            let mut v = self.buf.lock().unwrap();
            v.extend_from_slice(data);
            Ok(data.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for CapturedWriter {
        type Writer = CapturedWriter;
        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    /// Install a thread-local subscriber that captures all WARN-level logs
    /// into the returned `CapturedWriter`. The `DefaultGuard` MUST stay
    /// alive for the whole test body (drop it at the end to restore the
    /// previous subscriber).
    fn install_capturing_subscriber() -> (CapturedWriter, DefaultGuard) {
        let writer = CapturedWriter::new();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(writer.clone())
            .with_max_level(tracing::Level::WARN)
            .with_ansi(false)
            .without_time()
            .finish();
        let guard = tracing::subscriber::set_default(subscriber);
        (writer, guard)
    }

    /// Build a JobConfig with the given name + tags, otherwise minimally valid.
    fn make_job_with_tags(name: &str, tags: Vec<String>) -> JobConfig {
        JobConfig {
            name: name.into(),
            schedule: "* * * * *".into(),
            command: Some("true".into()),
            script: None,
            image: None,
            use_defaults: None,
            env: Default::default(),
            volumes: None,
            labels: None,
            network: None,
            container_name: None,
            timeout: None,
            delete: None,
            cmd: None,
            tags,
            webhook: None,
        }
    }

    // ---- check_tag_charset_and_reserved (Task 1, behaviors 1-12) ----

    #[test]
    fn tag_charset_empty_tags_no_errors_no_warn() {
        // Behavior 1: tags = [] → no errors, no WARN (early return)
        let (writer, _guard) = install_capturing_subscriber();
        let job = make_job_with_tags("j", vec![]);
        let mut errors = Vec::new();
        check_tag_charset_and_reserved(&job, Path::new("x"), &mut errors);
        assert!(errors.is_empty());
        assert!(writer.captured().is_empty(), "no WARN expected for empty");
    }

    #[test]
    fn tag_charset_capital_normalizes_then_passes() {
        // Behavior 2: tags = ["Backup"] → no errors (lowercase passes charset)
        let job = make_job_with_tags("j", vec!["Backup".to_string()]);
        let mut errors = Vec::new();
        check_tag_charset_and_reserved(&job, Path::new("x"), &mut errors);
        assert!(
            errors.is_empty(),
            "D-04 step 2 — capital normalizes-then-passes: got {errors:?}"
        );
    }

    #[test]
    fn tag_charset_special_char_rejected() {
        // Behavior 3: tags = ["MyTag!"] → ONE charset ConfigError
        let job = make_job_with_tags("j", vec!["MyTag!".to_string()]);
        let mut errors = Vec::new();
        check_tag_charset_and_reserved(&job, Path::new("x"), &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("charset"));
        assert!(errors[0].message.contains("`mytag!`"));
    }

    #[test]
    fn tag_charset_reserved_cronduit_rejected() {
        // Behavior 4
        let job = make_job_with_tags("j", vec!["cronduit".to_string()]);
        let mut errors = Vec::new();
        check_tag_charset_and_reserved(&job, Path::new("x"), &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("reserved tag names"));
        assert!(errors[0].message.contains("`cronduit`"));
    }

    #[test]
    fn tag_charset_reserved_system_rejected() {
        // Behavior 5
        let job = make_job_with_tags("j", vec!["system".to_string()]);
        let mut errors = Vec::new();
        check_tag_charset_and_reserved(&job, Path::new("x"), &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("reserved tag names"));
        assert!(errors[0].message.contains("`system`"));
    }

    #[test]
    fn tag_charset_reserved_internal_rejected() {
        // Behavior 6
        let job = make_job_with_tags("j", vec!["internal".to_string()]);
        let mut errors = Vec::new();
        check_tag_charset_and_reserved(&job, Path::new("x"), &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("reserved tag names"));
        assert!(errors[0].message.contains("`internal`"));
    }

    #[test]
    fn tag_charset_capital_reserved_rejected_post_normalization() {
        // Behavior 7: tags = ["Cronduit"] → reserved check runs on normalized form
        let job = make_job_with_tags("j", vec!["Cronduit".to_string()]);
        let mut errors = Vec::new();
        check_tag_charset_and_reserved(&job, Path::new("x"), &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("reserved tag names"));
        assert!(errors[0].message.contains("`cronduit`"));
    }

    #[test]
    fn tag_charset_empty_string_rejected() {
        // Behavior 8: tags = [""] → ONE empty-tag ConfigError
        let job = make_job_with_tags("j", vec!["".to_string()]);
        let mut errors = Vec::new();
        check_tag_charset_and_reserved(&job, Path::new("x"), &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("empty or whitespace-only"));
    }

    #[test]
    fn tag_charset_whitespace_only_rejected() {
        // Behavior 9: tags = ["   "] → ONE empty-tag ConfigError (Pitfall 6)
        let job = make_job_with_tags("j", vec!["   ".to_string()]);
        let mut errors = Vec::new();
        check_tag_charset_and_reserved(&job, Path::new("x"), &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("empty or whitespace-only"));
    }

    #[test]
    fn tag_dedup_collapse_warns_with_inputs_named() {
        // Behavior 10: tags = ["Backup", "backup ", "BACKUP"] → 0 errors,
        // tracing::warn! emitted naming all originals + canonical form.
        let (writer, _guard) = install_capturing_subscriber();
        let mut errors = Vec::new();
        let job = make_job_with_tags(
            "nightly-backup",
            vec![
                "Backup".to_string(),
                "backup ".to_string(),
                "BACKUP".to_string(),
            ],
        );
        let path = Path::new("test.toml");
        check_tag_charset_and_reserved(&job, path, &mut errors);

        // Zero ConfigErrors — WARN-only path.
        assert_eq!(
            errors.len(),
            0,
            "dedup-collapse must NOT push ConfigErrors; got {errors:?}"
        );

        // WARN captured + names original inputs + canonical form.
        let captured = writer.captured();
        assert!(
            captured.contains("nightly-backup"),
            "WARN must name the job; captured={captured}"
        );
        assert!(
            captured.contains("\"Backup\""),
            "WARN must name original input \"Backup\"; captured={captured}"
        );
        assert!(
            captured.contains("\"backup \""),
            "WARN must name original input \"backup \" (with trailing space); captured={captured}"
        );
        assert!(
            captured.contains("\"BACKUP\""),
            "WARN must name original input \"BACKUP\"; captured={captured}"
        );
        assert!(
            captured.contains("\"backup\""),
            "WARN must name canonical form \"backup\"; captured={captured}"
        );
    }

    #[test]
    fn tag_charset_valid_chars_accepted() {
        // Behavior 11: tags = ["a-1", "z_y", "abc-def_ghi"] → no errors
        let job = make_job_with_tags(
            "j",
            vec![
                "a-1".to_string(),
                "z_y".to_string(),
                "abc-def_ghi".to_string(),
            ],
        );
        let mut errors = Vec::new();
        check_tag_charset_and_reserved(&job, Path::new("x"), &mut errors);
        assert!(errors.is_empty(), "valid chars must pass: got {errors:?}");
    }

    #[test]
    fn tag_charset_digits_leading_accepted() {
        // Behavior 12: tags = ["123abc"] → no errors (digits-leading is allowed)
        let job = make_job_with_tags("j", vec!["123abc".to_string()]);
        let mut errors = Vec::new();
        check_tag_charset_and_reserved(&job, Path::new("x"), &mut errors);
        assert!(
            errors.is_empty(),
            "digits-leading must pass: got {errors:?}"
        );
    }

    // ---- check_tag_count_per_job (Task 2, behaviors 1-6) ----

    #[test]
    fn tag_count_empty_tags_no_error() {
        // Task 2 Behavior 1
        let job = make_job_with_tags("j", vec![]);
        let mut errors = Vec::new();
        check_tag_count_per_job(&job, Path::new("x"), &mut errors);
        assert!(errors.is_empty());
    }

    #[test]
    fn tag_count_at_cap_16_accepted() {
        // Task 2 Behavior 2: cap is INCLUSIVE — exactly 16 unique tags must pass
        let tags: Vec<String> = (0..16).map(|i| format!("t{:02}", i)).collect();
        let job = make_job_with_tags("j", tags);
        let mut errors = Vec::new();
        check_tag_count_per_job(&job, Path::new("x"), &mut errors);
        assert!(
            errors.is_empty(),
            "16 tags is at the cap, must pass: got {errors:?}"
        );
    }

    #[test]
    fn tag_count_over_cap_17_rejected() {
        // Task 2 Behavior 3: 17 unique tags → ONE count-cap ConfigError
        let tags: Vec<String> = (0..17).map(|i| format!("t{:02}", i)).collect();
        let job = make_job_with_tags("nightly-backup", tags);
        let mut errors = Vec::new();
        check_tag_count_per_job(&job, Path::new("x"), &mut errors);
        assert_eq!(errors.len(), 1);
        let msg = &errors[0].message;
        assert!(msg.contains("nightly-backup"));
        assert!(msg.contains("17"));
        assert!(msg.contains("16"));
        // Exact message shape (D-08 lock).
        assert_eq!(
            msg,
            "[[jobs]] `nightly-backup`: has 17 tags; max is 16. Remove tags or split into multiple jobs."
        );
    }

    #[test]
    fn tag_count_dedup_aware_no_error_for_duplicates() {
        // Task 2 Behavior 4: 20 duplicates of "a" → no error (post-dedup is 1)
        let tags: Vec<String> = std::iter::repeat_n("a".to_string(), 20).collect();
        let job = make_job_with_tags("j", tags);
        let mut errors = Vec::new();
        check_tag_count_per_job(&job, Path::new("x"), &mut errors);
        assert!(
            errors.is_empty(),
            "D-04 step 4 — cap on POST-dedup count: got {errors:?}"
        );
    }

    #[test]
    fn tag_count_normalize_then_dedup_aware() {
        // Task 2 Behavior 5: ["A", "a", "b", "B"] → post-normalize+dedup: 2 tags → no error
        let job = make_job_with_tags(
            "j",
            vec![
                "A".to_string(),
                "a".to_string(),
                "b".to_string(),
                "B".to_string(),
            ],
        );
        let mut errors = Vec::new();
        check_tag_count_per_job(&job, Path::new("x"), &mut errors);
        assert!(errors.is_empty(), "post-normalize+dedup count: got {errors:?}");
    }

    #[test]
    fn tag_count_message_shape_exact() {
        // Task 2 Behavior 6: error message exact shape — checked above in
        // tag_count_over_cap_17_rejected; this re-asserts with a different N.
        let tags: Vec<String> = (0..20).map(|i| format!("t{:02}", i)).collect();
        let job = make_job_with_tags("backup-job", tags);
        let mut errors = Vec::new();
        check_tag_count_per_job(&job, Path::new("x"), &mut errors);
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "[[jobs]] `backup-job`: has 20 tags; max is 16. Remove tags or split into multiple jobs."
        );
    }

    // ---- check_tag_substring_collision (Task 3, behaviors 1-6) ----

    #[test]
    fn tag_substring_collision_pair_back_backup() {
        // Task 3 Behavior 1: ["back"] + ["backup"] → 1 ConfigError
        let job_a = make_job_with_tags("job-a", vec!["back".to_string()]);
        let job_b = make_job_with_tags("job-b", vec!["backup".to_string()]);
        let mut errors = Vec::new();
        check_tag_substring_collision(&[job_a, job_b], Path::new("x"), &mut errors);
        assert_eq!(errors.len(), 1);
        let msg = &errors[0].message;
        // Expected: short tag first ('back' is a substring of 'backup').
        assert_eq!(
            msg,
            "tag 'back' (used by 'job-a') is a substring of 'backup' (used by 'job-b'); rename or remove one to avoid SQL substring false-positives at filter time."
        );
    }

    #[test]
    fn tag_substring_collision_identical_tags_zero_errors() {
        // Task 3 Behavior 2: both jobs ["backup"] → 0 errors (sharing allowed)
        let job_a = make_job_with_tags("job-a", vec!["backup".to_string()]);
        let job_b = make_job_with_tags("job-b", vec!["backup".to_string()]);
        let mut errors = Vec::new();
        check_tag_substring_collision(&[job_a, job_b], Path::new("x"), &mut errors);
        assert!(
            errors.is_empty(),
            "identical tags across jobs must be allowed: got {errors:?}"
        );
    }

    #[test]
    fn tag_substring_collision_three_way_three_errors() {
        // Task 3 Behavior 3: ["bac"] + ["back"] + ["backup"] → 3 ConfigErrors
        // (pairs: bac↔back, bac↔backup, back↔backup)
        let job_a = make_job_with_tags("job-a", vec!["bac".to_string()]);
        let job_b = make_job_with_tags("job-b", vec!["back".to_string()]);
        let job_c = make_job_with_tags("job-c", vec!["backup".to_string()]);
        let mut errors = Vec::new();
        check_tag_substring_collision(&[job_a, job_b, job_c], Path::new("x"), &mut errors);
        assert_eq!(
            errors.len(),
            3,
            "expected 3 pair errors; got {} -- {errors:?}",
            errors.len()
        );
        let all_messages: String = errors.iter().map(|e| e.message.as_str()).collect::<Vec<_>>().join("\n");
        // Each pair appears in shorter-substring-of-longer order.
        assert!(
            all_messages.contains("'bac' (used by 'job-a') is a substring of 'back'"),
            "missing bac↔back pair: {all_messages}"
        );
        assert!(
            all_messages.contains("'bac' (used by 'job-a') is a substring of 'backup'"),
            "missing bac↔backup pair: {all_messages}"
        );
        assert!(
            all_messages.contains("'back' (used by 'job-b') is a substring of 'backup'"),
            "missing back↔backup pair: {all_messages}"
        );
    }

    #[test]
    fn tag_substring_collision_preview_caps_at_three() {
        // Task 3 Behavior 4: 5 jobs use 'back' + 1 uses 'backup'
        // → 1 error; preview lists `'a', 'b', 'c' (+2 more)` for back.
        let jobs: Vec<JobConfig> = vec![
            make_job_with_tags("a", vec!["back".to_string()]),
            make_job_with_tags("b", vec!["back".to_string()]),
            make_job_with_tags("c", vec!["back".to_string()]),
            make_job_with_tags("d", vec!["back".to_string()]),
            make_job_with_tags("e", vec!["back".to_string()]),
            make_job_with_tags("z", vec!["backup".to_string()]),
        ];
        let mut errors = Vec::new();
        check_tag_substring_collision(&jobs, Path::new("x"), &mut errors);
        assert_eq!(errors.len(), 1);
        let msg = &errors[0].message;
        // Preview for 'back' (5 jobs) → "'a', 'b', 'c' (+2 more)"
        assert!(
            msg.contains("'a', 'b', 'c' (+2 more)"),
            "preview cap: got {msg}"
        );
        // Preview for 'backup' (1 job) → "'z'"
        assert!(msg.contains("'z'"), "single-job preview: got {msg}");
    }

    #[test]
    fn tag_substring_collision_non_substring_pair_zero_errors() {
        // Task 3 Behavior 5: ["backup"] + ["weekly"] → 0 errors
        let job_a = make_job_with_tags("job-a", vec!["backup".to_string()]);
        let job_b = make_job_with_tags("job-b", vec!["weekly".to_string()]);
        let mut errors = Vec::new();
        check_tag_substring_collision(&[job_a, job_b], Path::new("x"), &mut errors);
        assert!(errors.is_empty(), "non-substring pair: got {errors:?}");
    }

    #[test]
    fn tag_substring_collision_empty_fleet_zero_errors() {
        // Task 3 Behavior 6: empty fleet → 0 errors (no iteration)
        let mut errors = Vec::new();
        check_tag_substring_collision(&[], Path::new("x"), &mut errors);
        assert!(errors.is_empty());
    }

    // ---- D-04 order regression-lock (validator order in run_all_checks) ----

    #[test]
    fn run_all_checks_d04_order_capital_normalizes_then_passes() {
        // Locks the D-04 step-2 invariant end-to-end: a TOML with
        // `tags = ["Backup"]` must produce ZERO errors (capital normalizes
        // before charset/reserved checks). This guards against future
        // regressions that move the charset check before normalization.
        use crate::config::Config;
        let toml_text = r#"
[server]
bind = "127.0.0.1:0"
timezone = "UTC"

[[jobs]]
name = "j1"
schedule = "* * * * *"
command = "true"
tags = ["Backup"]
"#;
        let cfg: Config = toml::from_str(toml_text).expect("parse");
        let mut errors = Vec::new();
        run_all_checks(&cfg, Path::new("x"), toml_text, &mut errors);
        assert!(
            errors.is_empty(),
            "D-04 step 2 lock — capital normalizes-then-passes through full validator pipeline: got {errors:?}"
        );
    }

    #[test]
    fn run_all_checks_substring_collision_runs_after_per_job_loop() {
        // Locks D-04 step 5: substring-collision is fleet-level and runs
        // AFTER the per-job loop. Verified indirectly by constructing two
        // jobs with substring-colliding tags and asserting the error appears.
        use crate::config::Config;
        let toml_text = r#"
[server]
bind = "127.0.0.1:0"
timezone = "UTC"

[[jobs]]
name = "j1"
schedule = "* * * * *"
command = "true"
tags = ["back"]

[[jobs]]
name = "j2"
schedule = "* * * * *"
command = "true"
tags = ["backup"]
"#;
        let cfg: Config = toml::from_str(toml_text).expect("parse");
        let mut errors = Vec::new();
        run_all_checks(&cfg, Path::new("x"), toml_text, &mut errors);
        assert!(
            errors.iter().any(|e| e.message.contains("is a substring of")),
            "substring-collision must be raised: got {errors:?}"
        );
    }
}
