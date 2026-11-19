use cutline_lib::{
    ai::{Highlight, Word},
    engine::{uid, Engine, Job},
    media::SourceInfo,
    package::PackageRequest,
};
use std::{
    path::Path,
    sync::{atomic::AtomicBool, Arc},
    thread,
    time::{Duration, Instant},
};

struct Rig {
    engine: Arc<Engine>,
    _dir: tempfile::TempDir,
}
impl Rig {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let engine = Engine::new(dir.path().join("data"), Path::new(".")).unwrap();
        Self { engine, _dir: dir }
    }
    fn fixture(&self) -> String {
        let path = self.engine.root.join("fixture.mp4");
        let mut c = self.engine.tools.ffmpeg().unwrap();
        c.args([
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=640x360:rate=30",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000",
            "-t",
            "8",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
        ])
        .arg(&path);
        self.engine
            .run(
                c,
                &AtomicBool::new(false),
                Duration::from_secs(30),
                None,
                None,
            )
            .unwrap();
        path.to_string_lossy().into()
    }
}
impl Drop for Rig {
    fn drop(&mut self) {
        self.engine.shutdown();
    }
}
fn wait(e: &Engine, id: &str, seconds: u64) -> Job {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        let job = e.snapshot().jobs.into_iter().find(|j| j.id == id).unwrap();
        if job.status != "running" {
            return job;
        }
        assert!(Instant::now() < deadline, "Timed out: {}", job.message);
        thread::sleep(Duration::from_millis(200));
    }
}
fn complete(e: &Engine, id: &str, seconds: u64) -> Job {
    let job = wait(e, id, seconds);
    assert_eq!(job.status, "completed", "{}", job.message);
    job
}

#[test]
#[ignore = "requires FFmpeg; exercises real files and hardware when available"]
fn import_trim_captions_batch_and_cancel() {
    let mut rig = Rig::new();
    let fixture = rig.fixture();
    let e = &rig.engine;
    println!("Detected encoders: {:?}", e.encoders);
    let source = complete(e, &e.import_local(fixture).unwrap(), 60)
        .output_id
        .unwrap();
    e.save_transcript(
        &source,
        vec![
            Word {
                start: 1.0,
                end: 2.5,
                text: "Caption test".into(),
            },
            Word {
                start: 2.5,
                end: 4.0,
                text: "works".into(),
            },
        ],
    )
    .unwrap();
    for format in ["original", "vertical", "square"] {
        let id = e
            .export(source.clone(), 1.0, 4.0, format.into(), format.into(), true)
            .unwrap();
        let m = e.media(&complete(e, &id, 90).output_id.unwrap()).unwrap();
        assert!((m.duration - 3.0).abs() < 0.15, "duration {}", m.duration);
        match format {
            "vertical" => assert_eq!((m.width, m.height), (1080, 1920)),
            "square" => assert_eq!((m.width, m.height), (1080, 1080)),
            _ => assert_eq!((m.width, m.height), (640, 360)),
        }
    }
    let before = e.library.lock().unwrap().len();
    e.atomic_json(
        &format!("highlights/{source}.json"),
        &vec![
            Highlight {
                id: uid(),
                start: 0.0,
                end: 2.0,
                title: "First".into(),
                reason: String::new(),
                description: "A test clip.".into(),
                hashtags: vec!["#Test".into()],
                score: 90.0,
            },
            Highlight {
                id: uid(),
                start: 4.0,
                end: 6.0,
                title: "Second".into(),
                reason: String::new(),
                description: "A test clip.".into(),
                hashtags: vec!["#Test".into()],
                score: 80.0,
            },
        ],
    )
    .unwrap();
    complete(
        e,
        &e.export_highlights(source.clone(), "original".into(), false)
            .unwrap(),
        60,
    );
    assert_eq!(e.library.lock().unwrap().len(), before + 2);
    assert!(e
        .export(
            source.clone(),
            -1.0,
            2.0,
            "original".into(),
            "Invalid".into(),
            false
        )
        .is_err());
    let id = e
        .task("test", "Cancellation", |e, id, c| {
            let mut cmd = e.tools.ffmpeg()?;
            cmd.args([
                "-re",
                "-f",
                "lavfi",
                "-i",
                "color=size=64x64:rate=5",
                "-t",
                "60",
                "-f",
                "null",
                "-",
            ]);
            e.run(cmd, &c, Duration::from_secs(80), Some((&id, None)), None)?;
            Ok(None)
        })
        .unwrap();
    thread::sleep(Duration::from_millis(500));
    e.cancel(&id).unwrap();
    assert_eq!(wait(e, &id, 10).status, "cancelled");
    assert!(!e.active());
    // Force an unavailable GPU to verify the real CPU retry, not just encoder discovery.
    let e = Arc::get_mut(&mut rig.engine).unwrap();
    e.encoders = vec![cutline_lib::hardware::Encoder {
        id: "h264_nvenc".into(),
        label: "Unavailable test GPU".into(),
        available: true,
        detail: String::new(),
        device: None,
    }];
    let id = rig
        .engine
        .export(
            source,
            0.0,
            2.0,
            "original".into(),
            "CPU retry".into(),
            false,
        )
        .unwrap();
    complete(&rig.engine, &id, 60);
}

