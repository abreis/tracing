use crate::{
    field::RecordFields,
    fmt::{format, FormatEvent, FormatFields, MakeWriter},
    layer::{Context, Layer},
    registry::{LookupMetadata, LookupSpan, Registry, WithExtensions},
};
use std::{cell::RefCell, fmt, io, marker::PhantomData};
use tracing_core::{
    span::{Id, Record},
    Event, Subscriber,
};

/// A `Subscriber` that logs formatted representations of `tracing` events.
pub struct FmtLayer<
    S = Registry,
    N = format::DefaultFields,
    E = format::Format<format::Full>,
    W = fn() -> io::Stdout,
> {
    // TODO(david): don't force boxing here. consider:
    // - starting on https://github.com/tokio-rs/tracing/issues/302
    // - making it a generic param; defaulting to a boxed impl.
    // - rename this, because this isn't per-subscriber filtering.
    is_interested: Box<dyn Fn(&Event<'_>) -> bool + Send + Sync + 'static>,
    make_writer: W,
    fmt_fields: N,
    fmt_event: E,
    _inner: PhantomData<S>,
}

impl<S, N, E, W> fmt::Debug for FmtLayer<S, N, E, W>
where
    S: Subscriber + for<'a> LookupSpan<'a> + LookupMetadata,
    N: for<'writer> FormatFields<'writer> + 'static,
    E: FormatEvent<S, N> + 'static,
    W: MakeWriter + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FmtLayer").finish()
    }
}

/// A builder for `FmtLayer` that logs formatted representations of `tracing` events.
pub struct FmtLayerBuilder<
    S = Registry,
    N = format::DefaultFields,
    E = format::Format<format::Full>,
    W = fn() -> io::Stdout,
> {
    fmt_fields: N,
    fmt_event: E,
    make_writer: W,
    is_interested: Box<dyn Fn(&Event<'_>) -> bool + Send + Sync + 'static>,
    _inner: PhantomData<S>,
}

impl<S, N, E, W> fmt::Debug for FmtLayerBuilder<S, N, E, W>
where
    S: Subscriber + for<'a> LookupSpan<'a> + LookupMetadata,
    N: for<'writer> FormatFields<'writer> + 'static,
    E: FormatEvent<S, N> + 'static,
    W: MakeWriter + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FmtLayerBuilder").finish()
    }
}

impl FmtLayer {
    /// Creates a [FmtLayerBuilder].
    pub fn builder() -> FmtLayerBuilder {
        FmtLayerBuilder::default()
    }
}

