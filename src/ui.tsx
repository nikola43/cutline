import { convertFileSrc } from '@tauri-apps/api/core';
import { Film, Plus, Smartphone, Square, Monitor, LoaderCircle, X, Check, FolderOpen } from 'lucide-react';
import type { CaptionStyle,Format,Job,Media } from './types';
import {clock} from './types';
export const desktop=()=>('__TAURI_INTERNALS__' in window);
export const asset=(path:string)=>desktop()?convertFileSrc(path):path;
export function FormatPicker({value,onChange}:{value:Format;onChange:(v:Format)=>void}){return <div className="format-picker">{([['vertical','9:16','Vertical',Smartphone],['square','1:1','Quadrado',Square],['original','16:9','Original',Monitor]] as const).map(([key,label,desc,Icon])=><button key={key} className={value===key?'chosen':''} aria-pressed={value===key} onClick={()=>onChange(key)}><Icon size={21}/><b>{label}</b><span>{desc}</span></button>)}</div>}
export function CaptionSample({style,text='ISSO MUDA TUDO'}:{style:CaptionStyle;text?:string}){const words=(style.uppercase?text.toUpperCase():text).split(' ');return <span className={`caption-sample ${style.preset}`} style={{fontFamily:style.font,color:style.color,'--accent':style.accent} as React.CSSProperties}>{words.map((w,i)=><span key={i} className={i===words.length-1?'highlight':''}>{w} </span>)}</span>}
export function Empty({title,detail,action,onAction}:{title:string;detail:string;action?:string;onAction?:()=>void}){return <div className="empty"><div className="empty-icon"><Film size={26}/></div><h3>{title}</h3><p>{detail}</p>{action&&<button className="primary" onClick={onAction}><Plus size={16}/>{action}</button>}</div>}
export function MediaCard({media,onOpen,onReveal}:{media:Media;onOpen:()=>void;onReveal:()=>void}){return <article className="media-card"><button className="media-cover" onClick={onOpen} aria-label={`Editar ${media.title}`}><img src={asset(media.thumbnail)} alt="" loading="lazy"/><span className="media-tag">{media.kind==='clip'?'Clipe pronto':media.provider}</span><span className="duration">{clock(media.duration)}</span></button><div className="media-info"><button className="title-button" onClick={onOpen} title={media.title}>{media.title}</button><div><span>{new Date(media.createdAt*1000).toLocaleDateString('pt-BR')} · {media.width}×{media.height}</span><button className="icon-button" aria-label={`Mostrar ${media.title} na pasta`} onClick={onReveal}><FolderOpen size={15}/></button></div></div></article>}
export function JobCard({job,onCancel}:{job:Job;onCancel?:()=>void}){
  const running=job.status==='running';const completed=job.status==='completed';
  const percent=job.progress!==null&&Number.isFinite(job.progress)?Math.floor(Math.max(0,Math.min(1,job.progress))*100):undefined;
  return <div className={`job ${job.status}`} role="status">
    <div className="job-line">
      {running?<LoaderCircle className="spin" size={17}/>:completed?<Check size={17}/>:<X size={17}/>}
      <b>{job.label}</b>{percent!==undefined&&(running||completed)&&<span>{percent}%</span>}
      {running&&onCancel&&<button className="icon-button" onClick={onCancel} aria-label={job.kind==='live'?'Parar gravação':`Cancelar ${job.label}`}><X size={15}/></button>}
    </div>
    <p>{job.message}</p>
    {(running||completed)&&<div className={`progress ${percent===undefined?'indeterminate':''}`} role="progressbar" aria-label={job.label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent} aria-valuetext={percent===undefined?job.message:`${percent}% · ${job.message}`}><i style={{width:`${percent??35}%`}}/></div>}
  </div>
}
