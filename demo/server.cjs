const http = require('http');
const fs = require('fs');
const path = require('path');
const {spawn}=require('child_process');
const readline=require('readline');
const bridge=spawn(path.join(__dirname,'protection-demo.exe'),[],{stdio:['pipe','pipe','pipe'],windowsHide:true});
const pending=[];let bridgeFailure;
function failBridge(err){bridgeFailure=err;for(const p of pending.splice(0)){clearTimeout(p.timer);p.reject(err);}}
bridge.on('error',failBridge);bridge.on('exit',()=>failBridge(Error('Rust 控制模型已停止；请重启 Demo')));
bridge.stderr.on('data',b=>process.stderr.write(b));
readline.createInterface({input:bridge.stdout}).on('line',line=>{const p=pending.shift();if(!p)return;clearTimeout(p.timer);try{p.resolve(JSON.parse(line));}catch(e){p.reject(e);}});
function rust(command){return new Promise((resolve,reject)=>{if(bridgeFailure)return reject(bridgeFailure);const p={resolve,reject};p.timer=setTimeout(()=>{failBridge(Error('Rust 响应超时'));bridge.kill();},5000);pending.push(p);bridge.stdin.write(command+'\n',e=>{if(e){failBridge(e);bridge.kill();}});});}
const devices = [{id:'data-01',name:'研发数据盘',size:'256 MiB',path:'/demo/vdb',type:'加密数据盘'},{id:'root-01',name:'可信系统镜像',size:'256 MiB',path:'/demo/vdc',type:'只读可信镜像'}];
const tasks=[];
let nextTaskId=0;
function check(p){
 if(!devices.some(d=>d.id===p.device)) throw Error('请选择有效演示设备');
 if(!['crypt','verity','both'].includes(p.goal)) throw Error('无效保护目标');
 const checks=[{name:'机密性保护',ok:p.goal==='verity'||p.crypt===true,detail:p.goal==='verity'?'本策略不要求加密':p.crypt?'已配置加密层':'缺少加密层'}, {name:'完整性保护',ok:p.goal==='crypt'||p.verity===true,detail:p.goal==='crypt'?'本策略不要求完整性':p.verity?'已配置校验层':'缺少完整性校验层'}, {name:'可信根配置',ok:!p.verity||p.rootTrusted===true,detail:p.verity?(p.rootTrusted?'已声明演示可信根':'未声明可信根来源'):'未使用校验层'}, {name:'挂载路径一致性',ok:p.mount==='mapper',detail:p.mount==='mapper'?'指向演示受保护映射':'裸设备路径绕过保护层'}];
 return {ok:checks.every(c=>c.ok),checks};
}
const server=http.createServer(async(req,res)=>{
 res.setHeader('Cache-Control','no-store');
 const send=(code,obj)=>{res.writeHead(code,{'Content-Type':'application/json; charset=utf-8'});res.end(JSON.stringify(obj));};
 if(req.method==='GET'&&req.url==='/api/state')return send(200,{mode:'simulation',engine:'Rust control-plane model',devices,tasks,engineAvailable:!bridgeFailure});
 if(req.method==='POST'&&['/api/check','/api/tasks','/api/prepare','/api/activate','/api/change','/api/recreate','/api/close'].includes(req.url)){
  let raw='';try{for await(const c of req){raw+=c;if(raw.length>10000)throw Error('请求过大');} const p=JSON.parse(raw);
   if(req.url==='/api/change'||req.url==='/api/recreate'){
    if(!devices.some(d=>d.id===p.device))return send(400,{error:'无效设备'});
    const r=await rust(`${req.url==='/api/change'?'change':'recreate'} ${p.device}`);return send(200,r);
   }
   if(req.url==='/api/activate'||req.url==='/api/close'){
    const t=tasks.find(t=>t.id===p.id);if(!t)return send(404,{error:'任务不存在'});
    if(!['已核验，待激活','演示映射已创建'].includes(t.status))return send(409,{error:'任务不处于可操作状态'});
    const r=await rust(`${req.url==='/api/close'?'close':'activate'} ${t.id}`);
    if(r.ok){t.status=req.url==='/api/close'?'已关闭':'演示映射已创建';t.events.push(req.url==='/api/close'?'Rust释放本任务依赖引用':'Rust同锁复核身份版本并固定依赖');}
    else if(req.url==='/api/activate'&&t.status==='已核验，待激活'){t.status='核验失效';t.events.push('Rust拒绝激活：'+r.error);}
    return send(200,{...t,engineResult:r});
   }
   const result=check(p);if(req.url==='/api/check')return send(200,result);
   const t={id:'DEMO-'+String(++nextTaskId).padStart(3,'0'),created:new Date().toISOString(),device:p.device,goal:p.goal,status:'已拒绝',simulation:true,checks:result.checks,events:['后端接收配置','完成策略与路径规则检查'],config:p};
   if(result.ok){
    const r=await rust(`prepare ${t.id} ${p.device} ${p.crypt?1:0} ${p.verity?1:0} ${p.rootTrusted?1:0} ${p.goal!=='verity'?1:0} ${p.goal!=='crypt'?1:0} ${p.verity?1:0}`);
    t.engineResult=r;if(r.ok){t.status='已核验，待激活';t.events.push('Rust核验并绑定设备身份与依赖配置版本');if(req.url==='/api/tasks'){const activated=await rust(`activate ${t.id}`);t.engineResult=activated;t.status=activated.ok?'演示映射已创建':'核验失效';t.events.push(activated.ok?'Rust固定依赖引用（逻辑租约）':'拒绝激活：'+activated.error);}}else t.events.push('Rust拒绝核验：'+r.error);
   }else t.events.push('配置拒绝，未建立Rust租约');tasks.unshift(t);return send(201,t);
  }catch(e){return send(400,{error:e.message});}
 }
 if(req.method==='GET'&&req.url==='/'){res.writeHead(200,{'Content-Type':'text/html; charset=utf-8'});return res.end(fs.readFileSync(path.join(__dirname,'index.html')));}
 send(404,{error:'Not found'});
});
server.listen(3138,'127.0.0.1',()=>console.log('Demo http://127.0.0.1:3138 — Rust control model, simulated devices'));
process.on('exit',()=>bridge.kill());