impl<S, N, E, W> FmtLayerBuilder<S, N, E, W>
where
    S: Subscriber + for<'a> LookupSpan<'a> + LookupMetadata,
    N: for<'writer> FormatFields<'writer> + 'static,
    E: FormatEvent<S, N> + 'static,
    W: MakeWriter + 'static,
{
    /// Sets a filter for events. This filter applies to this, and
    /// subsequent, layers.
    pub fn with_interest<F>(self, f: F) -> Self
    where
        F: Fn(&Event<'_>) -> bool + Send + Sync + 'static,
    {
        FmtLayerBuilder {
            fmt_fields: self.fmt_fields,
            fmt_event: self.fmt_event,
            make_writer: self.make_writer,
            is_interested: Box::new(f),
            _inner: self._inner,
        }
    }
}

// This, like the MakeWriter block, needs to be a seperate impl block because we're
// overriding the `E` type parameter with `E2`.
impl<S, N, E, W> FmtLayerBuilder<S, N, E, W>
where
    S: Subscriber + for<'a> LookupSpan<'a> + LookupMetadata,
    N: for<'writer> FormatFields<'writer> + 'static,
    E: FormatEvent<S, N> + 'static,
    W: MakeWriter + 'static,
{
    /// Sets a [FormatEvent<S, N>].
    pub fn with_event_formatter<E2>(self, e: E2) -> FmtLayerBuilder<S, N, E2, W> {
        FmtLayerBuilder {
            fmt_fields: self.fmt_fields,
            fmt_event: e,
            make_writer: self.make_writer,
            is_interested: self.is_interested,
            _inner: self._inner,
        }
    }
}

// this needs to be a seperate impl block because we're re-assigning the the W2 (make_writer)
// type paramater from the default.
impl<S, N, E, W> FmtLayerBuilder<S, N, E, W> {
    /// Sets a [MakeWriter] for spans and events.
    pub fn with_writer<W2>(self, make_writer: W2) -> FmtLayerBuilder<S, N, E, W2>
    where
        W2: MakeWriter + 'static,
    {
        FmtLayerBuilder {
            fmt_fields: self.fmt_fields,
            fmt_event: self.fmt_event,
            is_interested: self.is_interested,
            make_writer,
            _inner: self._inner,
        }
    }
}

impl<S, N, E, W> FmtLayerBuilder<S, N, E, W>
where
    S: Subscriber + for<'a> LookupSpan<'a> + LookupMetadata,
    N: for<'writer> FormatFields<'writer> + 'static,
    E: FormatEvent<S, N> + 'static,
    W: MakeWriter + 'static,
{
    /// Builds a [FmtLayer] infalliably.
    pub fn build(self) -> FmtLayer<S, N, E, W> {
        FmtLayer {
            is_interested: self.is_interested,
            make_writer: self.make_writer,
            fmt_fields: self.fmt_fields,
            fmt_event: self.fmt_event,
            _inner: self._inner,
        }
    }
}

impl Default for FmtLayerBuilder {
    fn default() -> Self {
        Self {
            is_interested: Box::new(|_| true),
            fmt_fields: format::DefaultFields::default(),
            fmt_event: format::Format::default(),
            make_writer: io::stdout,
            _inner: PhantomData,
        }
    }
}

impl<S, N, E, W> FmtLayer<S, N, E, W>
where
    S: Subscriber + for<'a> LookupSpan<'a> + LookupMetadata,
    N: for<'writer> FormatFields<'writer> + 'static,
    E: FormatEvent<S, N> + 'static,
    W: MakeWriter + 'static,
{
    #[inline]
    fn make_ctx<'a>(&'a self, ctx: Context<'a, S>) -> FmtContext<'a, S, N> {
        FmtContext {
            ctx,
            fmt_fields: &self.fmt_fields,
        }
    }
}

// === impl Formatter ===

impl<S, N, E, W> Layer<S> for FmtLayer<S, N, E, W>
where
    S: Subscriber + for<'a> LookupSpan<'a> + LookupMetadata,
    N: for<'writer> FormatFields<'writer> + 'static,
    E: FormatEvent<S, N> + 'static,
    W: MakeWriter + 'static,
{
    fn on_close(&self, id: Id, _: Context<'_, S>) {
        // dbg!(id);
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            span.data.extensions_mut();
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        thread_local! {
            static BUF: RefCell<String> = RefCell::new(String::new());
        }

        BUF.with(|buf| {
            let borrow = buf.try_borrow_mut();
            let mut a;
            let mut b;
            let mut buf = match borrow {
                Ok(buf) => {
                    a = buf;
                    &mut *a
                }
                _ => {
                    b = String::new();
                    &mut b
                }
            };

            if (self.is_interested)(event) {
                let ctx = self.make_ctx(ctx);
                if self.fmt_event.format_event(&ctx, &mut buf, event).is_ok() {
                    let mut writer = self.make_writer.make_writer();
                    let _ = io::Write::write_all(&mut writer, buf.as_bytes());
                }
            }

            buf.clear();
        });
    }
}

/// `FmtContext` is used to propogate subscriber context to tracing_subscriber::fmt.
pub struct FmtContext<'a, S, N>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup> + LookupMetadata,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    pub(crate) ctx: Context<'a, S>,
    pub(crate) fmt_fields: &'a N,
}

impl<'a, S, N> fmt::Debug for FmtContext<'a, S, N>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup> + LookupMetadata,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FmtContext").finish()
    }
}

impl<'a, S, N> FormatFields<'a> for FmtContext<'a, S, N>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup> + LookupMetadata,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    fn format_fields<R: RecordFields>(
        &self,
        writer: &'a mut dyn fmt::Write,
        fields: R,
    ) -> fmt::Result {
        self.fmt_fields.format_fields(writer, fields)
    }
}

impl<'a, S, N> FmtContext<'a, S, N>
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup> + LookupMetadata,
    N: for<'writer> FormatFields<'writer> + 'static,
{
    // TODO(david): consider an alternative location for this.
    /// Visits parent spans. Used to visit parent spans when formatting spans
    /// and events
    pub fn visit_spans<E, F>(&self, f: F) -> Result<(), E>
    where
        F: FnMut(&Id) -> Result<(), E>,
    {
        self.ctx.visit_parents(f)
    }
}