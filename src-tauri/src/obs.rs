//! The crosshair for OBS: a page OBS shows as a Browser source above its game
//! capture, kept in step with the real overlay through server-sent events.
//! OBS's game capture reads the game's own frames, so the overlay window is
//! never in them; this page puts the same crosshair into the stream instead.
//!
//! Only on while the player turns it on. It listens on 127.0.0.1 alone, talks
//! only to OBS on the same computer, and never touches a game.

use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

pub const PORT: u16 = 47117;
const PING: Duration = Duration::from_secs(20);

pub fn address() -> String {
    format!("http://127.0.0.1:{PORT}/obs")
}

/// The latest state as JSON, with a version that grows on every change.
#[derive(Default)]
struct Shared {
    state: Mutex<(u64, String)>,
    changed: Condvar,
    stop: AtomicBool,
}

/// The image the page shows, by its file name, served at `/image/<name>`.
type Picture = Option<(String, Arc<Vec<u8>>)>;

#[derive(Default)]
pub struct Obs {
    running: Mutex<Option<Arc<Shared>>>,
    /// Kept while off, so turning it on serves the current crosshair at once.
    last: Mutex<String>,
    picture: Arc<Mutex<Picture>>,
}

impl Obs {
    pub fn enabled(&self) -> bool {
        self.running.lock().unwrap().is_some()
    }

    /// Starts or stops the server; an error says why it could not start.
    pub fn set_enabled(&self, on: bool) -> Result<(), String> {
        let mut running = self.running.lock().unwrap();
        match (on, running.is_some()) {
            (true, false) => {
                let listener = TcpListener::bind(("127.0.0.1", PORT))
                    .map_err(|error| format!("Port {PORT} is taken by another program ({error})."))?;
                let shared = Arc::new(Shared::default());
                *shared.state.lock().unwrap() = (1, self.last.lock().unwrap().clone());
                let serving = Arc::clone(&shared);
                let picture = Arc::clone(&self.picture);
                std::thread::Builder::new()
                    .name("scorpio-obs".into())
                    .spawn(move || serve(listener, serving, picture))
                    .map_err(|error| error.to_string())?;
                *running = Some(shared);
            }
            (false, true) => {
                let shared = running.take().expect("checked above");
                shared.stop.store(true, Ordering::SeqCst);
                shared.changed.notify_all();
                // Wake the accept call so the thread sees the stop.
                let _ = TcpStream::connect(("127.0.0.1", PORT));
            }
            _ => {}
        }
        Ok(())
    }

    /// The image the stream view points at; set before the view is published.
    pub fn set_picture(&self, picture: Picture) {
        *self.picture.lock().unwrap() = picture;
    }

    /// The crosshair as the stream should show it, as JSON.
    pub fn publish(&self, json: String) {
        {
            let mut last = self.last.lock().unwrap();
            if *last == json {
                return;
            }
            last.clone_from(&json);
        }
        if let Some(shared) = self.running.lock().unwrap().as_ref() {
            let mut state = shared.state.lock().unwrap();
            *state = (state.0 + 1, json);
            shared.changed.notify_all();
        }
    }
}

fn serve(listener: TcpListener, shared: Arc<Shared>, picture: Arc<Mutex<Picture>>) {
    log::info!("OBS source listening on {}", address());
    for stream in listener.incoming() {
        if shared.stop.load(Ordering::SeqCst) {
            break;
        }
        let Ok(stream) = stream else { continue };
        let client = Arc::clone(&shared);
        let picture = Arc::clone(&picture);
        let _ = std::thread::Builder::new().name("scorpio-obs-client".into()).spawn(move || answer(stream, client, &picture));
    }
    log::info!("OBS source stopped");
}

/// The request's path, from its first line; the rest of the headers are read
/// and ignored.
fn request_path(stream: &TcpStream) -> Option<String> {
    let mut reader = BufReader::new(stream.try_clone().ok()?);
    let mut first = String::new();
    reader.read_line(&mut first).ok()?;
    let mut line = String::new();
    for _ in 0..64 {
        line.clear();
        if reader.read_line(&mut line).ok()? <= 2 {
            break;
        }
    }
    let mut parts = first.split_whitespace();
    (parts.next()? == "GET").then(|| parts.next().map(str::to_string)).flatten()
}

fn answer(mut stream: TcpStream, shared: Arc<Shared>, picture: &Mutex<Picture>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let path = request_path(&stream).unwrap_or_default();
    let path = path.split('?').next().unwrap_or("");
    let result = match path {
        "/obs" | "/" => respond(&mut stream, "200 OK", "text/html; charset=utf-8", PAGE.as_bytes()),
        "/events" => events(&mut stream, &shared),
        _ => {
            // Only the image in use, by its exact name.
            let current = picture.lock().unwrap().clone().filter(|(name, _)| path.strip_prefix("/image/") == Some(name.as_str()));
            match current {
                Some((_, png)) => respond(&mut stream, "200 OK", "image/png", &png),
                None => respond(&mut stream, "404 Not Found", "text/plain", b"Not found"),
            }
        }
    };
    let _ = result;
    let _ = stream.shutdown(Shutdown::Both);
}

