use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Serialize, Debug)]
pub struct Encoder {
    pub id: String,
    pub label: String,
    pub available: bool,
    pub detail: String,
    #[serde(skip)]
    pub device: Option<PathBuf>,
}
impl Encoder {
    pub fn cpu() -> Self {
        Self {
            id: "libx264".into(),
            label: "CPU · H.264".into(),
            available: true,
            detail: "Software encoding".into(),
            device: None,
        }
    }
    pub fn prepare(&self, c: &mut Command) {
        if let Some(device) = &self.device {
            c.arg("-vaapi_device").arg(device);
        }
    }
    pub fn upload_filter(&self) -> &'static str {
        if self.id == "h264_vaapi" {
            ",format=nv12,hwupload"
        } else {
            ""
        }
    }
    pub fn args(&self, c: &mut Command) {
        c.args(["-c:v", &self.id]);
        match self.id.as_str() {
            "libx264" => {
                c.args([
                    "-threads", "2", "-preset", "fast", "-crf", "20", "-pix_fmt", "yuv420p",
                ]);
            }
            "h264_videotoolbox" => {
                c.args(["-allow_sw", "0", "-b:v", "12M", "-pix_fmt", "yuv420p"]);
            }
            "h264_nvenc" => {
                c.args([
                    "-preset", "p4", "-cq", "21", "-b:v", "12M", "-pix_fmt", "yuv420p",
                ]);
            }
            "h264_amf" => {
                c.args(["-quality", "balanced", "-b:v", "12M", "-pix_fmt", "yuv420p"]);
            }
            "h264_vaapi" => {
                c.args(["-b:v", "12M"]);
            }
            _ => unreachable!("encoder IDs are constructed by the backend"),
        }
    }
}

pub fn detect(ffmpeg: Option<&Path>) -> Vec<Encoder> {
    let candidates = [
        ("h264_videotoolbox", "Apple Silicon · VideoToolbox"),
        ("h264_nvenc", "NVIDIA · NVENC"),
        ("h264_amf", "AMD · AMF"),
        ("h264_vaapi", "AMD / Intel · VAAPI"),
    ];
    let listed = ffmpeg
        .and_then(|p| {
            Command::new(p)
                .args(["-hide_banner", "-encoders"])
                .output()
                .ok()
        })
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let mut devices: Vec<_> = std::fs::read_dir("/dev/dri")
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|s| s.to_string_lossy().starts_with("renderD"))
        })
        .collect();
    // Prefer AMD's render node on Linux machines with more than one GPU.
    devices.sort_by_key(|p| {
        let vendor = std::fs::read_to_string(
            Path::new("/sys/class/drm")
                .join(p.file_name().unwrap())
                .join("device/vendor"),
        )
        .unwrap_or_default();
        (!vendor.trim().eq_ignore_ascii_case("0x1002"), p.clone())
    });
    let mut out = Vec::new();
    for (id, label) in candidates {
        let mut e = Encoder {
            id: id.into(),
            label: label.into(),
            available: false,
            detail: "Not provided by this FFmpeg build".into(),
            device: None,
        };
        if listed.split_whitespace().any(|s| s == id) {
            e.device = if id == "h264_vaapi" {
                devices.first().cloned()
            } else {
                None
            };
            if id == "h264_vaapi" && e.device.is_none() {
                e.detail = "No Linux render device found".into();
            } else if let Some(ffmpeg) = ffmpeg {
                let mut c = Command::new(ffmpeg);
                c.args(["-hide_banner", "-loglevel", "error", "-nostdin"]);
                e.prepare(&mut c);
                c.args([
                    "-f",
                    "lavfi",
                    "-i",
                    "color=size=1280x720:rate=30",
                    "-t",
                    "0.15",
                    "-vf",
                ]);
                c.arg(format!("format=yuv420p{}", e.upload_filter()));
                e.args(&mut c);
                c.args(["-an", "-f", "null", "-"])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());
                if let Ok(mut child) = c.spawn() {
                    let deadline = Instant::now() + Duration::from_secs(8);
                    loop {
                        match child.try_wait() {
                            Ok(Some(s)) => {
                                e.available = s.success();
                                break;
                            }
                            Ok(None) if Instant::now() < deadline => {
                                thread::sleep(Duration::from_millis(50))
                            }
                            _ => {
                                let _ = child.kill();
                                let _ = child.wait();
                                break;
                            }
                        }
                    }
                }
                e.detail = if e.available {
                    "Hardware encode test passed"
                } else {
                    "Encoder exists, but the device or driver is unavailable"
                }
                .into();
            }
        }
        out.push(e);
    }
    let mut cpu = Encoder::cpu();
    cpu.available = listed.split_whitespace().any(|s| s == "libx264");
    out.push(cpu);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encoder_arguments_do_not_silently_enable_software_on_apple() {
        let apple = Encoder {
            id: "h264_videotoolbox".into(),
            label: "Apple".into(),
            available: true,
            detail: String::new(),
            device: None,
        };
        let mut c = Command::new("ffmpeg");
        apple.args(&mut c);
        let a: Vec<_> = c
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert!(a.windows(2).any(|p| p == ["-allow_sw", "0"]));
        let vaapi = Encoder {
            id: "h264_vaapi".into(),
            device: Some("/dev/dri/renderD128".into()),
            ..apple
        };
        assert!(vaapi.upload_filter().ends_with("hwupload"));
    }
}
