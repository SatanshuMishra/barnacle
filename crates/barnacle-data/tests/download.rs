use std::collections::BTreeMap;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use barnacle_data::extract::silhouette::sha256_hex;
use barnacle_data::store::Downloaded;
use barnacle_data::store::StoreError;
use barnacle_data::store::download_from;

const COMMIT: &str = "3f7a1c2b9d8e4f6a0b1c2d3e4f5a6b7c8d9e0f1a";
const BUILDS: &str = r#"[[builds]]
version = "15.8.0"
build = 13187581
dir = "15.8.0_13187581"
dumped_at = "2026-09-10T08:56:09-07:00"
"#;

type Files = Arc<Mutex<BTreeMap<String, Vec<u8>>>>;
type Requests = Arc<Mutex<Vec<String>>>;

struct Remote {
    base: String,
    files: Files,
    requests: Requests,
}

impl Remote {
    fn put(&self, path: &str, body: &[u8]) {
        self.files
            .lock()
            .unwrap()
            .insert(path.to_owned(), body.to_vec());
    }

    fn publish_build(&self, content: &[u8]) -> String {
        let hash = sha256_hex(content)[..20].to_owned();
        self.put("/builds.toml", BUILDS.as_bytes());
        self.put(
            "/15.8.0_13187581/metadata.toml",
            format!(
                "version = \"15.8.0\"\nbuild = 13187581\n\n[files]\n\"content/GameParams.data\" = \"{hash}\"\n"
            )
            .as_bytes(),
        );
        self.put(&format!("/common/{}/{}", &hash[..2], &hash[2..]), content);
        hash
    }

    fn requested(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

fn respond(stream: TcpStream, files: &Files, requests: &Requests) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    reader.read_line(&mut request_line).unwrap();
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_owned();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap() <= 2 {
            break;
        }
    }
    requests.lock().unwrap().push(path.clone());
    let response = match files.lock().unwrap().get(&path) {
        Some(body) => [
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .into_bytes(),
            body.clone(),
        ]
        .concat(),
        None => {
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()
        }
    };
    (&stream).write_all(&response).unwrap();
}

fn serve() -> Remote {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let files: Files = Arc::new(Mutex::new(BTreeMap::new()));
    let requests: Requests = Arc::new(Mutex::new(Vec::new()));
    let (served_files, seen_requests) = (files.clone(), requests.clone());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (files, requests) = (served_files.clone(), seen_requests.clone());
            std::thread::spawn(move || respond(stream, &files, &requests));
        }
    });
    Remote {
        base,
        files,
        requests,
    }
}

fn run(remote: &Remote, store: &Path, requested: Option<u32>) -> Result<Downloaded, StoreError> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let client = reqwest::Client::new();
    runtime.block_on(download_from(
        &client,
        &remote.base,
        COMMIT,
        store,
        requested,
    ))
}

fn object_path(store: &Path, hash: &str) -> std::path::PathBuf {
    store.join("common").join(&hash[..2]).join(&hash[2..])
}

#[test]
fn a_changed_build_is_fetched_again_on_the_next_download() {
    let remote = serve();
    let store = tempfile::tempdir().unwrap();

    let first_hash = remote.publish_build(b"first GameParams");
    let first = run(&remote, store.path(), None).unwrap();
    assert_eq!(first.entry.dir, "15.8.0_13187581");
    assert_eq!(first.data_repo_commit, COMMIT);
    assert!(object_path(store.path(), &first_hash).is_file());

    let second_hash = remote.publish_build(b"second GameParams");
    run(&remote, store.path(), Some(13187581)).unwrap();
    let metadata =
        std::fs::read_to_string(store.path().join("15.8.0_13187581/metadata.toml")).unwrap();
    assert!(metadata.contains(&second_hash), "{metadata}");
    assert_eq!(
        std::fs::read(object_path(store.path(), &second_hash)).unwrap(),
        b"second GameParams"
    );
}

#[test]
fn an_unsafe_build_directory_is_refused_before_anything_else_is_fetched() {
    let remote = serve();
    let store = tempfile::tempdir().unwrap();
    remote.put(
        "/builds.toml",
        BUILDS
            .replace("dir = \"15.8.0_13187581\"", "dir = \"../escape\"")
            .as_bytes(),
    );
    let error = run(&remote, store.path(), None).err();
    assert!(matches!(error, Some(StoreError::UnsafeBuildDir { .. })));
    assert_eq!(remote.requested(), ["/builds.toml"]);
}

#[test]
fn a_build_that_is_not_published_is_reported() {
    let remote = serve();
    let store = tempfile::tempdir().unwrap();
    remote.publish_build(b"first GameParams");
    let error = run(&remote, store.path(), Some(1)).err();
    assert!(matches!(
        error,
        Some(StoreError::BuildNotInIndex { build: 1 })
    ));
}
