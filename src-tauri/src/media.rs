use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
use url::Url;

pub type Result<T> = std::result::Result<T, String>;

pub const DOWNLOAD_PROGRESS: &str = "download:cutline-download:%(info.vcodec|video)s\t%(progress.status)s\t%(progress.downloaded_bytes|0)s\t%(progress.total_bytes|0)s\t%(progress.total_bytes_estimate|0)s\t%(progress.fragment_index|0)s\t%(progress.fragment_count|0)s";

pub fn download_progress(log: &str) -> Option<(String, Option<f64>)> {
    for line in log.lines().rev() {
        if line.trim() == "cutline-postprocess" {
            return Some(("Download concluído · preparando arquivo…".into(), None));
        }
        let Some(data) = line.strip_prefix("cutline-download:") else {
            continue;
        };
        let fields: Vec<_> = data.split('\t').collect();
        if fields.len() != 7 || !["downloading", "finished"].contains(&fields[1]) {
            continue;
        }
        let number = |index: usize| {
            fields[index]
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite() && *n >= 0.0)
        };
        let Some(downloaded) = number(2) else {
            continue;
        };
        let exact = number(3).filter(|n| *n > 0.0);
        let total = exact.or_else(|| number(4).filter(|n| *n > 0.0));
        let fragments = number(5).zip(number(6)).filter(|(_, count)| *count > 0.0);
        let finished = fields[1] == "finished";
        let progress = if finished {
            Some(1.0)
        } else {
            total
                .map(|t| downloaded / t)
                .or_else(|| fragments.map(|(n, total)| n / total))
                .map(|p| p.clamp(0.0, 0.999))
        };
        let track = if fields[0] == "none" {
            "áudio"
        } else {
            "vídeo"
        };
        let details = if let Some(total) = total {
            format!(
                "{:.1} / {:.1} MB{}",
                downloaded / 1_000_000.0,
                total / 1_000_000.0,
                if exact.is_none() { " (estimado)" } else { "" }
            )
        } else if let Some((part, count)) = fragments {
            format!("{part:.0} / {count:.0} partes")
        } else {
            format!(
                "{:.1} MB · tamanho total não informado",
                downloaded / 1_000_000.0
            )
        };
        return Some((
            format!(
                "{} {track} · {details}",
                if finished {
                    "Download concluído:"
                } else {
                    "Baixando"
                }
            ),
            progress,
        ));
    }
    None
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Media {
    pub id: String,
    pub title: String,
    pub path: String,
    pub thumbnail: String,
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub kind: String,
    pub provider: String,
    pub source_url: Option<String>,
    pub created_at: u64,
}

#[derive(Clone, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub url: String,
    pub id: String,
    pub title: String,
    pub provider: String,
    pub duration: Option<f64>,
    pub is_live: bool,
    pub thumbnail: Option<String>,
    pub channel: String,
}

#[derive(Clone, Serialize)]
pub struct ToolStatus {
    pub name: String,
    pub path: Option<String>,
    pub ready: bool,
}

#[derive(Clone)]
pub struct Tools {
    pub ffmpeg: Option<PathBuf>,
    pub ffprobe: Option<PathBuf>,
    pub ytdlp: Option<PathBuf>,
    pub node: Option<PathBuf>,
    pub subtitles: bool,
}

pub fn find_tool(name: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect();
    if matches!(name, "ffmpeg" | "ffprobe") {
        dirs.splice(
            0..0,
            [
                PathBuf::from("/opt/homebrew/opt/ffmpeg-full/bin"),
                PathBuf::from("/usr/local/opt/ffmpeg-full/bin"),
            ],
        );
    }
    dirs.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
    ]);
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        dirs.push(home.join(".local/bin"));
        if name == "node" {
            if let Ok(entries) = std::fs::read_dir(home.join(".nvm/versions/node")) {
                let mut versions: Vec<_> =
                    entries.flatten().map(|e| e.path().join("bin")).collect();
                versions.sort();
                versions.reverse();
                dirs.extend(versions);
            }
        }
    }
    dirs.into_iter()
        .map(|p| {
            p.join(if cfg!(windows) {
                format!("{name}.exe")
            } else {
                name.into()
            })
        })
        .find(|p| p.is_file())
}

