export type Page='dashboard'|'clone'|'create'|'projects'|'library'|'templates'|'captions'|'editor'|'live'|'settings'|'help';
export type Format='vertical'|'square'|'original';
export interface Media {id:string;title:string;path:string;thumbnail:string;duration:number;width:number;height:number;kind:string;provider:string;sourceUrl:string|null;createdAt:number}
export interface Job {id:string;kind:string;label:string;status:string;message:string;progress:number|null;outputId:string|null;outputDir:string|null;createdAt:number}
export interface CaptionStyle {preset:string;font:string;size:number;color:string;accent:string;uppercase:boolean;wordsPerLine:number}
export interface Preferences {encoder:string;aiMode:'local'|'codex';localModel:string;youtubeBrowser:'none'|'chrome'|'firefox'|'edge'|'brave'|'safari';captions:CaptionStyle}
export interface Encoder {id:string;label:string;available:boolean;detail:string}
export interface Live {id:string;title:string;url:string;provider:string;embedUrl:string|null;phase:string;startedAt:number;bufferSeconds:number;encoder:string}
export interface CloneVideo {id:string;url:string;title:string;provider:string;creator:string;views:number;approximateViews:boolean;duration:number|null;thumbnail:string|null;tags:string[];verification:{topic:string;evidence:string;confidence:number}|null}
export interface Discovery {query:string;minViews:number;searchedAt:number;videos:CloneVideo[];warnings:string[]}
export interface Snapshot {discovery:Discovery;library:Media[];jobs:Job[];live:Live|null;tools:{name:string;path:string|null;ready:boolean}[];encoders:Encoder[];preferences:Preferences;activeEncoder:string;outputDir:string}
export interface SourceInfo {url:string;id:string;title:string;provider:string;duration:number|null;isLive:boolean;thumbnail:string|null;channel:string}
export interface Word {start:number;end:number;text:string}
export interface Highlight {id:string;start:number;end:number;title:string;reason:string;score:number}
export interface AiStatus {whisperReady:boolean;modelReady:boolean;localModels:string[];codexInstalled:boolean;codexLoggedIn:boolean;codexStatus:string}
export interface ClipOptions {count:number;min:number;max:number;format:Format;lang:string}
export type Run=<T=unknown>(command:string,args?:Record<string,unknown>)=>Promise<T|null>;
export const defaultStyle:CaptionStyle={preset:'classic',font:'Arial Black',size:72,color:'#ffffff',accent:'#ffe500',uppercase:true,wordsPerLine:3};
export const emptySnapshot:Snapshot={discovery:{query:"",minViews:100000,searchedAt:0,videos:[],warnings:[]},library:[],jobs:[],live:null,tools:[],encoders:[],preferences:{encoder:'auto',aiMode:'local',localModel:'gemma3:1b',youtubeBrowser:'none',captions:defaultStyle},activeEncoder:'Verificando…',outputDir:''};
export const clock=(s:number)=>`${Math.floor(s/3600)?`${Math.floor(s/3600)}:`:''}${String(Math.floor(s/60)%60).padStart(2,'0')}:${String(Math.floor(s)%60).padStart(2,'0')}`;
export const styles=[['classic','Clássico'],['neon','Neon'],['karaoke','Karaokê'],['pop','Pop'],['minimal','Mínimo'],['box','Caixa']];
