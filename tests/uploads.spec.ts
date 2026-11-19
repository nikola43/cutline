import {test,expect} from '@playwright/test';
import {upload} from '../uploader/platforms.mjs';

test('YouTube adapter selects privacy and only records success after confirmation',async({page})=>{
  await page.route('**/*',route=>route.fulfill({contentType:'text/html',body:`
    <input type="file"><ytcp-entity-page-header><span id="entity-name">Test channel</span></ytcp-entity-page-header>
    <div id="title-textarea"><div id="textbox" contenteditable="true"></div></div>
    <div id="description-textarea"><div id="textbox" contenteditable="true"></div></div>
    <tp-yt-paper-radio-button name="VIDEO_MADE_FOR_KIDS_NOT_MFK" onclick="this.setAttribute('aria-checked','true')">Not for kids</tp-yt-paper-radio-button>
    <button id="next-button">Next</button>
    <tp-yt-paper-radio-button name="PRIVATE" onclick="this.setAttribute('aria-checked','true')">Private</tp-yt-paper-radio-button>
    <div id="video-url"><a href="https://youtu.be/abcdefghijk">Link</a></div>
    <button id="done-button" onclick="document.querySelector('ytcp-video-share-dialog').hidden=false">Save</button>
    <ytcp-video-share-dialog hidden><a href="https://youtu.be/abcdefghijk">Saved video</a></ytcp-video-share-dialog>
  `}));
  await page.goto('https://studio.youtube.com/channel/UCtest/videos/upload');
  const states:any[]=[];
  const result=await upload(page,{platform:'youtube',expectedAccount:'UCtest',privacy:'private',file:{name:'clip.mp4',mimeType:'video/mp4',buffer:Buffer.from('fixture')},title:'My clip',description:'Description #gta6'},async value=>{states.push(value)});
  expect(states.map(s=>s.status)).toEqual(['uploading','posting','uploaded']);
  expect(result.url).toBe('https://www.youtube.com/watch?v=abcdefghijk');
  await expect(page.locator('#title-textarea #textbox')).toHaveText('My clip');
});

test('TikTok adapter refuses account changes and preserves an uncertain post',async({page})=>{
  await page.route('**/*',route=>route.fulfill({contentType:'text/html',body:`
    <a href="https://www.tiktok.com/@creator">Account</a><input type="file">
    <div contenteditable="true" role="textbox"></div>
    <button role="combobox">Everyone</button>
    <span onclick="document.querySelector('[role=combobox]').textContent=this.textContent">Only you</span>
    <button onclick="window.postClicked=true">Post</button>
  `}));
  await page.goto('https://www.tiktok.com/tiktokstudio/upload');
  const request={platform:'tiktok',expectedAccount:'different',privacy:'private',file:{name:'clip.mp4',mimeType:'video/mp4',buffer:Buffer.from('fixture')},title:'Title',description:'Caption'};
  const states:any[]=[];
  await expect(upload(page,request,async value=>{states.push(value)})).rejects.toThrow('conta');
  expect(states).toHaveLength(0);
  const posting=upload(page,{...request,expectedAccount:'creator'},async value=>{states.push(value)});
  const checked=expect(posting).rejects.toThrow();
  await expect.poll(()=>page.evaluate(()=>(window as any).postClicked)).toBe(true);
  await page.close();
  await checked;
  expect(states.map(s=>s.status)).toEqual(['uploading','posting']);
});

test('TikTok adapter confirms an upload only after the success screen',async({page})=>{
  await page.route('**/*',route=>route.fulfill({contentType:'text/html',body:`
    <a href="https://www.tiktok.com/@creator">Account</a><input type="file">
    <div contenteditable="true" role="textbox"></div>
    <button role="combobox">Everyone</button>
    <span onclick="document.querySelector('[role=combobox]').textContent=this.textContent">Only you</span>
    <button onclick="document.getElementById('success').hidden=false">Post</button>
    <div id="success" hidden><p>Your video has been uploaded</p><a href="https://www.tiktok.com/@creator/video/1234567890123456789">Caption</a></div>
  `}));
  await page.goto('https://www.tiktok.com/tiktokstudio/upload');
  const states:any[]=[];
  await upload(page,{platform:'tiktok',expectedAccount:'creator',privacy:'private',file:{name:'clip.mp4',mimeType:'video/mp4',buffer:Buffer.from('fixture')},title:'Title',description:'Caption'},async value=>{states.push(value)});
  expect(states.at(-1)).toEqual({status:'uploaded',url:'https://www.tiktok.com/@creator/video/1234567890123456789'});
});

test('TikTok detects the signed-in profile and confirms the redirected content row',async({page})=>{
  await page.route('**/*',route=>{
    const path=new URL(route.request().url()).pathname;
    const body=path==='/'?'<a data-e2e="nav-profile" href="/@creator">Profile</a>':path==='/tiktokstudio/content'?'<a href="https://www.tiktok.com/@creator/video/1234567890123456789">Caption</a>':`
      <input type="file"><div contenteditable="true" role="textbox"></div>
      <button role="combobox">Everyone</button><span onclick="document.querySelector('[role=combobox]').textContent=this.textContent">Only you</span>
      <button onclick="location.href='/tiktokstudio/content'">Post</button>`;
    return route.fulfill({contentType:'text/html',body});
  });
  const {ready}=await import('../uploader/platforms.mjs');
  const identity=await ready(page,'tiktok');expect(identity.id).toBe('creator');
  const states:any[]=[];
  await upload(page,{platform:'tiktok',expectedAccount:'creator',verifiedAccount:identity,previousUrls:[],privacy:'private',file:{name:'clip.mp4',mimeType:'video/mp4',buffer:Buffer.from('fixture')},title:'Title',description:'Caption'},async value=>{states.push(value)});
  expect(states.at(-1)).toEqual({status:'uploaded',url:'https://www.tiktok.com/@creator/video/1234567890123456789'});
});