impl Tools {
    pub fn discover(resources: &Path) -> Self {
        let executable = if cfg!(windows) {
            "yt-dlp.exe"
        } else {
            "yt-dlp"
        };
        let bundled = resources.join("tools").join(executable);
        let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../.tools/bundled")
            .join(executable);
        let ffmpeg = find_tool("ffmpeg");
        let subtitles = ffmpeg
            .as_ref()
            .and_then(|p| {
                Command::new(p)
                    .args(["-hide_banner", "-filters"])
                    .output()
                    .ok()
            })
            .is_some_and(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .split_whitespace()
                    .any(|s| s == "ass")
            });
        Self {
            ffmpeg,
            subtitles,
            ffprobe: find_tool("ffprobe"),
            node: find_tool("node"),
            ytdlp: [bundled, dev]
                .into_iter()
                .find(|p| p.is_file())
                .or_else(|| find_tool("yt-dlp")),
        }
    }
    pub fn status(&self) -> Vec<ToolStatus> {
        let mut status: Vec<_> = [
            ("FFmpeg", &self.ffmpeg),
            ("FFprobe", &self.ffprobe),
            ("yt-dlp", &self.ytdlp),
            ("JavaScript runtime", &self.node),
        ]
        .into_iter()
        .map(|(name, p)| ToolStatus {
            name: name.into(),
            path: p.as_ref().map(|p| p.to_string_lossy().into()),
            ready: p.is_some(),
        })
        .collect();
        status.push(ToolStatus {
            name: "Legendas (libass)".into(),
            path: self.ffmpeg.as_ref().map(|p| p.to_string_lossy().into()),
            ready: self.subtitles,
        });
        status
    }
    pub fn ffmpeg(&self) -> Result<Command> {
        let mut c = Command::new(
            self.ffmpeg
                .as_ref()
                .ok_or("FFmpeg is missing. Install it with Homebrew: brew install ffmpeg-full")?,
        );
        c.args(["-hide_banner", "-loglevel", "warning", "-y"]);
        Ok(c)
    }
    pub fn ytdlp(&self) -> Result<Command> {
        let mut c =
            Command::new(self.ytdlp.as_ref().ok_or(
                "yt-dlp is missing. Run npm run tools in the project, then rebuild the app.",
            )?);
        c.args([
            "--ignore-config",
            "--no-playlist",
            "--no-warnings",
            "--socket-timeout",
            "20",
            "--retries",
            "2",
            "--no-colors",
        ]);
        if let Some(p) = &self.ffmpeg {
            c.arg("--ffmpeg-location").arg(p);
        }
        if let Some(p) = &self.node {
            c.arg("--js-runtimes").arg(format!("node:{}", p.display()));
        }
        Ok(c)
    }
}

pub fn source_url(raw: &str) -> Result<(String, String)> {
    let u = Url::parse(raw.trim())
        .map_err(|_| "Paste a complete YouTube or Kick URL, starting with https://".to_string())?;
    if u.scheme() != "https"
        || !u.username().is_empty()
        || u.password().is_some()
        || u.port().is_some()
    {
        return Err("Only standard HTTPS video or channel links are supported.".into());
    }
    let host = u.host_str().unwrap_or("");
    let path = u.path();
    let youtube = matches!(
        host,
        "www.youtube.com" | "youtube.com" | "m.youtube.com" | "youtu.be"
    );
    let kick = matches!(host, "kick.com" | "www.kick.com");
    let yt_path = if host == "youtu.be" {
        path.len() > 1
    } else {
        (path == "/watch" && u.query_pairs().any(|(k, v)| k == "v" && !v.is_empty()))
            || ["/live/", "/shorts/"]
                .iter()
                .any(|p| path.starts_with(p) && path.len() > p.len())
    };
    if !(youtube && yt_path || kick && path.trim_matches('/').len() > 1) {
        return Err("Use a YouTube video/live link or a Kick channel, video, or clip link. Playlists and other sites are not supported.".into());
    }
    Ok((u.into(), if youtube { "YouTube" } else { "Kick" }.into()))
}

pub fn valid_range(start: f64, end: f64, duration: f64) -> Result<()> {
    if !start.is_finite()
        || !end.is_finite()
        || !duration.is_finite()
        || start < 0.0
        || end - start < 0.1
        || end > duration + 0.05
    {
        return Err("Choose a valid in/out range inside the video (at least 0.1 seconds).".into());
    }
    Ok(())
}

pub fn encode_args(
    c: &mut Command,
    format: &str,
    encoder: &crate::hardware::Encoder,
    captions: bool,
) {
    let suffix = format!(
        "{}{}",
        if captions { ",ass=captions.ass" } else { "" },
        encoder.upload_filter()
    );
    if format != "original" {
        let height = if format == "square" { 1080 } else { 1920 };
        c.args(["-filter_complex", &format!("[0:v]split=2[bg][fg];[bg]scale=256:456:force_original_aspect_ratio=increase,crop=256:456,gblur=sigma=18,scale=1080:{height},eq=brightness=-0.22[b];[fg]scale=1080:{height}:force_original_aspect_ratio=decrease[f];[b][f]overlay=(W-w)/2:(H-h)/2,setsar=1{suffix}[v]"), "-map", "[v]", "-map", "0:a:0?"]);
    } else {
        c.args([
            "-map",
            "0:v:0",
            "-map",
            "0:a:0?",
            "-vf",
            &format!("scale=trunc(iw/2)*2:trunc(ih/2)*2,setsar=1{suffix}"),
        ]);
    }
    encoder.args(c);
    c.args([
        "-c:a",
        "aac",
        "-b:a",
        "192k",
        "-movflags",
        "+faststart",
        "-progress",
        "pipe:1",
        "-nostats",
    ]);
}

