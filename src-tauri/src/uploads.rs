use crate::{
    engine::{err, now, Engine},
    media::Result,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::Path,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UploadState {
    pub accounts: Vec<UploadAccount>,
    pub records: Vec<UploadRecord>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadAccount {
    pub platform: String,
    pub id: String,
    pub label: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadRecord {
    pub id: String,
    pub source_url: String,
    pub platform: String,
    pub account: String,
    pub title: String,
    pub status: String,
    pub url: Option<String>,
    pub message: String,
    pub updated_at: u64,
}

fn platform(value: &str) -> Result<()> {
    if ["youtube", "tiktok"].contains(&value) {
        Ok(())
    } else {
        Err("Plataforma de upload inválida.".into())
    }
}
fn key(source: &str, destination: &UploadAccount) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            format!("{}\n{}\n{source}", destination.platform, destination.id).as_bytes()
        )
    )
}
fn clean(text: &str, limit: usize) -> String {
    text.chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .filter(|c| !['<', '>'].contains(c))
        .take(limit)
        .collect::<String>()
        .trim()
        .into()
}

impl UploadState {
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join("uploads.json");
        let mut state: Self = if path.exists() {
            serde_json::from_slice(&fs::read(path).map_err(err)?).map_err(|_| {
                "Não foi possível ler o histórico de uploads. O arquivo foi preservado.".to_string()
            })?
        } else {
            Self::default()
        };
        for entry in &mut state.records {
            if entry.status == "running" {
                entry.status = "uncertain".into();
                entry.message =
                    "Envio interrompido. Confira a plataforma antes de reenviar.".into();
            }
        }
        Ok(state)
    }
    pub fn snapshot(&self) -> Self {
        let mut state = self.clone();
        state
            .records
            .sort_by_key(|r| std::cmp::Reverse(r.updated_at));
        state.records.truncate(20);
        state
    }
    fn destinations(&self) -> Result<Vec<UploadAccount>> {
        ["tiktok", "youtube"]
            .into_iter()
            .map(|p| {
                self.accounts
                    .iter()
                    .find(|a| a.platform == p)
                    .cloned()
                    .ok_or_else(|| {
                        format!("Conecte a conta {p} em Configurações → Upload automático.")
                    })
            })
            .collect()
    }
    fn needs_upload(&self, id: &str) -> Result<bool> {
        match self.records.iter().find(|r|r.id==id).map(|r|r.status.as_str()) {
            Some("uploaded"|"confirmed")=>Ok(false),
            Some("running"|"uncertain")=>Err("Um envio anterior não foi confirmado. Confira o histórico de uploads antes de continuar; não reenviamos automaticamente para evitar duplicatas.".into()),
            _=>Ok(true),
        }
    }
}

