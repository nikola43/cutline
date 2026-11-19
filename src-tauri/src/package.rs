use crate::{
    ai::{validate_clip_options, Highlight},
    engine::{err, Engine},
    media::{self, Media, Result},
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    fs,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageRequest {
    pub url: Option<String>,
    pub media_id: Option<String>,
    pub count: usize,
    pub min: f64,
    pub max: f64,
    pub format: String,
    pub lang: String,
}

impl Engine {
    pub fn generate_package(self: &Arc<Self>, request: PackageRequest) -> Result<String> {
        validate_clip_options(request.count, request.min, request.max, f64::MAX)?;
        if !["vertical", "square", "original"].contains(&request.format.as_str()) {
            return Err("Invalid export format.".into());
        }
        if !["auto", "pt", "en", "es", "fr", "de", "it", "ja"].contains(&request.lang.as_str()) {
            return Err("Unsupported transcription language.".into());
        }
        match (&request.url, &request.media_id) {
            (Some(url), None) => {
                media::source_url(url)?;
            }
            (None, Some(id)) => {
                self.media(id)?;
            }
            _ => return Err("Choose a video link or an imported video.".into()),
        }
        if !self.tools.subtitles
            || !self.whisper_model().is_file()
            || media::find_tool("whisper-cli").is_none()
        {
            return Err("Prepare a IA em Configurações antes de gerar clipes. Whisper e FFmpeg com legendas são necessários.".into());
        }
        self.task("package", "Gerar clipes prontos", move |e, job, ctl| {
            let source = match request.url {
                Some(url) => e.import_url_inner(&url, &job, &ctl)?,
                None => e.media(request.media_id.as_deref().unwrap())?,
            };
            let highlights = e.generate_highlights_inner(
                &source.id,
                request.count,
                request.min,
                request.max,
                &request.lang,
                &job,
                &ctl,
            )?;
            e.write_package(
                &source,
                &highlights,
                request.count,
                &request.format,
                &job,
                &ctl,
            )
            .map(Some)
        })
    }

    pub fn write_package(
        &self,
        source: &Media,
        highlights: &[Highlight],
        requested: usize,
        format: &str,
        job: &str,
        ctl: &AtomicBool,
    ) -> Result<Media> {
        if highlights.is_empty() || highlights.len() > 12 {
            return Err("No usable highlights to export.".into());
        }
        let relative = format!("exports/package-{job}");
        let folder = self.root.join(&relative);
        fs::create_dir(&folder).map_err(err)?;
        if let Some(j) = self.jobs.lock().unwrap().iter_mut().find(|j| j.id == job) {
            j.output_dir = Some(folder.to_string_lossy().into());
        }
        fs::write(folder.join("LEIA-ME.txt"), "Cada pasta numerada contém video.mp4, titulo.txt, descricao.txt, hashtags.txt, youtube.txt, tiktok.txt e metadata.json.\nUse o título e a descrição do youtube.txt no YouTube; copie a legenda do tiktok.txt no TikTok. Os vídeos já incluem legendas.\nmanifest.json lista os clipes finalizados. Se o processo falhar ou for cancelado, somente os clipes listados estão prontos.\nRevise os textos e as legendas antes de publicar.\n").map_err(err)?;
        let mut manifest = json!({"status":"running", "sourceTitle":source.title, "sourceUrl":source.source_url, "requestedClips":requested, "completedClips":0, "clips":[]});
        let manifest_path = format!("{relative}/manifest.json");
        self.atomic_json(&manifest_path, &manifest)?;
        let result = (|| {
            let mut last = None;
            for (index, h) in highlights.iter().enumerate() {
                if ctl.load(Ordering::Relaxed) {
                    return Err("Cancelled".into());
                }
                media::valid_range(h.start, h.end, source.duration)?;
                let metadata = upload_metadata(h, format)?;
                if let Some(j) = self.jobs.lock().unwrap().iter_mut().find(|j| j.id == job) {
                    j.label = format!("Gerando clipe {} de {}", index + 1, highlights.len());
                }
                let clip_dir = folder.join(format!("{:02}", index + 1));
                fs::create_dir(&clip_dir).map_err(err)?;
                let temporary = clip_dir.join("video.partial.mp4");
                let ass = self.caption_file(&source.id, h.start, h.end, format, job)?;
                self.transcode(
                    Path::new(&source.path),
                    &temporary,
                    h.start,
                    h.end,
                    format,
                    Some(&ass),
                    job,
                    ctl,
                )?;
                write_upload_text(&clip_dir, &metadata)?;
                let output = clip_dir.join("video.mp4");
                fs::rename(&temporary, &output).map_err(err)?;
                let clip = self.finish_media(
                    &output,
                    metadata["title"].as_str().unwrap().into(),
                    "clip",
                    &source.provider,
                    source.source_url.clone(),
                )?;
                manifest["clips"].as_array_mut().unwrap().push(json!({"video":format!("{:02}/video.mp4",index+1), "metadata":format!("{:02}/metadata.json",index+1), "title":clip.title, "start":h.start, "end":h.end}));
                manifest["completedClips"] = json!(index + 1);
                self.atomic_json(&manifest_path, &manifest)?;
                last = Some(clip);
            }
            Ok(last.unwrap())
        })();
        manifest["status"] = json!(match &result {
            Ok(_) => "completed",
            Err(e) if e == "Cancelled" => "cancelled",
            Err(_) => "failed",
        });
        if let Err(error) = &result {
            manifest["error"] = json!(error);
        }
        self.atomic_json(&manifest_path, &manifest)?;
        if result.is_ok() {
            if let Some(j) = self.jobs.lock().unwrap().iter_mut().find(|j| j.id == job) {
                j.label = format!(
                    "{} clipes prontos de {} solicitados",
                    highlights.len(),
                    requested
                );
            }
        }
        result
    }

    pub fn job_output_dir(&self, id: &str) -> Result<std::path::PathBuf> {
        let job = self
            .jobs
            .lock()
            .unwrap()
            .iter()
            .find(|j| j.id == id)
            .cloned()
            .ok_or("Task not found.")?;
        let path = match job.output_dir {
            Some(path) => path.into(),
            None if job.kind == "export" && job.status == "completed" => self.root.join("exports"),
            _ => return Err("This task has no export folder yet.".into()),
        };
        let path = fs::canonicalize(path).map_err(err)?;
        if !path.is_dir()
            || !path.starts_with(fs::canonicalize(self.root.join("exports")).map_err(err)?)
        {
            return Err("Invalid export folder.".into());
        }
        Ok(path)
    }
}

fn upload_metadata(h: &Highlight, format: &str) -> Result<Value> {
    let clean = |text: &str, limit| {
        text.chars()
            .filter(|c| !c.is_control() && *c != '<' && *c != '>')
            .take(limit)
            .collect::<String>()
            .trim()
            .to_owned()
    };
    let title = clean(&h.title, 100);
    let description = clean(&h.description, 1000);
    if title.is_empty() || description.is_empty() {
        return Err("A IA não gerou título e descrição para este clipe. Tente gerar novamente ou escolha outro modelo.".into());
    }
    let mut hashtags = Vec::new();
    for tag in h.hashtags.iter().take(5) {
        let text: String = tag
            .trim_start_matches('#')
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .take(40)
            .collect();
        let tag = format!("#{text}");
        if !text.is_empty()
            && !hashtags
                .iter()
                .any(|t: &String| t.to_lowercase() == tag.to_lowercase())
        {
            hashtags.push(tag);
        }
    }
    if hashtags.is_empty() {
        hashtags.push("#Clipes".into());
    }
    let mut youtube_tags = hashtags.clone();
    if format == "vertical"
        && !youtube_tags
            .iter()
            .any(|t| t.eq_ignore_ascii_case("#shorts"))
    {
        youtube_tags.push("#Shorts".into());
    }
    Ok(
        json!({"title":title, "description":description, "hashtags":hashtags, "start":h.start, "end":h.end,
        "youtube":{"title":title, "description":format!("{description}\n\n{}",youtube_tags.join(" ")), "hashtags":youtube_tags},
        "tiktok":{"caption":format!("{title}\n\n{description}\n\n{}",hashtags.join(" ")), "hashtags":hashtags}}),
    )
}

fn write_upload_text(folder: &Path, metadata: &Value) -> Result<()> {
    let title = metadata["title"].as_str().unwrap();
    let description = metadata["description"].as_str().unwrap();
    let hashtags = metadata["hashtags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_str().unwrap())
        .collect::<Vec<_>>()
        .join(" ");
    for (file, text) in [
        ("titulo.txt", title.to_owned()),
        ("descricao.txt", description.to_owned()),
        ("hashtags.txt", hashtags),
        (
            "youtube.txt",
            format!(
                "TÍTULO\n{title}\n\nDESCRIÇÃO\n{}",
                metadata["youtube"]["description"].as_str().unwrap()
            ),
        ),
        (
            "tiktok.txt",
            metadata["tiktok"]["caption"].as_str().unwrap().to_owned(),
        ),
    ] {
        fs::write(folder.join(file), format!("{text}\n")).map_err(err)?;
    }
    fs::write(
        folder.join("metadata.json"),
        serde_json::to_vec_pretty(metadata).map_err(err)?,
    )
    .map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upload_text_is_bounded_and_platform_specific() {
        let h = Highlight {
            id: "test".into(),
            start: 0.,
            end: 10.,
            title: "é".repeat(120),
            description: "A real moment.\n<script>".into(),
            hashtags: vec!["#GTA 6".into(), "#gta6".into(), "../../".into()],
            reason: "".into(),
            score: 90.,
        };
        let metadata = upload_metadata(&h, "vertical").unwrap();
        assert_eq!(metadata["title"].as_str().unwrap().chars().count(), 100);
        assert_eq!(metadata["hashtags"], json!(["#GTA6"]));
        assert!(metadata["youtube"]["description"]
            .as_str()
            .unwrap()
            .contains("#Shorts"));
        assert!(!metadata["tiktok"]["caption"]
            .as_str()
            .unwrap()
            .contains("#Shorts"));
        let folder = tempfile::tempdir().unwrap();
        write_upload_text(folder.path(), &metadata).unwrap();
        assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 6);
        assert!(fs::read_to_string(folder.path().join("youtube.txt"))
            .unwrap()
            .contains("DESCRIÇÃO"));
        let mut empty = h;
        empty.description.clear();
        assert!(upload_metadata(&empty, "vertical").is_err());
    }
}
