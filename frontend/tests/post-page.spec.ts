import {test,expect} from '@playwright/test';
test('post permalink displays all media, inline comments and share URL',async({page},info)=>{
 const post={id:'post',artist_id:'artist',title:'Moon studies',author:'Aoi',feed_name:'Aoi',url:'https://example.com/original',summary:'Two views of the same evening.',media:[{kind:'image',url:'https://example.com/one.svg'},{kind:'image',url:'https://example.com/two.svg'},{kind:'video',url:'https://example.com/movie.mp4'}],published_at:'2026-10-08T12:00:00Z',liked:false,favorite:false,likes:3};
 await page.route('https://example.com/*.svg',r=>r.fulfill({contentType:'image/svg+xml',body:'<svg xmlns="http://www.w3.org/2000/svg" width="700" height="450"><rect width="700" height="450" fill="#30232d"/><circle cx="350" cy="180" r="80" fill="#e5a0b3"/></svg>'}));
 await page.route('https://example.com/movie.mp4',r=>r.fulfill({status:404,body:''}));
 await page.addInitScript(()=>{Object.defineProperty(navigator,'clipboard',{value:{writeText:async(text:string)=>{(window as any).copied=text;}}});});
 await page.route('**/api/community/**',async r=>{const path=new URL(r.request().url()).pathname;let json:unknown;
 if(path.endsWith('/config'))json={discord_enabled:true};else if(path.endsWith('/me'))json={id:'user',display_name:'Collector',avatar_url:''};else if(path.endsWith('/servers'))json=[{id:'server',name:'Atelier',connected:true,external_id:'1'}];else if(path.endsWith('/posts/post'))json={post,can_manage:false};else if(path.endsWith('/comments'))json={items:[],total:0,page:1,pages:0};else if(path.endsWith('/posts'))json={items:[post],total:1,page:1,pages:1};else json={server:{id:'server',name:'Atelier'},can_manage:false};await r.fulfill({json});});
 await page.goto('/community?server=server');
 await page.getByRole('heading',{name:'Moon studies'}).getByRole('link').click();
 await expect(page).toHaveURL(/server=server&post=post/);
 await expect(page.getByRole('heading',{name:'Moon studies',level:1})).toBeVisible();
 await expect(page.locator('.post-gallery img')).toHaveCount(2);await expect(page.locator('.post-gallery video')).toHaveCount(1);
 await expect(page.getByRole('region',{name:'Post comments'})).toBeVisible();
 await expect(page.getByRole('dialog')).toHaveCount(0);
 await page.getByRole('button',{name:'Share post',exact:true}).click();await expect(page.getByRole('status')).toHaveText('Post link copied.');expect(await page.evaluate(()=>(window as any).copied)).toContain('/community?server=server&post=post');
 await page.reload();await expect(page.getByRole('heading',{name:'Moon studies',level:1})).toBeVisible();
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBeTruthy();
 await page.screenshot({path:`../.test-runtime/post-page-${info.project.name}.png`,fullPage:true});
});
test('unavailable shared posts disclose no content',async({page})=>{
 await page.route('**/api/community/**',async r=>{const p=new URL(r.request().url()).pathname;if(p.endsWith('/posts/missing')){await r.fulfill({status:403,json:{error:'Unable to verify your current server membership'}});return;}await r.fulfill({json:p.endsWith('/me')?{id:'u',display_name:'Member'}:p.endsWith('/servers')?[]:{discord_enabled:true}});});
 await page.goto('/community?server=server&post=missing');await expect(page.getByRole('heading',{name:'Post unavailable'})).toBeVisible();await expect(page.locator('.post-gallery')).toHaveCount(0);
});
test('shared link survives sign-in and clipboard fallback remains usable',async({page})=>{
 let signed=false;
 await page.addInitScript(()=>{Object.defineProperty(navigator,'clipboard',{value:{writeText:async()=>{throw Error('Denied');}}});});
 await page.route('**/api/community/**',async r=>{const p=new URL(r.request().url()).pathname;
 if(p.endsWith('/auth/login')){signed=true;await r.fulfill({status:302,headers:{location:'/community'},body:''});return;}
 if(p.endsWith('/me')&&!signed){await r.fulfill({status:401,json:{error:'Sign in'}});return;}
 const post={id:'p',title:'Shared drawing',feed_name:'Aoi',author:'Aoi',url:'https://example.com/post',media:[],summary:'',published_at:'2026-10-08T12:00:00Z',liked:false,favorite:false,likes:0};
 await r.fulfill({json:p.endsWith('/config')?{discord_enabled:true}:p.endsWith('/me')?{id:'u',display_name:'Member'}:p.endsWith('/servers')?[{id:'s',name:'Atelier',connected:true,external_id:'1'}]:p.endsWith('/posts/p')?{post,can_manage:false}:p.endsWith('/comments')?{items:[],total:0,page:1,pages:0}:{}});
 });
 await page.goto('/community?server=s&post=p');await page.getByRole('link',{name:'Continue with Discord'}).click();
 await expect(page.getByRole('heading',{name:'Shared drawing',level:1})).toBeVisible();await expect(page).toHaveURL(/server=s&post=p/);
 await page.getByRole('button',{name:'Share post',exact:true}).click();await expect(page.getByLabel('Copy this link')).toHaveValue(/community\?server=s&post=p/);
});
