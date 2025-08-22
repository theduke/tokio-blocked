use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
    time::{Duration, Instant},
};

use tracing::{self, Level};
use tracing_core::{callsite::Identifier, field::Visit, span, subscriber, Field, Metadata};
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer};

/// A standalone layer that measures "busy" time per callsite (span metadata),
/// and records each measured duration as a tracing event.
///
/// Busy time is measured as the wall-clock time between a span's enter and the
/// matching exit, counting only the outermost enter/exit pairs per span
/// instance (nested enters are ignored to avoid double-counting).
pub struct TokioBlockedLayer {
    callsites: Mutex<HashMap<CallsiteKey, CallsiteStats>>,
    // Locally cached set of callsites to consider.
    // Caching speeds up performance.
    allowed_callsites: Mutex<HashSet<Identifier>>,
    // Warn if a single outermost poll exceeds this duration.
    warn_busy_single_poll: Option<Duration>,
    // Warn on close if total busy time across the span exceeds this duration.
    warn_busy_total: Option<Duration>,
}

impl Default for TokioBlockedLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl TokioBlockedLayer {
    pub fn new() -> Self {
        Self {
            callsites: Mutex::new(HashMap::new()),
            allowed_callsites: Mutex::new(HashSet::new()),
            warn_busy_single_poll: Some(Duration::from_micros(150)),
            warn_busy_total: None,
        }
    }

    pub fn with_warn_busy_single_poll(mut self, duration: Option<Duration>) -> Self {
        self.warn_busy_single_poll = duration;
        self
    }

    pub fn with_warn_busy_total(mut self, duration: Option<Duration>) -> Self {
        self.warn_busy_total = duration;
        self
    }

