import {test,expect} from '@playwright/test';
test('administration contains operations only',async({page},info)=>{
 let settings={downloads_enabled:true,notifications_enabled:true};
 await page.route('**/api/**',async r=>{const path=new URL(r.request().url()).pathname;let json:unknown;
 if(path==='/api/session')json={ok:true};
 else if(path==='/api/admin/overview')json={servers:2,users:12,subscriptions:8,posts:51,comments:6,storage_bytes:10240,failed:1,pending:3,feed_errors:0};
 else if(path==='/api/admin/services')json={database:'connected',discord:'configured',rsshub:'configured',last_poll:'2026-10-08T12:00:00Z',max_download_mb:100};
 else if(path==='/api/admin/servers')json=[{id:'1',name:'Atelier',platform:'discord',enabled:true,channel_name:'art',artists:4,webhooks:1,created_at:'2026-10-08T12:00:00Z'}];
 else if(path==='/api/admin/queues')json=[{id:'job',kind:'webhook',server:'Atelier',status:'failed',attempts:8,last_error:'Destination unavailable',created_at:'2026-10-08T12:00:00Z'}];
 else if(path==='/api/admin/settings'){if(r.request().method()==='PUT')settings=r.request().postDataJSON();json=settings;}
 else {await r.fulfill({status:404,json:{error:'Retired endpoint'}});return;}
 await r.fulfill({json});});
 await page.goto('/');await expect(page.getByRole('heading',{name:'Installation overview',exact:true})).toBeVisible();
 const nav=page.getByRole('navigation',{name:'Administration'});
 for(const label of ['Library','Feeds','Favorites','Downloads','New posts'])await expect(nav.getByRole('button',{name:label,exact:true})).toHaveCount(0);
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBeTruthy();
 await page.screenshot({path:`../.test-runtime/admin-${info.project.name}.png`,fullPage:true});
 async function open(name:string){if(info.project.name==='mobile')await page.getByRole('button',{name:'Open navigation'}).click();await nav.getByRole('button',{name,exact:true}).click();}
 await open('Servers');await expect(page.getByRole('heading',{name:'Atelier'})).toBeVisible();
 await open('Queues 1');await expect(page.getByText('Destination unavailable',{exact:true})).toBeVisible();
 await open('Settings');await page.getByLabel('Allow notifications',{exact:false}).uncheck();await page.getByRole('button',{name:'Save settings'}).click();await expect(page.getByRole('status')).toHaveText('Settings saved.');expect(settings.notifications_enabled).toBe(false);
 await expect(page.getByRole('button',{name:'Add webhook'})).toHaveCount(0);
});
