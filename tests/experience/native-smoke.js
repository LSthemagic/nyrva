// Evaluated only by the isolated native example; never loaded by the product.
(async () => {
  const checks = [];
  const assert = (condition, message) => { if (!condition) throw new Error(message); };
  const until = async predicate => {
    const deadline = Date.now() + 15000;
    while (!predicate()) { if (Date.now() >= deadline) throw new Error(`condition timeout: ${predicate}`); await new Promise(resolve => setTimeout(resolve, 50)); }
  };
  const ipc = (name, args = {}) => window.__TAURI__.core.invoke(`plugin:observatory|${name}`, args);
  try {
    await until(() => document.querySelector('#quota-cards')?.textContent.includes('0%'));
    assert(!document.querySelector('#quota-cards img'), 'untrusted label became an image');
    assert(document.querySelector('#quota-cards').textContent.includes('<img src=x onerror=alert(1)>'), 'source label missing');
    checks.push('native modules load; zero and untrusted metadata render safely');
    const first = await ipc('read_cockpit');
    assert(first.schema_version === 1 && first.quotas[0].buckets[0].remaining_fraction === 0, 'wrong native cockpit contract');
    assert(Number.isSafeInteger(first.revision), 'missing ordered native revision');
    checks.push('real scoped read and ordered revisions');
    let eventCount = 0;
    const off = await window.__TAURI__.event.listen('observatory', () => eventCount++);
    const settings = await ipc('export_settings');
    settings.layout = 'bars';
    const changed = await ipc('save_settings', {settings});
    assert(changed.revision > first.revision && changed.settings.layout === 'bars', 'native save failed');
    await until(() => document.body.dataset.layout === 'bars' && eventCount > 0);
    off();
    checks.push('real preference persistence and live rendering');
    const lifecycle = await window.__TAURI__.core.invoke('experience_smoke_lifecycle');
    assert(lifecycle.close_hides && lifecycle.reopen_visible && lifecycle.event_callback_returned && lifecycle.window_count === 1 && lifecycle.rapid_requests === 32, 'window lifecycle failed');
    assert(document.body.dataset.layout === 'bars', 'reopening lost the current layout');
    checks.push('close hides; main-thread reopening returns; 32 rapid opens reuse one window and preserve layout');
    let rejected = false;
    try { await ipc('import_settings', {text:'{"secret":"MUST_NOT_PERSIST"}'}); } catch { rejected = true; }
    assert(rejected && (await ipc('export_settings')).layout === 'bars', 'invalid import replaced valid settings');
    checks.push('invalid import preserves preferences');
    const query = {provider:'antigravity',account_id:'active',source:'statusline',bucket_id:'weekly',hours:24};
    const history = await ipc('read_history', {query});
    assert(history.samples.length === 1 && history.samples[0].buckets[0].remaining_fraction === 0, 'native history lost zero');
    assert((await ipc('read_history', {query:{...query,account_id:'work'}})).samples.length === 0, 'history mixed accounts');
    checks.push('SQLite history is account/source scoped');
    rejected = false;
    try { await ipc('clear_history', {confirmation:'yes'}); } catch { rejected = true; }
    assert(rejected && (await ipc('read_history', {query})).samples.length === 1, 'unconfirmed clear changed history');
    document.querySelector('#tab-settings').click();
    document.querySelector('#clear').click();
    assert(document.querySelector('#clear-dialog').open, 'privacy dialog did not open');
    document.querySelector('#clear-cancel').click();
    assert(!document.querySelector('#clear-dialog').open, 'privacy dialog did not cancel');
    checks.push('confirmation boundary and native dialog cancellation');
    document.querySelector('#clear').click();
    document.querySelector('#clear-confirm').click();
    await until(() => !document.querySelector('#clear-dialog').open);
    assert((await ipc('read_history', {query})).samples.length === 0, 'confirmed UI clear did not reach SQLite');
    assert((await ipc('export_settings')).layout === 'bars', 'clear erased preferences');
    checks.push('confirmed UI clear reaches SQLite and preserves preferences');
    await window.__TAURI__.core.invoke('experience_smoke_finish', {report:{ok:true,checks,engine:navigator.userAgent}});
  } catch(error) {
    await window.__TAURI__.core.invoke('experience_smoke_finish', {report:{ok:false,checks,error:String(error),connection:document.querySelector('#connection')?.textContent,engine:navigator.userAgent}});
  }
})();
