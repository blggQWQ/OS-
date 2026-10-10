const {chromium}=require('C:/Users/潘丽园/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright');
const fs=require('fs');
(async()=>{const browser=await chromium.launch({headless:true,executablePath:'C:/Users/潘丽园/AppData/Local/ms-playwright/chromium-1200/chrome-win64/chrome.exe'});try{
 const page=await browser.newPage({viewport:{width:1440,height:1100},deviceScaleFactor:1.5});const errors=[];page.on('pageerror',e=>errors.push(e.message));
 await page.goto('http://127.0.0.1:3138');await page.locator('#create').click();await page.locator('#summary').filter({hasText:'通过'}).waitFor();await page.locator('#tasks').getByText('DEMO-001').waitFor();
 await page.screenshot({path:'demo/screenshot-pass.png',fullPage:true});
 await page.locator('#verity').uncheck();await page.locator('#create').click();await page.locator('#summary').filter({hasText:'拒绝'}).waitFor();await page.locator('#tasks').getByText('DEMO-002').waitFor();
 await page.locator('#verity').check();await page.locator('#mount').selectOption('raw');await page.locator('#create').click();await page.locator('#tasks').getByText('DEMO-003').waitFor();
 await page.screenshot({path:'demo/screenshot-reject.png',fullPage:true});
 await page.locator('#tasks button').last().click();await page.locator('#tasks').getByText('已关闭',{exact:true}).waitFor();
 const downloadPromise=page.waitForEvent('download');await page.locator('#export').click();const download=await downloadPromise;await download.saveAs('demo/verification-records.json');
 const state=JSON.parse(fs.readFileSync('demo/verification-records.json','utf8'));if(state.tasks.length!==3||state.tasks.filter(t=>t.status==='已拒绝').length!==2||state.tasks.filter(t=>t.status==='已关闭').length!==1||errors.length)throw Error('verification failed '+JSON.stringify(errors));
 console.log('PASS: valid task, missing integrity rejection, raw path rejection, close and JSON export; no browser errors');
}finally{await browser.close();}})().catch(e=>{console.error(e);process.exitCode=1;});
