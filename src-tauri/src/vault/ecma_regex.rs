//! ECMAScript RegExp constructor/test only; vault strings are arguments, not code.
use rquickjs::{context::intrinsic, Context, Function, Runtime};
use std::time::{Duration, Instant};

pub(super) const MAX_BYTES: usize = 4096;
const MEMORY_BYTES: usize = 8 * 1024 * 1024;
const STACK_BYTES: usize = 256 * 1024;
const TIME_LIMIT: Duration = Duration::from_millis(25);

fn run(pattern: &str, text: Option<&str>, deadline: Instant) -> Result<bool, ()> {
    if pattern.is_empty()
        || pattern.len() > MAX_BYTES
        || text.is_some_and(|s| s.len() > MAX_BYTES)
        || Instant::now() >= deadline
    {
        return Err(());
    }
    let runtime = Runtime::new().map_err(|_| ())?;
    runtime.set_memory_limit(MEMORY_BYTES);
    runtime.set_max_stack_size(STACK_BYTES);
    runtime.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));
    let context = Context::builder()
        .with::<intrinsic::Eval>()
        .with::<intrinsic::RegExpCompiler>()
        .with::<intrinsic::RegExp>()
        .build(&runtime)
        .map_err(|_| ())?;
    context.with(|ctx| {
        // Only these static functions are evaluated. There are no filesystem,
        // network, process, module loader or Rust callbacks in this context.
        if let Some(text) = text {
            let test: Function = ctx
                .eval("(function(pattern, text) { return new RegExp(pattern, 'i').test(text); })")
                .map_err(|_| ())?;
            test.call((pattern, text)).map_err(|_| ())
        } else {
            let validate: Function = ctx
                .eval("(function(pattern) { new RegExp(pattern, 'i'); return true; })")
                .map_err(|_| ())?;
            validate.call((pattern,)).map_err(|_| ())
        }
    })
}

pub(super) fn valid(pattern: &str) -> bool {
    run(pattern, None, Instant::now() + TIME_LIMIT) == Ok(true)
}

/// Invalid, exhausted and interrupted expressions never authorize a match.
pub(super) fn matches(pattern: &str, text: &str, deadline: Instant) -> bool {
    run(
        pattern,
        Some(text),
        deadline.min(Instant::now() + TIME_LIMIT),
    ) == Ok(true)
}

#[cfg(test)]
#[path = "ecma_regex_tests.rs"]
mod tests;
