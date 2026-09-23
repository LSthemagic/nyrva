import { absoluteTime, countdown } from './dashboard-format.mjs';
import { byId, textNode, renderOverview, renderSessions, renderProjects, renderSources, syncHistoryOptions, renderHistory } from './dashboard-render.mjs';

const api = window.__TAURI__;
const invoke = (command, args = {}) => {
  if (!api?.core?.invoke) return Promise.reject(new Error('Abra o observatório pelo menu do Nyrva desktop.'));
  return api.core.invoke(`plugin:observatory|${command}`, args);
};
const clone = value => JSON.parse(JSON.stringify(value));
let model = null, dirty = false, streamConnected = false, reading = false, historyGeneration = 0, optionsSignature = '', lastAgedRender = 0;
const unlisten = [];
const tabs = [...document.querySelectorAll('[role=tab]')];
function selectTab(button) {
  for (const tab of tabs) {
    const active = tab === button;
    tab.setAttribute('aria-selected', String(active)); tab.tabIndex = active ? 0 : -1;
    byId(tab.getAttribute('aria-controls')).hidden = !active;
  }
}
for (const tab of tabs) {
  tab.addEventListener('click', () => selectTab(tab));
  tab.addEventListener('keydown', event => {
    let index = tabs.indexOf(tab);
    if (event.key === 'ArrowRight') index = (index + 1) % tabs.length;
    else if (event.key === 'ArrowLeft') index = (index + tabs.length - 1) % tabs.length;
    else if (event.key === 'Home') index = 0;
    else if (event.key === 'End') index = tabs.length - 1;
    else return;
    event.preventDefault(); selectTab(tabs[index]); tabs[index].focus();
  });
}
function connection(message, error = false) {
  byId('connection').textContent = message;
  byId('connection').classList.toggle('error', error);
}
function feedback(message, error = false) {
  byId('settings-result').textContent = message;
  byId('settings-result').classList.toggle('error', error);
}
function errorText(error) { return String(error?.message ?? error).slice(0, 240); }
function markDirty() { dirty = true; byId('unsaved').textContent = 'Alterações ainda não salvas'; }
byId('preferences-form').addEventListener('input', markDirty);
byId('preferences-form').addEventListener('change', markDirty);
function fillPreferences(settings) {
  byId('layout').value = settings.layout;
  byId('retention').value = String(settings.retention_days);
  byId('reserve').value = String(settings.reserve_fraction * 100);
  byId('metadata').checked = settings.collect_session_metadata;
  for (const checkbox of document.querySelectorAll('.provider-check')) checkbox.checked = settings.providers[checkbox.value];
  byId('alerts-enabled').checked = settings.alerts.enabled;
  byId('thresholds').value = settings.alerts.thresholds.map(n => n * 100).join(', ');
  byId('aliases').value = JSON.stringify(settings.account_aliases ?? {}, null, 2);
  byId('unsaved').textContent = '';
}
function apply(next) {
  if (!next || next.schema_version !== 1 || !next.settings || !Array.isArray(next.quotas) || !Number.isSafeInteger(next.captured_at_ms)) throw new Error('Contrato de observação não suportado; a última leitura foi preservada.');
  if (model) {
    const revisioned = Number.isSafeInteger(next.revision) && Number.isSafeInteger(model.revision);
    if (revisioned ? next.revision <= model.revision : next.captured_at_ms < model.captured_at_ms) return;
  }
  const changedPreferences = model && JSON.stringify(model.settings) !== JSON.stringify(next.settings);
  model = next;
  if (changedPreferences) { historyGeneration++; byId('history-results').replaceChildren(textNode('p', 'Preferências alteradas. Consulte o histórico novamente.', 'hint')); }
  document.body.dataset.layout = ['compact', 'bars', 'reset-first'].includes(next.settings.layout) ? next.settings.layout : 'compact';
  renderOverview(next); renderSessions(next, byId('session-filter').value); renderProjects(next); renderSources(next);
  const signature = JSON.stringify([(next.quotas ?? []).map(s => [s.provider, s.account_id, s.source, (s.buckets ?? []).map(b => [b.id, b.label])]), next.settings.account_aliases]);
  if (signature !== optionsSignature) { syncHistoryOptions(next); optionsSignature = signature; }
  byId('preference-fields').disabled = false;
  if (!dirty) fillPreferences(next.settings);
  const until = next.settings.alerts.quiet_until_ms;
  byId('quiet-status').textContent = until > Date.now() ? `Alertas silenciados até ${absoluteTime(until)}.` : 'Alertas sem silêncio temporário. Eventos repetidos são deduplicados pelo núcleo.';
  connection(`Leitura local: ${absoluteTime(next.captured_at_ms)} · ${streamConnected ? 'atualização por eventos' : 'atualização manual'} · observações de cada fonte mantêm sua própria data.`);
}
async function refresh() {
  if (reading) return;
  reading = true;
  try { apply(await invoke('read_cockpit')); }
  catch (error) { connection(model ? `Não foi possível atualizar; mantendo a última leitura de ${absoluteTime(model.captured_at_ms)}. ${errorText(error)}` : errorText(error), true); }
  finally { reading = false; }
}
async function busy(button, operation) {
  button.disabled = true;
  try { await operation(); }
  catch (error) { feedback(errorText(error), true); }
  finally { button.disabled = false; }
}
byId('refresh').addEventListener('click', event => busy(event.currentTarget, refresh));
byId('session-filter').addEventListener('input', event => { if (model) renderSessions(model, event.target.value); });
byId('history-form').addEventListener('submit', event => {
  event.preventDefault();
  const button = event.currentTarget.querySelector('button');
  busy(button, async () => {
    const selected = byId('history-query').value;
    if (!selected) { byId('history-results').replaceChildren(textNode('p', 'Nenhuma quota disponível para consultar.', 'empty')); return; }
    const query = { ...JSON.parse(selected), hours: Number(byId('history-hours').value) };
    const generation = ++historyGeneration;
    byId('history-results').replaceChildren(textNode('p', 'Consultando o histórico local…', 'hint'));
    try {
      const result = await invoke('read_history', { query });
      if (generation === historyGeneration) renderHistory(result, query);
    } catch (error) { if (generation === historyGeneration) byId('history-results').replaceChildren(textNode('p', `Histórico indisponível: ${errorText(error)}`, 'feedback error')); }
  });
});
function formSettings() {
  if (!model) throw new Error('Aguarde a primeira leitura válida.');
  const settings = clone(model.settings);
  settings.layout = byId('layout').value;
  settings.retention_days = byId('retention').valueAsNumber;
  settings.reserve_fraction = byId('reserve').valueAsNumber / 100;
  settings.collect_session_metadata = byId('metadata').checked;
  for (const checkbox of document.querySelectorAll('.provider-check')) settings.providers[checkbox.value] = checkbox.checked;
  settings.alerts.enabled = byId('alerts-enabled').checked;
  const rawThresholds = byId('thresholds').value.split(',').map(s => s.trim());
  if (rawThresholds.some(s => !/^\d+(\.\d+)?$/.test(s))) throw new Error('Informe percentuais separados por vírgulas, como 20, 10.');
  settings.alerts.thresholds = rawThresholds.map(s => Number(s) / 100);
  try { settings.account_aliases = JSON.parse(byId('aliases').value); }
  catch { throw new Error('Os apelidos precisam ser um objeto JSON válido.'); }
  return settings;
}
byId('preferences-form').addEventListener('submit', event => {
  event.preventDefault();
  busy(event.currentTarget.querySelector('button[type=submit]'), async () => {
    const result = await invoke('save_settings', { settings: formSettings() });
    dirty = false; apply(result); feedback('Preferências salvas. Alterações aplicadas às visões e novas gravações do observatório.');
  });
});
async function silence(until) {
  if (!model) throw new Error('Aguarde a primeira leitura válida.');
  const settings = clone(model.settings); settings.alerts.quiet_until_ms = until;
  apply(await invoke('save_settings', { settings }));
  feedback(until ? 'Silêncio temporário salvo. Os demais campos ainda não salvos foram preservados no formulário.' : 'Silêncio temporário removido.');
}
byId('silence').addEventListener('click', event => busy(event.currentTarget, () => silence(Date.now() + 3600000)));
byId('resume-alerts').addEventListener('click', event => busy(event.currentTarget, () => silence(0)));
byId('export').addEventListener('click', event => busy(event.currentTarget, async () => {
  byId('portable-settings').value = JSON.stringify(await invoke('export_settings'), null, 2);
  feedback('Preferências exportadas para o campo JSON. Nenhum arquivo de provedor foi acessado.');
}));
byId('import').addEventListener('click', event => busy(event.currentTarget, async () => {
  const text = byId('portable-settings').value;
  if (new TextEncoder().encode(text).length > 32768) throw new Error('A importação excede o limite de 32 KiB.');
  const result = await invoke('import_settings', { text }); dirty = false; apply(result); feedback('Preferências importadas e aplicadas.');
}));
byId('rollback').addEventListener('click', event => busy(event.currentTarget, async () => {
  const result = await invoke('rollback_settings'); dirty = false; apply(result); feedback('Preferências anteriores restauradas.');
}));
const dialog = byId('clear-dialog');
byId('clear').addEventListener('click', () => { byId('clear-result').textContent = ''; dialog.showModal(); });
byId('clear-cancel').addEventListener('click', () => dialog.close());
byId('clear-confirm').addEventListener('click', async event => {
  const button = event.currentTarget; button.disabled = true;
  try {
    const result = await invoke('clear_history', { confirmation: 'CLEAR NYRVA' });
    historyGeneration++; byId('history-results').replaceChildren(); apply(result); dialog.close();
    feedback('Histórico e alertas locais limpos. Preferências e arquivos dos provedores foram preservados.');
  } catch (error) { byId('clear-result').textContent = errorText(error); }
  finally { button.disabled = false; }
});
dialog.addEventListener('close', () => byId('clear').focus());
const clock = window.setInterval(() => {
  if (document.hidden) return;
  for (const item of document.querySelectorAll('[data-reset-at]')) {
    const at = Number(item.dataset.resetAt); item.textContent = countdown(at);
    item.closest('.quota-card')?.classList.toggle('awaiting', at <= Date.now());
  }
  if (model && Date.now() - lastAgedRender >= 15000) { renderOverview(model); lastAgedRender = Date.now(); }
}, 1000);
window.addEventListener('beforeunload', () => { clearInterval(clock); for (const off of unlisten) off(); });
async function start() {
  if (api?.event?.listen) {
    try {
      unlisten.push(await api.event.listen('observatory', event => { try { apply(event.payload); } catch (error) { connection(errorText(error), true); } }));
      unlisten.push(await api.event.listen('observatory-error', () => connection('Atualização local temporariamente indisponível; mantendo a última leitura. Tente atualizar o painel.', true)));
      streamConnected = true;
    } catch { streamConnected = false; }
  }
  await refresh();
}
start();
