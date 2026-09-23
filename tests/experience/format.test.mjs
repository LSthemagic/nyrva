import test from 'node:test';
import assert from 'node:assert/strict';
import { percent, quotaView, countdown, absoluteTime, accountLabel, historySeries, contextLabel } from '../../nyrva/ui/dashboard-format.mjs';

const now = 1_800_000_000_000;
const snapshot = { provider: 'antigravity', account_id: 'active', source: 'statusline', status: 'live', observed_at_ms: now };
const bucket = { id: 'weekly', label: 'Weekly', remaining_fraction: 0, resets_at_ms: now + 3_600_000, estimated: false, count: null };

test('zero is measured exhaustion, not missing quota', () => {
  assert.equal(percent(0), '0%');
  assert.equal(quotaView(snapshot, bucket, now).value, '0%');
  assert.equal(quotaView(snapshot, bucket, now).fraction, 0);
});
test('missing, coercible and invalid fractions never become zero or a percentage', () => {
  for (const input of [null, undefined, '', false, '0.5', -0.1, 1.1, NaN, Infinity]) assert.equal(percent(input), 'Indisponível');
  assert.equal(quotaView(snapshot, { ...bucket, remaining_fraction: null }, now).fraction, null);
});
test('count-only buckets preserve their independent unit', () => {
  const v = quotaView(snapshot, { ...bucket, remaining_fraction: null, count: 0 }, now);
  assert.equal(v.value, '0 eventos');
  assert.equal(v.fraction, null);
});
test('past reset waits for confirmation without inventing a refill', () => {
  const v = quotaView(snapshot, { ...bucket, resets_at_ms: now - 1 }, now);
  assert.equal(v.awaiting, true);
  assert.equal(v.value, '0%');
  assert.match(v.reset, /Aguardando confirmação/);
});
test('stale and error snapshots retain measurements with explicit provenance', () => {
  assert.equal(quotaView({ ...snapshot, status: 'stale' }, bucket, now).status, 'stale');
  assert.equal(quotaView({ ...snapshot, status: 'error' }, bucket, now).status, 'error');
  assert.equal(quotaView(snapshot, bucket, now + 600_001).status, 'stale');
});
test('countdown handles absent, elapsed and far-future reset times', () => {
  assert.equal(countdown(null, now), 'Reset não informado');
  assert.equal(countdown(now - 1, now), 'Aguardando confirmação');
  assert.equal(countdown(now + 3_660_000, now), '1h 1min');
  assert.match(countdown(now + 172_800_000, now), /^2d/);
  assert.equal(absoluteTime(null), 'Não informado');
  assert.notEqual(absoluteTime(now), 'Não informado');
});
test('aliases only affect display; untrusted labels are returned as text, not HTML', () => {
  const aliases = { active: 'pessoal' };
  assert.equal(accountLabel('active', aliases), 'pessoal');
  assert.equal(accountLabel('work', aliases), 'work');
  assert.equal(accountLabel('<img src=x onerror=alert(1)>', {}), '<img src=x onerror=alert(1)>');
  assert.deepEqual(aliases, { active: 'pessoal' });
});
test('history never mixes source, account, provider, bucket or reset window', () => {
  const sample = (changes = {}, b = {}) => ({ ...snapshot, buckets: [{ ...bucket, remaining_fraction: 0.5, ...b }], ...changes });
  const items = [sample({ observed_at_ms: now - 1000 }), sample(), sample({ source: 'legacy_adapter' }), sample({ account_id: 'work' }), sample({ provider: 'claude' }), sample({}, { id: 'daily' }), sample({ observed_at_ms: now + 1000 }, { resets_at_ms: now + 7_200_000 })];
  const groups = historySeries(items, { provider: 'antigravity', account_id: 'active', source: 'statusline', bucket_id: 'weekly' });
  assert.equal(groups.length, 2);
  assert.deepEqual(groups.map(g => g.points.length), [2, 1]);
  assert.deepEqual(groups[0].points.map(p => p.observed_at_ms), [now - 1000, now]);
});
test('history retains missing points as gaps and does not coerce count into quota', () => {
  const items = [{ ...snapshot, buckets: [{ ...bucket, remaining_fraction: null, count: 12 }] }];
  assert.equal(historySeries(items, { provider: 'antigravity', account_id: 'active', source: 'statusline', bucket_id: 'weekly' })[0].points[0].remaining_fraction, null);
});
test('context presents source-provided utilization without inventing token usage', () => {
  assert.equal(contextLabel(null), 'Não informado pela fonte');
  assert.match(contextLabel({ used_percentage: 0, size: 200000 }), /^0%/);
  assert.match(contextLabel({ used_percentage: null, size: 200000 }), /Uso não informado/);
});