fn respond(stream: &mut TcpStream, status: &str, kind: &str, body: &[u8]) -> std::io::Result<()> {
    write!(stream, "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n", body.len())?;
    stream.write_all(body)
}

/// Server-sent events: the current state now, then every change, with a
/// comment line now and then so a closed connection is noticed.
fn events(stream: &mut TcpStream, shared: &Shared) -> std::io::Result<()> {
    stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: keep-alive\r\n\r\n")?;
    let mut seen = 0;
    loop {
        let message = {
            let mut state = shared.state.lock().unwrap();
            while state.0 == seen && !shared.stop.load(Ordering::SeqCst) {
                let (next, timeout) = shared.changed.wait_timeout(state, PING).unwrap();
                state = next;
                if timeout.timed_out() {
                    break;
                }
            }
            if shared.stop.load(Ordering::SeqCst) {
                return Ok(());
            }
            if state.0 == seen {
                None
            } else {
                seen = state.0;
                Some(state.1.clone())
            }
        };
        match message {
            Some(json) => write!(stream, "data: {json}\n\n")?,
            None => stream.write_all(b": ping\n\n")?,
        }
        stream.flush()?;
    }
}

/// The Browser source page. Sized by OBS to the canvas; the crosshair is
/// scaled from the game's screen height to the canvas height, so it lands
/// where it does on the player's screen.
const PAGE: &str = r#"<!doctype html>
<html><head><meta charset="utf-8"><title>Scorpio crosshair</title>
<style>
html, body { margin: 0; height: 100%; overflow: hidden; background: transparent; }
#c { position: absolute; left: 50%; top: 50%; line-height: 0; }
#c svg, #c img { display: block; }
</style></head>
<body><div id="c"></div>
<script>
const box = document.getElementById('c')
let last = null
const draw = () => {
  const s = last
  if (!s || !s.visible || !(s.svg || s.image)) { box.innerHTML = ''; return }
  const k = innerHeight / s.height
  const crisp = Math.abs(k - 1) < 0.001
  let w, h, dx = 0, dy = 0
  if (s.image) {
    let img = box.firstElementChild
    if (!img || img.tagName !== 'IMG') { box.innerHTML = ''; img = document.createElement('img'); img.alt = ''; box.append(img) }
    if (img.getAttribute('src') !== s.image.url) img.src = s.image.url
    w = s.image.width; h = s.image.height
    img.width = w * k; img.height = h * k
    dx = s.image.nudge[0]; dy = s.image.nudge[1]
    img.style.opacity = s.image.opacity
    img.style.imageRendering = crisp ? 'pixelated' : 'auto'
  } else {
    box.innerHTML = s.svg
    const svg = box.firstElementChild
    w = svg.viewBox.baseVal.width; h = svg.viewBox.baseVal.height
    svg.setAttribute('width', w * k)
    svg.setAttribute('height', h * k)
    svg.style.shapeRendering = crisp ? 'crispEdges' : 'auto'
  }
  box.style.transform = `translate(${-w * k / 2 + (s.offset[0] + dx) * k}px, ${-h * k / 2 + (s.offset[1] + dy) * k}px)`
}
const connect = () => {
  const source = new EventSource('/events')
  source.onmessage = e => { last = JSON.parse(e.data); draw() }
  source.onerror = () => { source.close(); box.innerHTML = ''; setTimeout(connect, 2000) }
}
addEventListener('resize', draw)
connect()
</script></body></html>
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn get(path: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", PORT)).unwrap();
        write!(stream, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
        stream.set_read_timeout(Some(Duration::from_millis(600))).unwrap();
        let mut out = Vec::new();
        let mut chunk = [0u8; 4096];
        while let Ok(n) = stream.read(&mut chunk) {
            if n == 0 {
                break;
            }
            out.extend_from_slice(&chunk[..n]);
        }
        String::from_utf8_lossy(&out).into_owned()
    }

    #[test]
    fn serves_the_page_streams_changes_and_stops() {
        let obs = Obs::default();
        obs.publish(r#"{"visible":true}"#.into());
        obs.set_picture(Some(("0123456789abcdef.png".into(), Arc::new(b"PNGDATA".to_vec()))));
        obs.set_enabled(true).unwrap();
        assert!(get("/obs").contains("EventSource('/events')"));
        assert!(get("/nothing").starts_with("HTTP/1.1 404"));
        let image = get("/image/0123456789abcdef.png");
        assert!(image.starts_with("HTTP/1.1 200") && image.contains("image/png") && image.ends_with("PNGDATA"), "{image}");
        assert!(get("/image/fedcba9876543210.png").starts_with("HTTP/1.1 404"), "only the image in use");
        assert!(get("/image/../settings.json").starts_with("HTTP/1.1 404"));

        let reader = std::thread::spawn(|| get("/events"));
        std::thread::sleep(Duration::from_millis(150));
        obs.publish(r#"{"visible":false}"#.into());
        let stream = reader.join().unwrap();
        assert!(stream.contains(r#"data: {"visible":true}"#), "{stream}");
        assert!(stream.contains(r#"data: {"visible":false}"#), "{stream}");

        obs.set_enabled(false).unwrap();
        std::thread::sleep(Duration::from_millis(100));
        assert!(TcpStream::connect(("127.0.0.1", PORT)).is_err(), "the port is closed when turned off");
    }
}
