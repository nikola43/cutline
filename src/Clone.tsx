import {useEffect,useState} from 'react';
import {Copy,Download,Search,FolderOpen,ExternalLink,Eye,Check,Film} from 'lucide-react';
import type {Discovery,Job,Run} from './types';
import {clock} from './types';
import {JobCard} from './ui';

export default function Clone({discovery,jobs,run,busy}:{discovery:Discovery;jobs:Job[];run:Run;busy:boolean}){
  const [query,setQuery]=useState(()=>{try{return localStorage.getItem('clone-topics-v2')??(discovery.query&&discovery.query!=='GTA 6, gaming'?discovery.query:'GTA 6')}catch{return 'GTA 6'}});
  useEffect(()=>{try{localStorage.setItem('clone-topics-v2',query)}catch{/* Keep edits usable if storage is unavailable. */}},[query]);
  const [platform,setPlatform]=useState('both');
  const [minViews,setMinViews]=useState(discovery.query?discovery.minViews:100000);
  const [count,setCount]=useState(5);
  const [selected,setSelected]=useState<string[]>([]);
  const [directUrl,setDirectUrl]=useState('');
  const [starting,setStarting]=useState(false);
  const [jobId,setJobId]=useState<string|null>(null);
  const job=jobs.find(j=>j.id===jobId)||jobs[0];
  useEffect(()=>{if(jobId&&job?.id===jobId)setStarting(false)},[jobId,job?.id]);
  useEffect(()=>{setSelected([])},[discovery.searchedAt]);
  const disabled=busy||starting;
  const themes=query.split(',').map(t=>t.trim()).filter(Boolean);
  const valid=themes.length>=1&&themes.length<=3&&themes.every(t=>t.length<=60&&/[\p{L}\p{N}]/u.test(t))&&Number.isInteger(count)&&count>=1&&count<=10&&Number.isInteger(minViews)&&minViews>=0&&minViews<=1e12;
  const pending:Job={id:jobId??'',kind:'clone_search',label:'Clone · iniciando',status:'running',message:'Conectando às plataformas…',progress:0,outputDir:null,outputId:null,createdAt:0};
  const shown=starting&&job?.id!==jobId?pending:job;
  async function start(command:string,args:Record<string,unknown>){if(disabled)return;setStarting(true);setJobId(null);const id=await run<string>(command,args);if(id)setJobId(id);else setStarting(false)}
  function search(download:boolean){if(valid)void start('clone_search',{request:{query,platform,minViews,count,download}})}
  function toggle(url:string){setSelected(values=>values.includes(url)?values.filter(v=>v!==url):values.length<10?[...values,url]:values)}
  const lastOutput=jobs.find(j=>j.outputDir);
  return <>
    <div className="page-heading"><div><h1><Copy size={25}/> Clone</h1><p>Encontre vídeos populares e baixe na maior qualidade disponível.</p></div><button className="secondary" onClick={()=>void run(lastOutput?'reveal_job_output':'reveal_exports',lastOutput?{id:lastOutput.id}:undefined)}><FolderOpen size={16}/> Abrir downloads</button></div>
    <section className="panel clone-search">
      <form onSubmit={e=>{e.preventDefault();search(true)}}>
        <label className="clone-topics">Temas<input aria-label="Temas" value={query} onChange={e=>setQuery(e.target.value)} placeholder="Ex.: GTA 6" maxLength={240}/><small>Edite os temas aqui; as alterações são salvas. Até 3 separados por vírgulas: cada um é uma busca independente. Para apenas GTA 6, remova “gaming”.</small></label>
        <div className="clone-controls"><label>Plataformas<select disabled={disabled} value={platform} onChange={e=>setPlatform(e.target.value)}><option value="both">TikTok + YouTube Shorts</option><option value="youtube">YouTube Shorts</option><option value="tiktok">TikTok</option></select></label><label>Mínimo de visualizações<input type="number" min={0} max={1e12} step={1} disabled={disabled} value={minViews} onChange={e=>setMinViews(Number(e.target.value))}/></label><label>Vídeos para baixar<input type="number" min={1} max={10} disabled={disabled} value={count} onChange={e=>setCount(Number(e.target.value))}/></label></div>
        <div className="button-row"><button className="primary" type="submit" disabled={disabled||!valid}><Download size={17}/>{disabled?'Processando…':'Buscar e baixar'}</button><button className="secondary" type="button" disabled={disabled||!valid} onClick={()=>search(false)}><Search size={16}/> Somente buscar</button></div>
        <p className="help-text">{disabled&&'Tema editado durante a tarefa vale para a próxima busca. '}Título, descrição e hashtags são cruzados com 6 quadros do vídeo pela IA de Configurações. Candidatos incertos são excluídos. A análise usa prévias de até 3 minutos e pode demorar. Os {count} mais vistos entre os encontrados serão baixados. Resolução, FPS e bitrate máximos disponíveis, sem re-encodificar. A pasta abre ao terminar.</p>
      </form>
    </section>
    {(shown||starting)&&<div className="clone-progress"><JobCard key={(shown??pending).id} job={shown??pending} onCancel={(shown?.id||jobId)?()=>void run('cancel_job',{id:shown?.id||jobId}):undefined}/>{shown?.outputDir&&shown.status!=='running'&&<button className="secondary" onClick={()=>void run('reveal_job_output',{id:shown.id})}><FolderOpen size={16}/> Abrir pasta desta tarefa</button>}</div>}
    {discovery.warnings.length>0&&<div className="panel clone-warnings" role="status">{discovery.warnings.map((warning,i)=><p key={i}>{warning}</p>)}</div>}
    <div className="section-heading clone-results-heading"><div><h2>Vídeos encontrados <span className="badge">{discovery.videos.length}</span></h2><p className="help-text">{discovery.query?`${discovery.query} · `:''}Ordenados por visualizações entre os resultados consultados.{discovery.searchedAt>0&&` Consulta: ${new Date(discovery.searchedAt*1000).toLocaleString('pt-BR')}`}</p></div>{selected.length>0&&<button className="primary" disabled={disabled} onClick={()=>void start('clone_download',{urls:selected})}><Download size={16}/> Baixar selecionados ({selected.length})</button>}</div>
    {discovery.videos.length>0?<div className="clone-grid">{discovery.videos.map(video=><article className="panel clone-card" key={video.id}>
      <div className="clone-cover">{video.thumbnail?<img src={video.thumbnail} alt="" loading="lazy" referrerPolicy="no-referrer"/>:<Film size={42}/>}<span className={`clone-platform ${video.provider.toLowerCase()}`}>{video.provider==='YouTube'?'YouTube Shorts':video.provider}</span><label className="clone-select"><input type="checkbox" aria-label={`Selecionar ${video.title}`} checked={selected.includes(video.url)} disabled={disabled||selected.length>=10&&!selected.includes(video.url)} onChange={()=>toggle(video.url)}/></label>{video.duration!==null&&<span className="duration">{clock(video.duration)}</span>}</div>
      <div className="clone-card-body"><div className="clone-views"><Eye size={15}/>{video.approximateViews?'≈ ':''}{new Intl.NumberFormat('pt-BR',{notation:'compact',maximumFractionDigits:1}).format(video.views)} visualizações</div><h3 title={video.title}>{video.title}</h3>{video.verification&&<div className="clone-verification"><b><Check size={14}/> IA: {video.verification.topic}</b><p>{video.verification.evidence}</p><small>6 quadros analisados · confira o original</small></div>}{video.tags.length>0&&<p className="clone-tags" title={video.tags.join(', ')}>{video.tags.slice(0,5).join(' · ')}</p>}{video.creator&&<p className="help-text">@{video.creator.replace(/^@/,'')}</p>}<div className="button-row"><button className="primary small" disabled={disabled} onClick={()=>void start('clone_download',{urls:[video.url]})}><Download size={14}/> Baixar original</button><button className="icon-button" aria-label={`Ver original: ${video.title}`} onClick={()=>void run('open_clone_source',{url:video.url})}><ExternalLink size={17}/></button></div></div>
    </article>)}</div>:<div className="empty clone-empty"><Search size={30}/><h3>Descubra os próximos vídeos</h3><p>Edite os temas acima e busque. Apenas candidatos com metadados e quadros compatíveis aparecem aqui.</p></div>}
    <details className="panel clone-direct"><summary>Já tem um link? Baixar diretamente</summary><p className="help-text">Links diretos são baixados como fornecidos, sem a verificação de tema da busca.</p><form className="url-row" onSubmit={e=>{e.preventDefault();void start('clone_download',{urls:[directUrl.trim()]})}}><input type="url" required aria-label="Link direto para Clone" placeholder="https://www.tiktok.com/@…/video/… ou https://youtube.com/shorts/…" value={directUrl} onChange={e=>setDirectUrl(e.target.value)} disabled={disabled}/><button className="secondary" disabled={disabled||!directUrl.trim()}><Download size={15}/> Baixar</button></form></details>
    <div className="clone-quality"><Check size={15}/><span>Original preservado · título, descrição e link de origem salvos junto ao vídeo · o formato pode ser MP4, WebM ou MKV.</span></div>
  </>;
}
