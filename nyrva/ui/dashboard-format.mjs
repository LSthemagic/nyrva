// Presentation only. Identities and measurements remain those supplied by nyrva-core.
export const providerNames = Object.freeze({ claude: 'Claude', codex: 'Codex', cursor: 'Cursor', antigravity: 'Antigravity' });
export const statusNames = Object.freeze({ live: 'Recente', stale: 'Desatualizado', unavailable: 'Sem dados', unsupported: 'Não suportado', needs_auth: 'Autenticação necessária', backoff: 'Em espera', error: 'Falha na fonte', disabled: 'Desativado', recent: 'Recente', ready: 'Disponível', pressure: 'Atenção', critical: 'Reserva atingida', unknown: 'Sem leitura recente' });
const numeric = value => typeof value === 'number' && Number.isFinite(value);
export const fraction = value => numeric(value) && value >= 0 && value <= 1 ? value : null;
export const number = value => numeric(value) && value >= 0 ? new Intl.NumberFormat('pt-BR', { maximumFractionDigits: 2 }).format(value) : 'Indisponível';
export function percent(value) {
  const valid = fraction(value);
  return valid === null ? 'Indisponível' : `${new Intl.NumberFormat('pt-BR', { maximumFractionDigits: 1 }).format(valid * 100)}%`;
}
export function absoluteTime(value) {
  if (!numeric(value) || value <= 0 || value > 253402300799999) return 'Não informado';
  return new Intl.DateTimeFormat('pt-BR', { day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit', second: '2-digit', timeZoneName: 'short' }).format(new Date(value));
}
export function duration(value) {
  if (!numeric(value) || value < 0) return 'Indisponível';
  const seconds = Math.ceil(value / 1000), minutes = Math.floor(seconds / 60), hours = Math.floor(minutes / 60), days = Math.floor(hours / 24);
  if (days) return `${days}d ${hours % 24}h`;
  if (hours) return `${hours}h ${minutes % 60}min`;
  if (minutes) return `${minutes}min ${seconds % 60}s`;
  return `${seconds}s`;
}
export function countdown(resetAt, now = Date.now()) {
  if (!numeric(resetAt) || resetAt <= 0) return 'Reset não informado';
  return resetAt <= now ? 'Aguardando confirmação' : duration(resetAt - now);
}
export function accountLabel(id, aliases = {}) {
  return Object.hasOwn(aliases, id) && typeof aliases[id] === 'string' ? aliases[id] : String(id ?? 'Não informado');
}
export function quotaView(snapshot, bucket, now = Date.now()) {
  const value = fraction(bucket.remaining_fraction);
  const observed = snapshot.observed_at_ms;
  const recent = numeric(observed) && observed > 0 && observed <= now + 30000 && now - observed <= 600000;
  return {
    fraction: value,
    value: value !== null ? percent(value) : numeric(bucket.count) && bucket.count >= 0 ? `${number(bucket.count)} eventos` : 'Indisponível',
    status: snapshot.status === 'live' && !recent ? 'stale' : snapshot.status,
    awaiting: numeric(bucket.resets_at_ms) && bucket.resets_at_ms > 0 && bucket.resets_at_ms <= now,
    reset: countdown(bucket.resets_at_ms, now),
    absolute: absoluteTime(bucket.resets_at_ms),
    age: numeric(observed) && observed > 0 ? duration(Math.max(0, now - observed)) : 'Não informado',
  };
}
export function contextLabel(context) {
  if (!context) return 'Não informado pela fonte';
  const usage = numeric(context.used_percentage) && context.used_percentage >= 0 && context.used_percentage <= 100 ? percent(context.used_percentage / 100) : 'Uso não informado';
  return `${usage}${numeric(context.size) && context.size > 0 ? ` · janela de ${number(context.size)} tokens` : ''}`;
}
export function costLabel(value) {
  return numeric(value) && value >= 0 ? new Intl.NumberFormat('pt-BR', { style: 'currency', currency: 'USD', maximumFractionDigits: 4 }).format(value) : 'Indisponível';
}
export function historySeries(samples, query) {
  const windows = new Map();
  for (const sample of samples) {
    if (sample.provider !== query.provider || sample.account_id !== query.account_id || sample.source !== query.source) continue;
    for (const bucket of sample.buckets ?? []) {
      if (bucket.id !== query.bucket_id) continue;
      const key = JSON.stringify([sample.provider, sample.account_id, sample.source, bucket.id, bucket.resets_at_ms ?? null, bucket.window_seconds ?? null]);
      if (!windows.has(key)) windows.set(key, { key, resets_at_ms: bucket.resets_at_ms ?? null, points: new Map() });
      windows.get(key).points.set(sample.observed_at_ms, { observed_at_ms: sample.observed_at_ms, remaining_fraction: fraction(bucket.remaining_fraction), count: bucket.count ?? null, status: sample.status });
    }
  }
  return [...windows.values()].map(group => ({ ...group, points: [...group.points.values()].sort((a, b) => a.observed_at_ms - b.observed_at_ms) })).sort((a, b) => (a.points[0]?.observed_at_ms ?? 0) - (b.points[0]?.observed_at_ms ?? 0));
}
