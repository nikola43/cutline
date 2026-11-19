import {writeFile, rename} from 'node:fs/promises';

export const studio = {tiktok:'https://www.tiktok.com/tiktokstudio/upload',youtube:'https://www.youtube.com/upload'};
const fileInput = page => page.locator('input[type="file"]').first();
const delay = ms => new Promise(resolve=>setTimeout(resolve,ms));

export async function receipt(path, value) {
  await writeFile(`${path}.tmp`, JSON.stringify(value), {mode:0o600});
  await rename(`${path}.tmp`, path);
}

export async function account(page, platform) {
  const url = new URL(page.url());
  if(platform==='youtube') {
    const id = url.hostname==='studio.youtube.com' && url.pathname.match(/^\/channel\/(UC[\w-]+)(?:\/|$)/)?.[1];
    if(!id || !await fileInput(page).count()) return null;
    const label = await page.locator('ytcp-entity-page-header #entity-name').first().textContent({timeout:1000}).catch(()=>null);
    return {id,label:label?.trim()||id};
  }
  if(url.hostname!=='www.tiktok.com' || !url.pathname.startsWith('/tiktokstudio') || !await fileInput(page).count()) return null;
  const handles = await page.locator('a[href*="/@"]').evaluateAll(links=>[...new Set(links.filter(a=>a.getClientRects().length).map(a=>{
    try {const u=new URL(a.href);return u.hostname==='www.tiktok.com'?u.pathname.match(/^\/@([\w.-]+)\/?$/)?.[1]:null}catch{return null}
  }).filter(Boolean))]);
  return handles.length===1?{id:handles[0],label:`@${handles[0]}`}:null;
}

export async function ready(page, platform, interactive=false) {
  await page.goto(platform==='tiktok'?'https://www.tiktok.com/':studio[platform], {waitUntil:'domcontentloaded',timeout:60000});
  const until=Date.now()+(interactive?10*60_000:30_000);
  do {
    if(platform==='tiktok') {
      const ownProfile=page.locator('a[data-e2e="nav-profile"], [data-e2e="nav-profile"] a, [data-e2e="profile-icon"] a').first();
      const href=await ownProfile.getAttribute('href',{timeout:1000}).catch(()=>null);
      const url=href?new URL(href,'https://www.tiktok.com'):null;
      const id=url?.hostname==='www.tiktok.com'?url.pathname.match(/^\/@([\w.-]+)\/?$/)?.[1]:null;
      if(id) {
        await page.goto(studio.tiktok,{waitUntil:'domcontentloaded',timeout:60000});
        await fileInput(page).waitFor({state:'attached',timeout:30_000});
        return {id,label:`@${id}`};
      }
    }
    const identity=await account(page,platform);
    if(identity) return identity;
    await delay(1000);
  } while(Date.now()<until && !page.isClosed());
  throw new Error('Entre na conta na janela de uploads, conclua as confirmações da plataforma e tente conectar novamente. A conta precisa estar identificada na página de envio.');
}

function videoUrl(raw, platform) {
  try {
    const u=new URL(raw);
    if(u.protocol!=='https:')return null;
    if(platform==='youtube' && u.hostname==='youtu.be' && /^\/[\w-]{11}$/.test(u.pathname)) return `https://www.youtube.com/watch?v=${u.pathname.slice(1)}`;
    if(platform==='youtube' && ['www.youtube.com','youtube.com'].includes(u.hostname) && /^[\w-]{11}$/.test(u.searchParams.get('v')||'')) return `https://www.youtube.com/watch?v=${u.searchParams.get('v')}`;
    if(platform==='tiktok' && u.hostname==='www.tiktok.com' && /^\/@[\w.-]+\/video\/\d{15,22}$/.test(u.pathname))return `${u.origin}${u.pathname}`;
  } catch {}
  return null;
}

async function clickVisible(locator) {
  await locator.waitFor({state:'visible',timeout:60_000});
  await locator.click({timeout:300_000});
}

