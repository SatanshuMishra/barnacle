use std::cell::RefCell;
use std::io;
use std::marker::PhantomData;
use std::sync::OnceLock;

use barnacle_bot::logging;
use barnacle_bot::logging::LogFormat;
use barnacle_bot::logging::LogSettings;
use tracing_subscriber::fmt::MakeWriter;

thread_local! {
    static WRITTEN: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

static INSTALLED: OnceLock<()> = OnceLock::new();

struct ThisThread;

impl io::Write for ThisThread {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        WRITTEN.with(|written| written.borrow_mut().extend_from_slice(bytes));
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct EachThread;

impl<'w> MakeWriter<'w> for EachThread {
    type Writer = ThisThread;

    fn make_writer(&'w self) -> ThisThread {
        ThisThread
    }
}

pub struct Logs {
    _this_thread: PhantomData<*const ()>,
}

pub fn capture() -> Logs {
    INSTALLED.get_or_init(|| {
        let settings = LogSettings {
            format: LogFormat::Json,
            level: "debug".to_owned(),
        };
        tracing::subscriber::set_global_default(logging::subscriber(&settings, EachThread, false))
            .unwrap();
        tracing::callsite::rebuild_interest_cache();
    });
    WRITTEN.with(|written| written.borrow_mut().clear());
    Logs {
        _this_thread: PhantomData,
    }
}

impl Logs {
    pub fn events(&self) -> Vec<serde_json::Value> {
        let written = WRITTEN.with(|written| written.borrow().clone());
        String::from_utf8(written)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    pub fn named(&self, event: &str) -> Vec<serde_json::Value> {
        self.events()
            .into_iter()
            .filter(|logged| logged["event.name"] == event)
            .collect()
    }
}
