pub mod ai;
pub mod discovery;
pub mod engine;
pub mod hardware;
pub mod media;
pub mod package;
use engine::{Engine, Preferences, Snapshot};
use media::{Result, SourceInfo};
use std::sync::Arc;
use tauri::{Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
fn clone_search(engine: State<Arc<Engine>>, request: discovery::CloneRequest) -> Result<String> {
    engine.inner().clone_search(request)
}
#[tauri::command]
fn clone_download(engine: State<Arc<Engine>>, urls: Vec<String>) -> Result<String> {
    engine.inner().clone_download(urls)
}
#[tauri::command]
fn open_clone_source(app: tauri::AppHandle, url: String) -> Result<()> {
    let (url, _) = discovery::clone_url(&url)?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(engine::err)
}
#[tauri::command]
fn get_snapshot(engine: State<Arc<Engine>>) -> Snapshot {
    engine.snapshot()
}
#[tauri::command]
async fn inspect_source(engine: State<'_, Arc<Engine>>, url: String) -> Result<SourceInfo> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || e.inspect(&url))
        .await
        .map_err(engine::err)?
}
#[tauri::command]
fn import_local(engine: State<Arc<Engine>>, path: String) -> Result<String> {
    engine.inner().import_local(path)
}
#[tauri::command]
fn import_url(engine: State<Arc<Engine>>, url: String) -> Result<String> {
    engine.inner().import_url(url)
}
#[tauri::command]
fn generate_package(
    engine: State<Arc<Engine>>,
    request: package::PackageRequest,
) -> Result<String> {
    engine.inner().generate_package(request)
}
#[tauri::command]
fn reveal_job_output(engine: State<Arc<Engine>>, app: tauri::AppHandle, id: String) -> Result<()> {
    app.opener()
        .open_path(engine.job_output_dir(&id)?.to_string_lossy(), None::<&str>)
        .map_err(engine::err)
}
#[tauri::command]
fn export_clip(
    engine: State<Arc<Engine>>,
    media_id: String,
    start: f64,
    end: f64,
    format: String,
    title: String,
    captions: bool,
) -> Result<String> {
    engine
        .inner()
        .export(media_id, start, end, format, title, captions)
}
#[tauri::command]
fn export_highlights(
    engine: State<Arc<Engine>>,
    media_id: String,
    format: String,
    captions: bool,
) -> Result<String> {
    engine.inner().export_highlights(media_id, format, captions)
}
#[tauri::command]
fn cancel_job(engine: State<Arc<Engine>>, id: String) -> Result<()> {
    engine.cancel(&id)
}
#[tauri::command]
fn start_live(engine: State<Arc<Engine>>, url: String) -> Result<String> {
    engine.inner().start_live(url)
}
#[tauri::command]
fn save_replay(engine: State<Arc<Engine>>, seconds: f64) -> Result<String> {
    engine.inner().save_replay(seconds)
}
#[tauri::command]
fn save_preferences(engine: State<Arc<Engine>>, preferences: Preferences) -> Result<()> {
    engine.save_preferences(preferences)
}
#[tauri::command]
async fn get_ai_status(engine: State<'_, Arc<Engine>>) -> Result<ai::AiStatus> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || e.ai_status())
        .await
        .map_err(engine::err)
}
#[tauri::command]
fn prepare_ai(engine: State<Arc<Engine>>) -> Result<String> {
    engine.inner().prepare_ai()
}
#[tauri::command]
fn login_codex(engine: State<Arc<Engine>>) -> Result<String> {
    engine.inner().login_codex()
}
#[tauri::command]
fn transcribe(engine: State<Arc<Engine>>, id: String, lang: String) -> Result<String> {
    engine.inner().transcribe(id, lang)
}
#[tauri::command]
fn get_transcript(engine: State<Arc<Engine>>, id: String) -> Result<Vec<ai::Word>> {
    engine.transcript(&id)
}
#[tauri::command]
fn save_transcript(engine: State<Arc<Engine>>, id: String, words: Vec<ai::Word>) -> Result<bool> {
    engine.save_transcript(&id, words)?;
    Ok(true)
}
#[tauri::command]
fn get_highlights(engine: State<Arc<Engine>>, id: String) -> Result<Vec<ai::Highlight>> {
    engine.highlights(&id)
}
#[tauri::command]
fn generate_highlights(
    engine: State<Arc<Engine>>,
    media_id: String,
    count: usize,
    min: f64,
    max: f64,
    lang: String,
) -> Result<String> {
    engine
        .inner()
        .generate_highlights(media_id, count, min, max, lang)
}
#[tauri::command]
fn reveal_media(engine: State<Arc<Engine>>, app: tauri::AppHandle, id: String) -> Result<()> {
    app.opener()
        .reveal_item_in_dir(engine.media(&id)?.path)
        .map_err(engine::err)
}
#[tauri::command]
fn reveal_exports(engine: State<Arc<Engine>>, app: tauri::AppHandle) -> Result<()> {
    app.opener()
        .open_path(engine.root.join("exports").to_string_lossy(), None::<&str>)
        .map_err(engine::err)
}
#[tauri::command]
fn open_source(engine: State<Arc<Engine>>, app: tauri::AppHandle) -> Result<()> {
    let live = engine
        .live
        .lock()
        .unwrap()
        .clone()
        .ok_or("No stream is selected.")?;
    let (url, _) = media::source_url(&live.url)?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(engine::err)
}
#[tauri::command]
fn open_auth_page(app: tauri::AppHandle) -> Result<()> {
    app.opener()
        .open_url("https://auth.openai.com/codex/device", None::<&str>)
        .map_err(engine::err)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let e = Engine::new(app.path().app_data_dir()?, &app.path().resource_dir()?)
                .map_err(std::io::Error::other)?;
            app.manage(e);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.state::<Arc<Engine>>().active() {
                    api.prevent_close();
                    let _ = window.emit("close-blocked", ());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            clone_search,
            clone_download,
            open_clone_source,
            inspect_source,
            import_local,
            import_url,
            generate_package,
            reveal_job_output,
            export_clip,
            export_highlights,
            cancel_job,
            start_live,
            save_replay,
            save_preferences,
            get_ai_status,
            prepare_ai,
            login_codex,
            transcribe,
            get_transcript,
            save_transcript,
            get_highlights,
            generate_highlights,
            reveal_media,
            reveal_exports,
            open_source,
            open_auth_page
        ])
        .build(tauri::generate_context!())
        .expect("Could not start Cutline");
    app.run(|app, event| match event {
        tauri::RunEvent::ExitRequested { api, .. } => {
            if app.state::<Arc<Engine>>().active() {
                api.prevent_exit();
                let _ = app.emit("close-blocked", ());
            }
        }
        tauri::RunEvent::Exit => app.state::<Arc<Engine>>().shutdown(),
        _ => {}
    });
}
