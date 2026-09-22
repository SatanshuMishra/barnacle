use std::io;
use std::sync::Arc;
use std::sync::Mutex;

use barnacle_bot::logging;
use barnacle_bot::logging::LogFormat;
use barnacle_bot::logging::LogSettings;
use tracing::subscriber::DefaultGuard;
use tracing_subscriber::fmt::MakeWriter;

#[derive(Clone, Default)]
struct Written(Arc<Mutex<Vec<u8>>>);

impl io::Write for Written {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'w> MakeWriter<'w> for Written {
    type Writer = Written;

    fn make_writer(&'w self) -> Written {
        self.clone()
    }
}

pub struct Logs {
    written: Written,
    _installed: DefaultGuard,
}

pub fn capture() -> Logs {
    let written = Written::default();
    let settings = LogSettings {
        format: LogFormat::Json,
        level: "debug".to_owned(),
    };
    let installed =
        tracing::subscriber::set_default(logging::subscriber(&settings, written.clone(), false));
    Logs {
        written,
        _installed: installed,
    }
}

impl Logs {
    pub fn events(&self) -> Vec<serde_json::Value> {
        let written = self.written.0.lock().unwrap().clone();
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
