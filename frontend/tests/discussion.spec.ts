import {test,expect} from '@playwright/test';
for (const admin of [true,false]) test(`server discussion and scoped management: admin=${admin}`,async({page},info)=>{
 let comments:any[]=[];let hooks:any[]=[];let mutationPath='';let settingsBody:any=null;
 const post={id:'post',artist_id:'artist',title:'Night garden',author:'Aoi',feed_name:'Aoi',url:'https://example.com/post',summary:'A study in moonlight.',media:[],published_at:'2026-10-07T12:00:00Z',favorite:false,liked:false,likes:2};
 await page.route('**/api/community/**',async route=>{
  const path=new URL(route.request().url()).pathname;const method=route.request().method();let json:any;
  if(path.endsWith('/config'))json={discord_enabled:true};
  else if(path.endsWith('/me'))json={id:'user',display_name:'Collector',avatar_url:''};
  else if(path.endsWith('/servers'))json=[{id:'server',name:'Atelier',external_id:'1',connected:true,can_manage:admin}];
  else if(path.endsWith('/comments')&&method==='POST'){const body=route.request().postDataJSON();comments=[{id:'comment',body:body.body,created_at:new Date().toISOString(),can_delete:true,author:{id:'user',platform:'discord',display_name:'Collector',avatar_url:''}}];json={id:'comment'};}
  else if(path.endsWith('/comments/comment')&&method==='DELETE'){comments=[];json={ok:true};}
  else if(path.endsWith('/comments'))json={items:comments,total:comments.length,page:1,pages:1};
  else if(path.endsWith('/webhooks')&&method==='POST'){mutationPath=path;hooks=[{...route.request().postDataJSON(),id:'hook'}];json=hooks[0];}
  else if(path.endsWith('/webhooks'))json=hooks;
  else if(path.endsWith('/posts'))json={items:[post],total:1,page:1,pages:1};
  else if(path.endsWith('/servers/server')&&method==='PUT'){settingsBody=route.request().postDataJSON();json={ok:true};}
  else json={server:{id:'server',name:'Atelier',channel_id:'123',channel_name:'art',enabled:true,mentions_enabled:true},can_manage:admin};
  await route.fulfill({json});
 });
 await page.goto('/community');
 await page.getByRole('button',{name:'Comments on Night garden'}).click();
 await expect(page.getByRole('heading',{name:'Start the conversation.'})).toBeVisible();
 await page.getByLabel('Add to the conversation').fill('The moonlight is lovely. <script>not executable</script>');
 await page.getByRole('button',{name:'Post comment',exact:true}).click();
 await expect(page.locator('.comment-content')).toContainText('Collector');
 await expect(page.locator('.comment-content')).toContainText('<script>not executable</script>');
 await expect(page.locator('.comment-provider')).toHaveText('discord');
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBeTruthy();
 await page.getByRole('dialog').screenshot({path:`../.test-runtime/comments-${admin}-${info.project.name}.png`});
 await page.getByRole('button',{name:'Delete comment by Collector'}).click();
 await page.locator('.comment-removal').getByRole('button',{name:'Delete',exact:true}).click();
 await expect(page.getByRole('heading',{name:'Start the conversation.'})).toBeVisible();
 await page.getByRole('button',{name:'Close',exact:true}).click();
 if(admin){
  await page.getByRole('button',{name:'Server settings',exact:true}).click();
  await page.getByLabel('Discord channel ID',{exact:true}).fill('');
  await page.getByRole('button',{name:'Save settings',exact:true}).click();
  await expect.poll(()=>settingsBody?.channel_id).toBe('');
  await page.getByRole('button',{name:'Add webhook',exact:true}).click();
  await page.getByLabel('Destination name',{exact:true}).fill('Art announcements');
  await page.getByLabel('Webhook URL',{exact:true}).fill('https://example.com/webhook');
  await page.getByRole('button',{name:'Save webhook',exact:true}).click();
  await expect(page.getByText('Art announcements',{exact:true})).toBeVisible();
  expect(mutationPath).toBe('/api/community/servers/server/webhooks');
 }else await expect(page.getByRole('button',{name:'Server settings',exact:true})).toHaveCount(0);
});