#[derive(Clone, Debug)]
pub struct Segment {
    pub name: String,
    pub start: f64,
    pub end: f64,
}
pub fn segments(csv: &str) -> Vec<Segment> {
    csv.lines()
        .filter_map(|line| {
            let fields: Vec<_> = line.trim().split(',').collect();
            if fields.len() != 3 {
                return None;
            }
            let name = fields[0].trim_matches('"');
            let digits = name.strip_prefix("segment_")?.strip_suffix(".ts")?;
            if digits.len() != 5 || !digits.bytes().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let start: f64 = fields[1].parse().ok()?;
            let end: f64 = fields[2].parse().ok()?;
            if !start.is_finite() || !end.is_finite() || start < 0.0 || end <= start {
                return None;
            }
            Some(Segment {
                name: name.into(),
                start,
                end,
            })
        })
        .collect()
}
pub fn recent_segments(all: &[Segment], seconds: f64) -> Result<(Vec<Segment>, f64)> {
    if !matches!(seconds, 30.0 | 60.0) {
        return Err("Choose a 30- or 60-second replay.".into());
    }
    let mut chosen = Vec::new();
    let mut total = 0.0;
    for s in all.iter().rev() {
        if let Some(next) = chosen.last() {
            let next: &Segment = next;
            if (s.end - next.start).abs() > 0.15 {
                return Err(
                    "The stream buffer has a gap. Wait for a fresh continuous buffer.".into(),
                );
            }
        }
        chosen.push(s.clone());
        total += s.end - s.start;
        if total + 0.01 >= seconds {
            break;
        }
    }
    if total + 0.01 < seconds {
        return Err(format!(
            "Buffer is warming up: {:.0} of {seconds:.0} seconds available.",
            total
        ));
    }
    chosen.reverse();
    Ok((chosen, (total - seconds).max(0.0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn download_progress_handles_tracks_estimates_unknown_sizes_and_partial_lines() {
        let parse = |data: &str| download_progress(&format!("cutline-download:{data}"));
        let (message, p) = parse("avc1\tdownloading\t5000000\t10000000\t0\t0\t0").unwrap();
        assert_eq!(p, Some(0.5));
        assert!(message.contains("vídeo · 5.0 / 10.0 MB"));
        let (message, p) = parse("none\tdownloading\t3\t0\t4\t0\t0").unwrap();
        assert_eq!(p, Some(0.75));
        assert!(message.contains("áudio"));
        assert!(message.contains("estimado"));
        assert_eq!(
            parse("avc1\tdownloading\t20\t0\t0\t2\t8").unwrap().1,
            Some(0.25)
        );
        assert_eq!(parse("avc1\tdownloading\t20\t0\t0\t0\t0").unwrap().1, None);
        assert_eq!(
            parse("avc1\tfinished\t20\t0\t0\t0\t0").unwrap().1,
            Some(1.0)
        );
        assert_eq!(
            parse("avc1\tdownloading\t20\t10\t0\t0\t0").unwrap().1,
            Some(0.999)
        );
        assert!(parse("avc1\tdownloading\tNaN\t10\t0\t0\t0").is_none());
        assert!(parse("avc1\tdownloading\t20").is_none());
        let (message, p) = download_progress(
            "cutline-download:avc1\tfinished\t10\t10\t0\t0\t0\ncutline-postprocess\n",
        )
        .unwrap();
        assert!(message.contains("preparando"));
        assert_eq!(p, None);
    }
    #[test]
    fn url_and_trim_boundaries() {
        for u in [
            "https://youtu.be/abc123",
            "https://www.youtube.com/live/abc",
            "https://youtube.com/watch?v=abc",
            "https://kick.com/channel/videos/123",
        ] {
            assert!(source_url(u).is_ok());
        }
        for u in [
            "file:///etc/passwd",
            "https://youtube.com.evil.test/watch?v=x",
            "https://user@kick.com/channel",
            "http://kick.com/channel",
            "https://youtube.com/playlist?list=x",
            "https://kick.com:123/channel",
            "--exec=bad",
        ] {
            assert!(source_url(u).is_err(), "{u}");
        }
        assert!(valid_range(2.0, 5.0, 10.0).is_ok());
        for (s, e) in [
            (f64::NAN, 5.0),
            (-1.0, 5.0),
            (5.0, 5.0),
            (2.0, 12.0),
            (0.0, f64::INFINITY),
        ] {
            assert!(valid_range(s, e, 10.0).is_err());
        }
    }
    #[test]
    fn replay_uses_completed_contiguous_segments() {
        let csv: String = (0..40)
            .map(|i| format!("segment_{i:05}.ts,{},{}\n", i * 2, i * 2 + 2))
            .collect();
        let list = segments(&(csv + "segment_00040.ts,80,\n../../secret.ts,0,99\n"));
        assert_eq!(list.len(), 40);
        let (selected, trim) = recent_segments(&list, 30.0).unwrap();
        assert_eq!(selected.first().unwrap().start, 50.0);
        assert_eq!(selected.len(), 15);
        assert_eq!(trim, 0.0);
        assert!(recent_segments(&list[..5], 30.0).is_err());
        let mut broken = list.clone();
        broken[30].end -= 1.0;
        assert!(recent_segments(&broken, 30.0).is_err());
    }
}
