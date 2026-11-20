import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import http from 'node:http';
import {randomUUID} from 'node:crypto';
import {setTimeout as wait} from 'node:timers/promises';
const base=process.env.TEST_BASE_URL||'http://127.0.0.1:8080',db=process.env.DATABASE_URL,token=process.env.API_TOKEN;
assert(db&&new URL(db).pathname.endsWith('_test'));assert(token);
const sql=q=>execFileSync(process.env.PSQL_PATH||'psql',[db,'-X','-tA','-v','ON_ERROR_STOP=1','-c',q],{encoding:'utf8'}).trim();
async function request(path,method='GET',body,status=200){const r=await fetch(base+path,{method,headers:{Authorization:`Bearer ${token}`,'Content-Type':'application/json'},body:body===undefined?undefined:JSON.stringify(body)});assert.equal(r.status,status,`${path}: ${r.status}`);return status===404?null:r.json();}
for (const path of ['/community', '/community/']) assert.equal((await fetch(base+path)).status,200,`Community page ${path} must return 200`);
assert.equal((await fetch(base+'/api/admin/overview')).status,401);
assert.equal((await fetch(base+'/api/admin/settings',{headers:{cookie:'la_user=not-an-admin'}})).status,401);
await request('/api/login','POST',{token:'wrong'},401);
for(const path of ['/api/feeds','/api/entries','/api/downloads','/api/webhooks','/api/stats','/api/settings','/api/jobs','/api/feeds/export'])await request(path,'GET',undefined,404);
const login=await fetch(base+'/api/login',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({token})});assert.equal(login.status,200);assert.match(login.headers.get('set-cookie'),/HttpOnly; SameSite=Strict/);
const previous=await request('/api/admin/settings');
let version=1;
const fixture=http.createServer((_,res)=>{res.writeHead(200,{'Content-Type':'application/rss+xml'});res.end(`<rss version="2.0"><channel><title>Integration artist</title><link>https://example.com</link><description>Test</description>${Array.from({length:version},(_,n)=>`<item><guid>${n}</guid><title>Art ${n}</title><link>https://example.com/${n}</link></item>`).join('')}</channel></rss>`);});
await new Promise(resolve=>fixture.listen(55888,'127.0.0.1',resolve));
const server=randomUUID(),feed=randomUUID(),artist=randomUUID(),webhook=randomUUID(),orphan=randomUUID();
async function until(fn){for(let n=0;n<80;n++){if(fn())return;await wait(250);}throw Error('Worker did not progress');}
try{
 await request('/api/admin/settings','PUT',{notifications_enabled:false,downloads_enabled:false});
 sql(`INSERT INTO communities(id,platform,external_id,name) VALUES('${server}','discord','integration-${server}','Integration server'); INSERT INTO feeds(id,name,url) VALUES('${feed}','Artist','http://127.0.0.1:55888/rss'),('${orphan}','Unassigned','http://127.0.0.1:55888/unassigned'); INSERT INTO community_artists(id,community_id,feed_id,name) VALUES('${artist}','${server}','${feed}','Artist'); INSERT INTO webhooks(id,community_id,name,url) VALUES('${webhook}','${server}','Test only','https://example.com/never-send');`);
 await until(()=>sql(`SELECT initialized FROM feeds WHERE id='${feed}'`)==='t');
 assert.equal(sql(`SELECT count(*) FROM jobs WHERE webhook_id='${webhook}'`),'0');
 assert.equal(sql(`SELECT initialized FROM feeds WHERE id='${orphan}'`),'f','Unassigned feeds must not be polled');
 version=2;sql(`UPDATE feeds SET next_check_at=now() WHERE id='${feed}'`);
 await until(()=>sql(`SELECT count(*) FROM entries WHERE feed_id='${feed}'`)==='2');
 assert.equal(sql(`SELECT count(*) FROM jobs WHERE webhook_id='${webhook}' AND status='pending'`),'1');
 assert.equal(sql(`SELECT count(*) FROM jobs WHERE webhook_id='${webhook}' AND attempts>0`),'0','Master pause must prevent sends');
 const overview=await request('/api/admin/overview');assert(overview.servers>=1&&overview.subscriptions>=1);
 const queues=await request('/api/admin/queues');assert(queues.some(j=>j.server==='Integration server'));assert(queues.every(j=>!('title' in j)&&!('url' in j)));
 assert((await request('/api/admin/servers')).some(s=>s.name==='Integration server'));
 assert.equal((await request('/api/admin/services')).database,'connected');
 console.log('PASS: admin auth, retired APIs, operational statistics, scoped ingestion, silent history, webhook queue and orphan feed isolation; no external messages sent');
}finally{sql(`DELETE FROM communities WHERE id='${server}'; DELETE FROM feeds WHERE id IN('${feed}','${orphan}')`);await request('/api/admin/settings','PUT',previous);await new Promise(resolve=>fixture.close(resolve));}
