use crate::{
    hardware::{self, Encoder},
    media::{self, Media, Result, SourceInfo, Tools},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn uid() -> String {
    Uuid::new_v4().to_string()
}
pub fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
pub fn tail(path: &Path, limit: u64) -> String {
    let Ok(mut f) = File::open(path) else {
        return String::new();
    };
    if let Ok(meta) = f.metadata() {
        let _ = f.seek(SeekFrom::Start(meta.len().saturating_sub(limit)));
    }
    let mut s = Vec::new();
    let _ = f.take(limit).read_to_end(&mut s);
    String::from_utf8_lossy(&s).into()
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Preferences {
    pub encoder: String,
    pub ai_mode: String,
    pub local_model: String,
    pub captions: crate::ai::CaptionStyle,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            encoder: "auto".into(),
            ai_mode: "local".into(),
            local_model: "gemma3:1b".into(),
            captions: Default::default(),
        }
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub status: String,
    pub message: String,
    pub progress: Option<f64>,
    pub output_id: Option<String>,
    pub output_dir: Option<String>,
    pub created_at: u64,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveSession {
    pub id: String,
    pub title: String,
    pub url: String,
    pub provider: String,
    pub embed_url: Option<String>,
    pub phase: String,
    pub started_at: u64,
    pub buffer_seconds: f64,
    pub encoder: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub discovery: crate::discovery::Discovery,
    pub library: Vec<Media>,
    pub jobs: Vec<Job>,
    pub live: Option<LiveSession>,
    pub tools: Vec<media::ToolStatus>,
    pub encoders: Vec<Encoder>,
    pub preferences: Preferences,
    pub active_encoder: String,
    pub output_dir: String,
}

pub struct Engine {
    pub root: PathBuf,
    pub tools: Tools,
    pub encoders: Vec<Encoder>,
    pub preferences: Mutex<Preferences>,
    pub discovery: Mutex<crate::discovery::Discovery>,
    pub library: Mutex<Vec<Media>>,
    pub jobs: Mutex<Vec<Job>>,
    pub controls: Mutex<HashMap<String, Arc<AtomicBool>>>,
    pub live: Mutex<Option<LiveSession>>,
    children: Mutex<HashSet<u32>>,
    pub services: Mutex<Vec<Child>>,
}
#[cfg(unix)]
fn signal(pid: u32, force: bool) {
    unsafe {
        libc::kill(
            -(pid as i32),
            if force { libc::SIGKILL } else { libc::SIGTERM },
        );
    }
}
#[cfg(windows)]
fn signal(pid: u32, _force: bool) {
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status();
}
struct OwnedChild<'a> {
    child: Child,
    engine: &'a Engine,
}
impl Drop for OwnedChild<'_> {
    fn drop(&mut self) {
        let pid = self.child.id();
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            signal(pid, true);
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        self.engine.children.lock().unwrap().remove(&pid);
    }
}

