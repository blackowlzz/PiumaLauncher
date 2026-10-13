use std::collections::{HashMap, VecDeque};
use std::sync::{LazyLock, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::instance::Instance;

const CAPACITY: usize = 10_000;

#[derive(Serialize, Clone)]
pub struct Line {
    pub seq: u64,
    pub text: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LineEvent<'a> {
    instance_id: &'a str,
    seq: u64,
    text: &'a str,
}

#[derive(Default)]
struct Buffer {
    next: u64,
    lines: VecDeque<Line>,
}

static BUFFERS: LazyLock<Mutex<HashMap<String, Buffer>>> = LazyLock::new(Default::default);

fn label(id: &str) -> String {
    format!("console-{id}")
}

pub fn history(id: &str) -> Vec<Line> {
    BUFFERS.lock().unwrap().get(id).map(|b| b.lines.iter().cloned().collect()).unwrap_or_default()
}

pub fn push(app: &AppHandle, id: &str, text: &str) {
    let seq = {
        let mut all = BUFFERS.lock().unwrap();
        let buf = all.entry(id.to_string()).or_default();
        let seq = buf.next;
        buf.next += 1;
        buf.lines.push_back(Line { seq, text: text.to_string() });
        if buf.lines.len() > CAPACITY {
            buf.lines.pop_front();
        }
        seq
    };
    let _ = app.emit_to(label(id), "console-line", LineEvent { instance_id: id, seq, text });
}

pub fn show(app: &AppHandle, inst: &Instance) {
    let label = label(&inst.id);
    if let Some(w) = app.get_webview_window(&label) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        return;
    }
    let url = WebviewUrl::App(format!("index.html?console={}", inst.id).into());
    let _ = WebviewWindowBuilder::new(app, &label, url)
        .title(format!("{} — Console", inst.name))
        .decorations(false)
        .inner_size(1000.0, 600.0)
        .min_inner_size(520.0, 320.0)
        .center()
        .build();
}

pub fn start(app: &AppHandle, inst: &Instance) {
    BUFFERS.lock().unwrap().remove(&inst.id);
    show(app, inst);
}