export async function upload(page, request, save) {
  const {platform,expectedAccount,privacy,file,title,description}=request;
  if(!['private','public'].includes(privacy))throw new Error('Visibilidade inválida.');
  const current=request.verifiedAccount||await account(page,platform);
  if(!current || current.id!==expectedAccount)throw new Error('A conta do navegador mudou. Reconecte a conta antes de enviar.');
  // Persist before bytes leave the computer; interrupted uploads must not be retried blindly.
  await save({status:'uploading',url:null});
  await fileInput(page).setInputFiles(file);
  if(platform==='youtube') {
    const titleBox=page.locator('#title-textarea #textbox');
    await titleBox.waitFor({state:'visible',timeout:180_000});
    await titleBox.fill(title.slice(0,100));
    await page.locator('#description-textarea #textbox').fill(description.slice(0,5000));
    await clickVisible(page.locator('tp-yt-paper-radio-button[name="VIDEO_MADE_FOR_KIDS_NOT_MFK"]'));
    for(let step=0;step<3;step++)await clickVisible(page.locator('#next-button'));
    const visibility=page.locator(`tp-yt-paper-radio-button[name="${privacy==='public'?'PUBLIC':'PRIVATE'}"]`);
    await clickVisible(visibility);
    if(!await visibility.evaluate(el=>el.checked===true||el.getAttribute('aria-checked')==='true'))throw new Error('A plataforma não confirmou a visibilidade escolhida.');
    const url=videoUrl(await page.locator('#video-url a').first().getAttribute('href').catch(()=>null),platform);
    await save({status:'posting',url});
    await clickVisible(page.locator('#done-button'));
    const done=page.locator('ytcp-video-share-dialog');
    await done.waitFor({state:'visible',timeout:300_000});
    const confirmed=videoUrl(await done.locator('a[href]').first().getAttribute('href').catch(()=>null),platform)||url;
    if(!confirmed)throw new Error('A plataforma não confirmou o link do vídeo. Confira o Studio antes de reenviar.');
    await save({status:'uploaded',url:confirmed});
    return {url:confirmed};
  }
  const caption=page.locator('[contenteditable="true"][data-contents="true"], .public-DraftEditor-content, [contenteditable="true"][role="textbox"]').first();
  await caption.waitFor({state:'visible',timeout:180_000});
  await caption.fill(description.slice(0,2200)||title.slice(0,2200));
  const visibility=page.getByRole('combobox').first();
  await clickVisible(visibility);
  const choice=privacy==='public'?/^(Everyone|Todos|Público)$/:/^(Only you|Only me|Somente você|Apenas eu)$/;
  await clickVisible(page.getByText(choice).last());
  if(!choice.test((await visibility.innerText()).trim()))throw new Error('A plataforma não confirmou a visibilidade escolhida.');
  await save({status:'posting',url:null});
  await clickVisible(page.getByRole('button',{name:/^(Post|Publicar|Postar)$/}).first());
  // Studio may show a success notice or redirect to the new content row.
  const success=page.locator('[role="status"], [role="alert"], [data-e2e="upload-success"]').filter({hasText:/^(Your video has been uploaded|Your video has been posted|Video uploaded|Vídeo enviado|Vídeo publicado|Seu vídeo foi publicado)/i}).first();
  const confirm=page.getByRole('dialog').getByRole('button',{name:/^(Post now|Post|Publicar agora|Publicar)$/});
  const deadline=Date.now()+300_000;
  let confirmed=false;
  do {
    if(page.isClosed())throw new Error('A janela foi fechada sem confirmação da plataforma.');
    if(!confirmed&&await confirm.isVisible().catch(()=>false)){await confirm.click();confirmed=true;}
    const row=page.getByRole('link',{name:description.slice(0,2200)||title.slice(0,2200),exact:true}).first();
    const url=videoUrl(await row.getAttribute('href',{timeout:500}).catch(()=>null),platform);
    const newRow=new URL(page.url()).pathname==='/tiktokstudio/content'&&url&&!request.previousUrls?.includes(url)&&new URL(url).pathname.startsWith(`/@${expectedAccount}/video/`);
    if(await success.isVisible().catch(()=>false)||newRow){await save({status:'uploaded',url});return {url};}
    await delay(500);
  } while(Date.now()<deadline);
  throw new Error('A plataforma não confirmou a publicação. Confira o Studio antes de reenviar.');
}