impl Engine {
    pub fn open_upload_browser(
        self: &Arc<Self>,
        destination: String,
        review: bool,
    ) -> Result<String> {
        platform(&destination)?;
        self.task(
            "upload_auth",
            &format!("Abrir login {destination}"),
            move |e, job, ctl| {
                e.browser_upload(
                    json!({"action":if review {"studio"}else{"login"},"platform":destination}),
                    &job,
                    &ctl,
                )?;
                Ok(None)
            },
        )
    }
    pub fn validate_auto_upload(&self) -> Result<()> {
        if self.preferences.lock().unwrap().auto_upload {
            self.uploads.lock().unwrap().destinations()?;
        }
        Ok(())
    }
    fn browser_upload(&self, mut request: Value, job: &str, ctl: &AtomicBool) -> Result<Value> {
        let node = self
            .tools
            .node
            .as_ref()
            .ok_or("Instale Node.js e Google Chrome para os uploads.")?;
        if !self.upload_runner.is_file() {
            return Err("O componente de upload não está instalado. Reinstale o Cutline.".into());
        }
        request["profile"] = json!(self.root.join("upload-browser"));
        request["exports"] = json!(self.root.join("exports"));
        let mut cmd = Command::new(node);
        cmd.arg(&self.upload_runner);
        let result = self.run(
            cmd,
            ctl,
            Duration::from_secs(900),
            Some((job, None)),
            Some(&request.to_string()),
        )?;
        serde_json::from_str(&result)
            .map_err(|_| "O navegador não confirmou o resultado do upload.".into())
    }
    pub fn connect_upload_account(self: &Arc<Self>, destination: String) -> Result<String> {
        platform(&destination)?;
        self.task("upload_auth",&format!("Conectar {destination}"),move |e,job,ctl|{
            e.phase(&job,"Entre na conta na janela do Chrome. Conclua qualquer confirmação solicitada pela plataforma. Esta janela usa uma sessão exclusiva de uploads.",None);
            let result=e.browser_upload(json!({"action":"connect","platform":destination}),&job,&ctl)?;
            let id=result["account"]["id"].as_str().filter(|s|!s.is_empty()&&s.len()<=120).ok_or("Não foi possível identificar a conta.")?;
            let label=clean(result["account"]["label"].as_str().unwrap_or(id),120);
            let mut state=e.uploads.lock().unwrap();
            state.accounts.retain(|a|a.platform!=destination);
            state.accounts.push(UploadAccount{platform:destination,id:id.into(),label});
            e.atomic_json("uploads.json",&*state)?;
            Ok(None)
        })
    }
    pub fn confirm_upload(&self, id: &str, uploaded: bool) -> Result<()> {
        if self.active() {
            return Err("Aguarde ou cancele a tarefa atual antes de alterar o histórico.".into());
        }
        let mut state = self.uploads.lock().unwrap();
        let record = state
            .records
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or("Envio não encontrado.")?;
        if record.status != "uncertain" {
            return Err("Este envio não precisa de confirmação.".into());
        }
        record.status = if uploaded { "confirmed" } else { "failed" }.into();
        record.message = if uploaded {
            "Confirmado pelo usuário na plataforma; não reenviar."
        } else {
            "Você confirmou que o vídeo não foi enviado; nova tentativa permitida."
        }
        .into();
        record.updated_at = now();
        self.atomic_json("uploads.json", &*state)
    }
    pub fn prepare_auto_upload(&self, job: &str, ctl: &AtomicBool) -> Result<Vec<UploadAccount>> {
        if !self.preferences.lock().unwrap().auto_upload {
            return Ok(Vec::new());
        }
        let accounts = self.uploads.lock().unwrap().destinations()?;
        for account in &accounts {
            self.phase(
                job,
                &format!(
                    "Verificando conta de upload · {} ({})",
                    account.platform, account.label
                ),
                None,
            );
            self.browser_upload(
                json!({"action":"check","platform":account.platform,"expectedAccount":account.id}),
                job,
                ctl,
            )?;
        }
        Ok(accounts)
    }
    fn save_upload_record(&self, record: UploadRecord) -> Result<()> {
        let mut state = self.uploads.lock().unwrap();
        if let Some(previous) = state.records.iter_mut().find(|r| r.id == record.id) {
            *previous = record;
        } else {
            state.records.push(record);
        }
        self.atomic_json("uploads.json", &*state)
    }
    pub fn upload_clone_file(
        &self,
        source: &str,
        file: &Path,
        accounts: &[UploadAccount],
        job: &str,
        ctl: &AtomicBool,
    ) -> Result<()> {
        if accounts.is_empty() || !self.preferences.lock().unwrap().auto_upload {
            return Ok(());
        }
        let file = fs::canonicalize(file).map_err(err)?;
        if !file.starts_with(fs::canonicalize(self.root.join("exports")).map_err(err)?) {
            return Err("Arquivo fora da pasta de exportações.".into());
        }
        let pending: Vec<_> = accounts
            .iter()
            .map(|a| {
                Ok((
                    a,
                    self.uploads.lock().unwrap().needs_upload(&key(source, a))?,
                ))
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .filter_map(|(a, needed)| needed.then_some(a))
            .collect();
        if pending.is_empty() {
            return Ok(());
        }
        let folder = file.parent().ok_or("Pasta do vídeo não encontrada.")?;
        let metadata: Value =
            serde_json::from_slice(&fs::read(folder.join("metadata.json")).map_err(err)?)
                .map_err(err)?;
        let title = clean(metadata["title"].as_str().unwrap_or("Vídeo"), 100);
        let description = clean(metadata["description"].as_str().unwrap_or(&title), 5000);
        let (duration, _, _) = self.probe(&file)?;
        if !(0.0..=180.0).contains(&duration) {
            return Err("O upload para Shorts aceita vídeos de até 3 minutos neste fluxo.".into());
        }
        let upload_file = folder.join("upload.mp4");
        if !upload_file.exists() {
            let temporary = folder.join("upload.partial.mp4");
            self.transcode(&file, &temporary, 0., duration, "vertical", None, job, ctl)?;
            self.probe(&temporary)?;
            fs::rename(temporary, &upload_file).map_err(err)?;
        }
        let privacy = self.preferences.lock().unwrap().upload_privacy.clone();
        for account in pending {
            if !self.preferences.lock().unwrap().auto_upload {
                break;
            }
            if ctl.load(Ordering::Relaxed) {
                return Err("Cancelled".into());
            }
            let id = key(source, account);
            let mut record = UploadRecord {
                id: id.clone(),
                source_url: source.into(),
                platform: account.platform.clone(),
                account: account.label.clone(),
                title: title.clone(),
                status: "running".into(),
                url: None,
                message: "Enviando pelo Chrome…".into(),
                updated_at: now(),
            };
            self.save_upload_record(record.clone())?;
            let receipt = self
                .root
                .join("work")
                .join(job)
                .join(format!("upload-{id}.json"));
            let _ = fs::remove_file(&receipt);
            self.phase(
                job,
                &format!(
                    "Enviando para {} · {} · {}",
                    account.platform,
                    account.label,
                    if privacy == "public" {
                        "público"
                    } else {
                        "privado"
                    }
                ),
                None,
            );
            let uploaded=self.browser_upload(json!({"action":"upload","platform":account.platform,"expectedAccount":account.id,"privacy":privacy,"file":upload_file,"title":title,"description":description,"receipt":receipt}),job,ctl);
            let proof: Value = fs::read(&receipt)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or(Value::Null);
            record.updated_at = now();
            if proof["status"] == "uploaded" {
                record.status = "uploaded".into();
                record.url = proof["url"].as_str().map(str::to_owned);
                record.message = "Envio confirmado pela plataforma.".into();
                self.save_upload_record(record)?;
                if ctl.load(Ordering::Relaxed) {
                    return Err("Cancelled".into());
                }
                continue;
            }
            record.status = if ["uploading", "posting"]
                .iter()
                .any(|s| proof["status"] == *s)
            {
                "uncertain"
            } else {
                "failed"
            }
            .into();
            let error = uploaded.err().unwrap_or(
                "A plataforma não confirmou o envio. Confira o histórico antes de reenviar.".into(),
            );
            record.message = if record.status == "uncertain" {
                format!("{error} Confira o Studio antes de reenviar.")
            } else {
                error.clone()
            };
            record.url = proof["url"].as_str().map(str::to_owned);
            self.save_upload_record(record)?;
            return Err(error);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upload_history_prevents_duplicates_and_requires_both_accounts() {
        let mut state = UploadState::default();
        assert!(state.destinations().is_err());
        let a = UploadAccount {
            platform: "tiktok".into(),
            id: "creator".into(),
            label: "@creator".into(),
        };
        let b = UploadAccount {
            platform: "youtube".into(),
            id: "UCchannel".into(),
            label: "Channel".into(),
        };
        assert_ne!(key("source", &a), key("source", &b));
        state.accounts = vec![a.clone(), b];
        assert_eq!(state.destinations().unwrap().len(), 2);
        let id = key("source", &a);
        state.records.push(UploadRecord {
            id: id.clone(),
            source_url: "source".into(),
            platform: a.platform,
            account: a.id,
            title: "Title".into(),
            status: "uploaded".into(),
            url: None,
            message: String::new(),
            updated_at: 0,
        });
        assert!(!state.needs_upload(&id).unwrap());
        state.records[0].status = "uncertain".into();
        assert!(state.needs_upload(&id).is_err());
        state.records[0].status = "failed".into();
        assert!(state.needs_upload(&id).unwrap());
        state.records[0].status = "running".into();
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("uploads.json"),
            serde_json::to_vec(&state).unwrap(),
        )
        .unwrap();
        assert!(UploadState::load(dir.path())
            .unwrap()
            .needs_upload(&id)
            .is_err());
        assert_eq!(clean("<title>\0", 100), "title");
    }
}
