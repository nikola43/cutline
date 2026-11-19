import {test,expect,type Page} from '@playwright/test';
import {emptySnapshot} from '../src/types';

async function desktop(page:Page){
  await page.addInitScript((initial)=>{
    const state=structuredClone(initial);state.activeEncoder='Apple Silicon · VideoToolbox';
    state.library=[{id:'source-1',title:'Vídeo de teste',path:'/fixture.mp4',thumbnail:'/fixture.jpg',duration:120,width:1920,height:1080,kind:'source',provider:'Local',sourceUrl:null,createdAt:1700000000}];
    state.encoders=[{id:'h264_videotoolbox',label:'Apple Silicon · VideoToolbox',available:true,detail:'Hardware encode test passed'},{id:'h264_nvenc',label:'NVIDIA · NVENC',available:false,detail:'No device'},{id:'libx264',label:'CPU',available:true,detail:'Software'}];
    const w=window as any;w.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener:()=>{}};w.testCalls=[];w.testState=state;w.testFailSave=false;w.testSlowImport=false;
    w.__TAURI_INTERNALS__={metadata:{currentWindow:{label:'main'},currentWebview:{label:'main'}},transformCallback:()=>1,unregisterCallback:()=>{},convertFileSrc:(path:string)=>path.endsWith('.jpg')?'data:image/svg+xml,%3Csvg xmlns=%22http://www.w3.org/2000/svg%22 width=%22640%22 height=%22360%22%3E%3Crect width=%22640%22 height=%22360%22 fill=%22%23131918%22/%3E%3C/svg%3E':'data:video/mp4;base64,',invoke:async(command:string,args:any)=>{
      w.testCalls.push({command,args});
      if(command==='get_snapshot')return structuredClone(state);
      if(command==='save_preferences'){if(w.testFailSave)throw 'Save failed';state.preferences=args.preferences;return;}
      if(command==='inspect_source')return {url:args.url,id:'video-1',title:'Vídeo remoto',provider:'YouTube',duration:120,isLive:false,thumbnail:null,channel:'Canal'};
      if(command==='generate_package'){if(w.testSlowImport)return 'job-progress';state.jobs=[{id:'job-1',kind:'package',label:'5 clipes prontos',status:'completed',message:'Concluído',progress:1,outputId:'clip-1',outputDir:'/exports/package-1',createdAt:1700000000}];return 'job-1';}
      if(command==='clone_search'){
        state.discovery={query:args.request.query,minViews:args.request.minViews,searchedAt:1700000002,videos:[
          {id:'YouTube:abcdefghijk',url:'https://www.youtube.com/shorts/abcdefghijk',title:'GTA 6: melhor momento',provider:'YouTube',creator:'gaming',views:2000000,approximateViews:true,duration:null,thumbnail:null,tags:['gta6'],verification:{topic:'GTA 6',evidence:'Cenas do trailer aparecem nos quadros.',confidence:0.96}},
          {id:'TikTok:1234567890123456789',url:'https://www.tiktok.com/@gaming/video/1234567890123456789',title:'Jogada incrível',provider:'TikTok',creator:'gaming',views:500000,approximateViews:false,duration:35,thumbnail:null,tags:['gta6'],verification:{topic:'GTA 6',evidence:'Quadros mostram o cenário do jogo.',confidence:0.94}}],warnings:[]};
        state.jobs=[{id:'clone-job',kind:args.request.download?'clone_download':'clone_search',label:'Clone',status:'completed',message:'Concluído',progress:1,outputId:null,outputDir:args.request.download?'/exports/clone-1':null,createdAt:1700000002}];return 'clone-job';
      }
      if(command==='clone_download'){state.jobs=[{id:'clone-download-2',kind:'clone_download',label:'Clone · vídeos salvos',status:'completed',message:'Concluído',progress:1,outputId:null,outputDir:'/exports/clone-2',createdAt:1700000003}];return 'clone-download-2';}
      if(command==='get_ai_status')return {whisperReady:true,modelReady:true,localModels:['gemma3:1b'],codexInstalled:true,codexLoggedIn:true,codexStatus:'Connected'};
      if(command==='get_transcript')return [{start:1,end:2,text:'Teste'},{start:2,end:3,text:'legenda'}];
      if(command==='get_highlights')return [{id:'1',start:10,end:40,title:'Primeiro momento',reason:'Ideia completa',score:91},{id:'2',start:50,end:80,title:'Segundo momento',reason:'Surpresa',score:87}];
      if(command==='save_transcript')return true;
      if(command.startsWith('plugin:'))return 1;
      return 'job-1';
    }};
  },emptySnapshot);
  await page.goto('/');await expect(page.getByRole('heading',{name:'Seu espaço criativo'})).toBeVisible();
}
test('AI mode and caption changes persist across polling; failed saves stay unsaved',async({page})=>{
  await desktop(page);await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await page.getByRole('button',{name:/ChatGPT Codex Login/}).click();
  await expect(page.getByText('ChatGPT conectado',{exact:true})).toBeVisible();
  await expect.poll(()=>page.evaluate(()=>(window as any).testState.preferences.aiMode)).toBe('codex');
  await page.getByRole('button',{name:'Legendas',exact:true}).click();
  await page.getByRole('slider').first().fill('94');
  await page.waitForTimeout(1500);await expect(page.getByRole('slider').first()).toHaveValue('94');
  await page.getByRole('button',{name:'Salvar como meu estilo'}).click();
  await expect(page.getByRole('button',{name:'Estilo salvo',exact:true})).toBeVisible();
  await page.getByRole('slider').first().fill('100');await page.evaluate(()=>(window as any).testFailSave=true);
  await page.getByRole('button',{name:'Salvar como meu estilo'}).click();
  await expect(page.getByRole('alert')).toContainText('Save failed');
  await expect(page.getByRole('button',{name:'Salvar como meu estilo'})).toBeVisible();
});
test('editor sends validated ranges, corrected transcript, and batch export',async({page})=>{
  await desktop(page);await page.getByRole('button',{name:'Editar Vídeo de teste',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Editor de vídeo'})).toBeVisible();
  await page.getByRole('spinbutton',{name:'Entrada (segundos)'}).fill('-1');
  await expect(page.getByRole('button',{name:'Exportar clipe',exact:true})).toBeDisabled();
  await page.getByRole('button',{name:/Primeiro momento/}).click();
  await page.getByRole('button',{name:'Exportar os 2 destaques'}).click();
  await expect.poll(()=>page.evaluate(()=>(window as any).testCalls.some((c:any)=>c.command==='export_highlights'&&c.args.mediaId==='source-1'))).toBe(true);
  await page.getByRole('button',{name:'Transcrição',exact:true}).click();
  await page.getByRole('textbox',{name:'Palavra 1',exact:true}).fill('Corrigido');
  await expect(page.getByRole('button',{name:'Exportar clipe',exact:true})).toBeDisabled();
  await page.getByRole('button',{name:'Salvar correções'}).click();
  await expect(page.getByRole('button',{name:'Exportar clipe',exact:true})).toBeEnabled();
});
test('replay buttons reflect completed buffer; stop targets the capture',async({page})=>{
  await desktop(page);await page.getByRole('button',{name:'Lives e replay',exact:true}).click();
  await expect(page.getByRole('button',{name:'Salvar últimos 30s'})).toBeDisabled();
  await page.evaluate(()=>{(window as any).testState.live={id:'live-1',phase:'recording',title:'Live',url:'https://kick.com/test',provider:'Kick',embedUrl:null,startedAt:1700000000,bufferSeconds:35,encoder:'VideoToolbox'}});
  await expect(page.getByRole('button',{name:'Salvar últimos 30s'})).toBeEnabled();
  await expect(page.getByRole('button',{name:'Salvar últimos 60s'})).toBeDisabled();
  await page.getByRole('button',{name:'Salvar últimos 30s'}).click();
  await page.getByRole('button',{name:'Parar e salvar gravação'}).click();
  const calls=await page.evaluate(()=>(window as any).testCalls);
  expect(calls).toContainEqual({command:'save_replay',args:{seconds:30}});
  expect(calls).toContainEqual({command:'cancel_job',args:{id:'live-1'}});
});
test('all screens render without page errors or horizontal overflow',async({page})=>{
  const errors:string[]=[];page.on('pageerror',e=>errors.push(e.message));await desktop(page);
  for(const width of [1440,1024,768,320]){
    await page.setViewportSize({width,height:1000});
    for(const name of ['Dashboard','Criar clipes','Clone','Meus projetos','Biblioteca','Modelos','Legendas','Editor de vídeo','Lives e replay','Configurações','Ajuda']){
      await page.getByRole('button',{name:new RegExp('^'+name)}).first().click();
      expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth+1),`${name} at ${width}px`).toBe(true);
      if(width===1440)await page.screenshot({path:`test-results/screens/${name.replaceAll(' ','-')}.png`,fullPage:true});
    }
  }
  expect(errors).toEqual([]);
});

