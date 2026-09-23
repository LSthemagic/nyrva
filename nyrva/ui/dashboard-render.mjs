import { providerNames, statusNames, percent, number, quotaView, countdown, absoluteTime, accountLabel, contextLabel, costLabel, duration, historySeries } from './dashboard-format.mjs';

export const byId = id => document.getElementById(id);
export function textNode(tag, text, className = '') {
  const element = document.createElement(tag);
  if (text !== null && text !== undefined) element.textContent = String(text);
  if (className) element.className = className;
  return element;
}
const empty = message => textNode('p', message, 'empty');
const meta = message => textNode('p', message, 'meta');
const provider = id => providerNames[id] ?? String(id ?? 'Não informado');
function badge(state) {
  const item = textNode('span', statusNames[state] ?? 'Estado não informado', 'badge');
  item.dataset.state = state ?? 'unknown';
  return item;
}
function timer(at) {
  const item = textNode('span', countdown(at), 'reset-label');
  if (typeof at === 'number') item.dataset.resetAt = String(at);
  return item;
}
function table(headers, rows, caption) {
  const t = document.createElement('table');
  t.append(textNode('caption', caption));
  const head = document.createElement('thead'), tr = document.createElement('tr');
  for (const header of headers) { const th = textNode('th', header); th.scope = 'col'; tr.append(th); }
  head.append(tr); t.append(head);
  const body = document.createElement('tbody');
  for (const row of rows) { const r = document.createElement('tr'); for (const value of row) { const cell = document.createElement('td'); if (value instanceof Node) cell.append(value); else cell.textContent = String(value ?? 'Não informado'); r.append(cell); } body.append(r); }
  t.append(body); return t;
}
export function renderOverview(data) {
  const aliases = data.settings.account_aliases ?? {};
  const pulse = (data.pulse?.providers ?? []).map(p => {
    const card = textNode('article', null, 'pulse-card'); card.dataset.state = p.state;
    card.append(textNode('h3', provider(p.provider)), textNode('p', statusNames[p.state] ?? 'Sem leitura recente'), meta('Estado do provedor; quotas não são somadas.')); return card;
  });
  byId('pulse').replaceChildren(...pulse);
  const quotas = [];
  for (const snapshot of data.quotas ?? []) for (const bucket of snapshot.buckets ?? []) {
    const view = quotaView(snapshot, bucket);
    const card = textNode('article', null, `quota-card${view.awaiting ? ' awaiting' : ''}`);
    const top = textNode('div', null, 'quota-top'); top.append(textNode('h3', `${provider(snapshot.provider)} · ${bucket.label}`), badge(view.status));
    const quantity = textNode('p', view.value, 'quota-value');
    card.append(top, meta(`${accountLabel(snapshot.account_id, aliases)} · ${snapshot.source} · ${bucket.estimated ? 'Estimativa informada' : 'Medição informada'}`), quantity, textNode('p', view.fraction === null ? 'Sem denominador de quota disponível' : 'da quota restante na última observação', 'quota-caption'));
    if (view.fraction !== null) { const meter = document.createElement('meter'); meter.min = 0; meter.max = 1; meter.value = view.fraction; meter.setAttribute('aria-label', `${bucket.label}: ${view.value} restante na última observação`); card.append(meter); }
    const reset = textNode('div', null, 'reset-info'); reset.append(timer(bucket.resets_at_ms), meta(view.absolute));
    card.append(reset, meta(`Observado: ${absoluteTime(snapshot.observed_at_ms)} · há ${view.age}`)); quotas.push(card);
  }
  byId('quota-cards').replaceChildren(...(quotas.length ? quotas : [empty('Nenhuma quota disponível para as fontes incluídas. Dados ausentes não são zero.')]));
  const resets = (data.resets ?? []).map(r => { const item = textNode('article', null, 'reset-item'); item.append(textNode('h3', `${provider(r.provider)} · ${r.label}`), timer(r.resets_at_ms), meta(absoluteTime(r.resets_at_ms)), meta(`${accountLabel(r.account_id, aliases)} · ${r.source}`)); return item; });
  byId('reset-list').replaceChildren(...(resets.length ? resets : [empty('Nenhum reset informado.')]));
  const alerts = (data.alerts ?? []).slice(0, 30).map(a => { const item = textNode('article', null, 'alert-item'); item.append(textNode('h3', a.kind === 'quota_reset' ? 'Reset confirmado pela fonte' : 'Limite de quota atingido'), textNode('p', `${provider(a.provider)} · ${a.bucket_label ?? a.bucket_id} · ${percent(a.remaining_fraction)} restante`), meta(`${accountLabel(a.account_id, aliases)} · ${a.source} · ${absoluteTime(a.created_at_ms)}`)); return item; });
  if (data.alerts_error) alerts.unshift(meta('Alertas temporariamente indisponíveis. As quotas continuam visíveis.'));
  byId('alerts-list').replaceChildren(...(alerts.length ? alerts : [empty('Nenhum alerta registrado.')]));
}
export function renderSessions(data, filter = '') {
  const query = filter.trim().toLocaleLowerCase('pt-BR');
  const rows = (data.sessions ?? []).filter(s => !query || [s.provider, s.account_id, s.model, s.session?.id, s.session?.project, s.session?.branch].some(v => String(v ?? '').toLocaleLowerCase('pt-BR').includes(query)));
  byId('session-summary').textContent = `${rows.length} sessão(ões) nesta visão${data.sessions_truncated ? ' · visão limitada às 500 mais recentes' : ''}. Custos são cumulativos por sessão; leituras repetidas não são somadas.`;
  const values = rows.map(s => [provider(s.provider), accountLabel(s.account_id, data.settings.account_aliases), `${s.session?.project ?? 'Não informado'} / ${s.session?.branch ?? 'ramo não informado'}`, s.model, `${s.session?.state ?? 'Estado não informado'} · ${statusNames[s.session_status] ?? s.session_status}`, contextLabel(s.context), costLabel(s.metrics?.estimated_cost_usd), duration(s.metrics?.duration_ms), s.session?.id]);
  byId('sessions-table').replaceChildren(values.length ? table(['Provedor', 'Conta', 'Projeto / ramo', 'Modelo', 'Estado / recência', 'Contexto', 'Estimativa USD', 'Duração informada', 'Identificador'], values, 'Sessões explicitamente informadas pelas fontes; não equivale a processos ativos verificados.') : empty('Nenhum metadado de sessão disponível para este filtro.'));
}
export function renderProjects(data) {
  const cards = (data.projects ?? []).map(p => { const card = textNode('article', null, 'project-card'); const coverage = { unavailable: 'Sem custo informado', partial: 'Cobertura parcial', complete_for_observed_sessions: 'Completo somente para sessões observadas' }[p.cost_coverage] ?? 'Cobertura não informada'; card.append(textNode('h3', p.project), textNode('p', `${number(p.session_count)} sessão(ões)`), textNode('p', `Estimativa: ${costLabel(p.estimated_cost_usd)}`), meta(`${coverage}. Cobrança não disponível. Quota por projeto não atribuída.`)); return card; });
  if (data.projects_history_capped) cards.unshift(meta('O agrupamento está limitado às sessões disponíveis no histórico local.'));
  byId('projects-list').replaceChildren(...(cards.length ? cards : [empty('Nenhum projeto informado.')]));
  const relation = { reported_and_resolved: 'Pai relatado e resolvido', unresolved: 'Pai relatado, não resolvido', invalid_cycle: 'Relação cíclica inválida; vínculo omitido', not_reported: 'Pai não informado' };
  const nodes = (data.agents?.nodes ?? []).map(n => [n.id, `${provider(n.provider)} / ${n.source}`, n.model, n.effort, n.parent_id ?? n.reported_parent_id, relation[n.relationship] ?? 'Não informado', statusNames[n.freshness] ?? n.freshness]);
  byId('agents-list').replaceChildren(nodes.length ? table(['Agente / sessão', 'Origem', 'Modelo', 'Esforço', 'Pai relatado', 'Evidência do vínculo', 'Recência'], nodes, 'Relações explícitas; não há árvore inferida nem contagem de produtividade.') : empty('A fonte ainda não informou relações entre agentes.'));
}
export function renderSources(data) {
  const cards = (data.providers ?? []).map(p => {
    const card = textNode('article', null, 'surface'); card.append(textNode('h3', provider(p.id)), badge(p.status));
    card.append(meta(`Quota: ${p.capabilities?.quota ? 'informada' : 'não informada'} · Sessões: ${p.capabilities?.sessions ? 'informadas' : 'não informadas'} · Cobrança: indisponível`));
    const list = textNode('ul', null, 'source-list');
    for (const s of p.sources ?? []) { const li = document.createElement('li'); li.append(textNode('p', `${s.source} · ${accountLabel(s.account_id, data.settings.account_aliases)}`), badge(s.status), meta(`Observado: ${absoluteTime(s.observed_at_ms)} · ${number(s.bucket_count)} quota(s)`)); if (s.source_version) li.append(meta(`Versão informada: ${s.source_version}`)); list.append(li); }
    card.append(list.childNodes.length ? list : meta(p.enabled ? 'Nenhuma leitura disponível. Verifique a integração do provedor sem compartilhar credenciais.' : 'Excluído das visões e novas gravações do observatório.')); return card;
  }); byId('sources-list').replaceChildren(...cards);
}
export function syncHistoryOptions(data) {
  const select = byId('history-query'), previous = select.value;
  const options = [], seen = new Set();
  for (const s of data.quotas ?? []) for (const b of s.buckets ?? []) {
    const value = JSON.stringify({ provider: s.provider, account_id: s.account_id, source: s.source, bucket_id: b.id });
    if (seen.has(value)) continue; seen.add(value);
    const option = textNode('option', `${provider(s.provider)} / ${accountLabel(s.account_id, data.settings.account_aliases)} / ${s.source} / ${b.label}`); option.value = value; options.push(option);
  }
  if (!options.length) { const option = textNode('option', 'Nenhuma quota disponível'); option.value = ''; options.push(option); }
  select.replaceChildren(...options); if (seen.has(previous)) select.value = previous;
}
function plot(points) {
  const ns = 'http://www.w3.org/2000/svg', svg = document.createElementNS(ns, 'svg'); svg.setAttribute('viewBox', '0 0 680 150'); svg.setAttribute('role', 'img'); svg.setAttribute('aria-label', 'Observações independentes de quota restante. Os mesmos dados estão na tabela abaixo.'); svg.classList.add('history-chart');
  for (const value of [0, 0.5, 1]) { const line = document.createElementNS(ns, 'line'); const y = 125 - value * 100; for (const [k, v] of Object.entries({ x1: 50, x2: 665, y1: y, y2: y })) line.setAttribute(k, String(v)); svg.append(line); const text = document.createElementNS(ns, 'text'); text.setAttribute('x', '0'); text.setAttribute('y', String(y + 4)); text.textContent = percent(value); svg.append(text); }
  const first = points[0]?.observed_at_ms ?? 0, span = Math.max(1, (points.at(-1)?.observed_at_ms ?? first) - first);
  for (const p of points) if (p.remaining_fraction !== null) { const dot = document.createElementNS(ns, 'circle'); dot.setAttribute('cx', String(55 + ((p.observed_at_ms - first) / span) * 600)); dot.setAttribute('cy', String(125 - p.remaining_fraction * 100)); dot.setAttribute('r', '3'); svg.append(dot); }
  return svg;
}
export function renderHistory(result, query) {
  const output = [], groups = historySeries(result.samples ?? [], query);
  if (result.truncated) output.push(meta('Resultado limitado às 500 observações mais recentes consultadas. Reduza o período; fontes menos frequentes podem ter cobertura parcial.'));
  const forecast = result.forecast;
  if (forecast) output.push(textNode('p', `Estimativa conservadora: ${number(forecast.fraction_per_hour * 100)} pontos percentuais por hora. Orçamento até o reset, preservando a reserva: ${number(forecast.budget_per_hour * 100)} pontos por hora. Esgotamento estimado: ${absoluteTime(forecast.estimated_exhaustion_at_ms)}. Base: ${forecast.sample_count} leituras em ${duration(forecast.sample_span_ms)}.`, 'surface'));
  else output.push(meta('Sem estimativa segura: são necessárias leituras recentes, suficientes e compatíveis na mesma janela.'));
  for (const group of groups) { const section = textNode('section', null, 'history-window'); section.append(textNode('h3', group.resets_at_ms === null ? 'Janela sem reset informado' : `Janela com reset em ${absoluteTime(group.resets_at_ms)}`), plot(group.points)); const wrap = textNode('div', null, 'table-wrap'); wrap.append(table(['Observado em', 'Quota restante', 'Contagem sem denominador', 'Estado na fonte'], group.points.map(p => [absoluteTime(p.observed_at_ms), percent(p.remaining_fraction), p.count === null ? '—' : number(p.count), statusNames[p.status] ?? p.status]), 'Cada ponto é uma observação. Lacunas e resets não são interpolados.')); section.append(wrap); output.push(section); }
  if (!groups.length) output.push(empty('Nenhuma observação no período para esta identidade.'));
  byId('history-results').replaceChildren(...output);
}
