import {test,expect} from '@playwright/test';

test('rankings switch windows and metrics, preserve private saves, and edit artist profiles',async({page},info)=>{
 let lastQuery=new URLSearchParams();
 let profile={name:'Aoi',avatar_url:'',official_links:[] as {label:string;url:string}[]};
 let saved=false;
 const post={id:'post',artist_id:'artist',title:'After the rain',author:'Aoi',feed_name:'Aoi',url:'https://example.com/post',summary:'A quiet evening.',media:[],published_at:'2026-10-07T12:00:00Z',liked:false,likes:12,saves:4,score:16,rank:1,artist_avatar:''};
 await page.route('**/api/community/**',async route=>{
  const url=new URL(route.request().url()),path=url.pathname;
  let json:unknown;
  if(path.endsWith('/config'))json={discord_enabled:true};
  else if(path.endsWith('/me'))json={id:'member',display_name:'Collector',avatar_url:''};
  else if(path.endsWith('/servers'))json=[{id:'server',external_id:'1',name:'Atelier',connected:true,can_manage:true}];
  else if(path.endsWith('/trending')){
   lastQuery=url.searchParams;
   json={items:[{...post,favorite:saved}],total:1,page:1,pages:1,days:7,summary:{posts:9,likes:12,saves:4,artists:3}};
  }else if(path.endsWith('/posts/post')){saved=route.request().postDataJSON().favorite;json={ok:true};}
  else if(path.endsWith('/posts'))json={items:[],total:0,page:1,pages:0};
  else if(path.endsWith('/artists/artist/profile')){profile=route.request().postDataJSON();json={ok:true};}
  else if(path.endsWith('/artists'))json=[{id:'artist',...profile,source:'pixiv',url:'https://example.com/feed',favorite:false,notify:false,last_error:null}];
  else json={server:{id:'server',name:'Atelier',channel_id:'123',channel_name:'art',enabled:true,mentions_enabled:true},can_manage:true};
  await route.fulfill({json});
 });
 await page.goto('/community');
 await page.getByRole('button',{name:'Trending',exact:true}).click();
 await expect(page.getByRole('heading',{name:'Worth a second look.'})).toBeVisible();
 await expect(page.getByRole('heading',{name:'After the rain'})).toBeVisible();
 await page.getByRole('button',{name:'Month 30 days'}).click();
 await expect.poll(()=>lastQuery.get('period')).toBe('month');
 await page.getByLabel('Rank by').selectOption('saves');
 await expect.poll(()=>lastQuery.get('metric')).toBe('saves');
 await page.getByRole('button',{name:'Year 365 days'}).click();
 await expect.poll(()=>lastQuery.get('period')).toBe('year');
 await page.getByRole('button',{name:'Save post',exact:true}).click();
 await expect(page.getByRole('button',{name:'Unsave post',exact:true})).toHaveAttribute('aria-pressed','true');
 await expect(page.getByText('Only totals are public.',{exact:false})).toBeVisible();
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBeTruthy();
 await page.screenshot({path:`../.test-runtime/trending-${info.project.name}.png`,fullPage:true});
 await page.getByRole('button',{name:'Artists',exact:true}).click();
 await page.getByRole('button',{name:'Edit profile',exact:true}).click();
 await page.getByLabel('Artist name',{exact:true}).fill('Aoi Studio');
 await page.getByLabel('Photo URL',{exact:false}).fill('https://example.com/avatar.png');
 await page.getByRole('button',{name:'Add link',exact:true}).click();
 await page.getByLabel('Link 1 label',{exact:true}).fill('Pixiv');
 await page.getByLabel('Link 1 URL',{exact:true}).fill('https://www.pixiv.net/users/123');
 await page.getByRole('button',{name:'Save profile',exact:true}).click();
 await expect(page.getByRole('dialog')).toHaveCount(0);
 await expect(page.getByRole('heading',{name:'Aoi Studio'})).toBeVisible();
 await expect(page.getByRole('link',{name:'Pixiv',exact:true})).toHaveAttribute('href','https://www.pixiv.net/users/123');
 expect(profile.avatar_url).toBe('https://example.com/avatar.png');
 await page.getByRole('button',{name:'Edit profile',exact:true}).click();
 await page.getByLabel('Photo URL',{exact:false}).fill('');
 await page.getByRole('button',{name:'Remove link 1',exact:true}).click();
 await page.getByRole('button',{name:'Save profile',exact:true}).click();
 await expect(page.getByRole('dialog')).toHaveCount(0);
 expect(profile.official_links).toEqual([]);
 expect(profile.avatar_url).toBe('');
});

test('ranking errors recover to an honest empty state',async({page})=>{
 let fail=true;
 await page.route('**/api/community/**',async route=>{
  const path=new URL(route.request().url()).pathname;
  if(path.endsWith('/trending')){
   await route.fulfill(fail?{status:503,json:{error:'Temporarily unavailable'}}:{json:{items:[],total:0,page:1,pages:0,days:7,summary:{posts:0,likes:0,saves:0,artists:0}}});return;
  }
  await route.fulfill({json:path.endsWith('/me')?{id:'member',display_name:'Collector'}:path.endsWith('/config')?{discord_enabled:true}:path.endsWith('/servers')?[{id:'server',external_id:'1',name:'Atelier',connected:true}]:path.endsWith('/posts')?{items:[],total:0,page:1,pages:0}:{server:{id:'server',name:'Atelier'},can_manage:false}});
 });
 await page.goto('/community');
 await page.getByRole('button',{name:'Trending',exact:true}).click();
 await expect(page.getByRole('alert')).toContainText('Could not load rankings');
 fail=false;
 await page.getByRole('button',{name:'Try again',exact:true}).click();
 await expect(page.getByRole('heading',{name:'A new chart is waiting to happen.'})).toBeVisible();
 await expect(page.getByRole('button',{name:'Track artist',exact:true})).toHaveCount(0);
});