test('paste and generate sends one command and opens the completed folder once',async({page})=>{
  await desktop(page);await page.getByRole('button',{name:'Criar clipes',exact:true}).click();
  await expect(page.getByRole('button',{name:'Gerar clipes prontos',exact:true})).toBeDisabled();
  await page.getByRole('textbox',{name:'Link do vídeo',exact:true}).fill('https://www.youtube.com/watch?v=example');
  await page.getByRole('button',{name:'Gerar clipes prontos',exact:true}).click();
  await expect(page.getByRole('button',{name:'Abrir pasta de saída'})).toBeVisible();
  const calls=await page.evaluate(()=>(window as any).testCalls);
  expect(calls).toContainEqual({command:'generate_package',args:{request:{url:'https://www.youtube.com/watch?v=example',mediaId:null,count:5,min:30,max:60,lang:'auto',format:'vertical'}}});
  expect(calls.some((c:any)=>['inspect_source','import_url','generate_highlights'].includes(c.command))).toBe(false);
  await expect.poll(()=>page.evaluate(()=>(window as any).testCalls.filter((c:any)=>c.command==='reveal_job_output').length)).toBe(1);
  await page.waitForTimeout(1500);
  expect(await page.evaluate(()=>(window as any).testCalls.filter((c:any)=>c.command==='reveal_job_output').length)).toBe(1);
  await page.getByRole('button',{name:'Abrir pasta de saída'}).click();
  await expect.poll(()=>page.evaluate(()=>(window as any).testCalls.filter((c:any)=>c.command==='reveal_job_output').length)).toBe(2);
});

