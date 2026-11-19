import { useEffect, useState } from 'react';
import { UploadCloud, Link, WandSparkles, Check, Minus, Plus, Captions, FolderOpen, ArrowRight, Radio } from 'lucide-react';
import type { CaptionStyle, ClipOptions, Job, Media, Run } from './types';
import { clock, styles } from './types';
import { CaptionSample, FormatPicker, JobCard, asset } from './ui';

export default function Create({ selected, select, library, options, setOptions, style, setStyle, run, busy, job, importFile, onEditor, onLive }: {
  selected: Media | undefined; select: (id: string) => void; library: Media[];
  options: ClipOptions; setOptions: (o: ClipOptions) => void; style: CaptionStyle;
  setStyle: (s: CaptionStyle) => void; run: Run; busy: boolean; job: Job | undefined;
  importFile: () => void; onEditor: () => void; onLive: () => void;
}) {
  const [tab, setTab] = useState<'url' | 'file'>('url');
  const [url, setUrl] = useState('');
  const [starting, setStarting] = useState(false);
  const [jobId, setJobId] = useState<string | null>(null);
  useEffect(() => { if (jobId && job?.id === jobId) setStarting(false); }, [jobId, job?.id]);
  const shownJob = !starting || job?.id === jobId ? job : undefined;
  const valid = options.count >= 1 && options.count <= 12 && Number.isInteger(options.count) && options.min >= 5 && options.max <= 180 && options.max >= options.min;
  const disabled = busy || starting;
  const pendingJob: Job = { id: jobId ?? '', kind: 'package', label: 'Gerar clipes prontos', status: 'running', message: 'Conectando ao vídeo…', progress: 0, outputId: null, outputDir: null, createdAt: 0 };
  function change<K extends keyof ClipOptions>(key: K, value: ClipOptions[K]) { setOptions({ ...options, [key]: value }); }
  async function generate() {
    if (disabled || !valid || (tab === 'url' ? !url.trim() : !selected)) return;
    setJobId(null); setStarting(true);
    const id = await run<string>('generate_package', { request: { ...options, url: tab === 'url' ? url.trim() : null, mediaId: tab === 'file' ? selected!.id : null } });
    if (id) setJobId(id); else setStarting(false);
  }
  return <>
    <div className="page-heading"><div><h1>Criar clipes</h1><p>Cole o link. Receba seus clipes prontos para publicar.</p></div><span className="quiet-label"><Check size={14} /> Sem marca d’água</span></div>
    <div className="simple-create">
      <div className="tabs"><button className={tab === 'url' ? 'active' : ''} disabled={disabled} onClick={() => setTab('url')}><Link size={14} /> Link do YouTube ou Kick</button><button className={tab === 'file' ? 'active' : ''} disabled={disabled} onClick={() => setTab('file')}><UploadCloud size={14} /> Arquivo ou projeto</button></div>
      <form className="panel quick-generate" onSubmit={e => { e.preventDefault(); void generate(); }}>
        {tab === 'url' ? <><label htmlFor="source-url">Link do vídeo</label><input id="source-url" type="url" required disabled={disabled} value={url} onChange={e => setUrl(e.target.value)} placeholder="https://youtube.com/watch?v=… ou link de VOD do Kick" /><p className="help-text">YouTube e Kick: vídeos e transmissões já encerradas.</p></> : <>
          <button type="button" className="secondary" disabled={disabled} onClick={importFile}><UploadCloud size={16} /> Escolher arquivo</button>
          <label className="existing-project">Vídeo da biblioteca<select disabled={disabled} value={selected?.id ?? ''} onChange={e => select(e.target.value)}><option value="">Escolher vídeo…</option>{library.filter(m => m.kind !== 'clip').map(m => <option value={m.id} key={m.id}>{m.title}</option>)}</select></label>
          {selected && <div className="selected-source"><img src={asset(selected.thumbnail)} alt="" /><div><h3>{selected.title}</h3><p>{clock(selected.duration)} · {selected.provider}</p><button type="button" className="text-button" onClick={onEditor}>Editar manualmente <ArrowRight size={14} /></button></div></div>}
        </>}
        <div className="quick-summary"><span>Até <b>{options.count} clipes</b></span><span>{options.min}–{options.max}s</span><span>{options.format === 'vertical' ? '9:16 vertical' : options.format === 'square' ? '1:1 quadrado' : 'Formato original'}</span><span>Legendas automáticas</span></div>
        <button className="primary full" type="submit" disabled={disabled || !valid || (tab === 'url' ? !url.trim() : !selected)}><WandSparkles size={18} />{disabled ? 'Processando…' : 'Gerar clipes prontos'}</button>
        <p className="center-muted">Download → legendas → melhores momentos → exportação. A pasta abre ao terminar.</p>
      </form>
      {(shownJob || starting) && <div className="import-progress" aria-label="Progresso da geração"><JobCard job={shownJob ?? pendingJob} onCancel={shownJob || jobId ? () => void run('cancel_job', { id: shownJob?.id ?? jobId }) : undefined} />
        {shownJob?.outputDir && <div className="package-result"><p>{shownJob.status === 'completed' ? 'Tudo salvo: vídeos, títulos, descrições e hashtags.' : 'Os clipes já finalizados estão salvos. Veja a lista em manifest.json.'}</p><button className="secondary" onClick={() => void run('reveal_job_output', { id: shownJob.id })}><FolderOpen size={16} /> Abrir pasta de saída</button></div>}
      </div>}
      <div className="output-summary"><FolderOpen size={22} /><div><h3>Uma pasta, tudo pronto</h3><p>Cada clipe inclui <b>video.mp4</b> com legendas, título, descrição, hashtags e textos separados para YouTube e TikTok.</p></div></div>
      <details className="panel advanced-options"><summary>Opções avançadas · quantidade, formato e legendas</summary><fieldset disabled={disabled}>
        <div className="create-layout"><section className="create-main">
          <label>Número de clipes</label><div className="stepper"><button type="button" aria-label="Menos clipes" onClick={() => change('count', Math.max(1, options.count - 1))}><Minus size={14} /></button><input type="number" aria-label="Número de clipes" min={1} max={12} value={options.count} onChange={e => change('count', Number(e.target.value))} /><button type="button" aria-label="Mais clipes" onClick={() => change('count', Math.min(12, options.count + 1))}><Plus size={14} /></button></div>
          <label>Duração de cada clipe</label><div className="duration-presets">{[[15, 30, 'Curtas'], [30, 60, 'Médias'], [60, 90, 'Longas']].map(([min, max, label]) => <button type="button" key={label} className={options.min === min && options.max === max ? 'active' : ''} onClick={() => setOptions({ ...options, min: Number(min), max: Number(max) })}><b>{label}</b><span>{min}–{max}s</span></button>)}</div>
          <div className="two-fields"><label>Mínimo (s)<input type="number" min={5} max={180} value={options.min} onChange={e => change('min', Number(e.target.value))} /></label><label>Máximo (s)<input type="number" min={5} max={180} value={options.max} onChange={e => change('max', Number(e.target.value))} /></label></div>
          {!valid && <p role="alert" className="error-text">Escolha 1–12 clipes e um intervalo entre 5 e 180 segundos.</p>}
          <label>Idioma do vídeo<select value={options.lang} onChange={e => change('lang', e.target.value)}><option value="auto">Detectar automaticamente</option><option value="pt">Português</option><option value="en">Inglês</option><option value="es">Espanhol</option></select></label>
          <div className="panel-title"><h3>Estilo de legendas</h3><Captions size={18} /></div><div className="style-chips">{styles.map(([id, label]) => <button type="button" key={id} className={style.preset === id ? 'active' : ''} onClick={() => setStyle({ ...style, preset: id })}><span className={`style-dot ${id}`}>Aᴀ</span>{label}</button>)}</div>
          <label>Fonte<select value={style.font} onChange={e => setStyle({ ...style, font: e.target.value })}>{['Arial Black', 'Arial', 'Impact', 'Verdana'].map(f => <option key={f}>{f}</option>)}</select></label>
        </section><aside className="creation-options"><label>Formato</label><FormatPicker value={options.format} onChange={v => change('format', v)} /><div className={`mini-preview ${options.format}`}><div className="preview-phone"><span>Seu título aparece aqui</span><CaptionSample style={style} /></div></div></aside></div>
      </fieldset></details>
      <button className="text-button" onClick={onLive}><Radio size={15} /> Transmissão acontecendo agora? Abrir Lives e replay <ArrowRight size={14} /></button>
    </div>
  </>;
}