#[test]
#[ignore = "requires FFmpeg and runs a real rolling capture for at least 65 seconds"]
fn rolling_buffer_and_full_recording() {
    let rig = Rig::new();
    let fixture = rig.fixture();
    let e = &rig.engine;
    let info = SourceInfo {
        url: "https://youtube.com/watch?v=fixture".into(),
        id: "fixture".into(),
        title: "Replay fixture".into(),
        provider: "Local".into(),
        duration: None,
        is_live: true,
        thumbnail: None,
        channel: String::new(),
    };
    let live = e
        .task("live", "Test live", move |e, id, c| {
            e.capture(&id, &c, &info, &fixture, "", true)
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(100);
    loop {
        let snapshot = e.snapshot();
        assert!(
            snapshot.jobs.iter().all(|j| j.status != "failed"),
            "Capture failed"
        );
        if snapshot
            .live
            .as_ref()
            .is_some_and(|l| l.buffer_seconds >= 62.0)
        {
            break;
        }
        assert!(Instant::now() < deadline, "Buffer did not fill");
        thread::sleep(Duration::from_secs(1));
    }
    for seconds in [30.0, 60.0] {
        let replay = e.save_replay(seconds).unwrap();
        let clip = e
            .media(&complete(e, &replay, 90).output_id.unwrap())
            .unwrap();
        assert!(
            (clip.duration - seconds).abs() < 0.2,
            "{} vs {seconds}",
            clip.duration
        );
    }
    e.cancel(&live).unwrap();
    let recording = e.media(&complete(e, &live, 90).output_id.unwrap()).unwrap();
    assert!(recording.duration >= 62.0);
    assert_eq!(recording.kind, "recording");
    assert!(!e.active());
}

#[test]
#[ignore = "downloads a public YouTube video to verify real progress updates"]
fn youtube_import_reports_download_progress() {
    let rig = Rig::new();
    let e = &rig.engine;
    let url = std::env::var("CUTLINE_TEST_YOUTUBE").expect("Set a public YouTube video URL");
    let id = e.import_url(url).unwrap();
    let deadline = Instant::now() + Duration::from_secs(300);
    let mut saw_download = false;
    let mut saw_preparation = false;
    let mut last_message = String::new();
    loop {
        let job = e.snapshot().jobs.into_iter().find(|j| j.id == id).unwrap();
        if job.message.starts_with("Baixando") && job.progress.is_some_and(|p| p > 0.0 && p < 1.0) {
            saw_download = true;
            if job.message != last_message {
                println!("{:.1}% · {}", job.progress.unwrap() * 100.0, job.message);
                last_message = job.message.clone();
            }
        }
        saw_preparation |= job.message.starts_with("Preparando vídeo");
        if job.status != "running" {
            assert_eq!(job.status, "completed", "{}", job.message);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Import timed out: {}",
            job.message
        );
        thread::sleep(Duration::from_millis(100));
    }
    assert!(
        saw_download,
        "No live download percentage reached the job snapshot"
    );
    assert!(
        saw_preparation,
        "Download and video preparation must be distinct stages"
    );
}

#[test]
#[ignore = "requires public YouTube/Kick URLs and network access"]
fn platform_download_and_live_capture() {
    let rig = Rig::new();
    let e = &rig.engine;
    let youtube = std::env::var("CUTLINE_TEST_YOUTUBE").expect("Set a public YouTube video URL");
    let imported = e.import_url(youtube).unwrap();
    let media = e
        .media(&complete(e, &imported, 300).output_id.unwrap())
        .unwrap();
    assert_eq!(media.provider, "YouTube");
    assert!(media.duration > 1.0);
    let kick = std::env::var("CUTLINE_TEST_KICK").expect("Set a currently live Kick channel");
    let live = e.start_live(kick).unwrap();
    let deadline = Instant::now() + Duration::from_secs(100);
    loop {
        let s = e.snapshot();
        let job = s.jobs.iter().find(|j| j.id == live).unwrap();
        assert_eq!(job.status, "running", "{}", job.message);
        if s.live.is_some_and(|l| l.buffer_seconds >= 8.0) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Live stream did not produce segments"
        );
        thread::sleep(Duration::from_secs(1));
    }
    e.cancel(&live).unwrap();
    let recorded = e
        .media(&complete(e, &live, 120).output_id.unwrap())
        .unwrap();
    assert_eq!(recorded.provider, "Kick");
    assert!(recorded.duration >= 8.0);
}

#[test]
#[ignore = "downloads local models and tests local Whisper/Ollama plus the existing Codex subscription"]
fn both_ai_modes() {
    let test_mode = std::env::var("CUTLINE_TEST_AI_MODE").unwrap_or("both".into());
    let root =
        std::env::var("CUTLINE_AI_ROOT").expect("Set CUTLINE_AI_ROOT to the app data directory");
    let rig = Rig::new();
    let e = rig.engine.clone();
    std::fs::hard_link(
        Path::new(&root).join("models/ggml-base.bin"),
        e.root.join("models/ggml-base.bin"),
    )
    .expect("Prepare the Whisper model in the app before this check");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut p = e.preferences.lock().unwrap().clone();
        p.ai_mode = if test_mode == "codex" {
            "codex"
        } else {
            "local"
        }
        .into();
        e.save_preferences(p.clone()).unwrap();
        complete(&e, &e.prepare_ai().unwrap(), 1800);
        let wav = Path::new(env!("CARGO_MANIFEST_DIR")).join("../.tools/jfk.wav");
        assert!(wav.is_file(), "Download the whisper.cpp JFK sample first");
        let path = e.root.join("work/speech-fixture.mp4");
        let mut c = e.tools.ffmpeg().unwrap();
        c.args(["-f", "lavfi", "-i", "color=c=0x131918:s=640x360:r=30", "-i"])
            .arg(wav)
            .args([
                "-shortest",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-c:a",
                "aac",
            ])
            .arg(&path);
        e.run(
            c,
            &AtomicBool::new(false),
            Duration::from_secs(30),
            None,
            None,
        )
        .unwrap();
        let m = e
            .finish_media(&path, "Speech verification".into(), "source", "Local", None)
            .unwrap();
        complete(&e, &e.transcribe(m.id.clone(), "en".into()).unwrap(), 180);
        let words = e.transcript(&m.id).unwrap();
        let text = words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        println!("Whisper transcript: {text}");
        assert!(text.to_lowercase().contains("country"));
        for mode in ["local", "codex"]
            .into_iter()
            .filter(|mode| test_mode == "both" || test_mode == *mode)
        {
            p.ai_mode = mode.into();
            e.save_preferences(p.clone()).unwrap();
            std::fs::remove_file(e.root.join("transcripts").join(format!("{}.json", m.id)))
                .unwrap();
            let exported = complete(
                &e,
                &e.generate_package(PackageRequest {
                    url: None,
                    media_id: Some(m.id.clone()),
                    count: 1,
                    min: 5.0,
                    max: 10.0,
                    lang: "en".into(),
                    format: "vertical".into(),
                })
                .unwrap(),
                600,
            );
            let folder = e.job_output_dir(&exported.id).unwrap();
            let manifest: serde_json::Value =
                serde_json::from_slice(&std::fs::read(folder.join("manifest.json")).unwrap())
                    .unwrap();
            assert_eq!(manifest["status"], "completed");
            assert_eq!(manifest["completedClips"], 1);
            let clip = e.media(&exported.output_id.unwrap()).unwrap();
            assert_eq!((clip.width, clip.height), (1080, 1920));
            assert!(clip.duration >= 4.9 && clip.duration <= 10.2);
            for file in [
                "video.mp4",
                "titulo.txt",
                "descricao.txt",
                "hashtags.txt",
                "youtube.txt",
                "tiktok.txt",
                "metadata.json",
            ] {
                assert!(
                    std::fs::metadata(folder.join("01").join(file))
                        .unwrap()
                        .len()
                        > 0
                );
            }
            let highlights = e.highlights(&m.id).unwrap();
            assert!(!highlights.is_empty());
            println!("{mode}: {:?}", highlights);
        }
        p.ai_mode = "local".into();
        e.save_preferences(p).unwrap();
    }));
    e.shutdown();
    if let Err(panic) = result {
        std::panic::resume_unwind(panic);
    }
}

