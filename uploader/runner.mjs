import {chromium} from 'playwright-core';
import {mkdir,realpath,access} from 'node:fs/promises';
import {sep,join} from 'node:path';
import {homedir} from 'node:os';
import {spawn,execFileSync} from 'node:child_process';
import {ready,receipt,upload,studio} from './platforms.mjs';

let context;
let request;
try {
  let input='';
  for await (const chunk of process.stdin)input+=chunk;
  request=JSON.parse(input);
  if(!['login','studio','connect','check','upload'].includes(request.action)||!['tiktok','youtube'].includes(request.platform))throw new Error('Comando de upload inválido.');
  await mkdir(request.profile,{recursive:true,mode:0o700});
  if(['login','studio'].includes(request.action)) {
    const candidates=process.platform==='darwin'?['/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',join(homedir(),'Applications/Google Chrome.app/Contents/MacOS/Google Chrome')]:process.platform==='win32'?[process.env.PROGRAMFILES,process.env['PROGRAMFILES(X86)'],process.env.LOCALAPPDATA].filter(Boolean).map(p=>join(p,'Google/Chrome/Application/chrome.exe')):[execFileSync('which',['google-chrome'],{encoding:'utf8'}).trim()];
    let chrome;
    for(const file of candidates)if(await access(file).then(()=>true,()=>false)){chrome=file;break;}
    if(!chrome)throw new Error('Instale Google Chrome para conectar as contas.');
    // Login uses a normal browser window. No credential extraction or automation flags.
    const child=spawn(chrome,[`--user-data-dir=${request.profile}`,'--no-first-run','--new-window',request.action==='studio'?(request.platform==='tiktok'?'https://www.tiktok.com/tiktokstudio/content':'https://studio.youtube.com'):studio[request.platform]],{detached:true,stdio:'ignore'});
    await new Promise((ok,fail)=>{child.once('spawn',ok);child.once('error',fail)});
    child.unref();
    process.stdout.write(JSON.stringify({opened:true}));
  } else {
  context=await chromium.launchPersistentContext(request.profile,{channel:'chrome',headless:false,viewport:null,locale:'en-US',timeout:60000}).catch(()=>{throw new Error('Encerre a sessão de uploads pelo menu Sair do Chrome (⌘Q no macOS) antes de verificar a conexão ou enviar. Sua janela normal do Chrome pode continuar aberta.');});
  const page=context.pages()[0]||await context.newPage();
  page.setDefaultTimeout(30000);
  const identity=await ready(page,request.platform,request.action==='connect');
  if(request.expectedAccount && request.expectedAccount!==identity.id)throw new Error('A conta do navegador mudou. Conecte novamente a conta desejada.');
  if(request.action==='upload') {
    const file=await realpath(request.file);
    const exports=await realpath(request.exports);
    if(!file.startsWith(exports+sep)||!file.endsWith('.mp4'))throw new Error('Arquivo fora da pasta de exportações.');
    let previousUrls=[];
    if(request.platform==='tiktok') {
      await page.goto('https://www.tiktok.com/tiktokstudio/content',{waitUntil:'networkidle',timeout:60000});
      previousUrls=await page.locator('a[href*="/video/"]').evaluateAll(nodes=>nodes.map(a=>a.href.split('?')[0]));
      await page.goto(studio.tiktok,{waitUntil:'domcontentloaded',timeout:60000});
      await page.locator('input[type="file"]').first().waitFor({state:'attached',timeout:30000});
    }
    const result=await upload(page,{...request,file,verifiedAccount:identity,previousUrls},state=>receipt(request.receipt,state));
    process.stdout.write(JSON.stringify({...result,account:identity}));
  } else process.stdout.write(JSON.stringify({account:identity}));
  }
} catch(error) {
  // Never dump browser traces, cookies or page HTML into application logs.
  const message=String(error?.message||error);
  const known=/conta|confirma|plataforma|Visibilidade|Arquivo fora|Comando|Encerre|Instale/.test(message);
  process.stderr.write(known?message:'Upload interrompido. Confira a janela do Chrome: login, confirmação, limite da plataforma ou mudança na tela de envio. Os arquivos locais foram preservados.');
  process.exitCode=1;
} finally {
  await context?.close().catch(()=>{});
}