    /// Returns a snapshot of totals per callsite.
    pub fn snapshot(&self) -> Vec<CallsiteStatsSnapshot> {
        let map = self.callsites.lock().unwrap();
        map.values()
            .map(|s| CallsiteStatsSnapshot {
                name: s.name,
                target: s.target,
                file: s.file,
                line: s.line,
                total_busy: s.total_busy,
                count: s.count,
            })
            .collect()
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
struct CallsiteKey(usize);

impl CallsiteKey {
    fn from_meta(meta: &'static Metadata<'static>) -> Self {
        Self(meta as *const _ as usize)
    }
}

#[derive(Debug, Default)]
struct CallsiteStats {
    name: &'static str,
    target: &'static str,
    file: Option<&'static str>,
    line: Option<u32>,
    total_busy: Duration,
    count: u64,
}

/// A serializable snapshot of per-callsite totals.
#[derive(Debug, Clone)]
pub struct CallsiteStatsSnapshot {
    pub name: &'static str,
    pub target: &'static str,
    pub file: Option<&'static str>,
    pub line: Option<u32>,
    pub total_busy: Duration,
    pub count: u64,
}

#[derive(Debug)]
struct SpanBusyExt {
    in_count: usize,
    start: Option<Instant>,
    callsite: CallsiteKey,
    // Original spawn/call location if provided via span fields (e.g. loc.file/line/col).
    origin_file: Option<String>,
    origin_line: Option<u32>,
    origin_col: Option<u32>,
    total_busy: Duration,
    // When the span instance was created, to compute total lifetime.
    created_at: Instant,
}

impl<S> Layer<S> for TokioBlockedLayer
where
    S: tracing_core::Subscriber + for<'a> LookupSpan<'a>,
{
    fn register_callsite(&self, meta: &'static Metadata<'static>) -> subscriber::Interest {
        if matches_tokio_poll(meta) {
            self.allowed_callsites
                .lock()
                .unwrap()
                .insert(meta.callsite());
        }
        subscriber::Interest::always()
    }

    // fn enabled(&self, metadata: &Metadata<'_>, _ctx: Context<'_, S>) -> bool {
    //     true
    // }

    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, cx: Context<'_, S>) {
        let Some(span) = cx.span(id) else { return };

        let meta = attrs.metadata();
        // Only track busy time for spans that correspond to Tokio poll spans.
        let is_allowed = {
            let allowed = self.allowed_callsites.lock().unwrap();
            allowed.contains(&meta.callsite())
        } || matches_tokio_poll(meta);

        if !is_allowed {
            return;
        }

        let key = CallsiteKey::from_meta(meta);

        // Try to extract an original source code location from attributes, if present.
        let mut loc = LocVisitor::default();
        attrs.record(&mut loc);
        let mut exts = span.extensions_mut();
        exts.insert(SpanBusyExt {
            in_count: 0,
            start: None,
            callsite: key,
            origin_file: loc.file,
            origin_line: loc.line,
            origin_col: loc.column,
            total_busy: Duration::new(0, 0),
            created_at: Instant::now(),
        });
    }

    fn on_enter(&self, id: &span::Id, cx: Context<'_, S>) {
        let Some(span) = cx.span(id) else { return };

        let mut exts = span.extensions_mut();
        let Some(ext) = exts.get_mut::<SpanBusyExt>() else {
            return;
        };

        if ext.in_count == 0 {
            ext.start = Some(Instant::now());
        }
        ext.in_count += 1;
    }

    fn on_exit(&self, id: &span::Id, cx: Context<'_, S>) {
        let Some(span) = cx.span(id) else { return };

        // Update span-local counters; if exiting the outermost enter,
        // accumulate into total_busy. Do not lock our own mutex here.
        let mut exts = span.extensions_mut();
        let Some(ext) = exts.get_mut::<SpanBusyExt>() else {
            return;
        };

        if ext.in_count == 0 {
            return;
        }

        ext.in_count -= 1;
        if ext.in_count != 0 {
            return;
        }
        let Some(start) = ext.start.take() else {
            return;
        };

        let end = Instant::now();
        let elapsed = end.saturating_duration_since(start);
        ext.total_busy += elapsed;

        let Some(threshold) = self.warn_busy_single_poll else {
            return; // No threshold configured, skip warning
        };

        // Warn if a single poll exceeded threshold.
        if elapsed >= threshold {
            // Emit a warning event for this poll occurrence.
            let meta = span.metadata();
            let file = ext
                .origin_file
                .as_deref()
                .or_else(|| meta.file())
                .unwrap_or("<unknown>")
                .to_string();
            let line = ext.origin_line.or(meta.line()).unwrap_or(0u32);
            tracing::event!(
                target: "tokio_blocked::task_poll_blocked",
                Level::WARN,
                poll_duration_ns = elapsed.as_nanos() as u64,
                callsite.name = meta.name(),
                callsite.target = meta.target(),
                callsite.file = &file[..],
                callsite.line = line,
                callsite.col = ext.origin_col.unwrap_or(0u32),
            );
        }
    }

    fn on_close(&self, id: span::Id, cx: Context<'_, S>) {
        let Some(span) = cx.span(&id) else { return };

        let mut extensions = span.extensions_mut();
        let Some(mut ext) = extensions.remove::<SpanBusyExt>() else {
            return; // No busy time tracking for this span
        };

        let meta = span.metadata();
        // Finish any in-progress busy interval and copy accumulated totals.
        let (callsite_key, total_busy, origin_file, origin_line, created_at) = {
            if ext.in_count > 0 {
                if let Some(start) = ext.start.take() {
                    let end = Instant::now();
                    let elapsed = end.saturating_duration_since(start);
                    ext.total_busy += elapsed;
                    ext.in_count = 0;
                }
            }
            (
                ext.callsite,
                ext.total_busy,
                ext.origin_file.clone(),
                ext.origin_line,
                ext.created_at,
            )
        };

        // Update per-callsite totals once per span instance.
        {
            let mut map = self.callsites.lock().unwrap();
            let stats = map.entry(callsite_key).or_insert_with(|| CallsiteStats {
                name: meta.name(),
                target: meta.target(),
                file: meta.file(),
                line: meta.line(),
                ..Default::default()
            });
            stats.total_busy += total_busy;
            stats.count += 1;
        }

        let Some(threshold) = self.warn_busy_total else {
            return; // No total busy time threshold configured
        };

        // Emit a warning for the span's total busy time and total lifetime only
        // if the configured threshold is exceeded.
        if total_busy >= threshold {
            let total_span = Instant::now().saturating_duration_since(created_at);
            let file = origin_file
                .as_deref()
                .or_else(|| meta.file())
                .unwrap_or("<unknown>")
                .to_string();
            let line = origin_line.or(meta.line()).unwrap_or(0u32);
            tracing::event!(
                target: "tokio_blocked::task_blocked_total",
                Level::WARN,
                busy_ns = total_busy.as_nanos() as u64,
                duration_ns = total_span.as_nanos() as u64,
                blocked_percent = (total_busy.as_secs_f64() / total_span.as_secs_f64()) * 100.0,
                callsite.name = meta.name(),
                callsite.target = meta.target(),
                callsite.file = &file[..],
                callsite.line = line,
                callsite.col = ext.origin_col.unwrap_or(0u32),
                "tokio task blocked for too long",
            );
        }
    }
}

// A simple visitor to extract `loc.file`, `loc.line`, and `loc.col` if present
// on a span's attributes. Tokio and other instrumentations often include these
// fields to indicate the original user code location.
#[derive(Default)]
struct LocVisitor {
    file: Option<String>,
    line: Option<u32>,
    column: Option<u32>,
}

impl Visit for LocVisitor {
    fn record_debug(&mut self, _field: &Field, _value: &dyn std::fmt::Debug) {}

    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "loc.file" {
            self.file = Some(value.to_string());
        }
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        match field.name() {
            "loc.line" => self.line = Some(value as u32),
            "loc.col" => self.column = Some(value as u32),
            _ => {}
        }
    }
}

fn matches_tokio_poll(meta: &Metadata<'_>) -> bool {
    match (meta.name(), meta.target()) {
        // Task spans (tokio::task or runtime.spawn)
        ("runtime.spawn", "tokio::task") => true,
        // Async op spans
        ("runtime.resource.async_op", _) => true,
        // Per-poll spans for async ops
        ("runtime.resource.async_op.poll", _) => true,
        _ => false,
    }
}