impl Engine {
    pub fn new(root: PathBuf, resources: &Path) -> Result<Arc<Self>> {
        for d in [
            "library",
            "exports",
            "work",
            "transcripts",
            "highlights",
            "models",
        ] {
            fs::create_dir_all(root.join(d)).map_err(err)?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).map_err(err)?;
        }
        let library = if root.join("library.json").exists() {
            serde_json::from_slice(&fs::read(root.join("library.json")).map_err(err)?).map_err(
                |e| format!("Could not read the saved library; it has been preserved: {e}"),
            )?
        } else {
            Vec::new()
        };
        let preferences = fs::read(root.join("preferences.json"))
            .ok()
            .and_then(|d| serde_json::from_slice(&d).ok())
            .unwrap_or_default();
        let discovery = fs::read(root.join("clone-search.json"))
            .ok()
            .and_then(|data| serde_json::from_slice(&data).ok())
            .unwrap_or_default();
        let tools = Tools::discover(resources);
        let encoders = hardware::detect(tools.ffmpeg.as_deref());
        Ok(Arc::new(Self {
            root,
            tools,
            encoders,
            preferences: Mutex::new(preferences),
            discovery: Mutex::new(discovery),
            library: Mutex::new(library),
            jobs: Mutex::new(Vec::new()),
            controls: Mutex::new(HashMap::new()),
            live: Mutex::new(None),
            children: Mutex::new(HashSet::new()),
            services: Mutex::new(Vec::new()),
        }))
    }
    pub fn snapshot(&self) -> Snapshot {
        let mut library = self.library.lock().unwrap().clone();
        library.reverse();
        let mut jobs = self.jobs.lock().unwrap().clone();
        jobs.reverse();
        jobs.truncate(20);
        let mut live = self.live.lock().unwrap().clone();
        if let Some(l) = live.as_mut() {
            let s = self.buffer(&l.id);
            l.buffer_seconds = s.iter().map(|s| s.end - s.start).sum();
        }
        let preferences = self.preferences.lock().unwrap().clone();
        let active_encoder = self.encoder().label;
        Snapshot {
            discovery: self.discovery.lock().unwrap().clone(),
            library,
            jobs,
            live,
            tools: self.tools.status(),
            encoders: self.encoders.clone(),
            preferences,
            active_encoder,
            output_dir: self.root.join("exports").to_string_lossy().into(),
        }
    }
    pub fn encoder(&self) -> Encoder {
        let choice = self.preferences.lock().unwrap().encoder.clone();
        self.encoders
            .iter()
            .find(|e| e.available && (choice == "auto" || e.id == choice))
            .cloned()
            .unwrap_or_else(Encoder::cpu)
    }
    pub fn save_preferences(&self, p: Preferences) -> Result<()> {
        if p.encoder != "auto"
            && !self
                .encoders
                .iter()
                .any(|e| e.id == p.encoder && e.available)
        {
            return Err("That encoder is unavailable on this computer.".into());
        }
        if !["local", "codex"].contains(&p.ai_mode.as_str())
            || p.local_model.len() > 120
            || p.local_model.chars().any(|c| c.is_control())
        {
            return Err("Invalid AI settings.".into());
        }
        p.captions.validate()?;
        let mut current = self.preferences.lock().unwrap();
        self.atomic_json("preferences.json", &p)?;
        *current = p;
        Ok(())
    }
    pub fn atomic_json(&self, name: &str, value: &impl Serialize) -> Result<()> {
        let p = self.root.join(name);
        let tmp = p.with_extension("tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(value).map_err(err)?).map_err(err)?;
        fs::rename(tmp, p).map_err(err)
    }
    pub fn media(&self, id: &str) -> Result<Media> {
        self.library
            .lock()
            .unwrap()
            .iter()
            .find(|m| m.id == id)
            .cloned()
            .ok_or_else(|| "This video is no longer in the library.".into())
    }
    pub fn phase(&self, id: &str, message: &str, progress: Option<f64>) {
        if let Some(j) = self.jobs.lock().unwrap().iter_mut().find(|j| j.id == id) {
            j.message = message.into();
            j.progress = progress;
        }
    }
    pub fn active(&self) -> bool {
        self.jobs
            .lock()
            .unwrap()
            .iter()
            .any(|j| j.status == "running")
            || !self.children.lock().unwrap().is_empty()
    }
    pub fn cancel(&self, id: &str) -> Result<()> {
        let c = self
            .controls
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or("This task has already finished.")?;
        c.store(true, Ordering::Relaxed);
        Ok(())
    }
    pub fn shutdown(&self) {
        for c in self.controls.lock().unwrap().values() {
            c.store(true, Ordering::Relaxed);
        }
        for &pid in self.children.lock().unwrap().iter() {
            signal(pid, true);
        }
        for c in self.services.lock().unwrap().iter_mut() {
            signal(c.id(), true);
            let _ = c.kill();
            let _ = c.wait();
        }
    }
    pub fn task<F>(self: &Arc<Self>, kind: &str, label: &str, work: F) -> Result<String>
    where
        F: FnOnce(Arc<Engine>, String, Arc<AtomicBool>) -> Result<Option<Media>> + Send + 'static,
    {
        let id = uid();
        let ctl = Arc::new(AtomicBool::new(false));
        {
            let mut jobs = self.jobs.lock().unwrap();
            if jobs.iter().any(|j| {
                j.status == "running"
                    && ((kind == "live" && j.kind == "live")
                        || (kind != "live" && j.kind != "live"))
            }) {
                return Err("Wait for the current task to finish, or cancel it first.".into());
            }
            fs::create_dir_all(self.root.join("work").join(&id)).map_err(err)?;
            jobs.push(Job {
                id: id.clone(),
                kind: kind.into(),
                label: label.into(),
                status: "running".into(),
                message: "Starting…".into(),
                progress: None,
                output_id: None,
                output_dir: None,
                created_at: now(),
            });
        }
        self.controls
            .lock()
            .unwrap()
            .insert(id.clone(), ctl.clone());
        let engine = self.clone();
        let job_id = id.clone();
        let kind = kind.to_owned();
        thread::spawn(move || {
            let result = work(engine.clone(), job_id.clone(), ctl);
            if result.is_ok() && kind != "live" {
                let _ = fs::remove_dir_all(engine.root.join("work").join(&job_id));
            }
            if kind == "live" && result.is_err() {
                if let Some(l) = engine.live.lock().unwrap().as_mut() {
                    l.phase = "error".into();
                }
            }
            let mut jobs = engine.jobs.lock().unwrap();
            let j = jobs.iter_mut().find(|j| j.id == job_id).unwrap();
            match result {
                Ok(m) => {
                    j.status = "completed".into();
                    j.message = "Concluído".into();
                    j.progress = Some(1.0);
                    j.output_id = m.map(|m| m.id);
                }
                Err(e) => {
                    j.status = if e == "Cancelled" {
                        "cancelled"
                    } else {
                        "failed"
                    }
                    .into();
                    j.message = e;
                }
            }
            engine.controls.lock().unwrap().remove(&job_id);
        });
        Ok(id)
    }
    pub fn run(
        &self,
        mut cmd: Command,
        ctl: &AtomicBool,
        timeout: Duration,
        job: Option<(&str, Option<f64>)>,
        stdin: Option<&str>,
    ) -> Result<String> {
        let downloading = cmd.get_args().any(|a| a == media::DOWNLOAD_PROGRESS);
        let tag = uid();
        let logs = self.root.join("work");
        let out = logs.join(format!("{tag}.out"));
        let stderr = logs.join(format!("{tag}.err"));
        cmd.stdin(Stdio::piped())
            .stdout(File::create(&out).map_err(err)?)
            .stderr(File::create(&stderr).map_err(err)?);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        let child = cmd
            .spawn()
            .map_err(|e| format!("Could not start the media tool: {e}"))?;
        self.children.lock().unwrap().insert(child.id());
        let mut child = OwnedChild {
            child,
            engine: self,
        };
        if let Some(text) = stdin {
            if let Some(mut pipe) = child.child.stdin.take() {
                pipe.write_all(text.as_bytes()).map_err(err)?;
            }
        }
        let started = Instant::now();
        let mut stopping = None;
        let mut last = Instant::now();
        let status = loop {
            if let Some(s) = child.child.try_wait().map_err(err)? {
                break s;
            }
            if ctl.load(Ordering::Relaxed) || started.elapsed() > timeout {
                if stopping.is_none() {
                    if let Some(pipe) = child.child.stdin.as_mut() {
                        let _ = pipe.write_all(b"q\n");
                    }
                    stopping = Some(Instant::now());
                }
                if stopping.unwrap().elapsed() > Duration::from_secs(2) {
                    signal(child.child.id(), false);
                }
                if stopping.unwrap().elapsed() > Duration::from_secs(4) {
                    signal(child.child.id(), true);
                    let _ = child.child.kill();
                }
            }
            if last.elapsed() > Duration::from_millis(500) {
                if let Some((id, _)) = job {
                    if downloading {
                        if let Some((message, progress)) =
                            media::download_progress(&tail(&out, 8192))
                                .or_else(|| media::download_progress(&tail(&stderr, 8192)))
                        {
                            self.phase(id, &message, progress);
                        }
                    }
                    let auth = self
                        .jobs
                        .lock()
                        .unwrap()
                        .iter()
                        .any(|j| j.id == id && j.kind == "auth");
                    if auth {
                        let message = format!("{}\n{}", tail(&out, 2400), tail(&stderr, 2400));
                        if !message.trim().is_empty() {
                            self.phase(id, message.trim(), None);
                        }
                    }
                }
                if let Some((id, Some(total))) = job {
                    let log = tail(&out, 8192);
                    let seconds = log.lines().rev().find_map(|l| {
                        l.strip_prefix("out_time_us=")
                            .and_then(|v| v.parse::<f64>().ok())
                            .map(|v| v / 1e6)
                    });
                    if let Some(s) = seconds {
                        if let Some(j) = self.jobs.lock().unwrap().iter_mut().find(|j| j.id == id) {
                            j.progress = Some((s / total).clamp(0.0, 0.99));
                        }
                    }
                }
                last = Instant::now();
            }
            thread::sleep(Duration::from_millis(100));
        };
        let stdout = tail(&out, 16 * 1024 * 1024);
        let error = tail(&stderr, 5000);
        let _ = fs::remove_file(out);
        let _ = fs::remove_file(stderr);
        if ctl.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        if started.elapsed() > timeout {
            return Err("The operation timed out. Check the connection and try again.".into());
        }
        if !status.success() {
            return Err(if error.trim().is_empty() {
                format!("Media tool exited with {status}")
            } else {
                error.trim().into()
            });
        }
        Ok(stdout)
    }
    pub fn run_auth(&self, cmd: Command, ctl: &AtomicBool, id: &str) -> Result<()> {
        self.run(cmd, ctl, Duration::from_secs(900), Some((id, None)), None)
            .map(|_| ())
    }
    pub fn inspect(&self, url: &str) -> Result<SourceInfo> {
        let (url, provider) = media::source_url(url)?;
        let mut c = self.tools.ytdlp()?;
        c.args([
            "--dump-single-json",
            "--skip-download",
            "--no-check-formats",
            "--",
            &url,
        ]);
        let raw = self.run(
            c,
            &AtomicBool::new(false),
            Duration::from_secs(70),
            None,
            None,
        )?;
        let v: Value = serde_json::from_str(&raw)
            .map_err(|_| "The platform returned unreadable video information.")?;
        Ok(SourceInfo {
            url,
            id: v["id"].as_str().unwrap_or("").into(),
            title: v["title"].as_str().unwrap_or("Untitled video").into(),
            provider,
            duration: v["duration"].as_f64(),
            is_live: v["is_live"].as_bool().unwrap_or(false),
            thumbnail: v["thumbnail"].as_str().map(String::from),
            channel: v["channel"]
                .as_str()
                .or_else(|| v["uploader"].as_str())
                .unwrap_or("")
                .into(),
        })
    }
    pub fn probe(&self, path: &Path) -> Result<(f64, u32, u32)> {
        let mut c = Command::new(
            self.tools
                .ffprobe
                .as_ref()
                .ok_or("FFprobe is not installed.")?,
        );
        c.args([
            "-v",
            "error",
            "-show_streams",
            "-show_format",
            "-of",
            "json",
        ])
        .arg(path);
        let raw = self.run(
            c,
            &AtomicBool::new(false),
            Duration::from_secs(30),
            None,
            None,
        )?;
        let v: Value = serde_json::from_str(&raw).map_err(err)?;
        let s = v["streams"]
            .as_array()
            .and_then(|a| a.iter().find(|s| s["codec_type"] == "video"))
            .ok_or("The file does not contain a video track.")?;
        let duration = v["format"]["duration"]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|d| d.is_finite() && *d > 0.0)
            .ok_or("Could not determine the video's duration.")?;
        Ok((
            duration,
            s["width"].as_u64().unwrap_or(0) as u32,
            s["height"].as_u64().unwrap_or(0) as u32,
        ))
    }
    pub fn finish_media(
        &self,
        path: &Path,
        title: String,
        kind: &str,
        provider: &str,
        url: Option<String>,
    ) -> Result<Media> {
        let (duration, width, height) = self.probe(path)?;
        let id = uid();
        let thumb = self.root.join("library").join(format!("{id}.jpg"));
        let mut c = self.tools.ffmpeg()?;
        c.args(["-ss", &(duration / 3.0).min(2.0).to_string(), "-i"])
            .arg(path)
            .args(["-frames:v", "1", "-vf", "scale=480:-2"])
            .arg(&thumb);
        self.run(
            c,
            &AtomicBool::new(false),
            Duration::from_secs(30),
            None,
            None,
        )?;
        let m = Media {
            id,
            title,
            path: path.to_string_lossy().into(),
            thumbnail: thumb.to_string_lossy().into(),
            duration,
            width,
            height,
            kind: kind.into(),
            provider: provider.into(),
            source_url: url,
            created_at: now(),
        };
        let mut library = self.library.lock().unwrap();
        library.push(m.clone());
        self.atomic_json("library.json", &*library)?;
        Ok(m)
    }
    pub fn transcode(
        &self,
        input: &Path,
        output: &Path,
        start: f64,
        end: f64,
        format: &str,
        ass: Option<&Path>,
        job: &str,
        ctl: &AtomicBool,
    ) -> Result<()> {
        let selected = self.encoder();
        let mut encoders = vec![selected.clone()];
        if selected.id != "libx264" {
            encoders.push(Encoder::cpu());
        }
        let preparing = output.starts_with(self.root.join("library"));
        let mut failure = String::new();
        for (attempt, encoder) in encoders.iter().enumerate() {
            self.phase(
                job,
                &format!(
                    "{} · {}",
                    if attempt == 0 {
                        if preparing {
                            "Preparando vídeo"
                        } else {
                            "Exportando"
                        }
                    } else {
                        "GPU indisponível; tentando CPU"
                    },
                    encoder.label
                ),
                Some(0.0),
            );
            let mut c = self.tools.ffmpeg()?;
            encoder.prepare(&mut c);
            c.args(["-ss", &start.to_string(), "-i"])
                .arg(input)
                .args(["-t", &(end - start).to_string()]);
            media::encode_args(&mut c, format, encoder, ass.is_some());
            if let Some(ass) = ass {
                c.current_dir(ass.parent().unwrap());
            }
            c.arg(output);
            match self.run(
                c,
                ctl,
                Duration::from_secs(86400),
                Some((job, Some(end - start))),
                None,
            ) {
                Ok(_) => return Ok(()),
                Err(e) if e == "Cancelled" => {
                    let _ = fs::remove_file(output);
                    return Err(e);
                }
                Err(e) => failure = e,
            }
        }
        let _ = fs::remove_file(output);
        Err(failure)
    }
    pub fn import_local(self: &Arc<Self>, path: String) -> Result<String> {
        let path = fs::canonicalize(path).map_err(err)?;
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !path.is_file()
            || !["mp4", "mov", "mkv", "webm", "avi", "m4v", "ts"].contains(&ext.as_str())
        {
            return Err("Choose an MP4, MOV, MKV, WEBM, AVI, M4V, or TS video.".into());
        }
        self.task("import", "Importar arquivo", move |e, id, c| {
            let (d, _, _) = e.probe(&path)?;
            let title = path.file_stem().unwrap().to_string_lossy().to_string();
            let output = e.root.join("library").join(format!("{id}.mp4"));
            e.transcode(&path, &output, 0.0, d, "original", None, &id, &c)?;
            e.finish_media(&output, title, "source", "Local", None)
                .map(Some)
        })
    }
    pub fn import_url(self: &Arc<Self>, url: String) -> Result<String> {
        media::source_url(&url)?;
        self.task("import", "Importar vídeo", move |e, id, c| {
            e.import_url_inner(&url, &id, &c).map(Some)
        })
    }
    pub fn import_url_inner(&self, url: &str, id: &str, c: &AtomicBool) -> Result<Media> {
        media::source_url(url)?;

        self.phase(id, "Lendo vídeo…", Some(0.0));
        let info = self.inspect(&url)?;
        if info.is_live {
            return Err("This is live now. Use the live recording tab instead.".into());
        }
        let work = self.root.join("work").join(id);
        let mut cmd = self.tools.ytdlp()?;
        cmd.args([
            "-f",
            "bv*[vcodec^=avc1][height<=1080]+ba[acodec^=mp4a]/b[height<=1080]/best[height<=1080]",
            "--merge-output-format",
            "mp4",
            "--newline",
            "--progress",
            "--progress-delta",
            "0.25",
            "--progress-template",
            media::DOWNLOAD_PROGRESS,
            "--progress-template",
            "postprocess:cutline-postprocess",
            "-o",
        ])
        .arg(work.join("source.%(ext)s"))
        .args(["--", &url]);
        self.phase(id, "Baixando vídeo…", Some(0.0));
        self.run(cmd, c, Duration::from_secs(86400), Some((id, None)), None)?;
        self.phase(id, "Download concluído · preparando vídeo…", Some(1.0));
        let input = fs::read_dir(&work)
            .map_err(err)?
            .flatten()
            .map(|f| f.path())
            .find(|p| {
                p.file_stem().is_some_and(|s| s == "source")
                    && p.extension().is_some_and(|s| s != "part" && s != "ytdl")
            })
            .ok_or("Download finished without a video file.")?;
        let (d, _, _) = self.probe(&input)?;
        let output = self.root.join("library").join(format!("{id}.mp4"));
        self.transcode(&input, &output, 0.0, d, "original", None, id, c)?;
        let m = self.finish_media(
            &output,
            info.title,
            "source",
            &info.provider,
            Some(url.to_owned()),
        )?;
        let _ = fs::remove_file(input);
        Ok(m)
    }
    pub fn export(
        self: &Arc<Self>,
        media_id: String,
        start: f64,
        end: f64,
        format: String,
        title: String,
        captions: bool,
    ) -> Result<String> {
        if !["original", "vertical", "square"].contains(&format.as_str()) {
            return Err("Invalid export format.".into());
        }
        let m = self.media(&media_id)?;
        media::valid_range(start, end, m.duration)?;
        self.task("export", "Exportar clipe", move |e, id, c| {
            let output = e
                .root
                .join("exports")
                .join(format!("clip-{}.mp4", &id[..8]));
            let ass = if captions {
                Some(e.caption_file(&m.id, start, end, &format, &id)?)
            } else {
                None
            };
            e.transcode(
                Path::new(&m.path),
                &output,
                start,
                end,
                &format,
                ass.as_deref(),
                &id,
                &c,
            )?;
            e.finish_media(
                &output,
                if title.trim().is_empty() {
                    format!("{} · clipe", m.title)
                } else {
                    title.chars().take(160).collect()
                },
                "clip",
                &m.provider,
                m.source_url,
            )
            .map(Some)
        })
    }
    pub fn export_highlights(
        self: &Arc<Self>,
        media_id: String,
        format: String,
        captions: bool,
    ) -> Result<String> {
        if !["original", "vertical", "square"].contains(&format.as_str()) {
            return Err("Invalid export format.".into());
        }
        let media = self.media(&media_id)?;
        let highlights = self.highlights(&media_id)?;
        if highlights.is_empty() || highlights.len() > 12 {
            return Err("Generate 1–12 highlights before exporting.".into());
        }
        for h in &highlights {
            media::valid_range(h.start, h.end, media.duration)?;
        }
        self.task("export", "Exportar destaques", move |e, id, c| {
            let mut last = None;
            for (index, h) in highlights.iter().enumerate() {
                if c.load(Ordering::Relaxed) {
                    return Err("Cancelled".into());
                }
                let output =
                    e.root
                        .join("exports")
                        .join(format!("clip-{}-{:02}.mp4", &id[..8], index + 1));
                let ass = if captions {
                    Some(e.caption_file(&media.id, h.start, h.end, &format, &id)?)
                } else {
                    None
                };
                e.transcode(
                    Path::new(&media.path),
                    &output,
                    h.start,
                    h.end,
                    &format,
                    ass.as_deref(),
                    &id,
                    &c,
                )?;
                last = Some(e.finish_media(
                    &output,
                    h.title.clone(),
                    "clip",
                    &media.provider,
                    media.source_url.clone(),
                )?);
            }
            Ok(last)
        })
    }
    pub fn buffer(&self, id: &str) -> Vec<media::Segment> {
        media::segments(
            &fs::read_to_string(self.root.join("work").join(id).join("buffer.csv"))
                .unwrap_or_default(),
        )
    }
    pub fn start_live(self: &Arc<Self>, url: String) -> Result<String> {
        media::source_url(&url)?;
        self.task("live", "Gravar transmissão", move |e, id, c| {
            e.phase(&id, "Conectando à transmissão…", None);
            let info = e.inspect(&url)?;
            if !info.is_live {
                return Err(
                    "This link is not live right now. Import it as a video instead.".into(),
                );
            }
            let mut cmd = e.tools.ytdlp()?;
            cmd.args([
                "--dump-single-json",
                "--skip-download",
                "-f",
                "b[height<=1080][vcodec^=avc1]/b[height<=1080]/b",
                "--",
                &url,
            ]);
            let raw = e.run(cmd, &c, Duration::from_secs(70), Some((&id, None)), None)?;
            let v: Value = serde_json::from_str(&raw).map_err(err)?;
            let stream = v["url"]
                .as_str()
                .ok_or("No compatible live stream was returned.")?;
            let parsed = url::Url::parse(stream).map_err(err)?;
            if !["http", "https"].contains(&parsed.scheme()) {
                return Err("Unsupported live stream protocol.".into());
            }
            let headers = v["http_headers"]
                .as_object()
                .map(|h| {
                    h.iter()
                        .filter_map(|(k, v)| {
                            v.as_str()
                                .filter(|s| !s.contains(['\r', '\n']) && !k.contains(['\r', '\n']))
                                .map(|v| format!("{k}: {v}\r\n"))
                        })
                        .collect::<String>()
                })
                .unwrap_or_default();
            e.capture(&id, &c, &info, stream, &headers, false)
        })
    }
    pub fn capture(
        &self,
        id: &str,
        ctl: &AtomicBool,
        info: &SourceInfo,
        stream: &str,
        headers: &str,
        fixture: bool,
    ) -> Result<Option<Media>> {
        let work = self.root.join("work").join(id);
        let recording = work.join("recording.mkv");
        let encoder = self.encoder();
        let embed = if info.provider == "YouTube" {
            Some(format!(
                "https://www.youtube-nocookie.com/embed/{}?autoplay=1",
                info.id
            ))
        } else if info.provider == "Kick" {
            url::Url::parse(&info.url).ok().and_then(|u| {
                u.path_segments()
                    .and_then(|mut p| p.next())
                    .map(|s| format!("https://player.kick.com/{s}"))
            })
        } else {
            None
        };
        *self.live.lock().unwrap() = Some(LiveSession {
            id: id.into(),
            title: info.title.clone(),
            url: info.url.clone(),
            provider: info.provider.clone(),
            embed_url: embed,
            phase: "recording".into(),
            started_at: now(),
            buffer_seconds: 0.0,
            encoder: encoder.label.clone(),
        });
        let mut cmd = self.tools.ffmpeg()?;
        encoder.prepare(&mut cmd);
        if fixture {
            cmd.args(["-re", "-stream_loop", "-1"]);
        } else {
            cmd.args([
                "-rw_timeout",
                "20000000",
                "-protocol_whitelist",
                "http,https,tcp,tls,crypto,data",
            ]);
            if !headers.is_empty() {
                cmd.args(["-headers", headers]);
            }
        }
        cmd.args([
            "-i", stream, "-map", "0:v:0", "-map", "0:a:0?", "-c", "copy",
        ])
        .arg(&recording);
        cmd.args(["-map","0:v:0","-map","0:a:0?","-vf",&format!("scale=1920:1080:force_original_aspect_ratio=decrease:force_divisible_by=2,setsar=1{}",encoder.upload_filter())]);
        encoder.args(&mut cmd);
        cmd.args([
            "-r",
            "30",
            "-g",
            "60",
            "-force_key_frames",
            "expr:gte(t,n_forced*2)",
            "-c:a",
            "aac",
            "-b:a",
            "160k",
            "-f",
            "segment",
            "-segment_time",
            "2",
            "-segment_format",
            "mpegts",
            "-reset_timestamps",
            "1",
            "-segment_list_type",
            "csv",
            "-segment_list_size",
            "90",
            "-segment_wrap",
            "120",
            "-segment_list",
            "buffer.csv",
            "segment_%05d.ts",
        ])
        .current_dir(&work);
        self.phase(id, &format!("Gravando · {}", encoder.label), None);
        let result = self.run(cmd, ctl, Duration::from_secs(86400), Some((id, None)), None);
        if let Some(l) = self.live.lock().unwrap().as_mut() {
            l.phase = "finalizing".into();
        }
        if let Err(ref error) = result {
            if error != "Cancelled" && self.probe(&recording).is_err() {
                return Err(error.clone());
            }
        }
        self.phase(id, "Salvando gravação…", None);
        let (duration, _, _) = self.probe(&recording)?;
        let output = self.root.join("library").join(format!("{id}.mp4"));
        self.transcode(
            &recording,
            &output,
            0.0,
            duration,
            "original",
            None,
            id,
            &AtomicBool::new(false),
        )?;
        let m = self.finish_media(
            &output,
            info.title.clone(),
            "recording",
            &info.provider,
            Some(info.url.clone()),
        )?;
        if let Some(l) = self.live.lock().unwrap().as_mut() {
            l.phase = "stopped".into();
        }
        let _ = fs::remove_file(recording);
        Ok(Some(m))
    }
    pub fn save_replay(self: &Arc<Self>, seconds: f64) -> Result<String> {
        let live = self
            .live
            .lock()
            .unwrap()
            .clone()
            .ok_or("Start a live stream first.")?;
        let (selected, trim) = media::recent_segments(&self.buffer(&live.id), seconds)?;
        // Keep 30 extra segment slots beyond the playlist so completed files cannot wrap during a snapshot.
        let source = self.root.join("work").join(&live.id);
        self.task("replay", "Salvar replay", move |e, id, c| {
            let work = e.root.join("work").join(&id);
            let mut list = String::new();
            for (n, s) in selected.iter().enumerate() {
                let name = format!("snapshot-{n}.ts");
                fs::copy(source.join(&s.name), work.join(&name))
                    .map_err(|x| format!("Could not snapshot the buffer: {x}"))?;
                list.push_str(&format!("file '{name}'\n"));
            }
            fs::write(work.join("input.txt"), list).map_err(err)?;
            let mut cmd = e.tools.ffmpeg()?;
            cmd.args([
                "-f",
                "concat",
                "-safe",
                "1",
                "-i",
                "input.txt",
                "-c",
                "copy",
            ])
            .arg(work.join("replay.mkv"))
            .current_dir(&work);
            e.run(cmd, &c, Duration::from_secs(90), Some((&id, None)), None)?;
            let output = e
                .root
                .join("exports")
                .join(format!("replay-{}.mp4", &id[..8]));
            e.transcode(
                &work.join("replay.mkv"),
                &output,
                trim,
                trim + seconds,
                "original",
                None,
                &id,
                &c,
            )?;
            e.finish_media(
                &output,
                format!("{} · últimos {seconds:.0}s", live.title),
                "clip",
                &live.provider,
                Some(live.url),
            )
            .map(Some)
        })
    }
}