#[test]
#[ignore = "requires FFmpeg with libass; validates actual exported packages and recovery"]
fn package_files_and_partial_results_survive_failure() {
    let rig = Rig::new();
    let e = &rig.engine;
    let source_id = complete(e, &e.import_local(rig.fixture()).unwrap(), 60)
        .output_id
        .unwrap();
    let source = e.media(&source_id).unwrap();
    e.save_transcript(
        &source.id,
        vec![Word {
            start: 0.0,
            end: 2.0,
            text: "A real clip".into(),
        }],
    )
    .unwrap();
    let good = Highlight {
        id: uid(),
        start: 0.0,
        end: 2.0,
        title: "Test title".into(),
        description: "A test scene with captions.".into(),
        hashtags: vec!["#Test".into()],
        reason: "".into(),
        score: 90.0,
    };
    let mut bad = good.clone();
    bad.description.clear();
    bad.start = 4.0;
    bad.end = 6.0;
    let copy = source.clone();
    let id = e
        .task("package", "Export test", move |e, job, ctl| {
            e.write_package(&copy, &[good, bad], 2, "original", &job, &ctl)
                .map(Some)
        })
        .unwrap();
    let failed = wait(e, &id, 60);
    assert_eq!(failed.status, "failed");
    let folder = e.job_output_dir(&id).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(folder.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["status"], "failed");
    assert_eq!(manifest["completedClips"], 1);
    assert_eq!(manifest["clips"].as_array().unwrap().len(), 1);
    assert!(folder.join("01/video.mp4").is_file());
    assert!(!folder.join("01/video.partial.mp4").exists());
    assert!(folder.join("01/youtube.txt").is_file());
    assert!(folder.join("01/tiktok.txt").is_file());
    assert_eq!(
        e.snapshot()
            .library
            .iter()
            .filter(|m| m.kind == "clip")
            .count(),
        1
    );
    assert!(e.job_output_dir("../../").is_err());
    let good = Highlight {
        id: uid(),
        start: 0.0,
        end: 2.0,
        title: "Cancel".into(),
        description: "Cancel test".into(),
        hashtags: vec![],
        reason: "".into(),
        score: 1.0,
    };
    let id = e
        .task("package", "Cancel test", move |e, job, ctl| {
            ctl.store(true, std::sync::atomic::Ordering::Relaxed);
            e.write_package(&source, &[good], 1, "original", &job, &ctl)
                .map(Some)
        })
        .unwrap();
    assert_eq!(wait(e, &id, 10).status, "cancelled");
    let folder = e.job_output_dir(&id).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(folder.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["status"], "cancelled");
    assert_eq!(manifest["completedClips"], 0);
    assert_eq!(
        e.snapshot()
            .library
            .iter()
            .filter(|m| m.kind == "clip")
            .count(),
        1
    );
}

