# Vanity sync performance hypothesis (napkin math)

```
❯ vanity sync
Creating vanity commits: █████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ 685/3701 [00:00:10<00:00:46]
```


## Observed data point

From your run:

- `685 / 3701` commits done in `10s`

That implies an observed write throughput of:

- $r \approx 685 / 10 = 68.5$ commits/sec
- Per-commit time during write phase: $t_{commit} \approx 1 / r \approx 14.6$ ms/commit

If throughput stayed constant for the remaining commits:

- Remaining commits: $3701 - 685 = 3016$
- Remaining time: $3016 / 68.5 \approx 44.0$s
- Total write-phase time: $3701 / 68.5 \approx 54.0$s

So the progress bar alone is consistent with roughly **~50–60 seconds** for this run.

---

## First-principles model

Treat total sync time as:

$$
T_{total} = T_{setup} + T_{target\_scan} + T_{source\_scan} + T_{plan} + T_{write}
$$

Where:

- $T_{setup}$: load config, open repos
- $T_{target\_scan}$: scan this-repo history for `Vanity-Source-Commit` markers
- $T_{source\_scan}$: walk source repo commits (parallelized across read repos with Rayon)
- $T_{plan}$: dedupe/sort/filter pending list
- $T_{write}$: create empty commits sequentially in this-repo

And:

$$
T_{write} \approx N_{pending} \cdot t_{commit}
$$

Given current behavior, writes are the dominant term once $N_{pending}$ is large.

---

## Why write phase dominates

Each empty commit still performs real git work:

1. Resolve HEAD/tree/parent
2. Build commit object
3. Write object to `.git/objects`
4. Update refs/reflog

Even without file diffs, this has non-trivial filesystem and libgit2 overhead. At ~14–15ms each, thousands of commits naturally land in the tens-of-seconds to minute range.

---

## Expected runtime bands (rule of thumb)

Using $t_{commit} \approx 15$ ms and modest fixed overhead:

- `500` pending commits: ~$7.5s + fixed` -> ~10–20s`
- `1000` pending commits: ~$15s + fixed` -> ~20–35s`
- `3000` pending commits: ~$45s + fixed` -> ~50–80s`
- `5000` pending commits: ~$75s + fixed` -> ~80–120s`

This matches what you’re seeing.

---

## Practical expectation for your shown run

For `3701` pending commits with current observed rate:

- Best napkin estimate: **~55s write phase**
- Real-world total including scans/planning: **~60–90s** (machine + disk dependent)

---

## Prediction to test next

If you run:

1. `vanity sync --dry_run` (to get pending count quickly)
2. `vanity sync` immediately after

then runtime should roughly follow:

$$
T_{sync} \approx T_{fixed} + N_{pending} \cdot 0.015s
$$

with $T_{fixed}$ probably in the single-digit to low-tens-of-seconds range depending on repo size and storage speed.

---

> given what is known about the git file structure, could we implement the API on our own to skip a lot of the ffi overhead

Short answer: **technically yes, practically not worth it** for this bottleneck.

- You *can* write Git objects yourself (commit/tree/blob encoding, zlib, object paths, refs, reflogs, lockfiles), but correctness is very hard.
- For your workload, **FFI overhead is usually tiny** compared to:
  - hashing + compression,
  - filesystem writes/fsync,
  - ref updates per commit,
  - object database lookups.
- So replacing `git2` with hand-rolled Git I/O is unlikely to give a big win unless you also redesign write strategy.

What is more likely to help:

- **Batch import path**: use `git fast-import` (or equivalent library support) to stream many commits in one session.
- **Reduce per-commit ref churn**: create objects first, update refs less frequently (if API allows safely).
- **Incremental sync state**: avoid scanning all source/target history every run.
- **Pure Rust without FFI**: consider `gitoxide` (`gix`) if you want to avoid libgit2 FFI, but this is mostly a maintainability/runtime architecture choice, not guaranteed speedup.

If you want, I can sketch a concrete “phase 1” optimization plan for your current codebase (lowest-risk changes first, expected gain per change).

> seems this is a one-time cost, not something that happens on subsequent syncs.

Exactly — that’s the key point.

- The long run is mostly **initial backfill** cost (`N_pending` is huge).
- Subsequent syncs should be much faster because idempotency markers mean only new source commits are added.
- After bootstrap, runtime scales with *new commits since last run*, not total history size.

So for daily action runs, you should expect near-constant small times unless there’s a burst of new commits.