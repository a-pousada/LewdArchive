#!/usr/bin/env node
// Dry-run by default. --apply upserts only /archive, leaving other commands intact.
const option = (name, description, type = 3, required = true, extra = {}) => ({name,description,type,required,...extra});
const sub = (name, description, options = []) => ({name,description,type:1,options});
const page = option('page','Page number',4,false,{min_value:1,max_value:10000});
const enabled = option('enabled','Enable or disable this preference (default: enabled)',5,false);
const post = option('post','Post ID from /archive latest or /archive favorites');
const artist = option('artist','Artist ID from /archive artists');
const command = {
  name:'archive', description:'Your server art library and private favorites', type:1,
  contexts:[0], integration_types:[0],
  options:[
    sub('setup','Choose the announcement channel (Administrator required)',[option('channel','Channel for new artwork',7,true,{channel_types:[0,5]})]),
    sub('track','Track an artist feed in this server (Administrator required)',[option('name','Artist name',3,true,{max_length:120}),option('url','RSS or RSSHub feed URL',3,true,{max_length:4096})]),
    sub('untrack','Remove an artist from this server (Administrator required)',[artist]),
    sub('trending','Rank artwork by likes, saves or total engagement',[option('period','Publication window',3,false,{choices:[{name:'Last 7 days',value:'week'},{name:'Last 30 days',value:'month'},{name:'Last 365 days',value:'year'}]}),option('metric','Rank by',3,false,{choices:[{name:'Most liked',value:'likes'},{name:'Most saved',value:'saves'},{name:'Likes + saves',value:'engagement'}]}),page]),
    sub('latest','Browse recent posts and their public like counts',[page]),
    sub('artists','Browse tracked artists',[page]),
    sub('favorites','Browse your private favorite posts or artists',[option('kind','Collection to view',3,true,{choices:[{name:'Posts',value:'posts'},{name:'Artists',value:'artists'}]}),page]),
    sub('like','Like or unlike a post in this server',[post,enabled]),
    sub('favorite-post','Save or unsave a post privately',[post,enabled]),
    sub('favorite-artist','Favorite or unfavorite an artist privately',[artist,enabled]),
    sub('notify','Opt into or out of mentions for an artist',[artist,enabled]),
    sub('help','Show available commands'),
  ],
};
if(process.argv.includes('--nsfw')) command.nsfw=true;
const guild=process.env.DISCORD_TEST_GUILD_ID;
if(guild){if(!/^\d{1,20}$/.test(guild))throw Error('Invalid DISCORD_TEST_GUILD_ID');delete command.contexts;delete command.integration_types;}
if(!process.argv.includes('--apply')) {
  console.log(JSON.stringify(command,null,2));
} else {
  const app=process.env.DISCORD_APPLICATION_ID, token=process.env.DISCORD_BOT_TOKEN;
  if(!/^\d{1,20}$/.test(app||'')||!token)throw Error('Set DISCORD_APPLICATION_ID and DISCORD_BOT_TOKEN');
  const response=await fetch(`https://discord.com/api/v10/applications/${app}${guild?`/guilds/${guild}`:''}/commands`,{method:'POST',headers:{Authorization:`Bot ${token}`,'Content-Type':'application/json'},body:JSON.stringify(command),signal:AbortSignal.timeout(15000)});
  if(!response.ok)throw Error(`Discord command registration failed (HTTP ${response.status})`);
  console.log(`Registered /archive ${guild?'in the test server':'globally'}.`);
}
