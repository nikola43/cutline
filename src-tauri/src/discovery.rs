use crate::{
    engine::{err, now, Engine},
    media::{self, Result},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use url::Url;

const MAX_CANDIDATES: usize = 12;
const QUALITY: &str = "bestvideo*+bestaudio/best";

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CloneVideo {
    pub id: String,
    pub url: String,
    pub title: String,
    pub provider: String,
    pub creator: String,
    pub views: u64,
    pub approximate_views: bool,
    pub duration: Option<f64>,
    pub thumbnail: Option<String>,
}
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Discovery {
    pub query: String,
    pub min_views: u64,
    pub searched_at: u64,
    pub videos: Vec<CloneVideo>,
    pub warnings: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneRequest {
    pub query: String,
    pub platform: String,
    pub min_views: u64,
    pub count: usize,
    pub download: bool,
}

// Discovery and downloads only accept canonical public video URLs, never arbitrary hosts.
pub fn clone_url(raw: &str) -> Result<(String, String)> {
    let u = Url::parse(raw.trim()).map_err(err)?;
    if u.scheme() != "https"
        || !u.username().is_empty()
        || u.password().is_some()
        || u.port().is_some()
    {
        return Err("Use a public HTTPS TikTok video or YouTube Shorts link.".into());
    }
    let segments: Vec<_> = u.path().trim_matches('/').split('/').collect();
    let host = u.host_str().unwrap_or("");
    if matches!(host, "youtube.com" | "www.youtube.com" | "m.youtube.com")
        && segments.len() == 2
        && segments[0] == "shorts"
        && segments[1].len() == 11
        && segments[1]
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
    {
        return Ok((
            format!("https://www.youtube.com/shorts/{}", segments[1]),
            "YouTube".into(),
        ));
    }
    if matches!(host, "tiktok.com" | "www.tiktok.com")
        && segments.len() == 3
        && segments[0].starts_with('@')
        && segments[0].len() > 1
        && segments[0][1..]
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
        && segments[1] == "video"
        && (15..=22).contains(&segments[2].len())
        && segments[2].bytes().all(|c| c.is_ascii_digit())
    {
        return Ok((
            format!(
                "https://www.tiktok.com/{}/video/{}",
                segments[0], segments[2]
            ),
            "TikTok".into(),
        ));
    }
    Err("Use a TikTok /@creator/video/… or YouTube /shorts/… URL.".into())
}

fn topics(query: &str) -> Result<Vec<String>> {
    if query.len() > 240 || query.chars().any(|c| c.is_control()) {
        return Err("Use up to three short topics, separated by commas.".into());
    }
    let topics: Vec<_> = query
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    if topics.is_empty()
        || topics.len() > 3
        || topics
            .iter()
            .any(|s| s.chars().count() > 60 || !s.chars().any(char::is_alphanumeric))
    {
        return Err("Enter 1–3 topics of up to 60 characters each.".into());
    }
    Ok(topics)
}

fn video_from_info(info: &Value, url: &str, approximate: bool) -> Option<CloneVideo> {
    let (url, provider) = clone_url(url).ok()?;
    let views = info["view_count"].as_u64()?;
    if info["is_live"] == true || info["live_status"] == "is_live" {
        return None;
    }
    let thumbnail = info["thumbnail"]
        .as_str()
        .or_else(|| info["thumbnails"].as_array()?.last()?["url"].as_str())
        .filter(|s| s.starts_with("https://"))
        .map(str::to_owned);
    Some(CloneVideo {
        id: format!("{}:{}", provider, url.rsplit('/').next()?),
        url,
        title: info["title"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("Vídeo sem título")
            .chars()
            .take(240)
            .collect(),
        provider,
        creator: info["uploader"]
            .as_str()
            .or_else(|| info["channel"].as_str())
            .unwrap_or("")
            .chars()
            .take(120)
            .collect(),
        views,
        approximate_views: approximate,
        duration: info["duration"]
            .as_f64()
            .filter(|n| n.is_finite() && *n > 0.),
        thumbnail,
    })
}
fn rank(videos: &mut Vec<CloneVideo>, min_views: u64) {
    let mut seen = HashSet::new();
    videos.retain(|v| v.views >= min_views && seen.insert(v.url.clone()));
    videos.sort_by(|a, b| b.views.cmp(&a.views).then_with(|| a.id.cmp(&b.id)));
}
fn tiktok_links(html: &str) -> Vec<String> {
    let html = html.replace("\\/", "/");
    let mut links = Vec::new();
    for (start, _) in html.match_indices("https://www.tiktok.com/@") {
        let text = html[start..]
            .split(|c: char| !c.is_ascii_alphanumeric() && !"_./:@-".contains(c))
            .next()
            .unwrap_or("");
        if let Ok((url, _)) = clone_url(text) {
            if !links.contains(&url) {
                links.push(url);
            }
        }
    }
    links
}
fn cancelled(ctl: &AtomicBool) -> Result<()> {
    if ctl.load(Ordering::Relaxed) {
        Err("Cancelled".into())
    } else {
        Ok(())
    }
}

impl Engine {
    fn store_discovery(&self, result: &Discovery) -> Result<()> {
        self.atomic_json("clone-search.json", result)?;
        *self.discovery.lock().unwrap() = result.clone();
        Ok(())
    }
    pub fn clone_search(self: &Arc<Self>, request: CloneRequest) -> Result<String> {
        topics(&request.query)?;
        if !["both", "youtube", "tiktok"].contains(&request.platform.as_str())
            || !(1..=10).contains(&request.count)
            || request.min_views > 1_000_000_000_000
        {
            return Err("Choose a platform, 1–10 downloads and a valid minimum view count.".into());
        }
        let kind = if request.download {
            "clone_download"
        } else {
            "clone_search"
        };
        self.task(kind, "Clone · buscando vídeos", move |e, job, ctl| {
            let videos = e.find_clone_videos(&request, &job, &ctl)?;
            if request.download {
                e.clone_batch(&videos[..videos.len().min(request.count)], &job, &ctl)?;
            }
            Ok(None)
        })
    }
    pub fn find_clone_videos(
        &self,
        request: &CloneRequest,
        job: &str,
        ctl: &AtomicBool,
    ) -> Result<Vec<CloneVideo>> {
        let mut result = Discovery {
            query: request.query.clone(),
            min_views: request.min_views,
            searched_at: now(),
            ..Default::default()
        };
        self.store_discovery(&result)?;
        let mut candidates = Vec::new();
        let topics = topics(&request.query)?;
        let candidates_per_topic = MAX_CANDIDATES / topics.len();
        for topic in topics {
            cancelled(ctl)?;
            let hashtag: String = topic
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '_')
                .flat_map(char::to_lowercase)
                .collect();
            if request.platform != "tiktok" {
                self.phase(job, &format!("YouTube Shorts · buscando {topic}…"), None);
                let mut url = Url::parse("https://www.youtube.com").unwrap();
                url.set_path(&format!("/hashtag/{hashtag}/shorts"));
                let mut cmd = self.tools.ytdlp()?;
                // shortcut: inspect the first 30 Shorts per topic; expand only if discovery coverage needs grow.
                cmd.args([
                    "--yes-playlist",
                    "--flat-playlist",
                    "--dump-single-json",
                    "--playlist-end",
                    "30",
                    "--extractor-retries",
                    "0",
                    "--",
                    url.as_str(),
                ]);
                match self
                    .run(cmd, ctl, Duration::from_secs(90), Some((job, None)), None)
                    .and_then(|s| serde_json::from_str::<Value>(&s).map_err(err))
                {
                    Ok(info) => {
                        for value in info["entries"].as_array().into_iter().flatten() {
                            if let Some(video) = value["url"]
                                .as_str()
                                .and_then(|url| video_from_info(value, url, true))
                            {
                                result.videos.push(video);
                            }
                        }
                    }
                    Err(error) if error == "Cancelled" => return Err(error),
                    Err(_) => result.warnings.push(format!(
                        "YouTube: não foi possível consultar o tema {topic}."
                    )),
                }
                rank(&mut result.videos, request.min_views);
                self.store_discovery(&result)?;
            }
            if request.platform != "youtube" {
                self.phase(
                    job,
                    &format!("TikTok · procurando vídeos de {topic}…"),
                    None,
                );
                let search = (|| -> Result<Vec<String>> {
                    let client = reqwest::blocking::Client::builder()
                        .timeout(Duration::from_secs(25))
                        .https_only(true)
                        .redirect(reqwest::redirect::Policy::limited(3))
                        .user_agent("Mozilla/5.0")
                        .build()
                        .map_err(err)?;
                    for term in [format!("{hashtag} tiktok"), format!("{topic} clips tiktok")] {
                        cancelled(ctl)?;
                        let mut url = Url::parse("https://www.bing.com/videos/search").unwrap();
                        url.query_pairs_mut()
                            .append_pair("q", &term)
                            .append_pair("count", "20");
                        let response = client
                            .get(url)
                            .send()
                            .map_err(err)?
                            .error_for_status()
                            .map_err(err)?;
                        let mut body = String::new();
                        response
                            .take(4_000_001)
                            .read_to_string(&mut body)
                            .map_err(err)?;
                        if body.len() > 4_000_000 {
                            return Err("Search response too large.".into());
                        }
                        let links = tiktok_links(&body);
                        if !links.is_empty() {
                            return Ok(links);
                        }
                    }
                    Ok(Vec::new())
                })();
                match search {
                    Ok(links) if !links.is_empty() => {
                        for url in links.into_iter().take(candidates_per_topic) {
                            if !candidates.contains(&url) { candidates.push(url); }
                        }
                    }
                    Ok(_) => result.warnings.push(format!("TikTok: a busca pública não retornou vídeos para {topic}. Tente outro tema ou um link direto.")),
                    Err(error) if error == "Cancelled" => return Err(error),
                    Err(error) => result.warnings.push(format!("TikTok: falha na busca pública de {topic}: {}", error.chars().take(240).collect::<String>())),
                }
                self.store_discovery(&result)?;
            }
        }
        let mut unavailable = 0;
        for (index, url) in candidates.iter().enumerate() {
            cancelled(ctl)?;
            self.phase(
                job,
                &format!(
                    "TikTok · verificando visualizações {}/{}",
                    index + 1,
                    candidates.len()
                ),
                Some(index as f64 / candidates.len() as f64),
            );
            let mut cmd = self.tools.ytdlp()?;
            cmd.args([
                "--skip-download",
                "--dump-single-json",
                "--extractor-retries",
                "0",
                "--retries",
                "0",
                "--",
                url,
            ]);
            match self
                .run(cmd, ctl, Duration::from_secs(45), Some((job, None)), None)
                .and_then(|s| serde_json::from_str::<Value>(&s).map_err(err))
            {
                Ok(info) => {
                    if let Some(video) = video_from_info(&info, url, false) {
                        result.videos.push(video);
                    } else {
                        unavailable += 1;
                    }
                }
                Err(error) if error == "Cancelled" => return Err(error),
                Err(_) => unavailable += 1,
            }
            rank(&mut result.videos, request.min_views);
            self.store_discovery(&result)?;
        }
        if unavailable > 0 {
            result.warnings.push(format!("TikTok: {unavailable} vídeos indisponíveis ou sem contagem de visualizações foram ignorados."));
        }
        result.searched_at = now();
        self.store_discovery(&result)?;
        if result.videos.is_empty() {
            return Err("Nenhum vídeo público encontrado acima do mínimo. Reduza as visualizações, troque o tema ou tente novamente. Veja os avisos do Clone.".into());
        }
        Ok(result.videos)
    }
    pub fn clone_download(self: &Arc<Self>, urls: Vec<String>) -> Result<String> {
        if urls.is_empty() || urls.len() > 10 {
            return Err("Choose 1–10 videos to download.".into());
        }
        let discovery = self.discovery.lock().unwrap().clone();
        let mut videos = Vec::new();
        for url in urls {
            let (url, provider) = clone_url(&url)?;
            if videos.iter().any(|v: &CloneVideo| v.url == url) {
                continue;
            }
            videos.push(
                discovery
                    .videos
                    .iter()
                    .find(|v| v.url == url)
                    .cloned()
                    .unwrap_or(CloneVideo {
                        url,
                        provider,
                        ..Default::default()
                    }),
            );
        }
        self.task(
            "clone_download",
            "Clone · baixar máxima qualidade",
            move |e, job, ctl| {
                e.clone_batch(&videos, &job, &ctl)?;
                Ok(None)
            },
        )
    }
    fn clone_batch(&self, videos: &[CloneVideo], job: &str, ctl: &AtomicBool) -> Result<()> {
        let relative = format!("exports/clone-{job}");
        let folder = self.root.join(&relative);
        fs::create_dir(&folder).map_err(err)?;
        if let Some(j) = self.jobs.lock().unwrap().iter_mut().find(|j| j.id == job) {
            j.output_dir = Some(folder.to_string_lossy().into());
        }
        let mut manifest = json!({"status":"running", "quality":"best available resolution / fps / bitrate; no re-encoding", "videos":[], "failures":[]});
        let manifest_path = format!("{relative}/manifest.json");
        self.atomic_json(&manifest_path, &manifest)?;
        let result = (|| -> Result<()> {
            for (index, video) in videos.iter().enumerate() {
                cancelled(ctl)?;
                if let Some(j) = self.jobs.lock().unwrap().iter_mut().find(|j| j.id == job) {
                    j.label = format!("Clone · baixando {} de {}", index + 1, videos.len());
                }
                let dir = folder.join(format!("{:02}", index + 1));
                fs::create_dir(&dir).map_err(err)?;
                match self.download_original(&video.url, &dir, job, ctl) {
                    Ok(file) => {
                        manifest["videos"].as_array_mut().unwrap().push(json!({"source":video.url,"file":format!("{:02}/{}",index+1,file.file_name().unwrap().to_string_lossy()), "metadata":format!("{:02}/metadata.json",index+1)}));
                    }
                    Err(error) if error == "Cancelled" => return Err(error),
                    Err(error) => {
                        manifest["failures"]
                            .as_array_mut()
                            .unwrap()
                            .push(json!({"url":video.url,"error":error}));
                    }
                }
                self.atomic_json(&manifest_path, &manifest)?;
            }
            if !manifest["failures"].as_array().unwrap().is_empty() {
                return Err(format!("{} de {} vídeos baixados. Alguns falharam; os arquivos prontos foram preservados. Veja manifest.json na pasta.", manifest["videos"].as_array().unwrap().len(), videos.len()));
            }
            Ok(())
        })();
        manifest["status"] = json!(match &result {
            Ok(_) => "completed",
            Err(e) if e == "Cancelled" => "cancelled",
            Err(_) => "failed",
        });
        self.atomic_json(&manifest_path, &manifest)?;
        if result.is_ok() {
            if let Some(j) = self.jobs.lock().unwrap().iter_mut().find(|j| j.id == job) {
                j.label = format!("Clone · {} vídeos salvos", videos.len());
            }
        }
        result
    }
    pub fn download_original(
        &self,
        raw: &str,
        folder: &std::path::Path,
        job: &str,
        ctl: &AtomicBool,
    ) -> Result<PathBuf> {
        let (url, _) = clone_url(raw)?;
        cancelled(ctl)?;
        let mut cmd = self.tools.ytdlp()?;
        cmd.args(["--no-simulate", "--no-overwrites", "-f", QUALITY, "--format-sort-force", "-S", "res,fps,br", "--merge-output-format", "mkv", "--newline", "--progress", "--progress-delta", "0.25", "--progress-template", media::DOWNLOAD_PROGRESS, "--progress-template", "postprocess:cutline-postprocess", "--print", "after_move:cutline-file:%(filepath)j", "--print-to-file", "after_move:%(.{id,title,description,uploader,view_count,duration,width,height,ext,format_id,vcodec,acodec,fps,webpage_url})j"])
            .arg(folder.join("metadata.json")).arg("-o").arg(folder.join("video.%(ext)s")).args(["--", &url]);
        self.phase(
            job,
            "Baixando original · máxima qualidade disponível…",
            Some(0.),
        );
        let output = self.run(cmd, ctl, Duration::from_secs(3600), Some((job, None)), None)?;
        let path = output
            .lines()
            .rev()
            .find_map(|line| line.strip_prefix("cutline-file:"))
            .ok_or("Download completed without a final file path.")?;
        let file =
            fs::canonicalize(serde_json::from_str::<String>(path).map_err(err)?).map_err(err)?;
        if !file.is_file() || !file.starts_with(fs::canonicalize(folder).map_err(err)?) {
            return Err("Invalid downloaded file path.".into());
        }
        self.probe(&file)?;
        let metadata: Value =
            serde_json::from_slice(&fs::read(folder.join("metadata.json")).map_err(err)?)
                .map_err(err)?;
        fs::write(
            folder.join("titulo.txt"),
            metadata["title"].as_str().unwrap_or("Vídeo"),
        )
        .map_err(err)?;
        fs::write(
            folder.join("descricao.txt"),
            metadata["description"].as_str().unwrap_or(""),
        )
        .map_err(err)?;
        fs::write(
            folder.join("origem.txt"),
            format!("{url}\nQualidade original disponível na plataforma. Sem re-encodificação.\n"),
        )
        .map_err(err)?;
        Ok(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_validates_links_filters_and_ranks_real_view_counts() {
        assert!(clone_url("https://www.youtube.com/shorts/abcdefghijk?feature=share").is_ok());
        for url in [
            "https://www.youtube.com/watch?v=abcdefghijk",
            "https://tiktok.com.evil.test/@name/video/1234567890123456789",
            "http://www.tiktok.com/@name/video/1234567890123456789",
            "https://user:pass@www.youtube.com/shorts/abcdefghijk",
            "https://www.tiktok.com/@name/video/1234567890123456789/extra",
        ] {
            assert!(clone_url(url).is_err(), "{url}");
        }
        let url = "https://www.tiktok.com/@name/video/1234567890123456789";
        assert_eq!(
            tiktok_links(&format!("href=\"{url}\" {url}?share=1 {url}")),
            vec![url]
        );
        assert!(topics(" , ").is_err());
        assert!(topics("a,b,c,d").is_err());
        assert_eq!(topics("GTA 6, gaming").unwrap().len(), 2);
        assert!(video_from_info(&json!({"title":"No views"}), url, false).is_none());
        let a = video_from_info(&json!({"title":"A","view_count":500_000}), url, false).unwrap();
        let b = video_from_info(
            &json!({"title":"B","view_count":2_000_000}),
            "https://www.youtube.com/shorts/abcdefghijk",
            true,
        )
        .unwrap();
        let mut videos = vec![a.clone(), b, a];
        rank(&mut videos, 1_000_000);
        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].views, 2_000_000);
    }
}