test('one-click job displays download percentage, unknown totals, preparation and cancellation',async({page})=>{
  await desktop(page);await page.evaluate(()=>(window as any).testSlowImport=true);
  await page.getByRole('button',{name:'Criar clipes',exact:true}).click();
  await page.getByRole('textbox',{name:'Link do vídeo',exact:true}).fill('https://www.youtube.com/watch?v=example');
  await page.getByRole('button',{name:'Gerar clipes prontos',exact:true}).click();
  const progress=page.locator('.import-progress');
  await expect(progress.getByRole('progressbar')).toHaveAttribute('aria-valuenow','0');
  await expect(page.getByRole('button',{name:'Processando…',exact:true})).toBeDisabled();
  await page.evaluate(()=>{(window as any).testState.jobs=[{id:'job-progress',kind:'package',label:'Gerar clipes prontos',status:'running',message:'Baixando vídeo · 4.2 / 10.0 MB',progress:.42,outputId:null,outputDir:null,createdAt:1700000000}]});
  await expect(progress.getByRole('progressbar')).toHaveAttribute('aria-valuenow','42');
  await page.screenshot({path:'test-results/package-progress.png',fullPage:true});
  await page.evaluate(()=>Object.assign((window as any).testState.jobs[0],{message:'Baixando vídeo · 4.2 MB · tamanho total não informado',progress:null}));
  await expect(progress.getByRole('progressbar')).not.toHaveAttribute('aria-valuenow');
  await page.evaluate(()=>Object.assign((window as any).testState.jobs[0],{message:'Preparando vídeo · Apple Silicon',progress:.18}));
  await expect(progress.getByRole('progressbar')).toHaveAttribute('aria-valuenow','18');
  await progress.getByRole('button',{name:'Cancelar Gerar clipes prontos'}).click();
  await expect.poll(()=>page.evaluate(()=>(window as any).testCalls.some((c:any)=>c.command==='cancel_job'&&c.args.id==='job-progress'))).toBe(true);
  await page.evaluate(()=>Object.assign((window as any).testState.jobs[0],{status:'cancelled',message:'Cancelled'}));
  await expect(page.getByRole('button',{name:'Gerar clipes prontos',exact:true})).toBeEnabled();
  await expect(progress.getByRole('progressbar')).toHaveCount(0);
  expect(await page.evaluate(()=>(window as any).testCalls.some((c:any)=>c.command==='reveal_job_output'))).toBe(false);
});