#[test]
#[ignore = "requires a public YouTube URL, prepared Whisper model and Codex login"]
fn youtube_generates_upload_package() {
    let url = std::env::var("CUTLINE_TEST_YOUTUBE").expect("Set CUTLINE_TEST_YOUTUBE");
    let root = std::env::var("CUTLINE_AI_ROOT").expect("Set CUTLINE_AI_ROOT");
    let rig = Rig::new();
    let e = &rig.engine;
    std::fs::hard_link(
        Path::new(&root).join("models/ggml-base.bin"),
        e.root.join("models/ggml-base.bin"),
    )
    .unwrap();
    let mut preferences = e.preferences.lock().unwrap().clone();
    preferences.ai_mode = "codex".into();
    e.save_preferences(preferences).unwrap();
    let id = e
        .generate_package(PackageRequest {
            url: Some(url),
            media_id: None,
            count: 1,
            min: 30.,
            max: 60.,
            format: "vertical".into(),
            lang: "auto".into(),
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(900);
    let mut downloaded = false;
    let mut last_message = String::new();
    loop {
        let job = e.snapshot().jobs.into_iter().find(|j| j.id == id).unwrap();
        if job.message != last_message {
            println!("{}: {}", job.label, job.message);
            last_message = job.message;
        }
        if last_message.starts_with("Baixando") && job.progress.is_some_and(|p| p > 0. && p < 1.) {
            downloaded = true;
        }
        if job.status != "running" {
            assert_eq!(job.status, "completed", "{last_message}");
            break;
        }
        assert!(Instant::now() < deadline, "Package timed out");
        thread::sleep(Duration::from_millis(200));
    }
    assert!(downloaded, "Expected measured download progress");
    let folder = e.job_output_dir(&id).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(folder.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["status"], "completed");
    assert_eq!(manifest["completedClips"], 1);
    let metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(folder.join("01/metadata.json")).unwrap()).unwrap();
    assert!(!metadata["description"].as_str().unwrap().is_empty());
    assert!(!metadata["hashtags"].as_array().unwrap().is_empty());
    let (duration, width, height) = e.probe(&folder.join("01/video.mp4")).unwrap();
    assert!((29.9..=60.2).contains(&duration));
    assert_eq!((width, height), (1080, 1920));
    println!(
        "Upload package verified: {} ({duration:.1}s)",
        metadata["title"]
    );
}

#[test]
#[ignore = "uses public YouTube Shorts and TikTok discovery and downloads real original streams"]
fn clone_discovers_and_downloads_both_platforms() {
    use cutline_lib::discovery::CloneRequest;
    let rig = Rig::new();
    let e = &rig.engine;
    for platform in ["youtube", "tiktok"].into_iter().filter(|platform| {
        std::env::var("CUTLINE_TEST_CLONE_PLATFORM")
            .map(|p| p == *platform)
            .unwrap_or(true)
    }) {
        let request = CloneRequest {
            query: "GTA 6".into(),
            platform: platform.into(),
            min_views: 100000,
            count: 1,
            download: false,
        };
        let searched = wait(e, &e.clone_search(request).unwrap(), 900);
        assert_eq!(
            searched.status,
            "completed",
            "{}; warnings: {:?}",
            searched.message,
            e.snapshot().discovery.warnings
        );
        let result = e.snapshot().discovery;
        assert!(
            !result.videos.is_empty(),
            "No results for {platform}: {:?}",
            result.warnings
        );
        assert!(result.videos.iter().all(|v| v.views >= 100000));
        assert!(result
            .videos
            .windows(2)
            .all(|pair| pair[0].views >= pair[1].views));
        let chosen = &result.videos[0];
        println!(
            "{platform}: {} candidates; top {} views: {}",
            result.videos.len(),
            chosen.views,
            chosen.url
        );
        let mut cmd = e.tools.ytdlp().unwrap();
        cmd.args([
            "--skip-download",
            "--dump-single-json",
            "-f",
            "bestvideo*+bestaudio/best",
            "--format-sort-force",
            "-S",
            "res,fps,br",
            "--",
            &chosen.url,
        ]);
        let info: serde_json::Value = serde_json::from_str(
            &e.run(
                cmd,
                &AtomicBool::new(false),
                Duration::from_secs(90),
                None,
                None,
            )
            .unwrap(),
        )
        .unwrap();
        let expected_width = info["width"].as_u64().unwrap();
        let expected_height = info["height"].as_u64().unwrap();
        let done = complete(e, &e.clone_download(vec![chosen.url.clone()]).unwrap(), 300);
        let folder = e.job_output_dir(&done.id).unwrap();
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(folder.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["status"], "completed");
        let file = folder.join(manifest["videos"][0]["file"].as_str().unwrap());
        let (duration, width, height) = e.probe(&file).unwrap();
        assert!(duration > 0.0);
        assert_eq!(
            (width as u64, height as u64),
            (expected_width, expected_height)
        );
        let metadata: serde_json::Value =
            serde_json::from_slice(&std::fs::read(folder.join("01/metadata.json")).unwrap())
                .unwrap();
        assert_eq!(metadata["format_id"], info["format_id"]);
        assert!(folder.join("01/titulo.txt").is_file());
        assert!(folder.join("01/origem.txt").is_file());
        println!(
            "{platform}: original download verified {width}x{height}, {:.1}s, format {}",
            duration, metadata["format_id"]
        );
    }
}

#[cfg(unix)]
#[test]
#[ignore = "requires FFmpeg; verifies original download settings and partial batch recovery without network"]
fn clone_preserves_completed_downloads_when_next_video_fails() {
    use std::os::unix::fs::PermissionsExt;
    let mut rig = Rig::new();
    let fixture = rig.fixture();
    let stub = rig.engine.root.join("download-fixture.sh");
    let script = format!(
        r#"#!/bin/sh
previous=''
output=''
format=''
sorting=''
for argument do
    case "$previous" in
        -o) output="$argument" ;;
        -f) format="$argument" ;;
        -S) sorting="$argument" ;;
    esac
    previous="$argument"
