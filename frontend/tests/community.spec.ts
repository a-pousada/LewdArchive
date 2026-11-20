import {test,expect} from '@playwright/test';
test('community preferences stay separate and server navigation resets collections',async({page},info)=>{
 let tracked: Record<string,unknown> | null = null;
 let imported=false;
 let favorite=false, notify=false, liked=false, saved=false;
 const first='first-server', second='second-server';
 await page.route('**/api/community/**', async route=>{
  const url=new URL(route.request().url()), path=url.pathname.replace('/api/community','');
  const body=route.request().postDataJSON();
  let value:unknown;
  if(path==='/config')value={discord_enabled:true,install_url:'https://discord.com/oauth2/authorize'};
  else if(path==='/feed-options')value={rsshub_base_url:'http://rsshub:1200'};
  else if(path.endsWith('/artists') && route.request().method()==='POST'){tracked=body;value={id:'new-artist'};}
  else if(path.endsWith('/feeds/import/preview'))value={new:1,duplicates:0,invalid:0,feeds:[{name:'Imported',url:'https://example.com/import',category:'Art',enabled:true,status:'new',error:null}]};
  else if(path.endsWith('/feeds/import')){imported=true;value={imported:1,duplicates:0,invalid:0};}
  else if(path.endsWith('/webhooks'))value=[];
  else if(path==='/me')value={id:'member',display_name:'Collector',avatar_url:''};
  else if(path==='/servers')value=[{id:first,external_id:'1',name:'Atelier',connected:true,can_manage:true},{id:second,external_id:'2',name:'Sketchbook',connected:true,can_manage:false}];
  else if(path.endsWith('/artists/artist') && route.request().method()==='PATCH'){favorite=body.favorite??favorite;notify=body.notify??notify;value={ok:true};}
  else if(path.endsWith('/posts/post') && route.request().method()==='PATCH'){liked=body.liked??liked;saved=body.favorite??saved;value={ok:true};}
  else if(path.endsWith('/artists'))value=[{id:'artist',name:'Aoi',source:'pixiv',url:'https://example.com/feed',favorite,notify,last_error:null}];
  else if(path.endsWith('/posts')){
   const items=url.searchParams.get('favorites')==='true'&&!saved?[]:[{id:'post',artist_id:'artist',title:'Evening study',author:'Aoi',feed_name:'Aoi',url:'https://example.com/post',summary:'Sketchbook notes.',media:[],published_at:'2026-10-06T12:00:00Z',favorite:saved,liked,likes:liked?1:0}];
   value={items,total:items.length,page:1,pages:1};
  }else value={server:{id:path.includes(first)?first:second,name:path.includes(first)?'Atelier':'Sketchbook',channel_id:'123',channel_name:'art',enabled:true,mentions_enabled:true},can_manage:path.includes(first)};
  await route.fulfill({json:value});
 });
 await page.goto('/community');
 await expect(page.getByRole('heading',{name:'Latest artwork'})).toBeVisible();
 await page.screenshot({path:`../.test-runtime/community-${info.project.name}.png`,fullPage:true});
 const tools=page.locator('.post-card-tools');
 await expect(tools).toBeVisible();
 const comments=await tools.getByRole('button',{name:'Comments on Evening study'}).boundingBox();
 const files=await tools.getByRole('button',{name:'Files for Evening study'}).boundingBox();
 expect(comments&&files&&(files.x>=comments.x+comments.width+8||files.y>=comments.y+comments.height+8)).toBeTruthy();
 await page.getByRole('button',{name:'Track artist',exact:true}).click();
 await page.getByLabel('Feed name',{exact:true}).fill('New artist');
 await page.getByLabel('Artist ID',{exact:true}).fill('22445');
 await expect(page.getByText('http://rsshub:1200/pawchive/fanbox/22445',{exact:true})).toBeVisible();
 await expect(page.getByText('Notify webhooks',{exact:true})).toHaveCount(0);
 await page.getByRole('button',{name:'Add feed',exact:true}).click();
 await expect(page.getByRole('dialog')).toHaveCount(0);
 expect(tracked).toMatchObject({name:'New artist',url:'http://rsshub:1200/pawchive/fanbox/22445'});
 await page.getByRole('button',{name:'Like post',exact:true}).click();
 await expect(page.getByRole('button',{name:'Unlike post',exact:true})).toHaveAttribute('aria-pressed','true');
 await page.getByRole('button',{name:'Save post',exact:true}).click();
 await page.getByRole('button',{name:'Saved posts',exact:true}).click();
 await expect(page.getByText('Only you can see this collection.')).toBeVisible();
 await expect(page.getByRole('heading',{name:'Evening study'})).toBeVisible();
 await page.getByRole('button',{name:'Artists',exact:true}).click();
 await expect(page.getByRole('link',{name:'Export OPML'})).toHaveAttribute('href',`/api/community/servers/${first}/feeds/export`);
 await page.getByRole('button',{name:'Import OPML',exact:true}).click();
 await page.getByLabel('OPML file').setInputFiles({name:'feeds.opml',mimeType:'text/xml',buffer:Buffer.from('<opml><body/></opml>')});
 await expect(page.getByLabel('Automatically download new files')).toHaveCount(0);
 await page.getByRole('button',{name:'Import 1 feeds',exact:true}).click();
 await expect(page.getByRole('heading',{name:'1 feeds imported'})).toBeVisible();
 expect(imported).toBe(true);
 await page.getByRole('button',{name:'Done',exact:true}).click();
 await page.getByRole('button',{name:'Favorite',exact:true}).click();
 await expect(page.getByRole('button',{name:'Favorited',exact:true})).toHaveAttribute('aria-pressed','true');
 await expect(page.getByRole('button',{name:'Notify me',exact:true})).toHaveAttribute('aria-pressed','false');
 await page.getByRole('button',{name:'Notify me',exact:true}).click();
 await expect(page.getByRole('button',{name:'Mentions on',exact:true})).toHaveAttribute('aria-pressed','true');
 await page.getByRole('button',{name:'Mentions on',exact:true}).click();
 await expect(page.getByRole('button',{name:'Notify me',exact:true})).toHaveAttribute('aria-pressed','false');
 await page.getByRole('button',{name:'Server settings',exact:true}).click();
 const toggles=page.locator('.community-settings input[type=checkbox]');
 await expect(toggles).toHaveCount(3);
 await expect(page.getByRole('checkbox',{name:'Automatically tag new posts',exact:true})).not.toBeChecked();
 for (const toggle of await toggles.all()) {
  const bounds=await toggle.boundingBox();
  expect(bounds?.width).toBeLessThanOrEqual(40);
  expect(bounds?.width).toBeGreaterThan(15);
 }
 await page.screenshot({path:`../.test-runtime/settings-fixed-${info.project.name}.png`,fullPage:true});
 await page.getByLabel('Your server').selectOption(second);
 await expect(page.getByRole('heading',{name:'Latest artwork'})).toBeVisible();
 await expect(page.getByRole('button',{name:'Server settings',exact:true})).toHaveCount(0);
 await expect(page.getByRole('button',{name:'Track artist',exact:true})).toHaveCount(0);
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBeTruthy();
});
test('community login explains a disabled Discord integration',async({page})=>{
 await page.route('**/api/community/config',r=>r.fulfill({json:{discord_enabled:false,install_url:null}}));
 await page.route('**/api/community/me',r=>r.fulfill({status:401,json:{error:'Sign in'}}));
 await page.goto('/community');
 await expect(page.getByText('The Discord integration is not configured.',{exact:false})).toBeVisible();
 await expect(page.getByRole('link',{name:'Continue with Discord'})).toHaveCount(0);
});