test('manual export opens its folder and advanced options validate before starting',async({page})=>{
  await desktop(page);await page.getByRole('button',{name:'Criar clipes',exact:true}).click();
  await page.getByRole('textbox',{name:'Link do vídeo',exact:true}).fill('https://www.youtube.com/watch?v=example');
  await page.locator('summary').click();
  await page.getByRole('spinbutton',{name:'Mínimo (s)',exact:true}).fill('100');
  await expect(page.getByRole('button',{name:'Gerar clipes prontos',exact:true})).toBeDisabled();
  await page.getByRole('spinbutton',{name:'Mínimo (s)',exact:true}).fill('30');
  await page.getByRole('button',{name:'Arquivo ou projeto'}).click();
  await page.getByLabel('Vídeo da biblioteca').selectOption('source-1');
  await expect(page.getByRole('button',{name:'Gerar clipes prontos',exact:true})).toBeEnabled();
  await page.evaluate(()=>{(window as any).testState.jobs=[{id:'manual-export',kind:'export',label:'Exportar clipe',status:'completed',message:'Concluído',progress:1,outputId:'clip-1',outputDir:null,createdAt:1700000001}]});
  await expect.poll(()=>page.evaluate(()=>(window as any).testCalls.some((c:any)=>c.command==='reveal_job_output'&&c.args.id==='manual-export'))).toBe(true);
});

test('Clone discovers high-view videos, auto-downloads defaults, and exposes source links',async({page})=>{
  await desktop(page);await page.getByRole('button',{name:'Clone',exact:true}).click();
  await expect(page.getByRole('textbox',{name:'Temas'})).toHaveValue('GTA 6');
  await page.getByRole('button',{name:'Buscar e baixar',exact:true}).click();
  await expect(page.getByRole('heading',{name:'GTA 6: melhor momento'})).toBeVisible();
  const calls=await page.evaluate(()=>(window as any).testCalls);
  expect(calls).toContainEqual({command:'clone_search',args:{request:{query:'GTA 6',platform:'both',minViews:100000,count:5,download:true}}});
  await expect.poll(()=>page.evaluate(()=>(window as any).testCalls.some((c:any)=>c.command==='reveal_job_output'&&c.args.id==='clone-job'))).toBe(true);
  await page.getByRole('checkbox',{name:'Selecionar Jogada incrível'}).check();
  await page.getByRole('button',{name:'Baixar selecionados (1)'}).click();
  expect(await page.evaluate(()=>(window as any).testCalls)).toContainEqual({command:'clone_download',args:{urls:['https://www.tiktok.com/@gaming/video/1234567890123456789']}});
  await page.getByRole('button',{name:'Ver original: GTA 6: melhor momento'}).click();
  expect(await page.evaluate(()=>(window as any).testCalls)).toContainEqual({command:'open_clone_source',args:{url:'https://www.youtube.com/shorts/abcdefghijk'}});
  for(const width of [1440,768,320]){
    await page.setViewportSize({width,height:1000});
    expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth+1)).toBe(true);
    if(width===1440){await expect(page.locator('.clone-progress')).toContainText('Clone · vídeos salvos');await page.locator('main').evaluate(el=>el.scrollTo(0,0));await page.screenshot({path:'test-results/clone-results.png',fullPage:true});}
  }
});

