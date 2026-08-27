# Time

Modules: `time.mod`, `time.time`, `time.timer`, `time.format`, and
`time.backoff`.

Seen exposes checked nanosecond time geometry, separate monotonic and wall
clocks, injectable manual time, bounded timers, strict RFC 3339 conversion, and
caller-owned deterministic backoff. Import the complete public surface with:

```seen
import time.mod.{Duration, Instant, SystemTime, Clock, TimerQueue, Timer,
    MissedTickPolicy, BackoffPolicy, parseRfc3339, formatRfc3339}
```

## Core types

| Type | Contract |
|------|----------|
| `Duration` | Signed `Int64` nanoseconds with saturating construction, addition, subtraction, and unit conversion |
| `Instant` | Monotonic timestamp for elapsed time and deadlines; it is never civil or Unix time |
| `SystemTime` | Unix nanoseconds for wall-clock timestamps; it is never used for elapsed-time deadlines |
| `Clock` | System-backed clock or explicitly advanced deterministic manual clock |
| `TimeError` | Stable `code`, `operation`, and `message` for validation/platform failures |
| `TimeoutError` | Stable timeout/cancellation code plus deadline and observed monotonic nanoseconds |

`Duration.fromMicroseconds`, `fromMilliseconds`, and `fromSeconds` saturate at
the signed 64-bit limits rather than wrapping. `Instant.now()` uses the platform
monotonic clock; `SystemTime.now()` uses the platform wall clock. The runtime
boundary contains only monotonic-now, system-now, and interrupt-safe sleep
adapters. Scheduling and policy remain native Seen.

```seen
let clock = Clock.manualAt(
    Instant{nanoseconds: 100 as Int64},
    SystemTime{unixNanoseconds: 1000 as Int64})
clock.advance(Duration.fromMilliseconds(10 as Int64))
```

## Timers and deadlines

`TimerQueue.withCapacity` creates a bounded, caller-owned readiness source. A
queue supports one-shot and interval registrations, cancellation by token, and
bounded polling. It creates no thread per timer. Reactor backends can call
`poll` when their platform timer source becomes ready.

Intervals require an explicit `MissedTickPolicy`:

- `Skip` schedules the next tick relative to the observation time.
- `Burst` advances one interval at a time, allowing subsequent bounded polls
  to catch up. Each `TimerEvent` reports `missedTicks` explicitly.

`Timer.wait(context)` observes cancellation and the caller's monotonic deadline
before sleeping. Manual clocks never block: an unelapsed manual timer returns a
typed timeout so deterministic tests control progress explicitly.

## RFC 3339

`parseRfc3339` accepts uppercase `T`/`Z`, validated Gregorian dates, numeric
offsets no larger than `14:00`, and zero to nine fractional digits. It rejects
leap seconds, trailing input, invalid dates, and values outside signed
nanosecond range. `formatRfc3339` emits canonical UTC with trimmed fractional
zeros.

```seen
let parsed = parseRfc3339("2024-02-29T12:34:56.123456789+02:30")
let utc = formatRfc3339(parsed.unwrap()).unwrap()
// 2024-02-29T10:04:56.123456789Z
```

## Deterministic backoff

`BackoffPolicy.create` validates the initial/maximum delay, rational
multiplier, jitter permille, explicit seed, and maximum attempts. `step` is a
pure, reproducible calculation for a caller-owned attempt number. The module
does not retry operations, sleep, allocate workers, or hide cancellation.

## Deterministic processes

When `SEEN_DETERMINISTIC=1`, clock reads require a valid `SOURCE_DATE_EPOCH` and
return its nanosecond value. This is intended for reproducible build/test
processes; production services should inject a manual `Clock` where controlled
time is part of application behavior.

The compatibility helpers `nowSeconds`, `nowMillis`, `nowNanos`,
`parseTimestamp`, and `since` remain available from `time.mod`.
