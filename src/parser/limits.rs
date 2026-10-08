//! Resource limits for evaluating untrusted scripts. The counters are
//! thread-local and reset by `run_guarded` for every top-level call.
use std::{
    any::Any,
    cell::Cell,
    time::{Duration, Instant},
};

use super::{ParserError, ParserResult, PestError, Val, value::ValError};

pub(crate) const MAX_EVAL_DEPTH: usize = 200;
pub(crate) const MAX_STEPS: u64 = 1_000_000;
pub(crate) const MAX_TIME: Duration = Duration::from_secs(10);
pub(crate) const MAX_STRING_LEN: usize = 16 << 20;
pub(crate) const MAX_ARRAY_LEN: usize = 1 << 20;
pub(crate) const MAX_TOTAL_BYTES: usize = 1 << 30;
pub(crate) const MAX_VALUE_DEPTH: usize = 64;
pub(crate) const MAX_IF_COLLECT_DEPTH: u32 = 8;
// real code needs ~100 rule calls per input byte
const PARSE_STEPS_BASE: u64 = 1_000_000;
const PARSE_STEPS_PER_BYTE: u64 = 1_000;
const STACK_SIZE: usize = 256 << 20;

thread_local! {
    static DEPTH: Cell<usize> = const { Cell::new(0) };
    static STEPS: Cell<u64> = const { Cell::new(0) };
    static PARSE_STEPS: Cell<u64> = const { Cell::new(0) };
    static PARSE_BUDGET: Cell<u64> = const { Cell::new(u64::MAX) };
    static BYTES: Cell<usize> = const { Cell::new(0) };
    static DEADLINE: Cell<Option<Instant>> = const { Cell::new(None) };
}

fn exceeded(what: &str) -> ValError {
    ValError::LimitExceeded(what.to_string())
}

/// Runs `f` on a thread with a big stack and fresh limits. A panic becomes an
/// error.
pub(crate) fn run_guarded<T: Send>(f: impl FnOnce() -> ParserResult<T> + Send) -> ParserResult<T> {
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(STACK_SIZE)
            .spawn_scoped(s, || {
                DEPTH.set(0);
                STEPS.set(0);
                BYTES.set(0);
                DEADLINE.set(Some(Instant::now() + MAX_TIME));
                f()
            })
            .map_err(|e| ParserError::Internal(e.to_string()))?
            .join()
            .unwrap_or_else(|panic| Err(ParserError::Internal(panic_message(panic))))
    })
}

fn panic_message(panic: Box<dyn Any + Send>) -> String {
    panic
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "panic".to_string())
}

/// Called on entry of every grammar rule; once the budget is spent all rules
/// fail at once, which also cuts PEG backtracking short.
pub(crate) fn parse_step<T>(state: T) -> Result<T, T> {
    let steps = PARSE_STEPS.get() + 1;
    PARSE_STEPS.set(steps);
    if steps > PARSE_BUDGET.get() {
        Err(state)
    } else {
        Ok(state)
    }
}

pub(crate) fn start_parse(input_len: usize) {
    PARSE_STEPS.set(0);
    PARSE_BUDGET.set(
        PARSE_STEPS_BASE.saturating_add(PARSE_STEPS_PER_BYTE.saturating_mul(input_len as u64)),
    );
}

pub(crate) fn parse_error(err: PestError) -> ParserError {
    if PARSE_STEPS.get() > PARSE_BUDGET.get() {
        exceeded("parse steps").into()
    } else {
        err.into()
    }
}

/// Counts one evaluation step and one level of nesting; pair with `leave`.
pub(crate) fn enter() -> Result<(), ValError> {
    let steps = STEPS.get() + 1;
    STEPS.set(steps);
    if steps > MAX_STEPS {
        return Err(exceeded("evaluation steps"));
    }
    if steps % 1024 == 0 && DEADLINE.get().is_some_and(|d| Instant::now() > d) {
        STEPS.set(MAX_STEPS);
        return Err(exceeded("evaluation time"));
    }
    if DEPTH.get() >= MAX_EVAL_DEPTH {
        return Err(exceeded("evaluation depth"));
    }
    DEPTH.set(DEPTH.get() + 1);
    Ok(())
}

pub(crate) fn leave() {
    DEPTH.set(DEPTH.get().saturating_sub(1));
}

/// Accounts bytes produced by the script against the total budget.
pub(crate) fn charge(bytes: usize) -> Result<(), ValError> {
    let total = BYTES.get().saturating_add(bytes);
    BYTES.set(total);
    if total > MAX_TOTAL_BYTES {
        return Err(exceeded("memory"));
    }
    Ok(())
}

/// Checks a string about to be allocated.
pub(crate) fn alloc_string(len: usize) -> Result<(), ValError> {
    if len > MAX_STRING_LEN {
        return Err(exceeded("string length"));
    }
    charge(len)
}

/// Checks an array about to be allocated.
pub(crate) fn alloc_array(len: usize) -> Result<(), ValError> {
    if len > MAX_ARRAY_LEN {
        return Err(exceeded("array length"));
    }
    charge(len.saturating_mul(size_of::<Val>()))
}

/// Checks the nesting of a value about to be wrapped in a collection.
pub(crate) fn check_value_depth(val: &Val) -> Result<(), ValError> {
    if val.depth() >= MAX_VALUE_DEPTH {
        return Err(exceeded("value nesting"));
    }
    Ok(())
}