test('Clone search-only never opens a folder; invalid input and source failures stay visible',async({page})=>{
  await desktop(page);await page.getByRole('button',{name:'Clone',exact:true}).click();
  await page.getByRole('spinbutton',{name:'Vídeos para baixar'}).fill('11');
  await expect(page.getByRole('button',{name:'Buscar e baixar',exact:true})).toBeDisabled();
  await page.getByRole('spinbutton',{name:'Vídeos para baixar'}).fill('2');
  await page.getByRole('combobox',{name:'Plataformas'}).selectOption('tiktok');
  await page.getByRole('button',{name:'Somente buscar',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Jogada incrível'})).toBeVisible();
  const calls=await page.evaluate(()=>(window as any).testCalls);
  expect(calls).toContainEqual({command:'clone_search',args:{request:{query:'GTA 6',platform:'tiktok',minViews:100000,count:2,download:false}}});
  expect(calls.some((c:any)=>c.command==='reveal_job_output')).toBe(false);
  await page.evaluate(()=>{const s=(window as any).testState;s.discovery.warnings=['TikTok: um vídeo ficou indisponível.'];s.jobs[0].status='failed';s.jobs[0].message='Nenhum vídeo encontrado';});
  await expect(page.getByText('TikTok: um vídeo ficou indisponível.')).toBeVisible();
  await expect(page.locator('.clone-progress')).toContainText('Nenhum vídeo encontrado');
  await expect(page.getByRole('button',{name:'Buscar e baixar',exact:true})).toBeEnabled();
});


test('Clone topics stay editable during work and persist across navigation and reload',async({page})=>{
  await desktop(page);await page.getByRole('button',{name:'Clone',exact:true}).click();
  const input=page.getByRole('textbox',{name:'Temas',exact:true});
  await input.fill('Minecraft');
  await page.getByRole('button',{name:'Dashboard',exact:true}).click();
  await page.getByRole('button',{name:'Clone',exact:true}).click();
  await expect(input).toHaveValue('Minecraft');
  await page.reload();await page.getByRole('button',{name:'Clone',exact:true}).click();
  await expect(input).toHaveValue('Minecraft');
  await page.getByRole('button',{name:'Somente buscar',exact:true}).click();
  expect(await page.evaluate(()=>(window as any).testCalls)).toContainEqual({command:'clone_search',args:{request:{query:'Minecraft',platform:'both',minViews:100000,count:5,download:false}}});
  await page.evaluate(()=>{(window as any).testState.jobs[0].status='running'});
  await expect(page.getByRole('button',{name:'Somente buscar',exact:true})).toBeDisabled();
  await expect(input).toBeEditable();await input.fill('GTA VI');
  await expect(page.getByText('Cenas do trailer aparecem nos quadros.')).toBeVisible();
  await page.evaluate(()=>{(window as any).testState.jobs[0].status='completed'});
  await input.fill('a,b,c,d');await expect(page.getByRole('button',{name:'Buscar e baixar',exact:true})).toBeDisabled();
});