done
[ "$format" = 'bestvideo*+bestaudio/best' ] || exit 8
[ "$sorting" = 'res,fps,br' ] || exit 9
case "$previous" in *failfailfai) printf '%s\n' 'Video unavailable' >&2; exit 7 ;; esac
output="${{output%.*}}.mp4"
cp '{fixture}' "$output" || exit 6
printf '%s\n' '{{"title":"Fixture video","description":"Original description","format_id":"original"}}' > "$(dirname "$output")/metadata.json"
printf 'cutline-file:"%s"\n' "$output"
"#,
        fixture = fixture.replace('\'', "'\\''")
    );
    std::fs::write(&stub, script).unwrap();
    std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
    Arc::get_mut(&mut rig.engine).unwrap().tools.ytdlp = Some(stub);
    let e = &rig.engine;
    let id = e
        .clone_download(vec![
            "https://www.youtube.com/shorts/abcdefghijk".into(),
            "https://www.youtube.com/shorts/failfailfai".into(),
        ])
        .unwrap();
    let job = wait(e, &id, 30);
    assert_eq!(job.status, "failed");
    let folder = e.job_output_dir(&id).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(folder.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["status"], "failed");
    assert_eq!(manifest["videos"].as_array().unwrap().len(), 1);
    assert_eq!(manifest["failures"].as_array().unwrap().len(), 1);
    assert_eq!(
        std::fs::read(folder.join("01/video.mp4")).unwrap(),
        std::fs::read(fixture).unwrap()
    );
    assert!(folder.join("01/metadata.json").is_file());
    assert!(folder.join("01/origem.txt").is_file());
}
